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
    let limit = request.limit.unwrap_or(10);
    
    // Generate embedding for query
    let embedding_service = crate::embedding::EmbeddingService::new(
        state.config.embedding_model.clone()
    );
    
    let query_embedding = embedding_service.generate_embedding(&request.query)
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get all files and their embeddings
    let files_with_embeddings = state.storage.get_all_embeddings()
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    // Calculate similarities
    let mut results: Vec<(crate::storage::FileMetadata, f32)> = files_with_embeddings
        .into_iter()
        .map(|(metadata, embedding)| {
            let similarity = cosine_similarity(&query_embedding, &embedding);
            (metadata, similarity)
        })
        .collect();

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
