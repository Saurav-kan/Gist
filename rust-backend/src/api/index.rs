use axum::{
    extract::State,
    response::Json,
};
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Deserialize)]
pub struct StartIndexingRequest {
    directory: String,
}

#[derive(Serialize)]
pub struct IndexStatusResponse {
    is_indexing: bool,
    current: Option<usize>,
    total: Option<usize>,
    current_file: Option<String>,
    directory: Option<String>,
    indexed_count: Option<usize>,
}

pub async fn start_indexing(
    State(state): State<AppState>,
    Json(request): Json<StartIndexingRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    // Create indexer with progress tracker
    let embedding_service = std::sync::Arc::new(
        crate::embedding::EmbeddingService::new(state.config.embedding_model.clone())
    );
    
    let parser_registry = std::sync::Arc::new(
        crate::parsers::ParserRegistry::new(&state.config.file_type_filters)
    );
    
    let indexer = crate::indexer::Indexer::new(
        state.storage.clone(),
        embedding_service,
        parser_registry,
        state.config.clone(),
    ).with_progress_tracker(state.indexing_progress.clone());

    // Start indexing in background
    let directory = request.directory.clone();
    let storage_clone = state.storage.clone();
    let hnsw_index_clone = state.hnsw_index.clone();
    tokio::spawn(async move {
        match indexer.index_directory(&directory).await {
            Ok(count) => {
                println!("Indexed {} files from {}", count, directory);
                
                // Rebuild HNSW index after indexing completes
                if let Ok(embeddings) = storage_clone.get_all_embeddings().await {
                    if !embeddings.is_empty() {
                        let dimensions = embeddings[0].1.len();
                        let mut new_index = crate::hnsw_index::HnswIndex::new(dimensions);
                        if new_index.rebuild_from_embeddings(embeddings).is_ok() {
                            let mut index_guard = hnsw_index_clone.write().await;
                            *index_guard = Some(new_index);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Indexing error: {}", e);
            }
        }
    });

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Indexing started"
    })))
}

pub async fn get_index_status(
    State(state): State<AppState>,
) -> Json<IndexStatusResponse> {
    let progress = state.indexing_progress.read().await.clone();
    
    if let Some(p) = progress {
        Json(IndexStatusResponse {
            is_indexing: p.is_indexing,
            current: Some(p.current),
            total: Some(p.total),
            current_file: Some(p.current_file),
            directory: Some(p.directory),
            indexed_count: None,
        })
    } else {
        Json(IndexStatusResponse {
            is_indexing: false,
            current: None,
            total: None,
            current_file: None,
            directory: None,
            indexed_count: None,
        })
    }
}

pub async fn clear_index(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    state.storage.clear_all()
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    // Clear HNSW index
    let mut hnsw_guard = state.hnsw_index.write().await;
    if let Some(ref mut index) = *hnsw_guard {
        let _ = index.clear();
    }
    *hnsw_guard = None;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Index cleared successfully"
    })))
}
