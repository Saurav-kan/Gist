pub mod api;
pub mod config;
pub mod embedding;
pub mod file_watcher;
pub mod hnsw_index;
pub mod indexer;
pub mod parsers;
pub mod query_parser;
pub mod search;
pub mod storage;
pub mod active_rag_agent;

use axum::{
    http::StatusCode,
    response::Json,
};
use std::sync::Arc;

pub use crate::config::PerformanceMode;
use crate::storage::Storage;
use crate::config::AppConfig;
use crate::file_watcher::FileWatcher;
use crate::indexer::IndexingProgress;
use crate::hnsw_index::HnswIndex;

#[derive(Clone)]
pub struct AppState {
    pub storage: Arc<Storage>,
    pub config: Arc<AppConfig>,
    pub file_watcher: Option<Arc<tokio::sync::Mutex<FileWatcher>>>,
    pub indexing_progress: Arc<tokio::sync::RwLock<Option<IndexingProgress>>>,
    pub hnsw_index: Arc<tokio::sync::RwLock<Option<HnswIndex>>>,
}

pub async fn health_check() -> Result<Json<serde_json::Value>, StatusCode> {
    Ok(Json(serde_json::json!({
        "status": "ok",
        "service": "gist-vector-search-backend"
    })))
}
