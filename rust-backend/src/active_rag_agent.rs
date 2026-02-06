use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::config::AiProvider;
use crate::api::ai::{call_ollama_chat, call_greenpt_chat, call_gemini_chat, ChatMessage};

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
        documents: Vec<(String, String, f32)>, // (file_path, content, relevance_score)
        user_question: &str,
        original_query: &str,
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

        // Create system prompt for document analysis
        let system_prompt = self.create_analysis_prompt(&documents, user_question, original_query);

        // Build conversation messages
        let mut messages = vec![ChatMessage {
            role: "system".to_string(),
            content: system_prompt,
        }];

        // Add user question
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: user_question.to_string(),
        });

        // Call appropriate AI provider
        let ai_response = match self.ai_provider {
            AiProvider::Ollama => {
                let model = self.ollama_model.as_deref().unwrap_or("llama3.2:1b");
                call_ollama_chat(model, &messages).await?
            }
            AiProvider::GreenPT => {
                let api_key = self.api_key.as_ref()
                    .ok_or("GreenPT API key not configured")?;
                call_greenpt_chat(api_key, &messages).await?
            }
            AiProvider::Gemini => {
                let api_key = self.api_key.as_ref()
                    .ok_or("Gemini API key not configured")?;
                let model = self.gemini_model.as_deref().unwrap_or("gemini-pro");
                call_gemini_chat(api_key, model, &messages).await?
            }
            AiProvider::OpenAI => {
                return Err("OpenAI provider not yet implemented for Active RAG".into());
            }
        };

        // Parse AI response and create structured response
        self.parse_ai_response(ai_response, documents, user_question).await
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

            prompt.push_str(&format!(
                "Document {} ({}): Relevance Score: {:.3}\n{}\n\n",
                i + 1,
                file_name,
                relevance_score,
                &content[..content.len().min(2000)] // Limit content length
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
        // Try to parse as JSON first
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&ai_response) {
            return self.create_structured_response(parsed, documents);
        }

        // Fallback: create response from plain text
        self.create_fallback_response(&ai_response, documents, user_question).await
    }

    fn create_structured_response(
        &self,
        parsed: serde_json::Value,
        documents: Vec<(String, String, f32)>,
    ) -> Result<ActiveRagResponse, Box<dyn std::error::Error>> {
        let answer = parsed.get("answer")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let confidence = parsed.get("confidence")
            .and_then(|v| v.as_f64())
            .map(|f| f as f32);

        let sources = parsed.get("sources")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter().filter_map(|source| {
                    let file_path = source.get("file_path")?.as_str()?.to_string();
                    let used_in_answer = source.get("used_in_answer")?.as_bool().unwrap_or(false);
                    let relevance_score = source.get("relevance_score")?.as_f64().unwrap_or(0.0) as f32;
                    
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
                        .unwrap_or_else(|| {
                            std::path::Path::new(&file_path)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown")
                                .to_string()
                        });

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
            .unwrap_or_default();

        Ok(ActiveRagResponse {
            success: true,
            answer,
            sources,
            action_performed: Some("Document analysis completed".to_string()),
            confidence,
            error: None,
        })
    }

    async fn create_fallback_response(
        &self,
        ai_response: &str,
        documents: Vec<(String, String, f32)>,
        _user_question: &str,
    ) -> Result<ActiveRagResponse, Box<dyn std::error::Error>> {
        // Create sources from available documents
        let sources = documents.iter().enumerate().map(|(i, (path, _, score))| {
            let file_name = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            ActiveRagSource {
                file_path: path.clone(),
                file_name,
                relevance_score: *score,
                used_in_answer: i == 0, // Assume first document is most relevant
                key_contributions: None,
                excerpt: None,
                comparison_data: None,
            }
        }).collect();

        Ok(ActiveRagResponse {
            success: true,
            answer: Some(ai_response.to_string()),
            sources,
            action_performed: Some("Document analysis completed".to_string()),
            confidence: Some(0.7), // Default confidence for fallback
            error: None,
        })
    }
}
