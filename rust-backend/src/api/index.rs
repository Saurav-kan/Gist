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
    indexed_count: Option<usize>,
}

pub async fn start_indexing(
    State(state): State<AppState>,
    Json(request): Json<StartIndexingRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    // Create indexer
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
    );

    // Start indexing in background
    let directory = request.directory;
    tokio::spawn(async move {
        match indexer.index_directory(&directory).await {
            Ok(count) => {
                println!("Indexed {} files from {}", count, directory);
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
    State(_state): State<AppState>,
) -> Json<IndexStatusResponse> {
    // For now, return simple status
    // In a full implementation, we'd track indexing progress
    Json(IndexStatusResponse {
        is_indexing: false,
        indexed_count: None,
    })
}

pub async fn clear_index(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    state.storage.clear_all()
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Index cleared successfully"
    })))
}
