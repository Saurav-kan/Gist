use axum::{
    extract::State,
    response::Json,
};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::search::{cosine_similarity, filename_similarity, hybrid_similarity};

/// Adjust similarity score based on file name length and content size
/// This helps reduce false positives from single-word files
fn adjust_similarity_for_file_length(
    base_similarity: f32,
    file_name: &str,
    file_size: i64,
    query_word_count: usize,
) -> f32 {
    let mut adjusted = base_similarity;
    
    // Count words in filename (split by common separators)
    let file_name_words: Vec<&str> = file_name
        .split(|c: char| c.is_whitespace() || c == '-' || c == '_' || c == '.')
        .filter(|s| !s.is_empty())
        .collect();
    let file_name_word_count = file_name_words.len();
    
    // Penalize very short filenames (1-2 words) more heavily
    if file_name_word_count <= 2 {
        // Apply penalty: reduce similarity by 15-25% for very short names
        let penalty = if file_name_word_count == 1 {
            0.25 // 25% penalty for single-word files
        } else {
            0.15 // 15% penalty for two-word files
        };
        adjusted = adjusted * (1.0 - penalty);
    }
    
    // Penalize very small files (likely minimal content)
    // Files under 100 bytes are likely to have minimal semantic content
    if file_size < 100 {
        adjusted = adjusted * 0.85; // 15% penalty
    } else if file_size < 500 {
        adjusted = adjusted * 0.92; // 8% penalty
    }
    
    // For short queries (1-2 words), be more strict with short filenames
    if query_word_count <= 2 && file_name_word_count <= 2 {
        // Additional penalty when both query and filename are short
        adjusted = adjusted * 0.90; // Additional 10% penalty
    }
    
    // Ensure similarity stays in valid range
    adjusted.max(0.0).min(1.0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterOptions {
    pub date_range: Option<DateRange>,
    pub file_types: Option<Vec<String>>,
    pub folder_paths: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    pub start: Option<i64>, // Unix timestamp
    pub end: Option<i64>,
    pub month: Option<u32>, // 1-12
    pub year: Option<i32>,
}

#[derive(Deserialize)]
pub struct SearchRequest {
    query: String,
    limit: Option<usize>,
    #[serde(default)]
    filters: Option<FilterOptions>,
}

#[derive(Serialize)]
pub struct SearchResponse {
    results: Vec<SearchResult>,
}

#[derive(Serialize)]
pub struct SearchResult {
    file_path: String,
    file_name: String,
    similarity: f32,
    preview: Option<String>,
}

pub async fn search_files(
    State(state): State<AppState>,
    Json(request): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, axum::http::StatusCode> {
    eprintln!("=== Search Request ===");
    eprintln!("Query: '{}'", request.query);
    eprintln!("Limit: {:?}", request.limit);
    eprintln!("Filters: {:?}", request.filters);
    
    // Validate query is not empty
    let query = request.query.trim();
    if query.is_empty() {
        eprintln!("ERROR: Empty query received");
        return Err(axum::http::StatusCode::BAD_REQUEST);
    }
    
    // Use config's max_search_results as default, but allow override up to 200
    let default_limit = state.config.max_search_results;
    let limit = request.limit.unwrap_or(default_limit).min(200);
    
    // Generate embedding for query
    let embedding_service = crate::embedding::EmbeddingService::new(
        state.config.embedding_model.clone()
    );
    
    eprintln!("Generating embedding for query: '{}'", query);
    let query_embedding = embedding_service.generate_embedding(query)
        .await
        .map_err(|e| {
            eprintln!("Error generating query embedding: {}", e);
            axum::http::StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    eprintln!("Generated query embedding with {} dimensions", query_embedding.len());

    // Try to use HNSW index if available, otherwise fall back to linear search
    let mut results: Vec<(crate::storage::FileMetadata, f32)> = Vec::new();
    
    // Calculate query word count for weighting
    let query_words: Vec<&str> = query.split_whitespace().collect();
    let query_word_count = query_words.len();
    eprintln!("Query word count: {}", query_word_count);
    
    let hnsw_guard = state.hnsw_index.read().await;
    if let Some(ref hnsw) = *hnsw_guard {
        // Use HNSW search (or optimized in-memory search)
        if hnsw.len() > 0 {
            eprintln!("[SEARCH] Using HNSW index with {} items", hnsw.len());
            if let Ok(hnsw_results) = hnsw.search(query_embedding.clone(), limit * 2) {
                eprintln!("[SEARCH] HNSW returned {} results", hnsw_results.len());
                // Apply hybrid search (vector + filename) to HNSW results
                results = hnsw_results.into_iter().map(|(meta, vector_sim)| {
                    // Calculate filename similarity
                    let filename_sim = filename_similarity(query, &meta.file_name);
                    
                    // Determine weights based on query characteristics
                    let query_lower = query.to_lowercase();
                    let word_count = query.split_whitespace().count();
                    let has_extension = query.contains('.');
                    let is_short = query.len() < 20;
                    
                    // Academic/technical terms that are single words but semantic
                    let semantic_keywords = [
                        "calculus", "algebra", "geometry", "physics", "chemistry", "biology",
                        "history", "literature", "philosophy", "psychology", "sociology",
                        "programming", "algorithm", "database", "network", "security",
                        "homework", "assignment", "project", "report", "essay", "thesis",
                        "mathematics", "math", "science", "engineering", "computer",
                    ];
                    
                    let is_semantic_keyword = semantic_keywords.iter()
                        .any(|kw| query_lower == *kw || query_lower.starts_with(kw));
                    
                    // Only treat as filename query if:
                    // - Has file extension, OR
                    // - Multiple words AND short AND high filename similarity, OR  
                    // - Single word BUT not a semantic keyword AND high filename similarity
                    let is_filename_query = has_extension || (
                        word_count > 1 && is_short && filename_sim > 0.7
                    ) || (
                        word_count == 1 && !is_semantic_keyword && filename_sim > 0.8
                    );
                    
                    let (vector_weight, filename_weight) = if is_filename_query {
                        (0.3, 0.7) // Favor filename matching for filename-like queries
                    } else {
                        (0.8, 0.2) // Favor vector similarity for semantic queries
                    };
                    
                    // Combine vector and filename similarity
                    let mut hybrid_sim = hybrid_similarity(vector_sim, filename_sim, (vector_weight, filename_weight));
                    
                    // Add content-based penalty to reduce false positives
                    if filename_sim < 0.1 && vector_sim > 0.6 {
                        hybrid_sim = hybrid_sim * 0.8;
                    }
                    
                    if word_count == 1 && filename_sim < 0.3 {
                        hybrid_sim = hybrid_sim * 0.85;
                    }
                    
                    // Apply penalties for short file names/content
                    let adjusted = adjust_similarity_for_file_length(
                        hybrid_sim,
                        &meta.file_name,
                        meta.file_size,
                        query_word_count
                    );
                    (meta, adjusted)
                }).collect();
            } else {
                eprintln!("[SEARCH] HNSW search failed, falling back to linear search");
            }
        } else {
            eprintln!("[SEARCH] HNSW index is empty, falling back to linear search");
        }
    } else {
        eprintln!("[SEARCH] No HNSW index available, using linear search");
    }
    drop(hnsw_guard);
    
    // If HNSW didn't return results, use linear search
    if results.is_empty() {
        let files_with_embeddings = match state.storage.get_all_embeddings().await {
            Ok(embeddings) => {
                if embeddings.is_empty() {
                    eprintln!("Warning: No embeddings found in storage");
                } else {
                    eprintln!("Found {} files with embeddings for search", embeddings.len());
                }
                embeddings
            }
            Err(e) => {
                eprintln!("Error getting embeddings: {}", e);
                return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

        // Calculate similarities in parallel chunks
        use futures::future::join_all;
        let chunk_size = 100;
        let mut all_results = Vec::new();
        
        for chunk in files_with_embeddings.chunks(chunk_size) {
            let chunk_tasks: Vec<_> = chunk.iter().map(|(metadata, embedding)| {
                let query_emb = query_embedding.clone();
                let emb = embedding.clone();
                let meta = metadata.clone();
                let query_str = query.to_string();
                tokio::spawn(async move {
                    // Calculate vector similarity
                    let vector_sim = cosine_similarity(&query_emb, &emb);
                    
                    // Calculate filename similarity
                    let filename_sim = filename_similarity(&query_str, &meta.file_name);
                    
                    // Determine weights based on query characteristics
                    // Single-word academic/technical terms should be treated as semantic queries
                    let query_lower = query_str.to_lowercase();
                    let word_count = query_str.split_whitespace().count();
                    let has_extension = query_str.contains('.');
                    let is_short = query_str.len() < 20;
                    
                    // Academic/technical terms that are single words but semantic
                    let semantic_keywords = [
                        "calculus", "algebra", "geometry", "physics", "chemistry", "biology",
                        "history", "literature", "philosophy", "psychology", "sociology",
                        "programming", "algorithm", "database", "network", "security",
                        "homework", "assignment", "project", "report", "essay", "thesis",
                        "mathematics", "math", "science", "engineering", "computer",
                    ];
                    
                    let is_semantic_keyword = semantic_keywords.iter()
                        .any(|kw| query_lower == *kw || query_lower.starts_with(kw));
                    
                    // Only treat as filename query if:
                    // - Has file extension, OR
                    // - Multiple words AND short AND high filename similarity, OR  
                    // - Single word BUT not a semantic keyword AND high filename similarity
                    let is_filename_query = has_extension || (
                        word_count > 1 && is_short && filename_sim > 0.7
                    ) || (
                        word_count == 1 && !is_semantic_keyword && filename_sim > 0.8
                    );
                    
                    let (vector_weight, filename_weight) = if is_filename_query {
                        (0.3, 0.7) // Favor filename matching for filename-like queries
                    } else {
                        (0.8, 0.2) // Favor vector similarity for semantic queries (increased from 0.7/0.3)
                    };
                    
                    // Combine vector and filename similarity
                    let mut hybrid_sim = hybrid_similarity(vector_sim, filename_sim, (vector_weight, filename_weight));
                    
                    // Add content-based penalty to reduce false positives
                    // If filename similarity is very low (< 0.1) but vector similarity is high,
                    // this might be a false positive - apply penalty
                    if filename_sim < 0.1 && vector_sim > 0.6 {
                        // Reduce similarity by 20% if filename doesn't match at all
                        hybrid_sim = hybrid_sim * 0.8;
                    }
                    
                    // Also penalize if query is a single word and filename doesn't contain it
                    if word_count == 1 && filename_sim < 0.3 {
                        // Additional penalty for single-word queries with poor filename match
                        hybrid_sim = hybrid_sim * 0.85;
                    }
                    
                    // Apply penalties for short file names/content
                    let adjusted_similarity = adjust_similarity_for_file_length(
                        hybrid_sim,
                        &meta.file_name,
                        meta.file_size,
                        query_word_count
                    );
                    
                    (meta, adjusted_similarity)
                })
            }).collect();
            
            let chunk_results = join_all(chunk_tasks).await;
            for task_result in chunk_results {
                if let Ok(result) = task_result {
                    all_results.push(result);
                }
            }
        }
        
        results = all_results;
    }

    // Apply filters if provided and not empty
    if let Some(ref filters) = request.filters {
        // Only apply filters if at least one filter is actually set
        let has_any_filters = filters.date_range.is_some() 
            || filters.file_types.is_some() 
            || filters.folder_paths.is_some();
        
        if has_any_filters {
            eprintln!("Applying filters: date_range={:?}, file_types={:?}, folder_paths={:?}", 
                filters.date_range.is_some(), 
                filters.file_types.is_some(), 
                filters.folder_paths.is_some());
            let before_count = results.len();
            results = apply_filters(results, filters);
            eprintln!("Filtered results: {} -> {} (removed {})", before_count, results.len(), before_count - results.len());
        } else {
            eprintln!("Filters provided but all empty, skipping filter application");
        }
    } else {
        eprintln!("No filters provided");
    }

    eprintln!("Results before sorting: {}", results.len());
    if !results.is_empty() {
        eprintln!("Sample similarities before sorting: {:?}", 
            results.iter().take(5).map(|(m, s)| (m.file_name.clone(), *s)).collect::<Vec<_>>());
    }

    // Sort by similarity (descending)
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Take top results
    let search_results: Vec<SearchResult> = results
        .into_iter()
        .take(limit)
        .map(|(metadata, similarity)| {
            SearchResult {
                file_path: metadata.file_path.clone(),
                file_name: metadata.file_name.clone(),
                similarity,
                preview: None, // Could add file preview logic here
            }
        })
        .collect();

    eprintln!("Returning {} search results", search_results.len());
    if !search_results.is_empty() {
        eprintln!("Top result similarity: {:.3} ({:.1}%)", 
            search_results[0].similarity, 
            search_results[0].similarity * 100.0);
    }

    Ok(Json(SearchResponse {
        results: search_results,
    }))
}

// Apply filters to search results
fn apply_filters(
    results: Vec<(crate::storage::FileMetadata, f32)>,
    filters: &FilterOptions,
) -> Vec<(crate::storage::FileMetadata, f32)> {
    results
        .into_iter()
        .filter(|(metadata, _)| {
            // Apply date filter
            if let Some(ref date_range) = filters.date_range {
                if !matches_date_range(metadata.modified_time, date_range) {
                    return false;
                }
            }

            // Apply file type filter
            if let Some(ref file_types) = filters.file_types {
                let file_ext = std::path::Path::new(&metadata.file_path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                
                if !file_types.iter().any(|ft| file_ext == *ft) {
                    return false;
                }
            }

            // Apply folder path filter
            if let Some(ref folder_paths) = filters.folder_paths {
                let file_path_lower = metadata.file_path.to_lowercase();
                let matches_folder = folder_paths.iter().any(|folder| {
                    let folder_lower = folder.to_lowercase();
                    // Check if file path contains folder name (case-insensitive)
                    file_path_lower.contains(&folder_lower)
                });
                
                if !matches_folder {
                    return false;
                }
            }

            true
        })
        .collect()
}

/// Check if a timestamp matches the date range filter
fn matches_date_range(timestamp: i64, date_range: &DateRange) -> bool {
    // If start/end timestamps are provided, use those
    if let Some(start) = date_range.start {
        if timestamp < start {
            return false;
        }
    }
    if let Some(end) = date_range.end {
        if timestamp > end {
            return false;
        }
    }

    // If month/year are specified, check those
    if date_range.month.is_some() || date_range.year.is_some() {
        use chrono::{Local, Datelike, TimeZone};
        if let Some(dt) = Local.timestamp_opt(timestamp, 0).single() {
            if let Some(month) = date_range.month {
                if dt.month() != month {
                    return false;
                }
            }
            if let Some(year) = date_range.year {
                if dt.year() != year {
                    return false;
                }
            }
        }
    }

    true
}
