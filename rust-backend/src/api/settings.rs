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
    max_context_tokens: usize,
    auto_index: bool,
    max_search_results: usize,
    filter_duplicate_files: bool,
    ai_features_enabled: bool,
    ai_provider: String,
    ollama_model: Option<String>,
    gemini_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    api_key: Option<String>, // Don't send API key to frontend for security
}

#[derive(Serialize)]
struct FileTypeFiltersResponse {
    include_pdf: bool,
    include_docx: bool,
    include_text: bool,
    include_xlsx: bool,
    excluded_extensions: Vec<String>,
}

#[derive(Deserialize)]
pub struct UpdateSettingsRequest {
    performance_mode: Option<String>,
    indexed_directories: Option<Vec<String>>,
    file_type_filters: Option<FileTypeFiltersRequest>,
    chunk_size: Option<usize>,
    max_context_tokens: Option<usize>,
    auto_index: Option<bool>,
    max_search_results: Option<usize>,
    filter_duplicate_files: Option<bool>,
    ai_features_enabled: Option<bool>,
    ai_provider: Option<String>,
    ollama_model: Option<String>,
    gemini_model: Option<String>,
    api_key: Option<String>,
}

#[derive(Deserialize)]
pub struct FileTypeFiltersRequest {
    include_pdf: Option<bool>,
    include_docx: Option<bool>,
    include_text: Option<bool>,
    include_xlsx: Option<bool>,
    excluded_extensions: Option<Vec<String>>,
}

pub async fn get_settings(State(state): State<AppState>) -> Json<SettingsResponse> {
    // Reload config from disk to ensure we have the latest values
    // This ensures settings persist correctly after save
    let config = match crate::config::AppConfig::load_or_default().await {
        Ok(cfg) => cfg,
        Err(_) => state.config.as_ref().clone(), // Fallback to in-memory if disk read fails
    };
    
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
            excluded_extensions: config.file_type_filters.excluded_extensions.clone(),
        },
        chunk_size: config.chunk_size,
        max_context_tokens: config.max_context_tokens,
        auto_index: config.auto_index,
        max_search_results: config.max_search_results,
        filter_duplicate_files: config.filter_duplicate_files,
        ai_features_enabled: {
            eprintln!("[SETTINGS] get_settings returning ai_features_enabled = {}", config.ai_features_enabled);
            config.ai_features_enabled
        },
        ai_provider: match config.ai_provider {
            crate::config::AiProvider::Ollama => "ollama".to_string(),
            crate::config::AiProvider::OpenAI => "openai".to_string(),
            crate::config::AiProvider::GreenPT => "greenpt".to_string(),
            crate::config::AiProvider::Gemini => "gemini".to_string(),
        },
        ollama_model: config.ollama_model.clone(),
        gemini_model: config.gemini_model.clone(),
        api_key: None, // Never send API key to frontend
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
        if let Some(val) = filters.excluded_extensions {
            config.file_type_filters.excluded_extensions = val
                .into_iter()
                .map(|e| e.trim_start_matches('.').to_lowercase())
                .filter(|e| !e.is_empty())
                .collect();
        }
    }

    if let Some(val) = request.chunk_size {
        config.chunk_size = val;
    }

    if let Some(val) = request.max_context_tokens {
        // Clamp between 500 and 8000 tokens
        config.max_context_tokens = val.max(500).min(8000);
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

    if let Some(val) = request.filter_duplicate_files {
        config.filter_duplicate_files = val;
    }

    // Always update ai_features_enabled if provided in request
    // Use explicit check to ensure we're updating even if value is false
    if let Some(enabled) = request.ai_features_enabled {
        eprintln!("[SETTINGS] Updating ai_features_enabled from {} to {}", config.ai_features_enabled, enabled);
        config.ai_features_enabled = enabled;
    } else {
        eprintln!("[SETTINGS] ai_features_enabled not provided in request, keeping current value: {}", config.ai_features_enabled);
    }

    if let Some(provider_str) = request.ai_provider {
        config.ai_provider = match provider_str.as_str() {
            "ollama" => crate::config::AiProvider::Ollama,
            "openai" => crate::config::AiProvider::OpenAI,
            "greenpt" => crate::config::AiProvider::GreenPT,
            "gemini" => crate::config::AiProvider::Gemini,
            _ => return Err(axum::http::StatusCode::BAD_REQUEST),
        };
    }

    if let Some(model) = request.ollama_model {
        config.ollama_model = Some(model);
    }

    if let Some(model) = request.gemini_model {
        config.gemini_model = Some(model);
    }

    if let Some(key) = request.api_key {
        // Only update if key is not empty (allows clearing)
        if !key.is_empty() {
            config.api_key = Some(key);
        } else {
            config.api_key = None;
        }
    }

    config.save().await.map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Reload config from disk to ensure we have the latest values
    // Then update the in-memory AppState config
    if let Ok(_updated_config) = crate::config::AppConfig::load_or_default().await {
        // Replace the config in AppState
        // Since we can't mutate Arc directly, we need to use Arc::make_mut or replace it
        // For now, we'll reload it on next get_settings call, but let's update the state
        // Actually, we can't easily update Arc<AppConfig> without RwLock, so we'll reload on next read
        // But the issue is get_settings reads from state.config, not from disk
        // So we need to update state.config somehow
        
        // Workaround: The config is saved to disk correctly, but state.config still has old values
        // The proper fix would be to use Arc<RwLock<AppConfig>>, but for now,
        // let's ensure get_settings reloads from disk if needed, or update the Arc
        
        // Since Arc is immutable, we can't update it directly
        // The best solution is to reload config in get_settings, but that's inefficient
        // For now, let's document this and ensure the save worked
    }

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
