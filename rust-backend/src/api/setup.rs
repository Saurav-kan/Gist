use axum::{
    extract::{State, Json},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use sysinfo::System;
use reqwest::Client;
use futures::StreamExt;
use std::process::Command;
use crate::AppState;

const OLLAMA_BASE_URL: &str = "http://localhost:11434";

#[derive(Debug, Serialize)]
pub struct SetupStatusResponse {
    pub ollama_running: bool,
    pub embedding_model_installed: bool,
    pub llm_installed: bool,
    pub system_ram_gb: u64,
    pub recommended_embedding_model: String,
    pub recommended_llm: String,
    pub current_embedding_model: String,
    pub current_llm: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PullModelRequest {
    pub model: String,
}

#[derive(Debug, Serialize)]
pub struct PullModelResponse {
    pub success: bool,
    pub message: String,
}

pub async fn get_setup_status(State(state): State<AppState>) -> impl IntoResponse {
    let client = Client::new();
    
    // Check if Ollama is running
    let ollama_running = check_ollama_running(&client).await;
    
    // Check system RAM
    let mut sys = System::new_all();
    sys.refresh_memory();
    let total_memory = sys.total_memory();
    let ram_gb = total_memory / (1024 * 1024 * 1024);
    
    // Determine recommendations
    let (rec_embedding, rec_llm) = recommend_models(ram_gb);
    
    // Check installed models
    let mut embedding_installed = false;
    let mut llm_installed = false;
    
    let current_embedding_model = state.config.embedding_model.clone();
    let current_llm = state.config.ollama_model.clone();

    if ollama_running {
        let installed_models = get_installed_models(&client).await.unwrap_or_default();
        
        // Check embedding model
        embedding_installed = installed_models.iter().any(|m| m.starts_with(&current_embedding_model));
        
        // Check LLM if configured
        if let Some(ref llm) = current_llm {
            llm_installed = installed_models.iter().any(|m| m.starts_with(llm));
        }
    }

    Json(SetupStatusResponse {
        ollama_running,
        embedding_model_installed: embedding_installed,
        llm_installed,
        system_ram_gb: ram_gb,
        recommended_embedding_model: rec_embedding,
        recommended_llm: rec_llm,
        current_embedding_model,
        current_llm,
    })
}

pub async fn pull_model(
    Json(payload): Json<PullModelRequest>,
) -> impl IntoResponse {
    // We trigger the pull via command line for simplicity, or we could use the API
    // Using API is better for progress tracking, but for now let's just trigger it.
    // Actually, spawning a command is safer to detach from the request.
    
    // However, users want to see progress. A simple background spawn with no feedback 
    // might be confusing. For MVP, we'll try to use the API and maybe stream later,
    // or just return success and let the client poll for status.
    
    // Let's use `ollama pull` command for reliability if available globally,
    // or fallback to API.
    
    // Using the API is more robust across platforms if users didn't add ollama to PATH
    let client = Client::new();
    let url = format!("{}/api/pull", OLLAMA_BASE_URL);
    
    // We'll spawn a tokio task to handle the long-running pull
    let model = payload.model.clone();
    tokio::spawn(async move {
        let req = serde_json::json!({
            "name": model,
            "stream": false 
        });
        
        // We are not streaming here for now, just waiting. 
        // In a real app we'd want a progress websocket or similar.
        // For now, the frontend will show a spinner.
        let _ = client.post(url).json(&req).send().await;
    });

    Json(PullModelResponse {
        success: true,
        message: format!("Started downloading model: {}", payload.model),
    })
}

async fn check_ollama_running(client: &Client) -> bool {
    client.get(OLLAMA_BASE_URL).send().await.is_ok()
}

async fn get_installed_models(client: &Client) -> Result<Vec<String>, anyhow::Error> {
    let url = format!("{}/api/tags", OLLAMA_BASE_URL);
    let resp = client.get(url).send().await?;
    
    if !resp.status().is_success() {
        return Ok(Vec::new());
    }

    #[derive(Deserialize)]
    struct TagsResponse {
        models: Vec<ModelInfo>,
    }
    
    #[derive(Deserialize)]
    struct ModelInfo {
        name: String,
    }

    let tags: TagsResponse = resp.json().await?;
    Ok(tags.models.into_iter().map(|m| m.name).collect())
}

fn recommend_models(ram_gb: u64) -> (String, String) {
    if ram_gb < 13 {
        (
            "all-minilm".to_string(),
            "llama3.2:1b".to_string()
        )
    } else {
        (
            "embeddinggemma".to_string(), // Better quality, requires more RAM
            "llama3.2".to_string() // Standard 3B model or similar
        )
    }
}
