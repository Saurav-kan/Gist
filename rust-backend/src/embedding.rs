use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const OLLAMA_URL: &str = "http://localhost:11434";

#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    model: String,
    prompt: String,
}

#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    embedding: Vec<f32>,
}

pub struct EmbeddingService {
    client: Client,
    model: String,
}

impl EmbeddingService {
    pub fn new(model: String) -> Self {
        Self {
            client: Client::new(),
            model,
        }
    }

    pub async fn generate_embedding(&self, text: &str) -> Result<Vec<f32>> {
        let request = EmbeddingRequest {
            model: self.model.clone(),
            prompt: text.to_string(),
        };

        let response = self
            .client
            .post(&format!("{}/api/embeddings", OLLAMA_URL))
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama API error: {} - {}", status, error_text);
        }

        let embedding_response: EmbeddingResponse = response.json().await?;
        Ok(embedding_response.embedding)
    }

    pub async fn check_model_available(&self) -> Result<bool> {
        let response = self
            .client
            .get(&format!("{}/api/tags", OLLAMA_URL))
            .send()
            .await?;

        if !response.status().is_success() {
            return Ok(false);
        }

        let models: serde_json::Value = response.json().await?;
        if let Some(models_array) = models.get("models").and_then(|v| v.as_array()) {
            for model in models_array {
                if let Some(name) = model.get("name").and_then(|v| v.as_str()) {
                    if name == self.model || name.starts_with(&format!("{}:", self.model)) {
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }
}
