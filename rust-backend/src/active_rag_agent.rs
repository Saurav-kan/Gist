use serde::{Deserialize, Serialize};
use crate::config::AiProvider;
use crate::api::ai::{call_ollama_chat, call_greenpt_chat, call_gemini_chat, ChatMessage};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecomposedIntent {
    pub vector_query: String,
    pub action_question: String,
    pub filters: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveRagRequest {
    pub query: String,
    pub user_question: String,
    pub document_limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveRagResponse {
    pub success: bool,
    pub answer: Option<String>,
    pub sources: Vec<ActiveRagSource>,
    pub action_performed: Option<String>,
    pub confidence: Option<f32>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveRagSource {
    pub file_path: String,
    pub file_name: String,
    pub relevance_score: f32,
    pub used_in_answer: bool,
    pub excerpt: Option<String>,
    pub key_contributions: Option<Vec<String>>,
    pub comparison_data: Option<ComparisonData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonData {
    pub similarities: Vec<DocumentSimilarity>,
    pub unique_insights: Vec<String>,
    pub contradictions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSimilarity {
    pub doc1_path: String,
    pub doc2_path: String,
    pub similarity_score: f32,
    pub shared_topics: Vec<String>,
}

pub struct ActiveRagAgent {
    ai_provider: AiProvider,
    ollama_model: Option<String>,
    gemini_model: Option<String>,
    api_key: Option<String>,
}

impl ActiveRagAgent {
    pub fn new(
        ai_provider: AiProvider,
        ollama_model: Option<String>,
        gemini_model: Option<String>,
        api_key: Option<String>,
    ) -> Self {
        Self {
            ai_provider,
            ollama_model,
            gemini_model,
            api_key,
        }
    }

    pub async fn analyze_documents(
        &self,
        documents: Vec<(String, String, f32)>,
        user_question: &str,
        original_query: &str,
        analysis_model: &str,
    ) -> Result<ActiveRagResponse, Box<dyn std::error::Error>> {
        if documents.is_empty() {
            return Ok(ActiveRagResponse {
                success: false,
                answer: None,
                sources: vec![],
                action_performed: None,
                confidence: None,
                error: Some("No documents to analyze".to_string()),
            });
        }

        eprintln!("[Active RAG Agent] analyze_documents called with {} documents", documents.len());
        eprintln!("[Active RAG Agent] User question: '{}'", user_question);
        eprintln!("[Active RAG Agent] Original query: '{}'", original_query);
        eprintln!("[Active RAG Agent] Analysis model setting: '{}'", analysis_model);
        
        // Log document details
        for (i, (path, content, score)) in documents.iter().enumerate() {
            let file_name = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            eprintln!("[Active RAG Agent]   Document {}: {} (score: {:.4}, content: {} chars)", 
                i + 1, file_name, score, content.len());
        }
        
        // Create system prompt for document analysis
        let system_prompt = self.create_analysis_prompt(&documents, user_question, original_query);
        
        eprintln!("[Active RAG Agent] === AI PROMPT CREATED ===");
        eprintln!("[Active RAG Agent] System prompt length: {} chars", system_prompt.len());
        let prompt_preview = if system_prompt.len() > 500 {
            &system_prompt[..500]
        } else {
            &system_prompt
        };
        eprintln!("[Active RAG Agent] System prompt preview:\n{}...", prompt_preview);
        eprintln!("[Active RAG Agent] ==============================");

        // Build conversation messages
        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_question.to_string(),
            },
        ];

        // Select AI provider based on analysis model setting
        eprintln!("[Active RAG Agent] Calling AI API with {} messages", messages.len());
        let ai_response = match analysis_model {
            "same-as-main" => {
                eprintln!("[Active RAG Agent] Using 'same-as-main' provider: {:?}", self.ai_provider);
                // Use the same AI provider as configured for main
                match self.ai_provider {
                    AiProvider::Ollama => {
                        let model = self.ollama_model.as_deref().unwrap_or("llama3.2:1b");
                        eprintln!("[Active RAG Agent] Calling Ollama with model: {} (timeout: 60s)", model);
                        match tokio::time::timeout(
                            std::time::Duration::from_secs(60),
                            call_ollama_chat(model, &messages)
                        ).await {
                            Ok(Ok(response)) => {
                                eprintln!("[Active RAG Agent] ✓ Ollama response received");
                                response
                            }
                            Ok(Err(e)) => {
                                eprintln!("[Active RAG Agent] ✗ Ollama API error: {}", e);
                                return Err(format!("Ollama API error: {}", e).into());
                            }
                            Err(_) => {
                                eprintln!("[Active RAG Agent] ✗ Ollama API call timed out after 60 seconds");
                                return Err("Ollama API call timed out after 60 seconds".into());
                            }
                        }
                    }
                    AiProvider::GreenPT => {
                        let api_key = self.api_key.as_ref().ok_or("GreenPT API key not configured")?;
                        eprintln!("[Active RAG Agent] Calling GreenPT (timeout: 60s)");
                        match tokio::time::timeout(
                            std::time::Duration::from_secs(60),
                            call_greenpt_chat(api_key, &messages)
                        ).await {
                            Ok(Ok(response)) => {
                                eprintln!("[Active RAG Agent] ✓ GreenPT response received");
                                response
                            }
                            Ok(Err(e)) => {
                                eprintln!("[Active RAG Agent] ✗ GreenPT API error: {}", e);
                                return Err(format!("GreenPT API error: {}", e).into());
                            }
                            Err(_) => {
                                eprintln!("[Active RAG Agent] ✗ GreenPT API call timed out after 60 seconds");
                                return Err("GreenPT API call timed out after 60 seconds".into());
                            }
                        }
                    }
                    AiProvider::Gemini => {
                        let api_key = self.api_key.as_ref().ok_or("Gemini API key not configured")?;
                        let model = self.gemini_model.as_deref().unwrap_or("gemini-pro");
                        eprintln!("[Active RAG Agent] Calling Gemini with model: {} (timeout: 60s)", model);
                        match tokio::time::timeout(
                            std::time::Duration::from_secs(60),
                            call_gemini_chat(api_key, model, &messages)
                        ).await {
                            Ok(Ok(response)) => {
                                eprintln!("[Active RAG Agent] ✓ Gemini response received");
                                response
                            }
                            Ok(Err(e)) => {
                                eprintln!("[Active RAG Agent] ✗ Gemini API error: {}", e);
                                return Err(format!("Gemini API error: {}", e).into());
                            }
                            Err(_) => {
                                eprintln!("[Active RAG Agent] ✗ Gemini API call timed out after 60 seconds");
                                return Err("Gemini API call timed out after 60 seconds".into());
                            }
                        }
                    }
                    AiProvider::OpenAI => return Err("OpenAI provider not yet implemented for Active RAG".into()),
                }
            }
            "ollama" => {
                // Force use Ollama for analysis
                // Use configured model if present; default to a fast local model
                let model = self.ollama_model.as_deref().unwrap_or("llama3.2:1b");
                eprintln!("[Active RAG Agent] Forcing Ollama with model: {} (timeout: 60s)", model);
                match tokio::time::timeout(
                    std::time::Duration::from_secs(60),
                    call_ollama_chat(model, &messages)
                ).await {
                    Ok(Ok(response)) => {
                        eprintln!("[Active RAG Agent] ✓ Ollama response received");
                        response
                    }
                    Ok(Err(e)) => {
                        eprintln!("[Active RAG Agent] ✗ Ollama API error: {}", e);
                        return Err(format!("Ollama API error: {}", e).into());
                    }
                    Err(_) => {
                        eprintln!("[Active RAG Agent] ✗ Ollama API call timed out after 60 seconds");
                        return Err("Ollama API call timed out after 60 seconds".into());
                    }
                }
            }
            "gemini" => {
                // Force use Gemini for analysis
                let api_key = self.api_key.as_ref().ok_or("Gemini API key not configured")?;
                let model = self.gemini_model.as_deref().unwrap_or("gemini-pro");
                eprintln!("[Active RAG Agent] Forcing Gemini with model: {} (timeout: 60s)", model);
                match tokio::time::timeout(
                    std::time::Duration::from_secs(60),
                    call_gemini_chat(api_key, model, &messages)
                ).await {
                    Ok(Ok(response)) => {
                        eprintln!("[Active RAG Agent] ✓ Gemini response received");
                        response
                    }
                    Ok(Err(e)) => {
                        eprintln!("[Active RAG Agent] ✗ Gemini API error: {}", e);
                        return Err(format!("Gemini API error: {}", e).into());
                    }
                    Err(_) => {
                        eprintln!("[Active RAG Agent] ✗ Gemini API call timed out after 60 seconds");
                        return Err("Gemini API call timed out after 60 seconds".into());
                    }
                }
            }
            _ => {
                eprintln!("[Active RAG Agent] ERROR: Unsupported analysis model: {}", analysis_model);
                return Err(format!("Unsupported analysis model: {}", analysis_model).into());
            }
        };

        eprintln!("[Active RAG Agent] ✓ AI API call completed");
        eprintln!("[Active RAG Agent] Raw response length: {} chars", ai_response.len());
        
        // Validate response is not empty
        if ai_response.trim().is_empty() {
            eprintln!("[Active RAG Agent] ✗ ERROR: AI returned empty response!");
            return Err("AI returned empty response".into());
        }
        
        let response_preview = if ai_response.len() > 500 {
            &ai_response[..500]
        } else {
            &ai_response
        };
        eprintln!("[Active RAG Agent] Raw response preview:\n{}...", response_preview);

        // Parse AI response and create structured response
        eprintln!("[Active RAG Agent] Parsing AI response...");
        let parsed_response = self.parse_ai_response(ai_response, documents, user_question).await;
        
        match &parsed_response {
            Ok(resp) => {
                eprintln!("[Active RAG Agent] ✓ Response parsed successfully");
                eprintln!("[Active RAG Agent] Parsed response - success: {}, answer present: {}, sources: {}", 
                    resp.success, resp.answer.is_some(), resp.sources.len());
            }
            Err(e) => {
                eprintln!("[Active RAG Agent] ✗ Response parsing failed: {}", e);
            }
        }
        
        parsed_response
    }

    pub async fn decompose_intent(
        &self,
        user_prompt: &str,
        original_query: &str,
        parsing_model: &str,
    ) -> Result<DecomposedIntent, Box<dyn std::error::Error>> {
        let system_prompt = "You are an AI assistant that decomposes complex user requests for a file search system. \
            Your goal is to take a raw prompt and separate it into: \
            1. vector_query: A clean, concise string for semantic search focusing on the core topic/content. \
            2. action_question: The specific question or action the user wants to perform on the search results. \
            3. filters: A JSON object of extracted constraints like file types (e.g., pdf, docx), date ranges, or size. \
            \
            Example: 'Give me a random question about limits from my calculus homework' \
            Output (must be valid JSON, no markdown, no code blocks): \
            { \
              \"vector_query\": \"calculus homework limits\", \
              \"action_question\": \"Give me a random question from these homework documents\", \
              \"filters\": { \"file_type\": null } \
            } \
            \
            IMPORTANT: Return ONLY a valid JSON object. Do not include markdown code blocks, explanations, or any other text. \
            Start with { and end with }.";

        let user_message = format!(
            "Analyze this prompt: \"{}\"\nContext (Original Search): \"{}\"",
            user_prompt, original_query
        );

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: user_message,
            },
        ];

        let ai_response = match parsing_model {
            "ollama" => {
                // Use configured model if present; default to a fast local model for parsing
                let model = self.ollama_model.as_deref().unwrap_or("llama3.2:1b");
                call_ollama_chat(model, &messages).await?
            }
            "gemini" => {
                let api_key = self.api_key.as_ref().ok_or("Gemini API key not configured")?;
                let model = self.gemini_model.as_deref().unwrap_or("gemini-pro");
                call_gemini_chat(api_key, model, &messages).await?
            }
            _ => {
                return Err(format!("Unsupported parsing model: {}", parsing_model).into());
            }
        };

        // Extract JSON from response (handling potential markdown formatting)
        eprintln!("[Active RAG Agent] decompose_intent: Raw response length: {} chars", ai_response.len());
        
        // Try to extract JSON (similar to parse_ai_response)
        let json_str = if let Some(json_start) = ai_response.find("```json") {
            let after_start = &ai_response[json_start + 7..];
            if let Some(end_marker) = after_start.find("```") {
                let json_content = after_start[..end_marker].trim();
                eprintln!("[Active RAG Agent] Found JSON in markdown code block");
                json_content
            } else if let Some(start) = after_start.find('{') {
                if let Some(end) = after_start.rfind('}') {
                    &after_start[start..=end]
                } else {
                    &ai_response
                }
            } else {
                &ai_response
            }
        } else if let Some(start) = ai_response.find('{') {
            if let Some(end) = ai_response.rfind('}') {
                &ai_response[start..=end]
            } else {
                eprintln!("[Active RAG Agent] Found '{{' but no matching '}}'");
                &ai_response
            }
        } else {
            eprintln!("[Active RAG Agent] No '{{' found in decomposition response");
            &ai_response
        };
        
        eprintln!("[Active RAG Agent] Extracted JSON for decomposition: {} chars", json_str.len());
        
        match serde_json::from_str::<DecomposedIntent>(json_str) {
            Ok(decomposed) => {
                eprintln!("[Active RAG] Intent Decomposed: {:?}", decomposed);
                Ok(decomposed)
            }
            Err(e) => {
                eprintln!("[Active RAG Agent] JSON parse error: {}", e);
                eprintln!("[Active RAG Agent] Attempted to parse: {}", json_str);
                Err(format!("Failed to parse decomposition JSON: {}", e).into())
            }
        }
    }

    fn create_analysis_prompt(
        &self,
        documents: &Vec<(String, String, f32)>,
        user_question: &str,
        original_query: &str,
    ) -> String {
        let mut prompt = format!(
            "You are analyzing documents to answer a user's question. Here are the documents:\n\n"
        );

        for (i, (file_path, content, relevance_score)) in documents.iter().enumerate() {
            let file_name = std::path::Path::new(file_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            let truncated_content: String = content.chars().take(2000).collect();
            prompt.push_str(&format!(
                "Document {} ({}): Relevance Score: {:.3}\n{}\n\n",
                i + 1,
                file_name,
                relevance_score,
                truncated_content
            ));
        }

        prompt.push_str(&format!(
            "\nOriginal Search Query: \"{}\"\n", original_query
        ));
        prompt.push_str(&format!(
            "User Question: \"{}\"\n\n", user_question
        ));

        prompt.push_str(
            "Instructions:\n\
            1. Analyze all provided documents to answer the user's question\n\
            2. Select the most relevant document(s) for the answer\n\
            3. Provide a comprehensive answer based on the selected document(s)\n\
            4. For each document, identify key contributions to the answer\n\
            5. If multiple documents provide complementary information, synthesize them\n\
            6. Include specific citations and file references\n\
            7. Rate your confidence in the answer (0.0-1.0)\n\
            8. If documents contradict each other, note the contradictions\n\n\
            Please provide your response in the following JSON format:\n\
            {{\n\
              \"answer\": \"your comprehensive answer\",\n\
              \"confidence\": 0.85,\n\
              \"sources\": [\n\
                {{\n\
                  \"file_path\": \"path/to/file1\",\n\
                  \"used_in_answer\": true,\n\
                  \"key_contributions\": [\"point1\", \"point2\"],\n\
                  \"relevance_score\": 0.92\n\
                }}\n\
              ]\n\
            }}"
        );

        prompt
    }

    async fn parse_ai_response(
        &self,
        ai_response: String,
        documents: Vec<(String, String, f32)>,
        user_question: &str,
    ) -> Result<ActiveRagResponse, Box<dyn std::error::Error>> {
        eprintln!("[Active RAG Agent] parse_ai_response: Attempting to parse response...");
        
        // Try to extract JSON from response (handling markdown code blocks)
        // First, try to find JSON in markdown code blocks (```json ... ```)
        let json_str = if let Some(json_start) = ai_response.find("```json") {
            // Found markdown code block with json
            let after_start = &ai_response[json_start + 7..]; // Skip "```json"
            if let Some(end_marker) = after_start.find("```") {
                let json_content = after_start[..end_marker].trim();
                eprintln!("[Active RAG Agent] Found JSON in markdown code block ({} chars)", json_content.len());
                json_content
            } else {
                // No closing ```, try to find JSON object
                if let Some(start) = after_start.find('{') {
                    if let Some(end) = after_start.rfind('}') {
                        let extracted = &after_start[start..=end];
                        eprintln!("[Active RAG Agent] Extracted JSON from code block ({} chars)", extracted.len());
                        extracted
                    } else {
                        eprintln!("[Active RAG Agent] Found '{{' but no matching '}}' in code block");
                        &ai_response
                    }
                } else {
                    eprintln!("[Active RAG Agent] No '{{' found in code block");
                    &ai_response
                }
            }
        } else if let Some(code_start) = ai_response.find("```") {
            // Found code block but not marked as json, try to extract anyway
            let after_start = &ai_response[code_start + 3..];
            if let Some(end_marker) = after_start.find("```") {
                let code_content = after_start[..end_marker].trim();
                if code_content.starts_with('{') {
                    eprintln!("[Active RAG Agent] Found JSON in code block ({} chars)", code_content.len());
                    code_content
                } else {
                    // Not JSON, fall through to regular extraction
                    if let Some(start) = ai_response.find('{') {
                        if let Some(end) = ai_response.rfind('}') {
                            let extracted = &ai_response[start..=end];
                            eprintln!("[Active RAG Agent] Extracted JSON substring ({} chars)", extracted.len());
                            extracted
                        } else {
                            eprintln!("[Active RAG Agent] Found '{{' but no matching '}}'");
                            &ai_response
                        }
                    } else {
                        eprintln!("[Active RAG Agent] No '{{' found in response");
                        &ai_response
                    }
                }
            } else {
                // No closing ```, try regular extraction
                if let Some(start) = ai_response.find('{') {
                    if let Some(end) = ai_response.rfind('}') {
                        let extracted = &ai_response[start..=end];
                        eprintln!("[Active RAG Agent] Extracted JSON substring ({} chars)", extracted.len());
                        extracted
                    } else {
                        eprintln!("[Active RAG Agent] Found '{{' but no matching '}}'");
                        &ai_response
                    }
                } else {
                    eprintln!("[Active RAG Agent] No '{{' found in response");
                    &ai_response
                }
            }
        } else if let Some(start) = ai_response.find('{') {
            // Regular JSON extraction
            if let Some(end) = ai_response.rfind('}') {
                let extracted = &ai_response[start..=end];
                eprintln!("[Active RAG Agent] Extracted JSON substring ({} chars)", extracted.len());
                extracted
            } else {
                eprintln!("[Active RAG Agent] Found '{{' but no matching '}}', using full response");
                &ai_response
            }
        } else {
            eprintln!("[Active RAG Agent] No '{{' found in response, treating as plain text");
            &ai_response
        };
        
        // Try to parse as JSON first
        eprintln!("[Active RAG Agent] Attempting JSON parse...");
        match serde_json::from_str::<serde_json::Value>(json_str) {
            Ok(parsed) => {
                eprintln!("[Active RAG Agent] ✓ JSON parse successful");
                eprintln!("[Active RAG Agent] Parsed JSON keys: {:?}", parsed.as_object().map(|o| o.keys().collect::<Vec<_>>()));
                return self.create_structured_response(parsed, documents);
            }
            Err(e) => {
                eprintln!("[Active RAG Agent] ✗ JSON parse failed: {}", e);
                eprintln!("[Active RAG Agent] Falling back to plain text response");
            }
        }

        // Fallback: create response from plain text
        eprintln!("[Active RAG Agent] Creating fallback response from plain text");
        self.create_fallback_response(&ai_response, documents, user_question).await
    }

    fn create_structured_response(
        &self,
        parsed: serde_json::Value,
        documents: Vec<(String, String, f32)>,
    ) -> Result<ActiveRagResponse, Box<dyn std::error::Error>> {
        eprintln!("[Active RAG Agent] create_structured_response: Extracting fields from JSON...");
        
        let answer = parsed.get("answer")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        
        eprintln!("[Active RAG Agent]   answer field: {}", 
            if answer.is_some() { "present" } else { "missing" });
        if let Some(ref ans) = answer {
            let preview = if ans.len() > 100 { &ans[..100] } else { ans };
            eprintln!("[Active RAG Agent]   answer preview: '{}...'", preview);
        }

        let confidence = parsed.get("confidence")
            .and_then(|v| v.as_f64())
            .map(|f| f as f32);
        
        eprintln!("[Active RAG Agent]   confidence field: {:?}", confidence);

        let sources = parsed.get("sources")
            .and_then(|v| v.as_array())
            .map(|arr| {
                eprintln!("[Active RAG Agent]   sources array found with {} items", arr.len());
                arr.iter().enumerate().filter_map(|(idx, source)| {
                    eprintln!("[Active RAG Agent]     Processing source {}...", idx + 1);
                    let file_path = source.get("file_path")?.as_str()?.to_string();
                    let used_in_answer = source.get("used_in_answer")?.as_bool().unwrap_or(false);
                    let relevance_score = source.get("relevance_score")?.as_f64().unwrap_or(0.0) as f32;
                    
                    eprintln!("[Active RAG Agent]       file_path: {}", file_path);
                    eprintln!("[Active RAG Agent]       used_in_answer: {}", used_in_answer);
                    eprintln!("[Active RAG Agent]       relevance_score: {:.4}", relevance_score);
                    
                    let key_contributions = source.get("key_contributions")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str())
                                .map(|s| s.to_string())
                                .collect()
                        });

                    // Find corresponding document
                    let doc_info = documents.iter()
                        .find(|(path, _, _)| path == &file_path);

                    let file_name = doc_info
                        .and_then(|(path, _, _)| {
                            std::path::Path::new(path)
                                .file_name()
                                .and_then(|n| n.to_str())
                        })
                        .unwrap_or("unknown")
                        .to_string();

                    // Create excerpt from document content
                    let excerpt = doc_info
                        .and_then(|(_, content, _)| {
                            if content.len() > 200 {
                                Some(content[..200].to_string() + "...")
                            } else {
                                Some(content.clone())
                            }
                        });

                    Some(ActiveRagSource {
                        file_path,
                        file_name,
                        relevance_score,
                        used_in_answer,
                        key_contributions,
                        excerpt,
                        comparison_data: None, // TODO: Implement comparison logic
                    })
                })
                .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                eprintln!("[Active RAG Agent]   WARNING: No 'sources' array found in JSON");
                vec![]
            });
        
        eprintln!("[Active RAG Agent]   Final sources count: {}", sources.len());
        
        // If answer is missing but we have documents, use the first document's content as fallback
        let final_answer = if answer.is_none() && !documents.is_empty() {
            eprintln!("[Active RAG Agent]   WARNING: No answer in JSON, using first document as fallback");
            let (_, content, _) = &documents[0];
            Some(format!("Based on the document '{}': {}", 
                std::path::Path::new(&documents[0].0)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown"),
                if content.len() > 1000 {
                    &content[..1000]
                } else {
                    content
                }
            ))
        } else {
            answer
        };

        let response = ActiveRagResponse {
            success: true,
            answer: final_answer,
            sources,
            action_performed: Some("Document analysis completed".to_string()),
            confidence,
            error: None,
        };
        
        eprintln!("[Active RAG Agent] ✓ Structured response created - success: {}, answer present: {}", 
            response.success, response.answer.is_some());
        
        Ok(response)
    }

    async fn create_fallback_response(
        &self,
        ai_response: &str,
        documents: Vec<(String, String, f32)>,
        user_question: &str,
    ) -> Result<ActiveRagResponse, Box<dyn std::error::Error>> {
        eprintln!("[Active RAG Agent] create_fallback_response: Creating response from plain text");
        eprintln!("[Active RAG Agent]   AI response length: {} chars", ai_response.len());
        eprintln!("[Active RAG Agent]   Documents count: {}", documents.len());
        
        // Try to determine which documents are actually used based on:
        // 1. Filename mentions in the answer
        // 2. Content relevance to the question
        // 3. Document mentions in the answer text
        let answer_lower = ai_response.to_lowercase();
        let question_lower = user_question.to_lowercase();
        
        // Extract keywords from question for matching
        let question_keywords: Vec<&str> = question_lower
            .split_whitespace()
            .filter(|w| w.len() > 3) // Only meaningful words
            .collect();
        
        // Create sources from available documents
        let sources = documents.iter().enumerate().map(|(i, (path, content, score))| {
            let file_name = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            
            let file_name_lower = file_name.to_lowercase();
            let content_lower = content.to_lowercase();
            
            // Check if document is mentioned in answer or matches question keywords
            let mentioned_in_answer = answer_lower.contains(&file_name_lower) 
                || file_name_lower.split('.').next().map(|name| answer_lower.contains(name)).unwrap_or(false);
            
            // Check if document content or filename matches question keywords
            let matches_question = question_keywords.iter().any(|kw| {
                file_name_lower.contains(kw) || content_lower.contains(kw)
            });
            
            // Check if document has substantial content (not just a few chars)
            let has_substantial_content = content.len() > 100;
            
            // Mark as used if:
            // - Mentioned in answer, OR
            // - Matches question keywords AND has substantial content, OR
            // - It's the document with highest score AND has substantial content
            let used_in_answer = mentioned_in_answer 
                || (matches_question && has_substantial_content)
                || (i == 0 && has_substantial_content && score > &0.4);

            eprintln!("[Active RAG Agent]     Source {}: {} (score: {:.4}, used: {}, mentioned: {}, matches: {}, substantial: {})", 
                i + 1, file_name, score, used_in_answer, mentioned_in_answer, matches_question, has_substantial_content);

            ActiveRagSource {
                file_path: path.clone(),
                file_name,
                relevance_score: *score,
                used_in_answer,
                key_contributions: None,
                excerpt: None,
                comparison_data: None,
            }
        }).collect();

        let response = ActiveRagResponse {
            success: true,
            answer: Some(ai_response.to_string()),
            sources,
            action_performed: Some("Document analysis completed".to_string()),
            confidence: Some(0.7), // Default confidence for fallback
            error: None,
        };
        
        eprintln!("[Active RAG Agent] ✓ Fallback response created - answer present: {}", 
            response.answer.is_some());
        
        Ok(response)
    }
}
