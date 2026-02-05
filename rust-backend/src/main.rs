use axum::{
    routing::{get, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;

use nlp_file_explorer_backend::{
    config::AppConfig,
    storage::Storage,
    file_watcher::FileWatcher,
    indexer::Indexer,
    hnsw_index::HnswIndex,
    AppState,
    api,
    health_check,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize config
    let config = Arc::new(AppConfig::load_or_default().await?);
    
    // Initialize storage
    let storage = Arc::new(Storage::new(&AppConfig::data_dir()).await?);
    
    // Initialize embedding service
    let embedding_service = Arc::new(nlp_file_explorer_backend::embedding::EmbeddingService::new(
        config.embedding_model.clone()
    ));
    
    // Initialize parser registry
    let parser_registry = Arc::new(nlp_file_explorer_backend::parsers::ParserRegistry::new(
        &config.file_type_filters
    ));
    
    // Initialize indexer
    let indexer = Arc::new(Indexer::new(
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
        .route("/api/search/parse", post(api::parse::parse_query))
        .route("/api/files", get(api::files::list_files))
        .route("/api/files/browse", get(api::files_browser::browse_directory))
        .route("/api/files/search", get(api::files_browser::search_files))
        .route("/api/files/tree", get(api::tree::get_file_tree))
        .route("/api/preview", get(api::preview::get_file_preview))
        .route("/api/files/special-folders", get(api::files_browser::get_special_folders))
        .route("/api/files/create-folder", post(api::files_browser::create_folder))
        .route("/api/files/delete", post(api::files_browser::delete_item))
        .route("/api/files/rename", put(api::files_browser::rename_item))
        .route("/api/index/start", post(api::index::start_indexing))
        .route("/api/index/status", get(api::index::get_index_status))
        .route("/api/index/clear", post(api::index::clear_index))
        .route("/api/ai/summarize", post(api::ai::summarize_document))
        .route("/api/ai/chat", post(api::ai::chat_about_document))
        .layer(CorsLayer::permissive())
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;
    println!("Backend server running on http://127.0.0.1:8080");
    
    axum::serve(listener, app).await?;
    
    Ok(())
}
