use axum::{
    extract::State,
    response::Json,
};
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Serialize)]
pub struct SettingsResponse {
    performance_mode: String,
    embedding_model: String,
    indexed_directories: Vec<String>,
    file_type_filters: FileTypeFiltersResponse,
    chunk_size: usize,
    auto_index: bool,
    max_search_results: usize,
}

#[derive(Serialize)]
struct FileTypeFiltersResponse {
    include_pdf: bool,
    include_docx: bool,
    include_text: bool,
    include_xlsx: bool,
}

#[derive(Deserialize)]
pub struct UpdateSettingsRequest {
    performance_mode: Option<String>,
    indexed_directories: Option<Vec<String>>,
    file_type_filters: Option<FileTypeFiltersRequest>,
    chunk_size: Option<usize>,
    auto_index: Option<bool>,
    max_search_results: Option<usize>,
}

#[derive(Deserialize)]
pub struct FileTypeFiltersRequest {
    include_pdf: Option<bool>,
    include_docx: Option<bool>,
    include_text: Option<bool>,
    include_xlsx: Option<bool>,
}

pub async fn get_settings(State(state): State<AppState>) -> Json<SettingsResponse> {
    let config = state.config.as_ref();
    
    Json(SettingsResponse {
        performance_mode: match config.performance_mode {
            crate::config::PerformanceMode::Lightweight => "lightweight".to_string(),
            crate::config::PerformanceMode::Normal => "normal".to_string(),
        },
        embedding_model: config.embedding_model.clone(),
        indexed_directories: config.indexed_directories.clone(),
        file_type_filters: FileTypeFiltersResponse {
            include_pdf: config.file_type_filters.include_pdf,
            include_docx: config.file_type_filters.include_docx,
            include_text: config.file_type_filters.include_text,
            include_xlsx: config.file_type_filters.include_xlsx,
        },
        chunk_size: config.chunk_size,
        auto_index: config.auto_index,
        max_search_results: config.max_search_results,
    })
}

pub async fn update_settings(
    State(state): State<AppState>,
    Json(request): Json<UpdateSettingsRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let mut config = (*state.config).clone();
    let mut needs_reindex = false;

    if let Some(mode_str) = request.performance_mode {
        let new_mode = match mode_str.as_str() {
            "lightweight" => crate::config::PerformanceMode::Lightweight,
            "normal" => crate::config::PerformanceMode::Normal,
            _ => return Err(axum::http::StatusCode::BAD_REQUEST),
        };
        
        if new_mode != config.performance_mode {
            config.set_performance_mode(new_mode);
            needs_reindex = true;
        }
    }

    if let Some(dirs) = request.indexed_directories {
        // Replace directories list (frontend sends complete updated list)
        let old_dirs = config.indexed_directories.clone();
        config.indexed_directories = dirs.clone();
        
        // Update file watcher if auto_index is enabled
        if config.auto_index {
            if let Some(watcher_mutex) = &state.file_watcher {
                let mut watcher = watcher_mutex.lock().await;
                
                // Remove directories that are no longer in the list
                for old_dir in &old_dirs {
                    if !dirs.contains(old_dir) {
                        if let Err(e) = watcher.remove_directory(old_dir) {
                            eprintln!("Warning: Failed to remove directory {} from watcher: {}", old_dir, e);
                        }
                    }
                }
                
                // Add new directories
                for new_dir in &dirs {
                    if !old_dirs.contains(new_dir) {
                        if let Err(e) = watcher.add_directory(new_dir) {
                            eprintln!("Warning: Failed to add directory {} to watcher: {}", new_dir, e);
                        }
                    }
                }
            }
        }
    }

    if let Some(filters) = request.file_type_filters {
        if let Some(val) = filters.include_pdf {
            config.file_type_filters.include_pdf = val;
        }
        if let Some(val) = filters.include_docx {
            config.file_type_filters.include_docx = val;
        }
        if let Some(val) = filters.include_text {
            config.file_type_filters.include_text = val;
        }
        if let Some(val) = filters.include_xlsx {
            config.file_type_filters.include_xlsx = val;
        }
    }

    if let Some(val) = request.chunk_size {
        config.chunk_size = val;
    }

    if let Some(val) = request.auto_index {
        config.auto_index = val;
        
        // If auto_index was enabled and directories exist, ensure watcher is set up
        // If auto_index was disabled, watcher will be None (handled on next restart)
        // Note: Full watcher recreation requires restart, but we can at least update existing one
        if val && !config.indexed_directories.is_empty() {
            if let Some(watcher_mutex) = &state.file_watcher {
                let mut watcher = watcher_mutex.lock().await;
                // Add any directories that aren't being watched
                for dir in &config.indexed_directories {
                    // Try to add - if it fails, it might already be watched
                    let _ = watcher.add_directory(dir);
                }
            }
        }
    }

    if let Some(val) = request.max_search_results {
        // Clamp between 10 and 200
        config.max_search_results = val.max(10).min(200);
    }

    config.save().await.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Note: In a real implementation, we'd need to use Arc::get_mut or a Mutex
    // For now, we'll just save to disk and the next request will load it

    Ok(Json(serde_json::json!({
        "success": true,
        "needs_reindex": needs_reindex,
        "message": if needs_reindex {
            "Settings saved. Re-indexing required due to model change."
        } else {
            "Settings saved successfully."
        }
    })))
}
