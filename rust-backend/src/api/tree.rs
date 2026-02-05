use axum::{
    extract::Query,
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;
use dirs;

#[derive(Deserialize)]
pub struct TreeRequest {
    path: Option<String>,
    depth: Option<usize>, // Maximum depth to load (for lazy loading)
}

#[derive(Serialize)]
pub struct TreeNode {
    name: String,
    path: String,
    is_directory: bool,
    size: Option<u64>,
    modified_time: Option<i64>,
    created_time: Option<i64>,
    file_type: Option<String>,
    children: Option<Vec<TreeNode>>, // None means not loaded yet, empty vec means no children
    expanded: bool,
}

#[derive(Serialize)]
pub struct TreeResponse {
    nodes: Vec<TreeNode>,
}

pub async fn get_file_tree(
    Query(params): Query<TreeRequest>,
) -> Result<Json<TreeResponse>, axum::http::StatusCode> {
    let target_path = params.path.unwrap_or_else(|| {
        dirs::home_dir()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .unwrap_or_else(|| ".".to_string())
    });
    
    let max_depth = params.depth.unwrap_or(2); // Default to 2 levels deep
    
    let path = PathBuf::from(&target_path);
    
    if !path.exists() || !path.is_dir() {
        return Err(axum::http::StatusCode::NOT_FOUND);
    }
    
    let nodes = build_tree_nodes(&path, 0, max_depth)?;
    
    Ok(Json(TreeResponse { nodes }))
}

fn build_tree_nodes(
    path: &PathBuf,
    current_depth: usize,
    max_depth: usize,
) -> Result<Vec<TreeNode>, axum::http::StatusCode> {
    if current_depth >= max_depth {
        // Return nodes without children (lazy loading)
        return Ok(get_directory_items(path, false)?);
    }
    
    let mut nodes = Vec::new();
    
    match fs::read_dir(path) {
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
                    
                    let children = if is_directory && current_depth < max_depth - 1 {
                        // Recursively load children
                        match build_tree_nodes(&entry_path, current_depth + 1, max_depth) {
                            Ok(children) => Some(children),
                            Err(_) => Some(Vec::new()), // Error loading children, return empty
                        }
                    } else if is_directory {
                        // Directory but at max depth, mark as not loaded
                        None
                    } else {
                        // File, no children
                        Some(Vec::new())
                    };
                    
                    nodes.push(TreeNode {
                        name,
                        path: full_path,
                        is_directory,
                        size,
                        modified_time,
                        created_time,
                        file_type,
                        children,
                        expanded: false,
                    });
                }
            }
        }
        Err(_) => {
            return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        }
    }
    
    // Sort: directories first, then files, both alphabetically
    nodes.sort_by(|a, b| {
        match (a.is_directory, b.is_directory) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });
    
    Ok(nodes)
}

fn get_directory_items(
    path: &PathBuf,
    include_children: bool,
) -> Result<Vec<TreeNode>, axum::http::StatusCode> {
    let mut nodes = Vec::new();
    
    match fs::read_dir(path) {
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
                    
                    nodes.push(TreeNode {
                        name,
                        path: full_path,
                        is_directory,
                        size,
                        modified_time,
                        created_time,
                        file_type,
                        children: if include_children && is_directory {
                            Some(Vec::new()) // Will be loaded on demand
                        } else if is_directory {
                            None // Not loaded yet
                        } else {
                            Some(Vec::new()) // File, no children
                        },
                        expanded: false,
                    });
                }
            }
        }
        Err(_) => {
            return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
        }
    }
    
    // Sort: directories first, then files, both alphabetically
    nodes.sort_by(|a, b| {
        match (a.is_directory, b.is_directory) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });
    
    Ok(nodes)
}
