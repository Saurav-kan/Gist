use axum::{
    extract::{State, Query},
    response::Json,
};
use serde::{Deserialize, Serialize};
use crate::AppState;
use crate::config::AiProvider;

const OLLAMA_BASE_URL: &str = "http://localhost:11434";

#[derive(Deserialize)]
pub struct SummarizeRequest {
    pub file_path: String,
}

#[derive(Deserialize)]
pub struct ChatRequest {
    pub file_path: String,
    pub message: String,
    pub conversation_history: Option<Vec<ChatMessage>>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String, // "user" or "assistant"
    pub content: String,
}

#[derive(Serialize)]
pub struct SummarizeResponse {
    pub success: bool,
    pub summary: Option<String>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct ChatResponse {
    pub success: bool,
    pub message: Option<String>,
    pub error: Option<String>,
}

// Summarize document using Ollama
pub async fn summarize_document(
    State(state): State<AppState>,
    Json(request): Json<SummarizeRequest>,
) -> Result<Json<SummarizeResponse>, axum::http::StatusCode> {
    // Reload config from disk to ensure we have the latest settings
    let config = match crate::config::AppConfig::load_or_default().await {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("[AI] Error loading config from disk: {}", e);
            // Fallback to in-memory config if disk read fails
            state.config.as_ref().clone()
        }
    };
    
    eprintln!("[AI] summarize_document: ai_features_enabled = {}", config.ai_features_enabled);
    
    // Check if AI features are enabled
    if !config.ai_features_enabled {
        return Ok(Json(SummarizeResponse {
            success: false,
            summary: None,
            error: Some("AI features are disabled in settings".to_string()),
        }));
    }

    // Get file content from preview endpoint logic
    let content = match get_file_content_for_ai(&request.file_path).await {
        Ok(c) => c,
        Err(e) => {
            return Ok(Json(SummarizeResponse {
                success: false,
                summary: None,
                error: Some(format!("Failed to read file: {}", e)),
            }));
        }
    };

    if content.is_empty() {
        return Ok(Json(SummarizeResponse {
            success: false,
            summary: None,
            error: Some("File is empty or cannot be read".to_string()),
        }));
    }

    // Create summarize prompt
    let prompt = format!(
        "Please provide a concise summary of the following document. Focus on the main points, key information, and important details:\n\n{}",
        content
    );

    // Call appropriate API based on provider
    let result = match config.ai_provider {
        AiProvider::Ollama => {
            let model = config.ollama_model.as_deref()
                .unwrap_or("llama3.2:1b");
            eprintln!("[AI] Calling Ollama (model: {}) for summary", model);
            call_ollama_generate(model, &prompt, false).await
        }
        AiProvider::GreenPT => {
            let api_key = config.api_key.as_ref()
                .ok_or_else(|| axum::http::StatusCode::BAD_REQUEST)?;
            eprintln!("[AI] Calling GreenPT for summary");
            call_greenpt_chat_single(api_key, &prompt).await
        }
        AiProvider::OpenAI => {
            return Ok(Json(SummarizeResponse {
                success: false,
                summary: None,
                error: Some("OpenAI provider not yet implemented".to_string()),
            }));
        }
        AiProvider::Gemini => {
            let api_key = config.api_key.as_ref()
                .ok_or_else(|| axum::http::StatusCode::BAD_REQUEST)?;
            let model = config.gemini_model.as_deref()
                .unwrap_or("gemini-pro");
            eprintln!("[AI] Calling Gemini (model: {}) for summary", model);
            call_gemini_chat_single(api_key, model, &prompt).await
        }
    };

    match result {
        Ok(summary) => Ok(Json(SummarizeResponse {
            success: true,
            summary: Some(summary),
            error: None,
        })),
        Err(e) => Ok(Json(SummarizeResponse {
            success: false,
            summary: None,
            error: Some(format!("Failed to generate summary: {}", e)),
        })),
    }
}

// Chat about document using Ollama
pub async fn chat_about_document(
    State(state): State<AppState>,
    Json(request): Json<ChatRequest>,
) -> Result<Json<ChatResponse>, axum::http::StatusCode> {
    // Reload config from disk to ensure we have the latest settings
    let config = match crate::config::AppConfig::load_or_default().await {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("[AI] Error loading config from disk: {}", e);
            // Fallback to in-memory config if disk read fails
            state.config.as_ref().clone()
        }
    };
    
    eprintln!("[AI] chat_about_document: ai_features_enabled = {}", config.ai_features_enabled);
    
    // Check if AI features are enabled
    if !config.ai_features_enabled {
        return Ok(Json(ChatResponse {
            success: false,
            message: None,
            error: Some("AI features are disabled in settings".to_string()),
        }));
    }

    // Get file content
    let content = match get_file_content_for_ai(&request.file_path).await {
        Ok(c) => c,
        Err(e) => {
            return Ok(Json(ChatResponse {
                success: false,
                message: None,
                error: Some(format!("Failed to read file: {}", e)),
            }));
        }
    };

    if content.is_empty() {
        return Ok(Json(ChatResponse {
            success: false,
            message: None,
            error: Some("File is empty or cannot be read".to_string()),
        }));
    }

    // Build conversation context
    let mut messages = Vec::new();
    
    // System message with document context
    messages.push(ChatMessage {
        role: "system".to_string(),
        content: format!(
            "You are a helpful assistant. The user is asking questions about the following document. Use the document content to answer their questions accurately.\n\nDocument content:\n{}",
            content
        ),
    });

    // Add conversation history if provided
    if let Some(history) = request.conversation_history {
        for msg in history {
            messages.push(msg);
        }
    }

    // Add current user message
    messages.push(ChatMessage {
        role: "user".to_string(),
        content: request.message,
    });

    // Call appropriate API based on provider
    let result = match config.ai_provider {
        AiProvider::Ollama => {
            let model = config.ollama_model.as_deref()
                .unwrap_or("llama3.2:1b");
            call_ollama_chat(model, &messages).await
        }
        AiProvider::GreenPT => {
            let api_key = config.api_key.as_ref()
                .ok_or_else(|| axum::http::StatusCode::BAD_REQUEST)?;
            call_greenpt_chat(api_key, &messages).await
        }
        AiProvider::OpenAI => {
            return Ok(Json(ChatResponse {
                success: false,
                message: None,
                error: Some("OpenAI provider not yet implemented".to_string()),
            }));
        }
        AiProvider::Gemini => {
            let api_key = config.api_key.as_ref()
                .ok_or_else(|| axum::http::StatusCode::BAD_REQUEST)?;
            let model = config.gemini_model.as_deref()
                .unwrap_or("gemini-pro");
            call_gemini_chat(api_key, model, &messages).await
        }
    };

    match result {
        Ok(response) => Ok(Json(ChatResponse {
            success: true,
            message: Some(response),
            error: None,
        })),
        Err(e) => Ok(Json(ChatResponse {
            success: false,
            message: None,
            error: Some(format!("Failed to get AI response: {}", e)),
        })),
    }
}

// Helper function to get file content for AI processing
async fn get_file_content_for_ai(file_path: &str) -> Result<String, Box<dyn std::error::Error>> {
    use crate::parsers::ParserRegistry;
    use crate::config::FileTypeFilters;
    
    let path = std::path::PathBuf::from(file_path);
    if !path.exists() {
        return Err("File does not exist".into());
    }

    if path.is_dir() {
        return Err("Cannot process directories".into());
    }

    // Use parser registry to extract text
    // Create default filters (include all file types for AI processing)
    let filters = FileTypeFilters {
        include_pdf: true,
        include_docx: true,
        include_text: true,
        include_xlsx: true,
        excluded_extensions: Vec::new(),
    };
    let registry = ParserRegistry::new(&filters);
    
    // Try to extract text using the registry's public API
    if registry.can_parse(file_path) {
        match registry.extract_text(file_path) {
            Ok(text) => return Ok(text),
            Err(e) => return Err(format!("Failed to parse file: {}", e).into()),
        }
    }

    // If no parser found, try to read as plain text
    match tokio::fs::read_to_string(file_path).await {
        Ok(content) => Ok(content),
        Err(e) => Err(format!("Failed to read file: {}", e).into()),
    }
}

// Call Ollama generate endpoint
async fn call_ollama_generate(
    model: &str,
    prompt: &str,
    stream: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    use reqwest::Client;
    
    #[derive(Serialize)]
    struct GenerateRequest {
        model: String,
        prompt: String,
        stream: bool,
    }

    #[derive(Deserialize)]
    struct GenerateResponse {
        response: String,
    }

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    let url = format!("{}/api/generate", OLLAMA_BASE_URL);
    
    let request_body = GenerateRequest {
        model: model.to_string(),
        prompt: prompt.to_string(),
        stream,
    };

    let response = client
        .post(&url)
        .json(&request_body)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Ollama API error: {}", response.status()).into());
    }

    let generate_response: GenerateResponse = response.json().await?;
    Ok(generate_response.response)
}

// Call Ollama chat endpoint
pub(crate) async fn call_ollama_chat(
    model: &str,
    messages: &[ChatMessage],
) -> Result<String, Box<dyn std::error::Error>> {
    use reqwest::Client;
    
    #[derive(Serialize)]
    struct ChatRequest {
        model: String,
        messages: Vec<ChatMessage>,
        stream: bool,
    }

    #[derive(Deserialize)]
    struct ChatMessageResponse {
        role: String,
        content: String,
    }

    #[derive(Deserialize)]
    struct ChatResponse {
        message: ChatMessageResponse,
    }

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    let url = format!("{}/api/chat", OLLAMA_BASE_URL);
    
    let request_body = ChatRequest {
        model: model.to_string(),
        messages: messages.to_vec(),
        stream: false,
    };

    let response = client
        .post(&url)
        .json(&request_body)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Ollama API error: {}", response.status()).into());
    }

    let chat_response: ChatResponse = response.json().await?;
    Ok(chat_response.message.content)
}

// Call GreenPT API (OpenAI-compatible endpoint)
pub(crate) async fn call_greenpt_chat(
    api_key: &str,
    messages: &[ChatMessage],
) -> Result<String, Box<dyn std::error::Error>> {
    use reqwest::Client;
    
    const GREENPT_BASE_URL: &str = "https://api.greenpt.ai/v1";
    
    #[derive(Serialize)]
    struct GreenPTMessage {
        role: String,
        content: String,
    }
    
    #[derive(Serialize)]
    struct GreenPTChatRequest {
        model: String,
        messages: Vec<GreenPTMessage>,
        temperature: f32,
        max_tokens: Option<u32>,
    }

    #[derive(Deserialize)]
    struct ChoiceMessage {
        role: String,
        content: String,
    }
    
    #[derive(Deserialize)]
    struct Choice {
        message: ChoiceMessage,
    }

    #[derive(Deserialize)]
    struct GreenPTChatResponse {
        choices: Vec<Choice>,
    }

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    let url = format!("{}/chat/completions", GREENPT_BASE_URL);
    
    // Convert messages to GreenPT format
    let greenpt_messages: Vec<GreenPTMessage> = messages
        .iter()
        .map(|m| GreenPTMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();
    
    let request_body = GreenPTChatRequest {
        model: "greenpt".to_string(), // Default model, can be made configurable
        messages: greenpt_messages,
        temperature: 0.7,
        max_tokens: Some(2000),
    };

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("GreenPT API error: {} - {}", status, error_text).into());
    }

    let chat_response: GreenPTChatResponse = response.json().await?;
    
    if let Some(choice) = chat_response.choices.first() {
        Ok(choice.message.content.clone())
    } else {
        Err("No response from GreenPT API".into())
    }
}

// Call GreenPT for single prompt (summarize)
async fn call_greenpt_chat_single(
    api_key: &str,
    prompt: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let messages = vec![ChatMessage {
        role: "user".to_string(),
        content: prompt.to_string(),
    }];
    call_greenpt_chat(api_key, &messages).await
}

// Fetch available Gemini models
pub async fn get_gemini_models(
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, axum::http::StatusCode> {
    use reqwest::Client;
    
    const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";
    
    let api_key = params.get("api_key")
        .ok_or(axum::http::StatusCode::BAD_REQUEST)?;
    
    let client = Client::new();
    let url = format!("{}/models", GEMINI_BASE_URL);
    
    let response = client
        .get(&url)
        .query(&[("key", api_key)])
        .send()
        .await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let status = response.status();
    if !status.is_success() {
        let _error_text = response.text().await.unwrap_or_default();
        eprintln!("Failed to fetch Gemini models: HTTP {}", status);
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }
    
    let models_response: serde_json::Value = response.json().await
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Extract model names from the response
    let models = models_response
        .get("models")
        .and_then(|m| m.as_array())
        .map(|models| {
            models.iter()
                .filter_map(|model| {
                    let name = model.get("name")?.as_str()?;
                    // Extract model ID from full name (e.g., "models/gemini-pro" -> "gemini-pro")
                    name.split('/').last().map(|s| s.to_string())
                })
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();
    
    Ok(Json(serde_json::json!({
        "success": true,
        "models": models
    })))
}

// Call Gemini API for chat
pub(crate) async fn call_gemini_chat(
    api_key: &str,
    model: &str,
    messages: &[ChatMessage],
) -> Result<String, Box<dyn std::error::Error>> {
    use reqwest::Client;
    
    const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";
    
    #[derive(Serialize)]
    struct GeminiPart {
        text: String,
    }
    
    #[derive(Serialize)]
    struct GeminiContent {
        parts: Vec<GeminiPart>,
        role: Option<String>, // "user" or "model"
    }
    
    #[derive(Serialize)]
    struct GeminiRequest {
        contents: Vec<GeminiContent>,
    }

    #[derive(Deserialize)]
    struct GeminiPartResponse {
        text: String,
    }
    
    #[derive(Deserialize)]
    struct GeminiCandidate {
        content: GeminiContentResponse,
    }
    
    #[derive(Deserialize)]
    struct GeminiContentResponse {
        parts: Vec<GeminiPartResponse>,
    }
    
    #[derive(Deserialize)]
    struct GeminiResponse {
        candidates: Vec<GeminiCandidate>,
    }

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    let url = format!("{}/models/{}:generateContent", GEMINI_BASE_URL, model);
    
    // Convert messages to Gemini format
    // Gemini uses a different format - we need to convert our messages
    // System messages are typically prepended to the first user message
    let mut contents = Vec::new();
    let mut system_message: Option<String> = None;
    
    for msg in messages {
        match msg.role.as_str() {
            "system" => {
                // Store system message to prepend to first user message
                system_message = Some(msg.content.clone());
            }
            "user" => {
                // Combine system message with user message if present
                let text = if let Some(sys_msg) = system_message.take() {
                    format!("{}\n\n{}", sys_msg, msg.content)
                } else {
                    msg.content.clone()
                };
                
                contents.push(GeminiContent {
                    parts: vec![GeminiPart { text }],
                    role: Some("user".to_string()),
                });
            }
            "assistant" => {
                contents.push(GeminiContent {
                    parts: vec![GeminiPart {
                        text: msg.content.clone(),
                    }],
                    role: Some("model".to_string()),
                });
            }
            _ => {
                // Treat unknown roles as user messages
                contents.push(GeminiContent {
                    parts: vec![GeminiPart {
                        text: msg.content.clone(),
                    }],
                    role: Some("user".to_string()),
                });
            }
        }
    }
    
    // If we have a system message but no user messages, create one
    if let Some(sys_msg) = system_message {
        contents.push(GeminiContent {
            parts: vec![GeminiPart { text: sys_msg }],
            role: Some("user".to_string()),
        });
    }
    
    let request_body = GeminiRequest {
        contents,
    };

    let response = client
        .post(&url)
        .query(&[("key", api_key)])
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await?;
    
    eprintln!("[AI] Gemini response status: {}", response.status());

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Gemini API error: {} - {}", status, error_text).into());
    }

    let gemini_response: GeminiResponse = response.json().await?;
    
    if let Some(candidate) = gemini_response.candidates.first() {
        if let Some(part) = candidate.content.parts.first() {
            Ok(part.text.clone())
        } else {
            Err("No content in Gemini response".into())
        }
    } else {
        Err("No candidates in Gemini response".into())
    }
}

// Call Gemini for single prompt (summarize)
async fn call_gemini_chat_single(
    api_key: &str,
    model: &str,
    prompt: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let messages = vec![ChatMessage {
        role: "user".to_string(),
        content: prompt.to_string(),
    }];
    call_gemini_chat(api_key, model, &messages).await
}
