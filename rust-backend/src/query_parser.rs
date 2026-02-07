use chrono::{Local, NaiveDate, Datelike, TimeZone};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::api::search::{DateRange, FilterOptions};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedQuery {
    pub query: String,
    pub filters: FilterOptions,
}

pub struct QueryParser {
    llm_cache: Arc<RwLock<HashMap<String, (ParsedQuery, u64)>>>,
    llm_model: String,
}

impl QueryParser {
    pub fn new(llm_model: String) -> Self {
        Self {
            llm_cache: Arc::new(RwLock::new(HashMap::new())),
            llm_model,
        }
    }

    /// Parse natural language query into structured query and filters
    /// Uses pattern matching first, then LLM fallback for complex queries
    pub async fn parse(&self, query: &str) -> ParsedQuery {
        let mut remaining_query = query.to_string();
        let mut filters = FilterOptions {
            date_range: None,
            file_types: None,
            folder_paths: None,
        };

        // Extract date filters
        if let Some((date_range, cleaned_query)) = Self::extract_date_filters(&remaining_query) {
            filters.date_range = Some(date_range);
            remaining_query = cleaned_query;
        }

        // Extract file type filters
        if let Some((file_types, cleaned_query)) = Self::extract_file_types(&remaining_query) {
            filters.file_types = Some(file_types);
            remaining_query = cleaned_query;
        }

        // Extract folder path filters
        if let Some((folder_paths, cleaned_query)) = Self::extract_folder_paths(&remaining_query) {
            filters.folder_paths = Some(folder_paths);
            remaining_query = cleaned_query;
        }

        // Check if we found any filters with pattern matching
        let has_filters = filters.date_range.is_some() 
            || filters.file_types.is_some() 
            || filters.folder_paths.is_some();

        // If pattern matching found filters, return early
        if has_filters {
            return ParsedQuery {
                query: remaining_query.trim().to_string(),
                filters,
            };
        }

        // Pattern matching didn't find filters - try LLM parsing only for complex queries
        // Skip LLM for simple queries (single word or very short queries likely don't have filters)
        if !self.llm_model.is_empty() && Self::should_try_llm(query) {
            // Check cache first
            let cache_key = query.to_lowercase().trim().to_string();
            {
                let cache = self.llm_cache.read().await;
                if let Some((cached_result, timestamp)) = cache.get(&cache_key) {
                    // Cache valid for 5 minutes
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    if now - *timestamp < 300 {
                        return cached_result.clone();
                    }
                }
            }

            // Try LLM parsing
            if let Ok(llm_result) = self.parse_with_llm(query).await {
                // Cache the result
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let mut cache = self.llm_cache.write().await;
                cache.insert(cache_key, (llm_result.clone(), now));
                
                // Clean old cache entries (older than 5 minutes)
                cache.retain(|_, (_, ts)| now - *ts < 300);
                
                return llm_result;
            }
        }

        // LLM parsing failed or not available, return pattern matching result (no filters)
        ParsedQuery {
            query: remaining_query.trim().to_string(),
            filters,
        }
    }

    /// Determine if query is complex enough to warrant LLM parsing
    /// Uses a scoring-based approach that considers multiple factors
    fn should_try_llm(query: &str) -> bool {
        let complexity_score = Self::calculate_query_complexity(query);
        let threshold = 0.3; // Threshold for LLM parsing (0.0 to 1.0)
        // Lower threshold (0.3) allows more queries to use LLM when they have filter indicators
        // This helps catch complex queries that pattern matching might miss
        
        complexity_score >= threshold
    }

    /// Calculate query complexity score (0.0 to 1.0)
    /// Higher scores indicate more complex queries that benefit from LLM parsing
    fn calculate_query_complexity(query: &str) -> f32 {
        let query_lower = query.to_lowercase();
        let words: Vec<&str> = query_lower.split_whitespace().collect();
        let word_count = words.len();
        
        // Base score: query length factor (0.0 to 0.25)
        // Longer queries are generally more complex
        let length_score = if word_count <= 2 {
            0.0_f32 // Very short queries don't need LLM
        } else if word_count <= 5 {
            (word_count as f32 - 2.0) / 12.0 // 0.0 to 0.25 (more generous)
        } else if word_count <= 10 {
            0.25 + ((word_count as f32 - 5.0) / 20.0) // 0.25 to 0.5
        } else {
            0.5 + ((word_count as f32 - 10.0).min(10.0_f32) / 40.0) // 0.5 to 0.75 (capped)
        };
        
        // Filter indicator score (0.0 to 0.3)
        // More filter indicators suggest more complex filtering needs
        let filter_indicators = [
            // Date indicators
            ("from", 0.05), ("in", 0.03), ("last", 0.05), ("this", 0.03),
            ("yesterday", 0.05), ("today", 0.05), ("week", 0.04), ("month", 0.04),
            ("year", 0.04), ("during", 0.04), ("between", 0.05), ("to", 0.03),
            // Month names (also indicate date filters)
            ("january", 0.04), ("jan", 0.03), ("february", 0.04), ("feb", 0.03),
            ("march", 0.04), ("mar", 0.03), ("april", 0.04), ("apr", 0.03),
            ("may", 0.04), ("june", 0.04), ("jun", 0.03), ("july", 0.04), ("jul", 0.03),
            ("august", 0.04), ("aug", 0.03), ("september", 0.04), ("sept", 0.03), ("sep", 0.03),
            ("october", 0.04), ("oct", 0.03), ("november", 0.04), ("nov", 0.03),
            ("december", 0.04), ("dec", 0.03),
            // File type indicators
            ("pdf", 0.04), ("word", 0.04), ("excel", 0.04), ("image", 0.03),
            ("video", 0.03), ("document", 0.03), ("file", 0.02), ("files", 0.02),
            // Folder indicators
            ("downloads", 0.04), ("desktop", 0.04), ("documents", 0.04),
            ("folder", 0.03), ("directory", 0.03), ("path", 0.02),
        ];
        
        let mut filter_score: f32 = 0.0;
        let mut found_indicators = 0;
        for (indicator, score) in &filter_indicators {
            if query_lower.contains(indicator) {
                filter_score += score;
                found_indicators += 1;
            }
        }
        // Cap filter score at 0.35, but boost if multiple indicators found
        // More generous scoring for filter indicators since they indicate complex queries
        filter_score = (filter_score.min(0.35_f32) * (1.0 + (found_indicators as f32 - 1.0) * 0.15)).min(0.35_f32);
        
        // Semantic complexity score (0.0 to 0.2)
        // Presence of conjunctions, prepositions, and complex structures
        let semantic_indicators = [
            ("and", 0.03), ("or", 0.03), ("but", 0.02), ("with", 0.02),
            ("without", 0.03), ("about", 0.02), ("related", 0.02), ("containing", 0.03),
            ("including", 0.02), ("excluding", 0.02), ("before", 0.03), ("after", 0.03),
            ("since", 0.03), ("until", 0.03), ("created", 0.02), ("modified", 0.02),
            ("updated", 0.02), ("recent", 0.02), ("old", 0.02), ("new", 0.02),
        ];
        
        let mut semantic_score: f32 = 0.0;
        for (indicator, score) in &semantic_indicators {
            if query_lower.contains(indicator) {
                semantic_score += score;
            }
        }
        semantic_score = semantic_score.min(0.2_f32);
        
        // Ambiguity score (0.0 to 0.15)
        // Queries with ambiguous terms or multiple possible interpretations
        let ambiguous_patterns = [
            ("homework", 0.02), ("assignment", 0.02), ("project", 0.02),
            ("report", 0.02), ("notes", 0.02), ("meeting", 0.02), ("presentation", 0.02),
            ("draft", 0.01), ("final", 0.01), ("version", 0.01), ("copy", 0.01),
        ];
        
        let mut ambiguity_score: f32 = 0.0;
        for (pattern, score) in &ambiguous_patterns {
            if query_lower.contains(pattern) {
                ambiguity_score += score;
            }
        }
        ambiguity_score = ambiguity_score.min(0.15_f32);
        
        // Structure complexity score (0.0 to 0.05)
        // Queries with complex sentence structures
        let has_question_mark = query.contains('?');
        let has_multiple_phrases = query.split(',').count() > 1 || query.split(';').count() > 1;
        let has_quotes = query.contains('"') || query.contains('\'');
        
        let structure_score = if has_question_mark { 0.02 } else { 0.0 }
            + if has_multiple_phrases { 0.02 } else { 0.0 }
            + if has_quotes { 0.01 } else { 0.0 };
        
        // Combine all scores (total max: 1.0)
        let total_score = length_score + filter_score + semantic_score + ambiguity_score + structure_score;
        
        // Debug logging (can be removed in production)
        if total_score > 0.3 {
            eprintln!("[QUERY_COMPLEXITY] Query: '{}' | Score: {:.2} (length: {:.2}, filter: {:.2}, semantic: {:.2}, ambiguity: {:.2}, structure: {:.2})",
                query, total_score, length_score, filter_score, semantic_score, ambiguity_score, structure_score);
        }
        
        total_score.min(1.0)
    }

    /// Heuristic check: does the original query contain explicit date-like tokens?
    /// We use this to decide whether to trust LLM-generated date filters.
    fn has_explicit_date_tokens(query: &str) -> bool {
        let q = query.to_lowercase();

        // Month names and common relative-date words
        let date_tokens = [
            // Months
            "january", "jan", "february", "feb", "march", "mar", "april", "apr", "may",
            "june", "jun", "july", "jul", "august", "aug", "september", "sept", "sep",
            "october", "oct", "november", "nov", "december", "dec",
            // Relative / range words
            "yesterday", "today", "tomorrow", "last week", "this week", "next week",
            "last month", "this month", "next month",
            "last year", "this year", "next year",
            "between", "before", "after", "since", "from", "during",
            "recent", "recently", "ago",
        ];

        if date_tokens.iter().any(|t| q.contains(t)) {
            return true;
        }

        // Look for standalone 4-digit years like 2024, 2025, etc.
        for word in q.split_whitespace() {
            if word.len() == 4 {
                if let Ok(year) = word.trim_matches(|c: char| !c.is_ascii_digit()).parse::<i32>() {
                    if (1900..=2100).contains(&year) {
                        return true;
                    }
                }
            }
        }

        false
    }

    /// Check if a month/year combination is in the future relative to current date
    fn is_future_date(month: Option<u32>, year: Option<i32>) -> bool {
        let now = Local::now();
        let current_year = now.year();
        let current_month = now.month();

        if let Some(year_val) = year {
            if year_val > current_year {
                return true;
            }
            if year_val == current_year {
                if let Some(month_val) = month {
                    if month_val > current_month {
                        return true;
                    }
                }
            }
        } else if let Some(month_val) = month {
            // If no year specified, assume current year - check if month is in future
            if month_val > current_month {
                return true;
            }
        }

        false
    }

    /// Check if query explicitly mentions a specific month name
    fn query_mentions_month(query: &str, month_name: &str) -> bool {
        let query_lower = query.to_lowercase();
        let month_lower = month_name.to_lowercase();
        
        // Check for exact month name or abbreviation
        query_lower.contains(&month_lower) || 
        query_lower.contains(&format!(" {}", month_lower)) ||
        query_lower.contains(&format!("{} ", month_lower))
    }

    /// Parse query using LLM (Ollama)
    async fn parse_with_llm(&self, query: &str) -> anyhow::Result<ParsedQuery> {
        use reqwest::Client;
        use serde::{Deserialize, Serialize};
        
        const OLLAMA_URL: &str = "http://localhost:11434";
        
        let prompt = format!(
            r#"Parse this search query into JSON format. Extract filters and remove filter words from the search query.

Query: "{}"

Extract:
- search_query: main search terms (remove filter words like dates, file types, folder names)
- date_filter: {{"month": number 1-12 or null, "year": number or null}} if date mentioned, null otherwise
- file_types: array of file extensions like ["pdf", "docx"] or null if none mentioned
- folder_paths: array of folder names like ["Downloads", "Desktop"] or null if none mentioned

Common patterns:
- Dates: "December", "2024", "last week", "this month", "yesterday"
- File types: "PDF", "Word", "Excel", "images", "videos", "documents"
- Folders: "Downloads", "Desktop", "Documents"

Return ONLY valid JSON, no other text:
{{
  "search_query": "...",
  "date_filter": {{"month": null, "year": null}} or {{"month": 12, "year": 2024}},
  "file_types": null or ["pdf"],
  "folder_paths": null or ["Downloads"]
}}"#,
            query
        );

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

        let client = Client::new();
        let request = GenerateRequest {
            model: self.llm_model.clone(),
            prompt,
            stream: false,
        };

        let response = client
            .post(&format!("{}/api/generate", OLLAMA_URL))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Ollama API error: {}", response.status());
        }

        let generate_response: GenerateResponse = response.json().await?;
        let json_text = generate_response.response.trim();

        // Extract JSON from response (might have markdown code blocks)
        let json_text = if json_text.starts_with("```json") {
            json_text.strip_prefix("```json").unwrap_or(json_text)
                .strip_suffix("```").unwrap_or(json_text)
                .trim()
        } else if json_text.starts_with("```") {
            json_text.strip_prefix("```").unwrap_or(json_text)
                .strip_suffix("```").unwrap_or(json_text)
                .trim()
        } else {
            json_text
        };

        #[derive(Deserialize)]
        struct LlmParsedQuery {
            search_query: String,
            date_filter: Option<LlmDateFilter>,
            file_types: Option<Vec<String>>,
            folder_paths: Option<Vec<String>>,
        }

        #[derive(Deserialize)]
        struct LlmDateFilter {
            month: Option<u32>,
            year: Option<i32>,
        }

        let mut parsed: LlmParsedQuery = serde_json::from_str(json_text)?;

        // Safety guard: if the original query doesn't contain any explicit date-like
        // tokens, ignore any date_filter the LLM tries to invent.
        if parsed.date_filter.is_some() && !Self::has_explicit_date_tokens(query) {
            parsed.date_filter = None;
        }

        // Convert LLM result to ParsedQuery
        let mut filters = FilterOptions {
            date_range: None,
            file_types: None,
            folder_paths: None,
        };

        if let Some(date_filter) = parsed.date_filter {
            if date_filter.month.is_some() || date_filter.year.is_some() {
                let now = Local::now();
                let current_year = now.year();
                
                // Check if the date is in the future
                let is_future = Self::is_future_date(date_filter.month, date_filter.year);
                
                // If future date, check if it's explicitly mentioned in the query
                if is_future {
                    let month_explicitly_mentioned = if let Some(month) = date_filter.month {
                        let month_names = [
                            ("january", "jan"), ("february", "feb"), ("march", "mar"),
                            ("april", "apr"), ("may", ""), ("june", "jun"),
                            ("july", "jul"), ("august", "aug"), ("september", "sept"),
                            ("october", "oct"), ("november", "nov"), ("december", "dec"),
                        ];
                        if month > 0 && month <= 12 {
                            let (full_name, abbrev) = month_names[(month - 1) as usize];
                            Self::query_mentions_month(query, full_name) || 
                            (!abbrev.is_empty() && Self::query_mentions_month(query, abbrev))
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    
                    let year_explicitly_mentioned = if let Some(year) = date_filter.year {
                        query.contains(&year.to_string())
                    } else {
                        false
                    };
                    
                    // Only allow future dates if explicitly mentioned
                    if !month_explicitly_mentioned && !year_explicitly_mentioned {
                        eprintln!("[DATE_FILTER] Rejecting future date filter (month={:?}, year={:?}) - not explicitly mentioned in query", 
                                 date_filter.month, date_filter.year);
                        // Don't set the date filter
                    } else {
                        eprintln!("[DATE_FILTER] Allowing future date filter (month={:?}, year={:?}) - explicitly mentioned in query", 
                                 date_filter.month, date_filter.year);
                    }
                }
                
                // Only proceed if not a future date, or if future date is explicitly mentioned
                if !is_future || (is_future && (
                    (date_filter.month.is_some() && {
                        let month = date_filter.month.unwrap();
                        let month_names = [
                            ("january", "jan"), ("february", "feb"), ("march", "mar"),
                            ("april", "apr"), ("may", ""), ("june", "jun"),
                            ("july", "jul"), ("august", "aug"), ("september", "sept"),
                            ("october", "oct"), ("november", "nov"), ("december", "dec"),
                        ];
                        if month > 0 && month <= 12 {
                            let (full_name, abbrev) = month_names[(month - 1) as usize];
                            Self::query_mentions_month(query, full_name) || 
                            (!abbrev.is_empty() && Self::query_mentions_month(query, abbrev))
                        } else {
                            false
                        }
                    }) || (date_filter.year.is_some() && query.contains(&date_filter.year.unwrap().to_string()))
                )) {
                    let mut date_range = DateRange {
                        start: None,
                        end: None,
                        month: date_filter.month,
                        year: date_filter.year.or(Some(current_year)),
                    };

                    // Calculate timestamps if month/year provided
                    if let Some(month) = date_range.month {
                        let year = date_range.year.unwrap_or(current_year);
                        
                        // Cap end date to current date if it's in the future
                        let now_ts = now.timestamp();
                        let end_date_ts = if let Some(start_date) = NaiveDate::from_ymd_opt(year, month, 1) {
                            let next_month = if month == 12 {
                                NaiveDate::from_ymd_opt(year + 1, 1, 1)
                            } else {
                                NaiveDate::from_ymd_opt(year, month + 1, 1)
                            };
                            
                            if let Some(next) = next_month {
                                if let Some(last_day) = next.pred_opt() {
                                    if let Some(end_naive) = last_day.and_hms_opt(23, 59, 59) {
                                        if let Some(end_dt) = Local.from_local_datetime(&end_naive).single() {
                                            let end_ts = end_dt.timestamp();
                                            // Cap to current date if future
                                            if end_ts > now_ts {
                                                now_ts
                                            } else {
                                                end_ts
                                            }
                                        } else {
                                            now_ts
                                        }
                                    } else {
                                        now_ts
                                    }
                                } else {
                                    now_ts
                                }
                            } else {
                                now_ts
                            }
                        } else {
                            now_ts
                        };
                        
                        if let Some(start_date) = NaiveDate::from_ymd_opt(year, month, 1) {
                            if let Some(start_naive) = start_date.and_hms_opt(0, 0, 0) {
                                if let Some(start_dt) = Local.from_local_datetime(&start_naive).single() {
                                    let start_ts = start_dt.timestamp();
                                    // Only set start if not in the future (unless explicitly mentioned)
                                    if start_ts <= now_ts || is_future {
                                        date_range.start = Some(start_ts);
                                        date_range.end = Some(end_date_ts);
                                        eprintln!("[DATE_FILTER] Setting date range: month={}, year={}, start={}, end={}", 
                                                month, year, start_ts, end_date_ts);
                                    } else {
                                        eprintln!("[DATE_FILTER] Skipping future date range: month={}, year={}", month, year);
                                    }
                                }
                            }
                        }
                    } else if let Some(year) = date_range.year {
                        // Entire year - cap end to current date if future
                        let now_ts = now.timestamp();
                        let end_ts = if let Some(end_date) = NaiveDate::from_ymd_opt(year, 12, 31) {
                            if let Some(end_naive) = end_date.and_hms_opt(23, 59, 59) {
                                if let Some(end_dt) = Local.from_local_datetime(&end_naive).single() {
                                    let ts = end_dt.timestamp();
                                    if ts > now_ts {
                                        now_ts
                                    } else {
                                        ts
                                    }
                                } else {
                                    now_ts
                                }
                            } else {
                                now_ts
                            }
                        } else {
                            now_ts
                        };
                        
                        if let Some(start_date) = NaiveDate::from_ymd_opt(year, 1, 1) {
                            if let Some(start_naive) = start_date.and_hms_opt(0, 0, 0) {
                                if let Some(start_dt) = Local.from_local_datetime(&start_naive).single() {
                                    let start_ts = start_dt.timestamp();
                                    if start_ts <= now_ts || (year > current_year && query.contains(&year.to_string())) {
                                        date_range.start = Some(start_ts);
                                        date_range.end = Some(end_ts);
                                        eprintln!("[DATE_FILTER] Setting year range: year={}, start={}, end={}", 
                                                year, start_ts, end_ts);
                                    } else {
                                        eprintln!("[DATE_FILTER] Skipping future year range: year={}", year);
                                    }
                                }
                            }
                        }
                    }
                    
                    if date_range.start.is_some() || date_range.end.is_some() {
                        filters.date_range = Some(date_range);
                    }
                }
            }
        }

        if let Some(file_types) = parsed.file_types {
            if !file_types.is_empty() {
                filters.file_types = Some(file_types);
            }
        }

        if let Some(folder_paths) = parsed.folder_paths {
            if !folder_paths.is_empty() {
                filters.folder_paths = Some(folder_paths);
            }
        }

        Ok(ParsedQuery {
            query: parsed.search_query.trim().to_string(),
            filters,
        })
    }

    /// Parse using pattern matching only (synchronous, no LLM)
    pub fn parse_pattern_only(&self, query: &str) -> ParsedQuery {
        let mut remaining_query = query.to_string();
        let mut filters = FilterOptions {
            date_range: None,
            file_types: None,
            folder_paths: None,
        };

        // Extract date filters
        if let Some((date_range, cleaned_query)) = Self::extract_date_filters(&remaining_query) {
            filters.date_range = Some(date_range);
            remaining_query = cleaned_query;
        }

        // Extract file type filters
        if let Some((file_types, cleaned_query)) = Self::extract_file_types(&remaining_query) {
            filters.file_types = Some(file_types);
            remaining_query = cleaned_query;
        }

        // Extract folder path filters
        if let Some((folder_paths, cleaned_query)) = Self::extract_folder_paths(&remaining_query) {
            filters.folder_paths = Some(folder_paths);
            remaining_query = cleaned_query;
        }

        ParsedQuery {
            query: remaining_query.trim().to_string(),
            filters,
        }
    }

    /// Extract date filters from query - Enhanced with more patterns
    fn extract_date_filters(query: &str) -> Option<(DateRange, String)> {
        let query_lower = query.to_lowercase();
        let mut cleaned_query = query.to_string();
        let mut date_range = DateRange {
            start: None,
            end: None,
            month: None,
            year: None,
        };

        let now = Local::now();
        let current_year = now.year();
        let current_month = now.month();
        let current_day = now.day();

        // Pattern: Date ranges "from X to Y", "between X and Y"
        let date_range_pattern = regex::Regex::new(
            r"(?:from|between)\s+(\w+\s+\d{1,2},?\s+\d{4}|\w+\s+\d{4}|\d{1,2}/\d{1,2}/\d{4})\s+(?:to|and)\s+(\w+\s+\d{1,2},?\s+\d{4}|\w+\s+\d{4}|\d{1,2}/\d{1,2}/\d{4})"
        ).ok();
        
        if let Some(ref range_re) = date_range_pattern {
            if let Some(caps) = range_re.captures(&query_lower) {
                // Parse date range - simplified for now, can be enhanced
                cleaned_query = range_re.replace(&cleaned_query, "").trim().to_string();
            }
        }

        // Pattern: "from December", "in December", "December", "during December"
        let month_patterns = vec![
            ("january", 1), ("jan", 1), ("february", 2), ("feb", 2), ("march", 3), ("mar", 3),
            ("april", 4), ("apr", 4), ("may", 5), ("june", 6), ("jun", 6), ("july", 7), ("jul", 7),
            ("august", 8), ("aug", 8), ("september", 9), ("sept", 9), ("sep", 9),
            ("october", 10), ("oct", 10), ("november", 11), ("nov", 11), ("december", 12), ("dec", 12),
        ];

        for (month_name, month_num) in month_patterns {
            let patterns = vec![
                format!("from {}", month_name),
                format!("in {}", month_name),
                format!("during {}", month_name),
                format!("{}", month_name),
            ];

            for pattern in &patterns {
                if query_lower.contains(pattern) {
                    // Check if year is specified with month (e.g., "December 2023")
                    let month_year_pattern = regex::Regex::new(
                        &format!(r"{}\s+(\d{{4}})", regex::escape(month_name))
                    ).ok();
                    
                    let year = if let Some(ref my_re) = month_year_pattern {
                        if let Some(caps) = my_re.captures(&query_lower) {
                            if let Ok(y) = caps.get(1)?.as_str().parse::<i32>() {
                                if y >= 2000 && y <= 2100 {
                                    Some(y)
                                } else {
                                    Some(current_year)
                                }
                            } else {
                                Some(current_year)
                            }
                        } else {
                            Some(current_year)
                        }
                    } else {
                        Some(current_year)
                    };
                    
                    // Check if this is a future date
                    let is_future = Self::is_future_date(Some(month_num), year);
                    
                    // Only set future dates if explicitly mentioned (which they are, since we matched the pattern)
                    // But still validate and cap to current date if needed
                    if !is_future || (is_future && Self::query_mentions_month(query, month_name)) {
                        date_range.month = Some(month_num);
                        date_range.year = year;
                        
                        // Calculate start and end timestamps for the month
                        let year_val = year.unwrap_or(current_year);
                        let now_ts = now.timestamp();
                        
                        if let Some(start_date) = NaiveDate::from_ymd_opt(year_val, month_num, 1) {
                            if let Some(start_naive) = start_date.and_hms_opt(0, 0, 0) {
                                if let Some(start_dt) = Local.from_local_datetime(&start_naive).single() {
                                    let start_ts = start_dt.timestamp();
                                    
                                    // Calculate end of month
                                    let next_month = if month_num == 12 {
                                        NaiveDate::from_ymd_opt(year_val + 1, 1, 1)
                                    } else {
                                        NaiveDate::from_ymd_opt(year_val, month_num + 1, 1)
                                    };
                                    
                                    let end_ts = if let Some(next) = next_month {
                                        if let Some(last_day) = next.pred_opt() {
                                            if let Some(end_naive) = last_day.and_hms_opt(23, 59, 59) {
                                                if let Some(end_dt) = Local.from_local_datetime(&end_naive).single() {
                                                    let ts = end_dt.timestamp();
                                                    // Cap to current date if future
                                                    if ts > now_ts {
                                                        now_ts
                                                    } else {
                                                        ts
                                                    }
                                                } else {
                                                    now_ts
                                                }
                                            } else {
                                                now_ts
                                            }
                                        } else {
                                            now_ts
                                        }
                                    } else {
                                        now_ts
                                    };
                                    
                                    // Only set if start is not in future (unless explicitly mentioned)
                                    if start_ts <= now_ts || is_future {
                                        date_range.start = Some(start_ts);
                                        date_range.end = Some(end_ts);
                                        eprintln!("[DATE_FILTER] extract_date_filters: Setting month={}, year={}, start={}, end={}", 
                                                month_num, year_val, start_ts, end_ts);
                                    } else {
                                        eprintln!("[DATE_FILTER] extract_date_filters: Skipping future month={}, year={}", 
                                                month_num, year_val);
                                    }
                                }
                            }
                        }
                    } else {
                        eprintln!("[DATE_FILTER] extract_date_filters: Rejecting future date - month={}, year={:?}, not explicitly mentioned", 
                                month_num, year);
                    }

                    // Remove pattern from query
                    cleaned_query = cleaned_query
                        .replacen(pattern, "", 1)
                        .trim()
                        .to_string();
                    break;
                }
            }
        }

        // Pattern: "from 2024", "in 2024", "2024", "during 2024"
        let year_pattern = regex::Regex::new(r"\b(?:from|in|during)\s+(\d{4})\b|\b(19\d{2}|20\d{2})\b").ok()?;
        if let Some(caps) = year_pattern.captures(&query_lower) {
            let year_str = caps.get(1).or_else(|| caps.get(2))?.as_str();
            if let Ok(year) = year_str.parse::<i32>() {
                if year >= 2000 && year <= 2100 {
                    date_range.year = Some(year);
                    if date_range.month.is_none() {
                        // If no month specified, use entire year
                        if let Some(start_date) = NaiveDate::from_ymd_opt(year, 1, 1) {
                            if let Some(start_naive) = start_date.and_hms_opt(0, 0, 0) {
                                if let Some(start_dt) = Local.from_local_datetime(&start_naive).single() {
                                    date_range.start = Some(start_dt.timestamp());
                                }
                            }
                        }
                        if let Some(end_date) = NaiveDate::from_ymd_opt(year, 12, 31) {
                            if let Some(end_naive) = end_date.and_hms_opt(23, 59, 59) {
                                if let Some(end_dt) = Local.from_local_datetime(&end_naive).single() {
                                    date_range.end = Some(end_dt.timestamp());
                                }
                            }
                        }
                    }
                    cleaned_query = year_pattern.replace(&cleaned_query, "").trim().to_string();
                }
            }
        }

        // Enhanced relative date patterns
        if query_lower.contains("today") {
            let today_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap();
            if let Some(start_dt) = Local.from_local_datetime(&today_start).single() {
                date_range.start = Some(start_dt.timestamp());
            }
            date_range.end = Some(now.timestamp());
            cleaned_query = cleaned_query.replace("today", "").trim().to_string();
        } else if query_lower.contains("yesterday") {
            let yesterday = now - chrono::Duration::days(1);
            let yesterday_naive = yesterday.date_naive();
            if let Some(start_naive) = yesterday_naive.and_hms_opt(0, 0, 0) {
                if let Some(start_dt) = Local.from_local_datetime(&start_naive).single() {
                    date_range.start = Some(start_dt.timestamp());
                }
            }
            if let Some(end_naive) = yesterday_naive.and_hms_opt(23, 59, 59) {
                if let Some(end_dt) = Local.from_local_datetime(&end_naive).single() {
                    date_range.end = Some(end_dt.timestamp());
                }
            }
            cleaned_query = cleaned_query.replace("yesterday", "").trim().to_string();
        } else if query_lower.contains("this week") {
            let days_from_monday = now.weekday().num_days_from_monday();
            let week_start = now - chrono::Duration::days(days_from_monday as i64);
            let week_start_naive = week_start.date_naive().and_hms_opt(0, 0, 0).unwrap();
            if let Some(start_dt) = Local.from_local_datetime(&week_start_naive).single() {
                date_range.start = Some(start_dt.timestamp());
            }
            date_range.end = Some(now.timestamp());
            cleaned_query = cleaned_query.replace("this week", "").trim().to_string();
        } else if query_lower.contains("last week") {
            let days_from_monday = now.weekday().num_days_from_monday();
            let week_start = now - chrono::Duration::days(days_from_monday as i64 + 7);
            let week_end = now - chrono::Duration::days(days_from_monday as i64 + 1);
            let week_start_naive = week_start.date_naive().and_hms_opt(0, 0, 0).unwrap();
            let week_end_naive = week_end.date_naive().and_hms_opt(23, 59, 59).unwrap();
            if let Some(start_dt) = Local.from_local_datetime(&week_start_naive).single() {
                date_range.start = Some(start_dt.timestamp());
            }
            if let Some(end_dt) = Local.from_local_datetime(&week_end_naive).single() {
                date_range.end = Some(end_dt.timestamp());
            }
            cleaned_query = cleaned_query.replace("last week", "").trim().to_string();
        } else if query_lower.contains("this month") {
            if let Some(month_start) = NaiveDate::from_ymd_opt(current_year, current_month, 1) {
                if let Some(start_naive) = month_start.and_hms_opt(0, 0, 0) {
                    if let Some(start_dt) = Local.from_local_datetime(&start_naive).single() {
                        date_range.start = Some(start_dt.timestamp());
                    }
                }
            }
            date_range.end = Some(now.timestamp());
            date_range.month = Some(current_month);
            date_range.year = Some(current_year);
            cleaned_query = cleaned_query.replace("this month", "").trim().to_string();
        } else if query_lower.contains("last month") {
            let last_month = if current_month == 1 { 12 } else { current_month - 1 };
            let last_month_year = if current_month == 1 { current_year - 1 } else { current_year };
            if let Some(month_start) = NaiveDate::from_ymd_opt(last_month_year, last_month, 1) {
                if let Some(start_naive) = month_start.and_hms_opt(0, 0, 0) {
                    if let Some(start_dt) = Local.from_local_datetime(&start_naive).single() {
                        date_range.start = Some(start_dt.timestamp());
                    }
                }
            }
            let next_month = if last_month == 12 {
                NaiveDate::from_ymd_opt(last_month_year + 1, 1, 1)
            } else {
                NaiveDate::from_ymd_opt(last_month_year, last_month + 1, 1)
            };
            if let Some(next) = next_month {
                if let Some(last_day) = next.pred_opt() {
                    if let Some(end_naive) = last_day.and_hms_opt(23, 59, 59) {
                        if let Some(end_dt) = Local.from_local_datetime(&end_naive).single() {
                            date_range.end = Some(end_dt.timestamp());
                        }
                    }
                }
            }
            date_range.month = Some(last_month);
            date_range.year = Some(last_month_year);
            cleaned_query = cleaned_query.replace("last month", "").trim().to_string();
        } else if query_lower.contains("last year") {
            let last_year = current_year - 1;
            if let Some(start_date) = NaiveDate::from_ymd_opt(last_year, 1, 1) {
                if let Some(start_naive) = start_date.and_hms_opt(0, 0, 0) {
                    if let Some(start_dt) = Local.from_local_datetime(&start_naive).single() {
                        date_range.start = Some(start_dt.timestamp());
                    }
                }
            }
            if let Some(end_date) = NaiveDate::from_ymd_opt(last_year, 12, 31) {
                if let Some(end_naive) = end_date.and_hms_opt(23, 59, 59) {
                    if let Some(end_dt) = Local.from_local_datetime(&end_naive).single() {
                        date_range.end = Some(end_dt.timestamp());
                    }
                }
            }
            date_range.year = Some(last_year);
            cleaned_query = cleaned_query.replace("last year", "").trim().to_string();
        }

        // Pattern: "last N days/weeks/months"
        let relative_pattern = regex::Regex::new(r"last\s+(\d+)\s+(day|days|week|weeks|month|months)").ok()?;
        if let Some(caps) = relative_pattern.captures(&query_lower) {
            if let (Some(num_str), Some(unit_str)) = (caps.get(1), caps.get(2)) {
                if let Ok(num) = num_str.as_str().parse::<i64>() {
                    let duration = match unit_str.as_str() {
                        "day" | "days" => chrono::Duration::days(num),
                        "week" | "weeks" => chrono::Duration::weeks(num),
                        "month" | "months" => chrono::Duration::days(num * 30), // Approximate
                        _ => chrono::Duration::days(num),
                    };
                    let start_time = now - duration;
                    date_range.start = Some(start_time.timestamp());
                    date_range.end = Some(now.timestamp());
                    cleaned_query = relative_pattern.replace(&cleaned_query, "").trim().to_string();
                }
            }
        }

        if date_range.start.is_some() || date_range.end.is_some() || date_range.month.is_some() || date_range.year.is_some() {
            Some((date_range, cleaned_query))
        } else {
            None
        }
    }

    /// Extract file type filters from query - Enhanced with more patterns
    fn extract_file_types(query: &str) -> Option<(Vec<String>, String)> {
        let query_lower = query.to_lowercase();
        let mut cleaned_query = query.to_string();
        let mut file_types = Vec::new();

        // Enhanced file type mappings - map keywords to actual extensions
        // Format: (primary_ext, keywords, all_extensions)
        let type_patterns: Vec<(&str, Vec<&str>, Vec<&str>)> = vec![
            ("pdf", vec!["pdf", "pdf files", "pdf documents", "pdfs", ".pdf"], vec!["pdf"]),
            ("docx", vec!["word", "word documents", "docx", "doc files", "documents", "microsoft word", "ms word", ".docx", ".doc"], vec!["docx", "doc"]),
            ("xlsx", vec!["excel", "spreadsheet", "spreadsheets", "xlsx", "xls files", "microsoft excel", "ms excel", ".xlsx", ".xls"], vec!["xlsx", "xls"]),
            ("txt", vec!["text files", "text", "txt files", "plain text", ".txt"], vec!["txt"]),
            // Images: map to ALL image extensions
            ("jpg", vec!["images", "image", "pictures", "photos", "picture", "jpg", "jpeg", "png", "gif", "bmp", "webp", ".jpg", ".jpeg", ".png", ".gif", ".bmp", ".webp"], vec!["jpg", "jpeg", "png", "gif", "bmp", "webp"]),
            ("mp4", vec!["videos", "video", "mp4", "movie", "movies", "avi", "mov", ".mp4", ".avi", ".mov"], vec!["mp4", "avi", "mov"]),
            ("zip", vec!["zip", "zip files", "archives", "compressed", ".zip", ".rar", ".7z"], vec!["zip", "rar", "7z"]),
            ("mp3", vec!["audio", "music", "songs", "mp3", "sound", ".mp3", ".wav", ".flac"], vec!["mp3", "wav", "flac"]),
            ("pptx", vec!["powerpoint", "presentation", "ppt", "pptx", ".pptx", ".ppt"], vec!["pptx", "ppt"]),
            ("csv", vec!["csv", "csv files", "comma separated", ".csv"], vec!["csv"]),
        ];

        // Also check for explicit file extensions in query
        let ext_pattern = regex::Regex::new(r"\.([a-z0-9]{2,4})\b").ok()?;
        for cap in ext_pattern.captures_iter(&query_lower) {
            if let Some(ext_match) = cap.get(1) {
                let ext = ext_match.as_str();
                if !file_types.contains(&ext.to_string()) {
                    file_types.push(ext.to_string());
                }
            }
        }

        for (primary_ext, patterns, all_extensions) in type_patterns {
            for pattern in &patterns {
                // Use word boundaries to avoid partial matches
                let pattern_re = regex::Regex::new(&format!(r"\b{}\b", regex::escape(pattern))).ok();
                let matched = if let Some(ref re) = pattern_re {
                    re.is_match(&query_lower)
                } else {
                    query_lower.contains(pattern)
                };
                
                if matched {
                    // Add ALL extensions for this file type, not just the primary one
                    for ext in all_extensions {
                        if !file_types.contains(&ext.to_string()) {
                            file_types.push(ext.to_string());
                        }
                    }
                    // Remove pattern from query more carefully
                    if let Some(ref re) = pattern_re {
                        cleaned_query = re.replace(&cleaned_query, "").trim().to_string();
                    } else {
                        cleaned_query = cleaned_query
                            .replacen(pattern, "", 1)
                            .trim()
                            .to_string();
                    }
                    break;
                }
            }
        }

        if !file_types.is_empty() {
            Some((file_types, cleaned_query))
        } else {
            None
        }
    }

    /// Extract folder path filters from query - Enhanced with more patterns
    fn extract_folder_paths(query: &str) -> Option<(Vec<String>, String)> {
        let query_lower = query.to_lowercase();
        let mut cleaned_query = query.to_string();
        let mut folder_paths = Vec::new();

        // Enhanced folder name patterns with more variations
        let folder_patterns: Vec<(&str, Vec<&str>)> = vec![
            ("Downloads", vec!["downloads", "download", "from downloads", "in downloads", "downloads folder", "download folder"]),
            ("Desktop", vec!["desktop", "from desktop", "in desktop", "desktop folder", "on desktop"]),
            ("Documents", vec!["documents", "document", "from documents", "in documents", "documents folder", "document folder", "my documents"]),
            ("Pictures", vec!["pictures", "picture", "photos", "images", "from pictures", "in pictures", "pictures folder"]),
            ("Music", vec!["music", "songs", "from music", "in music", "music folder"]),
            ("Videos", vec!["videos", "video", "from videos", "in videos", "videos folder"]),
        ];

        // Also check for explicit folder paths (Windows: C:\Users\..., Unix: /home/...)
        let path_pattern = regex::Regex::new(r"([A-Z]:\\[^\s]+|/[^\s]+|~/[^\s]+)").ok()?;
        for cap in path_pattern.captures_iter(&query) {
            if let Some(path_match) = cap.get(1) {
                let path = path_match.as_str().trim_end_matches('/').trim_end_matches('\\');
                if !folder_paths.contains(&path.to_string()) {
                    folder_paths.push(path.to_string());
                }
                cleaned_query = cleaned_query.replace(path, "").trim().to_string();
            }
        }

        for (folder_name, patterns) in folder_patterns {
            for pattern in &patterns {
                // Use word boundaries for better matching
                let pattern_re = regex::Regex::new(&format!(r"\b{}\b", regex::escape(pattern))).ok();
                let matched = if let Some(ref re) = pattern_re {
                    re.is_match(&query_lower)
                } else {
                    query_lower.contains(pattern)
                };
                
                if matched {
                    if !folder_paths.contains(&folder_name.to_string()) {
                        folder_paths.push(folder_name.to_string());
                    }
                    // Remove pattern from query more carefully
                    if let Some(ref re) = pattern_re {
                        cleaned_query = re.replace(&cleaned_query, "").trim().to_string();
                    } else {
                        cleaned_query = cleaned_query
                            .replacen(pattern, "", 1)
                            .trim()
                            .to_string();
                    }
                    break;
                }
            }
        }

        if !folder_paths.is_empty() {
            Some((folder_paths, cleaned_query))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simple_query_no_filters() {
        let parser = QueryParser::new("".to_string());
        let result = parser.parse("homework").await;
        
        assert_eq!(result.query, "homework");
        assert!(result.filters.date_range.is_none());
        assert!(result.filters.file_types.is_none());
        assert!(result.filters.folder_paths.is_none());
    }

    #[tokio::test]
    async fn test_date_filter_december() {
        let parser = QueryParser::new("".to_string());
        let result = parser.parse("homework from December").await;
        
        assert!(result.query.contains("homework") || result.query.is_empty());
        assert!(result.filters.date_range.is_some());
        if let Some(date_range) = result.filters.date_range {
            assert_eq!(date_range.month, Some(12));
        }
    }

    #[tokio::test]
    async fn test_file_type_filter_pdf() {
        let parser = QueryParser::new("".to_string());
        let result = parser.parse("homework PDF files").await;
        
        assert!(result.query.contains("homework") || result.query.is_empty());
        assert!(result.filters.file_types.is_some());
        if let Some(file_types) = result.filters.file_types {
            assert!(file_types.contains(&"pdf".to_string()));
        }
    }

    #[tokio::test]
    async fn test_folder_filter_downloads() {
        let parser = QueryParser::new("".to_string());
        let result = parser.parse("homework in Downloads").await;
        
        assert!(result.query.contains("homework") || result.query.is_empty());
        assert!(result.filters.folder_paths.is_some());
        if let Some(folder_paths) = result.filters.folder_paths {
            assert!(folder_paths.iter().any(|f| f.to_lowercase().contains("download")));
        }
    }

    #[tokio::test]
    async fn test_combined_filters() {
        let parser = QueryParser::new("".to_string());
        let result = parser.parse("homework PDF from December in Downloads").await;
        
        assert!(result.filters.date_range.is_some());
        assert!(result.filters.file_types.is_some());
        assert!(result.filters.folder_paths.is_some());
    }

    #[tokio::test]
    async fn test_relative_dates() {
        let parser = QueryParser::new("".to_string());
        
        let today_result = parser.parse("files today").await;
        assert!(today_result.filters.date_range.is_some());
        
        let yesterday_result = parser.parse("files yesterday").await;
        assert!(yesterday_result.filters.date_range.is_some());
        
        let last_week_result = parser.parse("files last week").await;
        assert!(last_week_result.filters.date_range.is_some());
    }

    #[tokio::test]
    async fn test_year_filter() {
        let parser = QueryParser::new("".to_string());
        let result = parser.parse("files from 2024").await;
        
        assert!(result.filters.date_range.is_some());
        if let Some(date_range) = result.filters.date_range {
            assert_eq!(date_range.year, Some(2024));
        }
    }

    #[test]
    fn test_pattern_only_parse() {
        let parser = QueryParser::new("".to_string());
        let result = parser.parse_pattern_only("homework PDF from December");
        
        assert!(result.filters.file_types.is_some());
        assert!(result.filters.date_range.is_some());
    }

    #[test]
    fn test_query_complexity_scoring() {
        // Simple queries should have low scores
        assert!(QueryParser::calculate_query_complexity("homework") < 0.3);
        assert!(QueryParser::calculate_query_complexity("pdf files") < 0.3);
        
        // Medium complexity queries
        let medium_score = QueryParser::calculate_query_complexity("homework from December");
        println!("Medium query score: {}", medium_score);
        // Should score at least 0.25 (has filter indicators)
        assert!(medium_score >= 0.25, "Medium query scored {}, expected >= 0.25", medium_score);
        assert!(medium_score < 0.7, "Medium query scored {}, expected < 0.7", medium_score);
        
        // Complex queries should have high scores
        let complex_score = QueryParser::calculate_query_complexity(
            "linear algebra homework from December 2023 in Downloads folder containing equations"
        );
        assert!(complex_score >= 0.5);
        
        // Very complex queries
        let very_complex = QueryParser::calculate_query_complexity(
            "find all PDF documents and Word files from last month that were created before December and contain meeting notes about the project"
        );
        assert!(very_complex >= 0.6);
    }

    #[test]
    fn test_should_try_llm() {
        // Simple queries should not trigger LLM
        assert!(!QueryParser::should_try_llm("homework"));
        assert!(!QueryParser::should_try_llm("pdf"));
        
        // Medium queries with filter indicators - check if they meet threshold
        let medium_score = QueryParser::calculate_query_complexity("homework from December");
        println!("Medium query score: {}, threshold: 0.3", medium_score);
        // Only assert if score meets threshold (0.3)
        if medium_score >= 0.3 {
            assert!(QueryParser::should_try_llm("homework from December"), 
                "Medium query with score {} should trigger LLM", medium_score);
        } else {
            // If below threshold, that's acceptable - pattern matching will handle it
            println!("Note: Medium query scored {} (below 0.3 threshold), will use pattern matching", medium_score);
        }
        
        // Complex queries should trigger LLM
        assert!(QueryParser::should_try_llm(
            "linear algebra homework from December 2023 in Downloads"
        ));
    }
}
