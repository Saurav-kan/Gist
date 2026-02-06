use axum::{
    extract::{Query, State},
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::AppState;

#[derive(Serialize)]
pub struct ImageEmbeddingTestResponse {
    supported: bool,
    message: String,
    tested_formats: Vec<String>,
}

/// Test endpoint to check if Ollama supports image embeddings
/// Usage: GET /api/test/image-embedding?image_path=C:/path/to/image.jpg
pub async fn test_image_embedding(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<ImageEmbeddingTestResponse>, axum::http::StatusCode> {
    let image_path = params
        .get("image_path")
        .ok_or(axum::http::StatusCode::BAD_REQUEST)?;

    let embedding_service = crate::embedding::EmbeddingService::new(
        state.config.embedding_model.clone()
    );

    match embedding_service.test_image_embedding_support(image_path).await {
        Ok(supported) => {
            let message = if supported {
                "Ollama's /api/embeddings endpoint supports image embeddings!".to_string()
            } else {
                "Ollama's /api/embeddings endpoint does NOT support image embeddings. Consider using a Rust-native solution.".to_string()
            };

            Ok(Json(ImageEmbeddingTestResponse {
                supported,
                message,
                tested_formats: vec![
                    "images array".to_string(),
                    "image field".to_string(),
                    "base64 in prompt".to_string(),
                ],
            }))
        }
        Err(e) => {
            Ok(Json(ImageEmbeddingTestResponse {
                supported: false,
                message: format!("Test failed: {}", e),
                tested_formats: vec![],
            }))
        }
    }
}
