mod api;
mod config;
mod embedding;
mod file_watcher;
mod hnsw_index;
mod indexer;
mod parsers;
mod search;
mod storage;

use axum::{
    http::StatusCode,
    response::Json,
    routing::{get, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use crate::config::AppConfig;
use crate::storage::Storage;
use crate::file_watcher::FileWatcher;
use crate::indexer::IndexingProgress;
use crate::hnsw_index::HnswIndex;

pub use crate::config::PerformanceMode;

#[derive(Clone)]
pub struct AppState {
    pub storage: Arc<Storage>,
    pub config: Arc<AppConfig>,
    pub file_watcher: Option<Arc<tokio::sync::Mutex<FileWatcher>>>,
    pub indexing_progress: Arc<tokio::sync::RwLock<Option<IndexingProgress>>>,
    pub hnsw_index: Arc<tokio::sync::RwLock<Option<HnswIndex>>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize config
    let config = Arc::new(AppConfig::load_or_default().await?);
    
    // Initialize storage
    let storage = Arc::new(Storage::new(&AppConfig::data_dir()).await?);
    
    // Initialize embedding service
    let embedding_service = Arc::new(crate::embedding::EmbeddingService::new(
        config.embedding_model.clone()
    ));
    
    // Initialize parser registry
    let parser_registry = Arc::new(crate::parsers::ParserRegistry::new(
        &config.file_type_filters
    ));
    
    // Initialize indexer
    let indexer = Arc::new(crate::indexer::Indexer::new(
        storage.clone(),
        embedding_service,
        parser_registry,
        config.clone(),
    ));
    
    // Initialize file watcher if auto_index is enabled
    let file_watcher = if config.auto_index && !config.indexed_directories.is_empty() {
        match FileWatcher::new(indexer.clone(), storage.clone(), config.indexed_directories.clone()) {
            Ok(watcher) => Some(Arc::new(tokio::sync::Mutex::new(watcher))),
            Err(e) => {
                eprintln!("Warning: Failed to initialize file watcher: {}", e);
                None
            }
        }
    } else {
        None
    };
    
    // Initialize HNSW index (will be built lazily on first search or after indexing)
    let hnsw_index = Arc::new(tokio::sync::RwLock::new(None));
    
    let app_state = AppState { 
        storage, 
        config,
        file_watcher,
        indexing_progress: Arc::new(tokio::sync::RwLock::new(None)),
        hnsw_index,
    };

    // Build router
    let app = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/settings", get(api::settings::get_settings))
        .route("/api/settings", put(api::settings::update_settings))
        .route("/api/system-info", get(api::system_info::get_system_info))
        .route("/api/search", post(api::search::search_files))
        .route("/api/files", get(api::files::list_files))
        .route("/api/files/browse", get(api::files_browser::browse_directory))
        .route("/api/files/special-folders", get(api::files_browser::get_special_folders))
        .route("/api/files/create-folder", post(api::files_browser::create_folder))
        .route("/api/files/delete", post(api::files_browser::delete_item))
        .route("/api/files/rename", put(api::files_browser::rename_item))
        .route("/api/index/start", post(api::index::start_indexing))
        .route("/api/index/status", get(api::index::get_index_status))
        .route("/api/index/clear", post(api::index::clear_index))
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;
    println!("Backend server running on http://127.0.0.1:8080");
    
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn health_check() -> Result<Json<serde_json::Value>, StatusCode> {
    Ok(Json(serde_json::json!({
        "status": "ok",
        "service": "nlp-file-explorer-backend"
    })))
}
