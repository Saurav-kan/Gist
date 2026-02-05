use axum::{
    extract::{State, Query},
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;
use dirs;
use walkdir::WalkDir;

use crate::AppState;

#[derive(Deserialize)]
pub struct BrowseRequest {
    path: Option<String>,
    sort: Option<String>, // name, date_modified, date_created, size, type
    order: Option<String>, // asc, desc
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
    created_time: Option<i64>,
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
    // Check if this is a special "This PC" request (empty path or special marker)
    let is_this_pc = params.path.is_none() || params.path.as_ref().map(|p| p.is_empty() || p == "::this-pc").unwrap_or(false);
    
    if is_this_pc {
        // Return drives/root directories
        let mut items = Vec::new();
        
        #[cfg(target_os = "windows")]
        {
            // On Windows, return drive letters
            for drive_letter in b'A'..=b'Z' {
                let drive = format!("{}:\\", drive_letter as char);
                let drive_path = PathBuf::from(&drive);
                if drive_path.exists() {
                    items.push(DirectoryItem {
                        name: drive.clone(),
                        path: drive,
                        is_directory: true,
                        size: None,
                        modified_time: None,
                        created_time: None,
                        file_type: None,
                    });
                }
            }
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            // On Linux/Mac, return common root directories
            let root_dirs = vec![
                ("Home", dirs::home_dir()),
                ("Desktop", dirs::desktop_dir()),
                ("Documents", dirs::document_dir()),
                ("Downloads", dirs::download_dir()),
            ];
            
            for (name, opt_path) in root_dirs {
                if let Some(path) = opt_path {
                    if let Some(path_str) = path.to_str() {
                        items.push(DirectoryItem {
                            name: name.to_string(),
                            path: path_str.to_string(),
                            is_directory: true,
                            size: None,
                            modified_time: None,
                            created_time: None,
                            file_type: None,
                        });
                    }
                }
            }
        }
        
        return Ok(Json(BrowseResponse {
            path: "::this-pc".to_string(),
            items,
        }));
    }
    
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
                    
                    let (size, modified_time, created_time, file_type) = if is_directory {
                        (None, None, None, None)
                    } else {
                        let metadata = fs::metadata(&entry_path).ok();
                        let size = metadata.as_ref().map(|m| m.len());
                        let modified_time = metadata
                            .as_ref()
                            .and_then(|m| m.modified().ok())
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs() as i64);
                        let created_time = metadata
                            .as_ref()
                            .and_then(|m| m.created().ok())
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs() as i64);
                        let file_type = entry_path
                            .extension()
                            .and_then(|e| e.to_str())
                            .map(|s| s.to_string());
                        (size, modified_time, created_time, file_type)
                    };

                    items.push(DirectoryItem {
                        name,
                        path: full_path,
                        is_directory,
                        size,
                        modified_time,
                        created_time,
                        file_type,
                    });
                }
            }
        }
        Err(_) => {
            return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    // Apply sorting
    let sort_by = params.sort.as_deref().unwrap_or("name");
    let order = params.order.as_deref().unwrap_or("asc");
    let is_desc = order == "desc";
    
    items.sort_by(|a, b| {
        // Always keep directories first (or last if sorting desc by name)
        let dir_order = match (a.is_directory, b.is_directory, sort_by, is_desc) {
            (true, false, "name", true) => std::cmp::Ordering::Greater, // Desc: dirs last
            (true, false, _, _) => std::cmp::Ordering::Less, // Asc or other: dirs first
            (false, true, "name", true) => std::cmp::Ordering::Less,
            (false, true, _, _) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        };
        
        if dir_order != std::cmp::Ordering::Equal {
            return dir_order;
        }
        
        // Both are directories or both are files, apply sorting
        let comparison = match sort_by {
            "name" => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            "date_modified" => {
                let a_time = a.modified_time.unwrap_or(0);
                let b_time = b.modified_time.unwrap_or(0);
                a_time.cmp(&b_time)
            },
            "date_created" => {
                let a_time = a.created_time.unwrap_or(0);
                let b_time = b.created_time.unwrap_or(0);
                a_time.cmp(&b_time)
            },
            "size" => {
                let a_size = a.size.unwrap_or(0);
                let b_size = b.size.unwrap_or(0);
                a_size.cmp(&b_size)
            },
            "type" => {
                let a_type = a.file_type.as_deref().unwrap_or("");
                let b_type = b.file_type.as_deref().unwrap_or("");
                a_type.cmp(b_type)
            },
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        };
        
        if is_desc {
            comparison.reverse()
        } else {
            comparison
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

#[derive(Deserialize)]
pub struct FileSearchRequest {
    query: String,
    path: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize)]
pub struct FileSearchResponse {
    results: Vec<DirectoryItem>,
    count: usize,
}

pub async fn search_files(
    Query(params): Query<FileSearchRequest>,
) -> Result<Json<FileSearchResponse>, axum::http::StatusCode> {
    let search_query = params.query.to_lowercase();
    if search_query.is_empty() {
        return Ok(Json(FileSearchResponse {
            results: Vec::new(),
            count: 0,
        }));
    }
    
    let search_path = params.path.unwrap_or_else(|| {
        dirs::home_dir()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| ".".to_string())
    });
    
    let limit = params.limit.unwrap_or(100);
    let mut results = Vec::new();
    
    let path_buf = PathBuf::from(&search_path);
    if !path_buf.exists() || !path_buf.is_dir() {
        return Err(axum::http::StatusCode::NOT_FOUND);
    }
    
    // Walk directory recursively
    for entry in WalkDir::new(&path_buf)
        .into_iter()
        .filter_map(|e| e.ok())
        .take(limit * 10) // Limit traversal to avoid excessive searching
    {
        let entry_path = entry.path();
        let name = entry_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        
        let is_directory = entry_path.is_dir();
        let name_lower = name.to_lowercase();
        
        // Check filename match
        let matches_name = name_lower.contains(&search_query);
        
        // For files, also check content if it's a text file
        let matches_content = if !is_directory {
            let ext = entry_path
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_lowercase())
                .unwrap_or_default();
            
            // Check if it's a text file we can search
            let is_text_file = matches!(
                ext.as_str(),
                "txt" | "md" | "js" | "ts" | "py" | "rs" | "java" | "cpp" | "c" | "h" | "hpp"
                    | "json" | "xml" | "html" | "css" | "yaml" | "yml" | "toml" | "ini" | "log"
            );
            
            if is_text_file {
                // Try to read and search file content
                if let Ok(content) = fs::read_to_string(entry_path) {
                    content.to_lowercase().contains(&search_query)
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };
        
        if matches_name || matches_content {
            let full_path = entry_path.to_string_lossy().to_string();
            let (size, modified_time, created_time, file_type) = if is_directory {
                (None, None, None, None)
            } else {
                let metadata = fs::metadata(entry_path).ok();
                let size = metadata.as_ref().map(|m| m.len());
                let modified_time = metadata
                    .as_ref()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64);
                let created_time = metadata
                    .as_ref()
                    .and_then(|m| m.created().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64);
                let file_type = entry_path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.to_string());
                (size, modified_time, created_time, file_type)
            };
            
            results.push(DirectoryItem {
                name,
                path: full_path,
                is_directory,
                size,
                modified_time,
                created_time,
                file_type,
            });
            
            if results.len() >= limit {
                break;
            }
        }
    }
    
    Ok(Json(FileSearchResponse {
        count: results.len(),
        results,
    }))
}
