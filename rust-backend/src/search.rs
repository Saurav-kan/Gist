// Vector search utilities
// Note: For simplicity, we're using linear search with cosine similarity
// For better performance with large datasets, consider using HNSW or other approximate nearest neighbor algorithms

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    
    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    
    dot_product / (norm_a * norm_b)
}

/// Calculate filename similarity score (0.0 to 1.0)
/// Uses fuzzy matching to find files by name even if query doesn't match exactly
/// Stricter matching to avoid false positives
pub fn filename_similarity(query: &str, filename: &str) -> f32 {
    let query_lower = query.to_lowercase();
    let filename_lower = filename.to_lowercase();
    
    // Exact match (case-insensitive) - highest score
    if filename_lower == query_lower {
        return 1.0;
    }
    
    // Exact substring match - but require minimum length to avoid false positives
    // Only match if query is substantial (>= 4 chars) to avoid "cal" matching "close"
    if query_lower.len() >= 4 && filename_lower.contains(&query_lower) {
        // Boost score if match is at the start of filename
        if filename_lower.starts_with(&query_lower) {
            return 0.95;
        }
        return 0.85;
    }
    
    // Check if query words appear in filename (order-independent)
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();
    if query_words.is_empty() {
        return 0.0;
    }
    
    let filename_words: Vec<&str> = filename_lower
        .split(|c: char| c.is_whitespace() || c == '-' || c == '_' || c == '.')
        .filter(|s| !s.is_empty())
        .collect();
    
    if filename_words.is_empty() {
        return 0.0;
    }
    
    // Count how many query words appear in filename
    // STRICTER: Only count exact word matches or very close matches (not loose substring)
    let mut matched_words = 0;
    for query_word in &query_words {
        // Skip very short query words (1-2 chars) to avoid false matches
        if query_word.len() < 3 {
            continue;
        }
        
        for filename_word in &filename_words {
            // Exact word match
            if filename_word == query_word {
                matched_words += 1;
                break;
            }
            // Only allow substring match if query word is substantial (>= 4 chars)
            // and filename word is not much longer (to avoid "calculus" matching "close")
            // Also require that the substring match is at word boundaries or start
            if query_word.len() >= 4 {
                // Check if query word appears as a complete word or at start of filename word
                if filename_word == query_word {
                    matched_words += 1;
                    break;
                }
                // Allow substring only if filename word is similar length (not much longer)
                if filename_word.len() <= query_word.len() + 2 
                    && filename_word.contains(query_word) {
                    // Additional check: ensure it's not a false match like "cal" in "close"
                    // Require that the match starts at the beginning or is a substantial portion
                    if filename_word.starts_with(query_word) 
                        || query_word.len() as f32 / filename_word.len() as f32 > 0.6 {
                        matched_words += 1;
                        break;
                    }
                }
            }
        }
    }
    
    // If no words matched, return 0 (don't use character similarity for false positives)
    if matched_words == 0 {
        return 0.0;
    }
    
    // Calculate score based on matched words ratio
    let word_match_ratio = matched_words as f32 / query_words.len() as f32;
    
    // Only use character similarity if we have some word matches
    let char_similarity = if matched_words > 0 {
        calculate_char_similarity(&query_lower, &filename_lower)
    } else {
        0.0
    };
    
    // Combine word matching and character similarity
    // Weight word matching more heavily (80% word match, 20% char similarity)
    (word_match_ratio * 0.8) + (char_similarity * 0.2)
}

/// Calculate character-level similarity using a simple approach
fn calculate_char_similarity(query: &str, filename: &str) -> f32 {
    if query.is_empty() || filename.is_empty() {
        return 0.0;
    }
    
    // Count common characters (case-insensitive)
    let query_chars: Vec<char> = query.chars().collect();
    let filename_chars: Vec<char> = filename.chars().collect();
    
    let mut common_chars = 0;
    let mut query_pos = 0;
    let mut filename_pos = 0;
    
    // Simple longest common subsequence approximation
    while query_pos < query_chars.len() && filename_pos < filename_chars.len() {
        if query_chars[query_pos] == filename_chars[filename_pos] {
            common_chars += 1;
            query_pos += 1;
            filename_pos += 1;
        } else {
            // Try to find the character in remaining filename
            let mut found = false;
            for i in filename_pos..filename_chars.len() {
                if query_chars[query_pos] == filename_chars[i] {
                    common_chars += 1;
                    query_pos += 1;
                    filename_pos = i + 1;
                    found = true;
                    break;
                }
            }
            if !found {
                query_pos += 1;
            }
        }
    }
    
    // Normalize by the length of the longer string
    let max_len = query_chars.len().max(filename_chars.len()) as f32;
    if max_len == 0.0 {
        return 0.0;
    }
    
    (common_chars as f32 / max_len).min(1.0)
}

/// Combine vector similarity and filename similarity into a hybrid score
/// weights: (vector_weight, filename_weight) - should sum to 1.0
pub fn hybrid_similarity(
    vector_sim: f32,
    filename_sim: f32,
    weights: (f32, f32),
) -> f32 {
    let (vector_weight, filename_weight) = weights;
    (vector_sim * vector_weight) + (filename_sim * filename_weight)
}
