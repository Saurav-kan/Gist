use axum::{
    extract::{State, Query},
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;
use dirs;

use crate::AppState;

#[derive(Deserialize)]
pub struct BrowseRequest {
    path: Option<String>,
}

#[derive(Serialize)]
pub struct BrowseResponse {
    path: String,
    items: Vec<DirectoryItem>,
}

#[derive(Serialize)]
pub struct DirectoryItem {
    name: String,
    path: String,
    is_directory: bool,
    size: Option<u64>,
    modified_time: Option<i64>,
    file_type: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateFolderRequest {
    path: String,
    name: String,
}

#[derive(Deserialize)]
pub struct DeleteRequest {
    path: String,
}

#[derive(Deserialize)]
pub struct RenameRequest {
    path: String,
    new_name: String,
}

pub async fn browse_directory(
    Query(params): Query<BrowseRequest>,
) -> Result<Json<BrowseResponse>, axum::http::StatusCode> {
    let target_path = params.path.unwrap_or_else(|| {
        dirs::home_dir()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| ".".to_string())
    });

    let path = PathBuf::from(&target_path);
    
    if !path.exists() {
        return Err(axum::http::StatusCode::NOT_FOUND);
    }

    if !path.is_dir() {
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }

    let mut items = Vec::new();

    match fs::read_dir(&path) {
        Ok(entries) => {
            for entry in entries {
                if let Ok(entry) = entry {
                    let entry_path = entry.path();
                    let name = entry_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "unknown".to_string());

                    let full_path = entry_path.to_string_lossy().to_string();
                    let is_directory = entry_path.is_dir();
                    
                    let (size, modified_time, file_type) = if is_directory {
                        (None, None, None)
                    } else {
                        let metadata = fs::metadata(&entry_path).ok();
                        let size = metadata.as_ref().map(|m| m.len());
                        let modified_time = metadata
                            .as_ref()
                            .and_then(|m| m.modified().ok())
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs() as i64);
                        let file_type = entry_path
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|s| s.to_string());
                        (size, modified_time, file_type)
                    };

                    items.push(DirectoryItem {
                        name,
                        path: full_path,
                        is_directory,
                        size,
                        modified_time,
                        file_type,
                    });
                }
            }
        }
        Err(_) => {
            return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    // Sort: directories first, then files, both alphabetically
    items.sort_by(|a, b| {
        match (a.is_directory, b.is_directory) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    Ok(Json(BrowseResponse {
        path: target_path,
        items,
    }))
}

pub async fn get_special_folders() -> Json<serde_json::Value> {
    let mut folders = serde_json::json!({});
    
    if let Some(desktop) = dirs::desktop_dir() {
        if let Some(path) = desktop.to_str() {
            folders["desktop"] = serde_json::json!(path);
        }
    }
    
    if let Some(downloads) = dirs::download_dir() {
        if let Some(path) = downloads.to_str() {
            folders["downloads"] = serde_json::json!(path);
        }
    }
    
    if let Some(documents) = dirs::document_dir() {
        if let Some(path) = documents.to_str() {
            folders["documents"] = serde_json::json!(path);
        }
    }
    
    if let Some(home) = dirs::home_dir() {
        if let Some(path) = home.to_str() {
            folders["home"] = serde_json::json!(path);
        }
    }

    Json(folders)
}

pub async fn create_folder(
    State(_state): State<AppState>,
    Json(request): Json<CreateFolderRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let parent_path = PathBuf::from(&request.path);
    let new_folder_path = parent_path.join(&request.name);

    match fs::create_dir(&new_folder_path) {
        Ok(_) => {
            // If auto_index is enabled and parent is indexed, we could auto-add this folder
            // For now, just return success
            Ok(Json(serde_json::json!({
                "success": true,
                "path": new_folder_path.to_string_lossy().to_string()
            })))
        }
        Err(e) => {
            Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn delete_item(
    State(state): State<AppState>,
    Json(request): Json<DeleteRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let path = PathBuf::from(&request.path);

    if !path.exists() {
        return Err(axum::http::StatusCode::NOT_FOUND);
    }

    // Remove from index if it's a file
    if path.is_file() {
        if let Some(path_str) = path.to_str() {
            let _ = state.storage.delete_file(path_str).await;
        }
    } else if path.is_dir() {
        // Remove all files in directory from index
        if let Ok(all_files) = state.storage.get_all_files().await {
            for file in all_files {
                if file.file_path.starts_with(&request.path) {
                    let _ = state.storage.delete_file(&file.file_path).await;
                }
            }
        }
    }

    // Delete from filesystem
    let result = if path.is_dir() {
        fs::remove_dir_all(&path)
    } else {
        fs::remove_file(&path)
    };

    match result {
        Ok(_) => Ok(Json(serde_json::json!({
            "success": true
        }))),
        Err(_) => Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn rename_item(
    State(state): State<AppState>,
    Json(request): Json<RenameRequest>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    let old_path = PathBuf::from(&request.path);
    let parent = old_path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
    let new_path = parent.join(&request.new_name);

    if !old_path.exists() {
        return Err(axum::http::StatusCode::NOT_FOUND);
    }

    // Update index if it's a file - remove old entry, will need to re-index
    if old_path.is_file() {
        if let Some(old_str) = old_path.to_str() {
            let _ = state.storage.delete_file(old_str).await;
        }
    } else if old_path.is_dir() {
        // Remove all files in directory from index
        if let Ok(all_files) = state.storage.get_all_files().await {
            for file in all_files {
                if file.file_path.starts_with(&request.path) {
                    let _ = state.storage.delete_file(&file.file_path).await;
                }
            }
        }
    }

    // Rename in filesystem
    match fs::rename(&old_path, &new_path) {
        Ok(_) => Ok(Json(serde_json::json!({
            "success": true,
            "new_path": new_path.to_string_lossy().to_string()
        }))),
        Err(_) => Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
    }
}
