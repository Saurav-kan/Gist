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

    /// Test if Ollama's /api/embeddings endpoint supports image embeddings
    /// Returns Ok(true) if images are supported, Ok(false) if not, or Err if test failed
    pub async fn test_image_embedding_support(&self, image_path: &str) -> Result<bool> {
        use std::fs;
        use base64::{Engine as _, engine::general_purpose::STANDARD};

        // Read image file
        let image_data = fs::read(image_path)
            .map_err(|e| anyhow::anyhow!("Failed to read image file {}: {}", image_path, e))?;
        
        // Encode as base64
        let base64_image = STANDARD.encode(&image_data);
        
        // Try multiple possible request formats that Ollama might support
        
        // Format 1: images array in request
        let request_format1 = serde_json::json!({
            "model": self.model.clone(),
            "prompt": "",
            "images": [base64_image.clone()]
        });
        
        // Format 2: image field in request
        let request_format2 = serde_json::json!({
            "model": self.model.clone(),
            "prompt": "",
            "image": base64_image.clone()
        });
        
        // Format 3: images as part of prompt (multimodal format)
        let request_format3 = serde_json::json!({
            "model": self.model.clone(),
            "prompt": format!("data:image/png;base64,{}", base64_image)
        });
        
        // Try format 1
        let response1 = self
            .client
            .post(&format!("{}/api/embeddings", OLLAMA_URL))
            .json(&request_format1)
            .send()
            .await;
        
        if let Ok(resp) = response1 {
            if resp.status().is_success() {
                if let Ok(embedding_resp) = resp.json::<EmbeddingResponse>().await {
                    if !embedding_resp.embedding.is_empty() {
                        eprintln!("[IMAGE_EMBEDDING_TEST] Format 1 (images array) succeeded!");
                        return Ok(true);
                    }
                }
            }
        }
        
        // Try format 2
        let response2 = self
            .client
            .post(&format!("{}/api/embeddings", OLLAMA_URL))
            .json(&request_format2)
            .send()
            .await;
        
        if let Ok(resp) = response2 {
            if resp.status().is_success() {
                if let Ok(embedding_resp) = resp.json::<EmbeddingResponse>().await {
                    if !embedding_resp.embedding.is_empty() {
                        eprintln!("[IMAGE_EMBEDDING_TEST] Format 2 (image field) succeeded!");
                        return Ok(true);
                    }
                }
            }
        }
        
        // Try format 3
        let response3 = self
            .client
            .post(&format!("{}/api/embeddings", OLLAMA_URL))
            .json(&request_format3)
            .send()
            .await;
        
        if let Ok(resp) = response3 {
            if resp.status().is_success() {
                if let Ok(embedding_resp) = resp.json::<EmbeddingResponse>().await {
                    if !embedding_resp.embedding.is_empty() {
                        eprintln!("[IMAGE_EMBEDDING_TEST] Format 3 (base64 in prompt) succeeded!");
                        return Ok(true);
                    }
                }
            }
        }
        
        eprintln!("[IMAGE_EMBEDDING_TEST] All formats failed - images not supported by /api/embeddings endpoint");
        Ok(false)
    }
}
