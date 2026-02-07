use axum::{
    extract::State,
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub limit: Option<usize>,
    #[serde(default)]
    pub filters: Option<FilterOptions>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    results: Vec<SearchResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub file_path: String,
    pub file_name: String,
    pub similarity: f32,
    pub preview: Option<String>,
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
            let stats = hnsw.get_stats();
            eprintln!("[SEARCH] HNSW index available: {} items, {} dimensions, ready={}", 
                     stats.item_count, stats.dimensions, stats.is_ready);
            
            // Verify index integrity (only log, don't fail)
            let verification = hnsw.verify_index();
            if !verification.is_valid {
                eprintln!("[SEARCH] WARNING: HNSW index verification failed with {} errors", 
                         verification.errors.len());
                for error in &verification.errors {
                    eprintln!("[SEARCH]   Error: {}", error);
                }
            }
            if !verification.warnings.is_empty() {
                eprintln!("[SEARCH] HNSW index has {} warnings", verification.warnings.len());
                for warning in &verification.warnings {
                    eprintln!("[SEARCH]   Warning: {}", warning);
                }
            }
            
            let search_start = std::time::Instant::now();
            eprintln!("[SEARCH] Using HNSW index with {} items", hnsw.len());
            if let Ok(hnsw_results) = hnsw.search(query_embedding.clone(), limit * 2) {
                let search_duration = search_start.elapsed();
                eprintln!("[SEARCH] HNSW search completed in {:.2}ms, returned {} results", 
                         search_duration.as_secs_f64() * 1000.0, hnsw_results.len());
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
            eprintln!("[SEARCH] HNSW index is empty (0 items), falling back to linear search");
        }
    } else {
        eprintln!("[SEARCH] No HNSW index available (None), using linear search");
    }
    drop(hnsw_guard);
    
    // If HNSW didn't return results, use linear search
    if results.is_empty() {
        eprintln!("[SEARCH] HNSW returned no results, falling back to linear search");
        let linear_search_start = std::time::Instant::now();
        let files_with_embeddings = match state.storage.get_all_embeddings().await {
            Ok(embeddings) => {
                if embeddings.is_empty() {
                    eprintln!("[SEARCH] Warning: No embeddings found in storage");
                } else {
                    eprintln!("[SEARCH] Linear search: Found {} files with embeddings", embeddings.len());
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
        let linear_search_duration = linear_search_start.elapsed();
        eprintln!("[SEARCH] Linear search completed in {:.2}ms, found {} results", 
                 linear_search_duration.as_secs_f64() * 1000.0, results.len());
    }
    
    // Add keyword-based search for files without embeddings
    eprintln!("[SEARCH] Performing keyword search for files without embeddings");
    match state.storage.get_files_without_embeddings().await {
        Ok(files_without) => {
            eprintln!("[SEARCH] Found {} files without embeddings", files_without.len());
            for meta in files_without {
                // Calculate filename similarity
                let filename_sim = filename_similarity(query, &meta.file_name);
                
                // Only include if there's a decent keyword match
                if filename_sim > 0.1 {
                    // Apply penalties for short file names
                    let adjusted = adjust_similarity_for_file_length(
                        filename_sim,
                        &meta.file_name,
                        meta.file_size,
                        query_word_count
                    );
                    
                    // Add to results
                    // Check if already present (unlikely since we split by embedding existence)
                    results.push((meta, adjusted));
                }
            }
        }
        Err(e) => {
            eprintln!("[SEARCH] Error getting files without embeddings: {}", e);
        }
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
            results = apply_filters(results, filters, &state.config.file_type_filters.excluded_extensions);
            eprintln!("Filtered results: {} -> {} (removed {})", before_count, results.len(), before_count - results.len());
        } else {
            eprintln!("Filters provided but all empty, skipping filter application");
        }
    } else {
        eprintln!("No filters provided");
        // Still apply global exclusion if no per-request filters
        if !state.config.file_type_filters.excluded_extensions.is_empty() {
            results = results.into_iter().filter(|(meta, _)| {
                let file_ext = std::path::Path::new(&meta.file_path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                !state.config.file_type_filters.excluded_extensions.iter().any(|e| e.to_lowercase() == file_ext)
            }).collect();
        }
    }

    eprintln!("Results before sorting: {}", results.len());
    if !results.is_empty() {
        eprintln!("Sample similarities before sorting: {:?}", 
            results.iter().take(5).map(|(m, s)| (m.file_name.clone(), *s)).collect::<Vec<_>>());
    }

    // Deduplicate by identical embeddings when enabled (keep lexicographically smaller path)
    if state.config.filter_duplicate_files {
        results = deduplicate_by_embedding(results, &state).await;
        eprintln!("Results after deduplication: {}", results.len());
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
    excluded_extensions: &[String],
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

            // Apply global file type exclusion
            if !excluded_extensions.is_empty() {
                let file_ext = std::path::Path::new(&metadata.file_path)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                
                if excluded_extensions.iter().any(|e| e.to_lowercase() == file_ext) {
                    return false;
                }
            }

            true
        })
        .collect()
}

/// Deduplicate results by identical embeddings. When two files have the same embedding,
/// keep only the one with the lexicographically smaller file_path (e.g., fileA.pdf before fileA (1).pdf).
/// Files without embeddings (metadata-only) are kept as-is.
async fn deduplicate_by_embedding(
    results: Vec<(crate::storage::FileMetadata, f32)>,
    state: &AppState,
) -> Vec<(crate::storage::FileMetadata, f32)> {
    let mut with_embedding: Vec<(crate::storage::FileMetadata, f32)> = Vec::new();
    let mut without_embedding: Vec<(crate::storage::FileMetadata, f32)> = Vec::new();

    for (meta, score) in results {
        if meta.embedding_length <= 0 {
            without_embedding.push((meta, score));
        } else {
            with_embedding.push((meta, score));
        }
    }

    if with_embedding.is_empty() {
        return without_embedding;
    }

    // Map: embedding_key -> (metadata, score); when duplicate, keep lexicographically smaller path
    let mut seen: HashMap<Vec<u8>, (crate::storage::FileMetadata, f32)> = HashMap::new();

    for (meta, score) in with_embedding {
        let Ok(embedding) = state.storage.get_embedding(&meta).await else {
            // Failed to load embedding, keep the result
            without_embedding.push((meta, score));
            continue;
        };

        let key = match bincode::serialize(&embedding) {
            Ok(k) => k,
            Err(_) => {
                without_embedding.push((meta, score));
                continue;
            }
        };

        match seen.get_mut(&key) {
            None => {
                seen.insert(key, (meta, score));
            }
            Some((existing_meta, existing_score)) => {
                // Keep the lexicographically smaller file_path
                if meta.file_path < existing_meta.file_path {
                    *existing_meta = meta;
                    *existing_score = score;
                }
            }
        }
    }

    let mut deduped: Vec<_> = seen.into_values().collect();
    deduped.extend(without_embedding);
    deduped
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
