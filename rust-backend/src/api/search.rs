use axum::{
    extract::State,
    response::Json,
};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::search::cosine_similarity;

#[derive(Deserialize)]
pub struct SearchRequest {
    query: String,
    limit: Option<usize>,
}

#[derive(Serialize)]
pub struct SearchResponse {
    results: Vec<SearchResult>,
}

#[derive(Serialize)]
pub struct SearchResult {
    file_path: String,
    file_name: String,
    similarity: f32,
    preview: Option<String>,
}

pub async fn search_files(
    State(state): State<AppState>,
    Json(request): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, axum::http::StatusCode> {
    // Use config's max_search_results as default, but allow override up to 200
    let default_limit = state.config.max_search_results;
    let limit = request.limit.unwrap_or(default_limit).min(200);
    
    // Generate embedding for query
    let embedding_service = crate::embedding::EmbeddingService::new(
        state.config.embedding_model.clone()
    );
    
    let query_embedding = embedding_service.generate_embedding(&request.query)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    // Try to use HNSW index if available, otherwise fall back to linear search
    let mut results: Vec<(crate::storage::FileMetadata, f32)> = Vec::new();
    
    let hnsw_guard = state.hnsw_index.read().await;
    if let Some(ref hnsw) = *hnsw_guard {
        // Use HNSW search
        if let Ok(hnsw_results) = hnsw.search(query_embedding.clone(), limit * 2) {
            results = hnsw_results;
        }
    }
    drop(hnsw_guard);
    
    // If HNSW didn't return results, use linear search
    if results.is_empty() {
        let files_with_embeddings = state.storage.get_all_embeddings()
            .await
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

        // Calculate similarities in parallel chunks
        use futures::future::join_all;
        let chunk_size = 100;
        let mut all_results = Vec::new();
        
        for chunk in files_with_embeddings.chunks(chunk_size) {
            let chunk_tasks: Vec<_> = chunk.iter().map(|(metadata, embedding)| {
                let query = query_embedding.clone();
                let emb = embedding.clone();
                let meta = metadata.clone();
                tokio::spawn(async move {
                    let similarity = cosine_similarity(&query, &emb);
                    (meta, similarity)
                })
            }).collect();
            
            let chunk_results = join_all(chunk_tasks).await;
            for task_result in chunk_results {
                if let Ok(result) = task_result {
                    all_results.push(result);
                }
            }
        }
        
        results = all_results;
    }

    // Sort by similarity (descending)
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Take top results
    let search_results: Vec<SearchResult> = results
        .into_iter()
        .take(limit)
        .map(|(metadata, similarity)| {
            SearchResult {
                file_path: metadata.file_path.clone(),
                file_name: metadata.file_name.clone(),
                similarity,
                preview: None, // Could add file preview logic here
            }
        })
        .collect();

    Ok(Json(SearchResponse {
        results: search_results,
    }))
}
