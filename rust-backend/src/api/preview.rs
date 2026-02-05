use axum::{
    extract::Query,
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;

use crate::parsers::{DocumentParser, PdfParser, DocxParser, TextParser};

#[derive(Deserialize)]
pub struct PreviewRequest {
    path: String,
}

#[derive(Serialize)]
pub struct PreviewResponse {
    success: bool,
    file_type: String, // text, code, pdf, docx, image, binary, unknown
    content: Option<String>, // Extracted text content
    preview_available: bool,
    size: u64,
    modified_time: Option<i64>,
    created_time: Option<i64>,
    error: Option<String>,
}

pub async fn get_file_preview(
    Query(params): Query<PreviewRequest>,
) -> Result<Json<PreviewResponse>, axum::http::StatusCode> {
    let file_path = PathBuf::from(&params.path);
    
    if !file_path.exists() {
        return Ok(Json(PreviewResponse {
            success: false,
            file_type: "unknown".to_string(),
            content: None,
            preview_available: false,
            size: 0,
            modified_time: None,
            created_time: None,
            error: Some("File not found".to_string()),
        }));
    }
    
    if file_path.is_dir() {
        return Ok(Json(PreviewResponse {
            success: false,
            file_type: "directory".to_string(),
            content: None,
            preview_available: false,
            size: 0,
            modified_time: None,
            created_time: None,
            error: Some("Cannot preview directories".to_string()),
        }));
    }
    
    // Get file metadata
    let metadata = match fs::metadata(&file_path) {
        Ok(m) => m,
        Err(_) => {
            return Ok(Json(PreviewResponse {
                success: false,
                file_type: "unknown".to_string(),
                content: None,
                preview_available: false,
                size: 0,
                modified_time: None,
                created_time: None,
                error: Some("Failed to read file metadata".to_string()),
            }));
        }
    };
    
    let size = metadata.len();
    let modified_time = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64);
    let created_time = metadata
        .created()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64);
    
    // Determine file type
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    
    let file_type = determine_file_type(&ext);
    let preview_available = matches!(
        file_type.as_str(),
        "text" | "code" | "pdf" | "docx" | "image"
    );
    
    // Extract content if preview is available
    let content = if preview_available {
        match extract_preview_content(&file_path, &file_type) {
            Ok(text) => Some(text),
            Err(e) => {
                return Ok(Json(PreviewResponse {
                    success: false,
                    file_type,
                    content: None,
                    preview_available: true,
                    size,
                    modified_time,
                    created_time,
                    error: Some(format!("Failed to extract content: {}", e)),
                }));
            }
        }
    } else {
        None
    };
    
    Ok(Json(PreviewResponse {
        success: true,
        file_type,
        content,
        preview_available,
        size,
        modified_time,
        created_time,
        error: None,
    }))
}

fn determine_file_type(ext: &str) -> String {
    // Code files
    if matches!(
        ext,
        "js" | "ts" | "py" | "rs" | "java" | "cpp" | "c" | "h" | "hpp" | "go" | "rb" | "php"
            | "swift" | "kt" | "scala" | "clj" | "sh" | "bash" | "zsh" | "fish"
    ) {
        return "code".to_string();
    }
    
    // Text files
    if matches!(ext, "txt" | "md" | "log" | "ini" | "conf" | "cfg") {
        return "text".to_string();
    }
    
    // Markdown
    if ext == "md" {
        return "text".to_string();
    }
    
    // PDF
    if ext == "pdf" {
        return "pdf".to_string();
    }
    
    // DOCX
    if ext == "docx" || ext == "doc" {
        return "docx".to_string();
    }
    
    // Images
    if matches!(
        ext,
        "jpg" | "jpeg" | "png" | "gif" | "svg" | "webp" | "bmp" | "ico" | "tiff" | "tif"
    ) {
        return "image".to_string();
    }
    
    // Other text-like files
    if matches!(ext, "json" | "xml" | "html" | "css" | "yaml" | "yml" | "toml") {
        return "text".to_string();
    }
    
    // Binary/unknown
    "binary".to_string()
}

fn extract_preview_content(
    file_path: &PathBuf,
    file_type: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let file_path_str = file_path.to_string_lossy();
    
    match file_type {
        "text" | "code" => {
            // Use text parser for code files too
            let parser: &dyn DocumentParser = &TextParser;
            parser.extract_text(&file_path_str).map_err(|e| e.into())
        }
        "pdf" => {
            // Use PDF parser
            let parser: &dyn DocumentParser = &PdfParser;
            parser.extract_text(&file_path_str).map_err(|e| e.into())
        }
        "docx" => {
            // Use DOCX parser
            let parser: &dyn DocumentParser = &DocxParser;
            parser.extract_text(&file_path_str).map_err(|e| e.into())
        }
        "image" => {
            // For images, we don't extract text content
            // The frontend will handle image display
            Ok("".to_string())
        }
        _ => Err("Unsupported file type for preview".into()),
    }
}
