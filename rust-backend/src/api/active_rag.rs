use axum::{
    extract::State,
    response::Json,
};
use serde::{Deserialize, Serialize};
use crate::AppState;
use crate::active_rag_agent::{ActiveRagAgent, ActiveRagResponse, DecomposedIntent};
use crate::api::search::{deduplicate_by_embedding, SearchRequest, SearchResult};
use crate::parsers::ParserRegistry;
use crate::config::FileTypeFilters;

#[derive(Deserialize)]
pub struct ActiveRagApiRequest {
    pub query: String,
    pub user_question: String,
    pub document_limit: Option<usize>,
}

pub async fn active_rag_search(
    State(state): State<AppState>,
    Json(request): Json<ActiveRagApiRequest>,
) -> Result<Json<ActiveRagResponse>, axum::http::StatusCode> {
    // Create a unique request ID to detect duplicates
    let request_id = format!("{}_{}", request.query.trim(), request.user_question.trim());
    eprintln!("=== Active RAG Search Request ===");
    eprintln!("[Active RAG] Request ID: {}", request_id);
    eprintln!("[Active RAG] Query: '{}'", request.query);
    eprintln!("[Active RAG] User Question: '{}'", request.user_question);
    eprintln!("[Active RAG] Document Limit: {:?}", request.document_limit);
    
    // Validate inputs
    let query = request.query.trim();
    let user_question = request.user_question.trim();
    
    if query.is_empty() {
        eprintln!("ERROR: Empty query received");
        return Ok(Json(ActiveRagResponse {
            success: false,
            answer: None,
            sources: vec![],
            action_performed: None,
            confidence: None,
            error: Some("Search query cannot be empty".to_string()),
        }));
    }
    
    if user_question.is_empty() {
        eprintln!("ERROR: Empty user question received");
        return Ok(Json(ActiveRagResponse {
            success: false,
            answer: None,
            sources: vec![],
            action_performed: None,
            confidence: None,
            error: Some("User question cannot be empty".to_string()),
        }));
    }

    // Reload config to get latest AI settings
    let config = match crate::config::AppConfig::load_or_default().await {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("[Active RAG] Error loading config: {}", e);
            state.config.as_ref().clone()
        }
    };
    
    eprintln!("[Active RAG] AI Features Enabled: {}", config.ai_features_enabled);
    
    // Check if AI features are enabled
    if !config.ai_features_enabled {
        return Ok(Json(ActiveRagResponse {
            success: false,
            answer: None,
            sources: vec![],
            action_performed: None,
            confidence: None,
            error: Some("AI features are disabled in settings".to_string()),
        }));
    }

    // Wrap analysis in a timeout to prevent indefinite hangs
    use tokio::time::{timeout, Duration};
    
    let analysis_future = async {
        // Create Active RAG agent
        let agent = ActiveRagAgent::new(
            config.ai_provider.clone(),
            config.ollama_model.clone(),
            config.gemini_model.clone(),
            config.api_key.clone(),
        );

        // DECOMPOSITION STEP: Parse intent using AI
        eprintln!("[Active RAG] Decomposing intent for prompt: '{}' (Query: '{}')", user_question, query);
        let decomposed = match agent.decompose_intent(user_question, query, &config.action_search_parsing_model).await {
            Ok(d) => {
                eprintln!("[Active RAG] Decomposition successful. Vector query: '{}'", d.vector_query);
                d
            }
            Err(e) => {
                eprintln!("[Active RAG] Decomposition failed, falling back to raw inputs: {}", e);
                crate::active_rag_agent::DecomposedIntent {
                    vector_query: query.to_string(),
                    action_question: user_question.to_string(),
                    filters: None,
                }
            }
        };

        // Use decomposed vector_query for retrieval
        let search_request = SearchRequest {
            query: decomposed.vector_query.clone(),
            limit: request.document_limit.or(Some(3)),
            filters: None, // TODO: Apply AI-extracted filters if possible
        };

        eprintln!("[Active RAG] Performing vector search for Active RAG...");
        eprintln!("[Active RAG] Search query: '{}'", search_request.query);
        eprintln!("[Active RAG] Document limit: {:?}", search_request.limit);
        
        let search_results: Vec<SearchResult> = match perform_vector_search(&state, &search_request).await {
            Ok(results) => {
                eprintln!("[Active RAG] Vector search returned {} results", results.len());
                for (i, result) in results.iter().enumerate() {
                    eprintln!("[Active RAG]   Result {}: {} (score: {:.4})", 
                        i + 1, 
                        result.file_name, 
                        result.similarity
                    );
                }
                results
            },
            Err(e) => {
                eprintln!("[Active RAG] ERROR: Search failed: {}", e);
                return ActiveRagResponse {
                    success: false,
                    answer: None,
                    sources: vec![],
                    action_performed: None,
                    confidence: None,
                    error: Some(format!("Search failed: {}", e)),
                };
            },
        };

        if search_results.is_empty() {
            return ActiveRagResponse {
                success: false,
                answer: None,
                sources: vec![],
                action_performed: None,
                confidence: None,
                error: Some("No search results found to analyze".to_string()),
            };
        }

        // Extract content from top documents
        eprintln!("[Active RAG] Extracting content from {} documents...", search_results.len());
        let documents_with_content = match extract_document_content(&search_results).await {
            Ok(docs) => {
                eprintln!("[Active RAG] Successfully extracted content from {} documents", docs.len());
                for (i, (path, content, score)) in docs.iter().enumerate() {
                    let file_name = std::path::Path::new(path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");
                    let content_preview = if content.len() > 100 {
                        &content[..100]
                    } else {
                        content
                    };
                    eprintln!("[Active RAG]   Doc {}: {} (score: {:.4}, content length: {} chars, preview: '{}...')", 
                        i + 1, 
                        file_name,
                        score,
                        content.len(),
                        content_preview
                    );
                }
                docs
            },
            Err(e) => {
                eprintln!("[Active RAG] ERROR: Failed to extract document content: {}", e);
                return ActiveRagResponse {
                    success: false,
                    answer: None,
                    sources: vec![],
                    action_performed: None,
                    confidence: None,
                    error: Some(format!("Failed to read documents: {}", e)),
                };
            },
        };

        eprintln!("[Active RAG] Starting AI analysis of {} documents...", documents_with_content.len());
        eprintln!("[Active RAG] Action question: '{}'", decomposed.action_question);
        eprintln!("[Active RAG] Analysis model setting: '{}'", config.action_search_analysis_model);
        
        let analysis_result = agent.analyze_documents(
            documents_with_content.clone(),
            &decomposed.action_question,
            &decomposed.vector_query,
            &config.action_search_analysis_model,
        ).await;
        
        match analysis_result {
            Ok(response) => {
                eprintln!("[Active RAG] ✓ Analysis completed successfully");
                eprintln!("[Active RAG] Response success: {}", response.success);
                eprintln!("[Active RAG] Answer present: {}", response.answer.is_some());
                if let Some(ref answer) = response.answer {
                    let answer_preview = if answer.len() > 200 {
                        &answer[..200]
                    } else {
                        answer
                    };
                    eprintln!("[Active RAG] Answer preview: '{}...'", answer_preview);
                }
                eprintln!("[Active RAG] Confidence: {:?}", response.confidence);
                eprintln!("[Active RAG] Sources count: {}", response.sources.len());
                for (i, source) in response.sources.iter().enumerate() {
                    eprintln!("[Active RAG]   Source {}: {} (used: {}, score: {:.4})", 
                        i + 1, 
                        source.file_name, 
                        source.used_in_answer,
                        source.relevance_score
                    );
                }
                if let Some(ref error) = response.error {
                    eprintln!("[Active RAG] WARNING: Response has error: {}", error);
                }
                response
            }
            Err(e) => {
                eprintln!("[Active RAG] ERROR: Analysis failed: {}", e);
                eprintln!("[Active RAG] Error details: {:?}", e);
                ActiveRagResponse {
                    success: false,
                    answer: None,
                    sources: vec![],
                    action_performed: None,
                    confidence: None,
                    error: Some(format!("Analysis failed: {}", e)),
                }
            }
        }
    };

    match timeout(Duration::from_secs(90), analysis_future).await {
        Ok(response) => Ok(Json(response)),
        Err(_) => {
            eprintln!("[Active RAG] Analysis timed out after 90 seconds");
            Ok(Json(ActiveRagResponse {
                success: false,
                answer: None,
                sources: vec![],
                action_performed: None,
                confidence: None,
                error: Some("AI analysis timed out. Try a simpler question or fewer documents.".to_string()),
            }))
        }
    }
}

async fn perform_vector_search(
    state: &AppState,
    request: &SearchRequest,
) -> Result<Vec<SearchResult>, Box<dyn std::error::Error>> {
    eprintln!("[Vector Search] Starting vector search...");
    eprintln!("[Vector Search] Query: '{}'", request.query);
    eprintln!("[Vector Search] Limit: {:?}", request.limit);
    
    // This is a simplified version - we'll reuse the existing search logic
    // In a full implementation, we'd call the existing search endpoint logic directly
    let embedding_service = crate::embedding::EmbeddingService::new(
        state.config.embedding_model.clone()
    );
    
    eprintln!("[Vector Search] Generating query embedding using model: {}", state.config.embedding_model);
    let query_embedding = embedding_service.generate_embedding(&request.query)
        .await?;
    eprintln!("[Vector Search] ✓ Query embedding generated (dimension: {})", query_embedding.len());

    // Get all embeddings and calculate similarities
    eprintln!("[Vector Search] Retrieving all file embeddings from storage...");
    let files_with_embeddings = state.storage.get_all_embeddings().await?;
    eprintln!("[Vector Search] Found {} files with embeddings", files_with_embeddings.len());
    
    let mut results = Vec::new();
    let mut below_threshold = 0;
    for (metadata, embedding) in files_with_embeddings {
        let similarity = crate::search::cosine_similarity(&query_embedding, &embedding);
        if similarity > 0.3 { // Basic relevance threshold
            results.push((metadata, similarity));
        } else {
            below_threshold += 1;
        }
    }
    
    eprintln!("[Vector Search] Similarity calculation complete:");
    eprintln!("[Vector Search]   Results above threshold (0.3): {}", results.len());
    eprintln!("[Vector Search]   Results below threshold: {}", below_threshold);

    // Deduplicate by identical embeddings when enabled (same logic as main search)
    if state.config.filter_duplicate_files {
        results = deduplicate_by_embedding(results, state).await;
        eprintln!("[Vector Search] Results after deduplication: {}", results.len());
    }

    // Sort by similarity and take top results
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    
    let limit = request.limit.unwrap_or(3);
    eprintln!("[Vector Search] Taking top {} results", limit);
    
    let search_results: Vec<SearchResult> = results
        .into_iter()
        .take(limit)
        .map(|(metadata, similarity)| {
            eprintln!("[Vector Search]   Selected: {} (score: {:.4})", metadata.file_name, similarity);
            SearchResult {
                file_path: metadata.file_path,
                file_name: metadata.file_name,
                similarity,
                preview: None,
            }
        })
        .collect();

    eprintln!("[Vector Search] ✓ Search complete, returning {} results", search_results.len());
    Ok(search_results)
}

async fn extract_document_content(
    search_results: &[SearchResult],
) -> Result<Vec<(String, String, f32)>, Box<dyn std::error::Error>> {
    eprintln!("[Content Extraction] Starting content extraction from {} files...", search_results.len());
    
    let mut documents = Vec::new();
    
    // Create parser registry with all file types enabled
    let filters = FileTypeFilters {
        include_pdf: true,
        include_docx: true,
        include_text: true,
        include_xlsx: true,
        excluded_extensions: Vec::new(),
    };
    let registry = ParserRegistry::new(&filters);

    for (i, result) in search_results.iter().enumerate() {
        eprintln!("[Content Extraction] Processing file {}: {}", i + 1, result.file_name);
        eprintln!("[Content Extraction]   Path: {}", result.file_path);
        eprintln!("[Content Extraction]   Similarity: {:.4}", result.similarity);
        
        match registry.extract_text(&result.file_path) {
            Ok(content) => {
                let original_len = content.chars().count();
                // Limit content length for AI processing (safe char-aware truncation)
                let max_chars = 3000;
                let truncated_content: String = if original_len > max_chars {
                    content.chars().take(max_chars).collect::<String>() + "..."
                } else {
                    content
                };
                
                eprintln!("[Content Extraction]   ✓ Extracted {} chars (truncated to {} chars)", 
                    original_len, truncated_content.chars().count());
                
                documents.push((result.file_path.clone(), truncated_content, result.similarity));
            }
            Err(e) => {
                eprintln!("[Content Extraction]   ✗ Parser failed: {}", e);
                eprintln!("[Content Extraction]   Attempting plain text fallback...");
                // Try to read as plain text fallback
                match tokio::fs::read_to_string(&result.file_path).await {
                    Ok(content) => {
                        let original_len = content.chars().count();
                        let max_chars = 3000;
                        let truncated_content: String = if original_len > max_chars {
                            content.chars().take(max_chars).collect::<String>() + "..."
                        } else {
                            content
                        };
                        
                        eprintln!("[Content Extraction]   ✓ Plain text read successful ({} chars, truncated to {} chars)", 
                            original_len, truncated_content.chars().count());
                        
                        documents.push((result.file_path.clone(), truncated_content, result.similarity));
                    }
                    Err(read_err) => {
                        eprintln!("[Content Extraction]   ✗ Could not read as plain text: {}", read_err);
                        eprintln!("[Content Extraction]   Skipping this file");
                    }
                }
            }
        }
    }

    eprintln!("[Content Extraction] ✓ Extraction complete: {} documents extracted", documents.len());
    Ok(documents)
}
