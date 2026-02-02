use axum::{
    extract::State,
    response::Json,
};
use serde::Serialize;

use crate::AppState;

#[derive(Serialize)]
pub struct FilesResponse {
    files: Vec<FileInfo>,
    total_count: usize,
}

#[derive(Serialize)]
pub struct FileInfo {
    id: i64,
    file_path: String,
    file_name: String,
    file_size: i64,
    file_type: String,
    modified_time: i64,
    embedding_dimensions: Option<usize>,
}

pub async fn list_files(State(state): State<AppState>) -> Result<Json<FilesResponse>, axum::http::StatusCode> {
    let files = state.storage.get_all_files()
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;

    let file_infos: Vec<FileInfo> = files.iter().map(|f| {
        // Calculate embedding dimensions from length (each f32 is 4 bytes)
        let embedding_dimensions = if f.embedding_length > 0 {
            Some((f.embedding_length / 4) as usize)
        } else {
            None
        };

        FileInfo {
            id: f.id,
            file_path: f.file_path.clone(),
            file_name: f.file_name.clone(),
            file_size: f.file_size,
            file_type: f.file_type.clone(),
            modified_time: f.modified_time,
            embedding_dimensions,
        }
    }).collect();

    Ok(Json(FilesResponse {
        total_count: file_infos.len(),
        files: file_infos,
    }))
}
