use axum::{
    extract::State,
    response::Json,
};
use serde::Serialize;

use crate::AppState;
use crate::query_parser::{ParsedQuery, QueryParser};

#[derive(Serialize)]
pub struct ParseResponse {
    success: bool,
    data: ParsedQuery,
}

pub async fn parse_query(
    State(_state): State<AppState>,
    Json(request): Json<serde_json::Value>,
) -> Result<Json<ParseResponse>, axum::http::StatusCode> {
    let query = request
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or(axum::http::StatusCode::BAD_REQUEST)?;

    // Create parser with LLM model (use llama3.2:1b for parsing)
    let parser = QueryParser::new("llama3.2:1b".to_string());
    
    // Parse query (will try pattern matching first, then LLM if needed)
    // If LLM fails, it falls back to pattern matching automatically
    let parsed = parser.parse(query).await;

    Ok(Json(ParseResponse {
        success: true,
        data: parsed,
    }))
}
