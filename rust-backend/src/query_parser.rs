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

        // Pattern matching didn't find filters - try LLM parsing if model is available
        if !self.llm_model.is_empty() {
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

        let parsed: LlmParsedQuery = serde_json::from_str(json_text)?;

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
                
                let mut date_range = DateRange {
                    start: None,
                    end: None,
                    month: date_filter.month,
                    year: date_filter.year.or(Some(current_year)),
                };

                // Calculate timestamps if month/year provided
                if let Some(month) = date_range.month {
                    let year = date_range.year.unwrap_or(current_year);
                    if let Some(start_date) = NaiveDate::from_ymd_opt(year, month, 1) {
                        if let Some(start_naive) = start_date.and_hms_opt(0, 0, 0) {
                            if let Some(start_dt) = Local.from_local_datetime(&start_naive).single() {
                                date_range.start = Some(start_dt.timestamp());
                            }
                        }

                        // End of month
                        let next_month = if month == 12 {
                            NaiveDate::from_ymd_opt(year + 1, 1, 1)
                        } else {
                            NaiveDate::from_ymd_opt(year, month + 1, 1)
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
                    }
                } else if let Some(year) = date_range.year {
                    // Entire year
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

                filters.date_range = Some(date_range);
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

    /// Extract date filters from query
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

        // Pattern: "from December", "in December", "December"
        let month_patterns = vec![
            ("january", 1), ("february", 2), ("march", 3), ("april", 4),
            ("may", 5), ("june", 6), ("july", 7), ("august", 8),
            ("september", 9), ("october", 10), ("november", 11), ("december", 12),
        ];

        for (month_name, month_num) in month_patterns {
            let patterns = vec![
                format!("from {}", month_name),
                format!("in {}", month_name),
                format!("{}", month_name),
            ];

            for pattern in &patterns {
                if query_lower.contains(pattern) {
                    date_range.month = Some(month_num);
                    date_range.year = Some(current_year);
                    
                    // Calculate start and end timestamps for the month
                    if let Some(start_date) = NaiveDate::from_ymd_opt(current_year, month_num, 1) {
                        if let Some(start_naive) = start_date.and_hms_opt(0, 0, 0) {
                            let start_dt = Local.from_local_datetime(&start_naive).single();
                            if let Some(dt) = start_dt {
                                date_range.start = Some(dt.timestamp());
                            }
                        }

                        // End of month - get last day
                        let next_month = if month_num == 12 {
                            NaiveDate::from_ymd_opt(current_year + 1, 1, 1)
                        } else {
                            NaiveDate::from_ymd_opt(current_year, month_num + 1, 1)
                        };
                        
                        if let Some(next) = next_month {
                            if let Some(last_day) = next.pred_opt() {
                                if let Some(end_naive) = last_day.and_hms_opt(23, 59, 59) {
                                    let end_dt = Local.from_local_datetime(&end_naive).single();
                                    if let Some(dt) = end_dt {
                                        date_range.end = Some(dt.timestamp());
                                    }
                                }
                            }
                        }
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

        // Pattern: "from 2024", "in 2024", "2024"
        let year_pattern = regex::Regex::new(r"\b(?:from|in)\s+(\d{4})\b|\b(\d{4})\b").ok()?;
        if let Some(caps) = year_pattern.captures(&query_lower) {
            let year_str = caps.get(1).or_else(|| caps.get(2))?.as_str();
            if let Ok(year) = year_str.parse::<i32>() {
                    if year >= 2000 && year <= 2100 {
                        date_range.year = Some(year);
                        if date_range.month.is_none() {
                            // If no month specified, use entire year
                            if let Some(start_date) = NaiveDate::from_ymd_opt(year, 1, 1) {
                                if let Some(start_naive) = start_date.and_hms_opt(0, 0, 0) {
                                    let start_dt = Local.from_local_datetime(&start_naive).single();
                                    if let Some(dt) = start_dt {
                                        date_range.start = Some(dt.timestamp());
                                    }
                                }
                            }
                            if let Some(end_date) = NaiveDate::from_ymd_opt(year, 12, 31) {
                                if let Some(end_naive) = end_date.and_hms_opt(23, 59, 59) {
                                    let end_dt = Local.from_local_datetime(&end_naive).single();
                                    if let Some(dt) = end_dt {
                                        date_range.end = Some(dt.timestamp());
                                    }
                                }
                            }
                        }
                        cleaned_query = year_pattern.replace(&cleaned_query, "").trim().to_string();
                    }
            }
        }

        // Pattern: "last week", "this month", "yesterday"
        if query_lower.contains("last week") {
            let week_ago = now - chrono::Duration::days(7);
            date_range.start = Some(week_ago.timestamp());
            date_range.end = Some(now.timestamp());
            cleaned_query = cleaned_query.replace("last week", "").trim().to_string();
        } else if query_lower.contains("this month") {
            if let Some(month_start) = NaiveDate::from_ymd_opt(current_year, current_month, 1) {
                if let Some(start_naive) = month_start.and_hms_opt(0, 0, 0) {
                    let start_dt = Local.from_local_datetime(&start_naive).single();
                    if let Some(dt) = start_dt {
                        date_range.start = Some(dt.timestamp());
                    }
                }
            }
            date_range.end = Some(now.timestamp());
            date_range.month = Some(current_month);
            date_range.year = Some(current_year);
            cleaned_query = cleaned_query.replace("this month", "").trim().to_string();
        } else if query_lower.contains("yesterday") {
            let yesterday = now - chrono::Duration::days(1);
            let yesterday_naive = yesterday.date_naive();
            if let Some(start_naive) = yesterday_naive.and_hms_opt(0, 0, 0) {
                let start_dt = Local.from_local_datetime(&start_naive).single();
                if let Some(dt) = start_dt {
                    date_range.start = Some(dt.timestamp());
                }
            }
            if let Some(end_naive) = yesterday_naive.and_hms_opt(23, 59, 59) {
                let end_dt = Local.from_local_datetime(&end_naive).single();
                if let Some(dt) = end_dt {
                    date_range.end = Some(dt.timestamp());
                }
            }
            cleaned_query = cleaned_query.replace("yesterday", "").trim().to_string();
        }

        if date_range.start.is_some() || date_range.end.is_some() || date_range.month.is_some() {
            Some((date_range, cleaned_query))
        } else {
            None
        }
    }

    /// Extract file type filters from query
    fn extract_file_types(query: &str) -> Option<(Vec<String>, String)> {
        let query_lower = query.to_lowercase();
        let mut cleaned_query = query.to_string();
        let mut file_types = Vec::new();

        // File type mappings
        let type_patterns: Vec<(&str, Vec<&str>)> = vec![
            ("pdf", vec!["pdf", "pdf files", "pdf documents"]),
            ("docx", vec!["word", "word documents", "docx", "doc files", "documents"]),
            ("xlsx", vec!["excel", "spreadsheet", "spreadsheets", "xlsx", "xls files"]),
            ("txt", vec!["text files", "text", "txt files"]),
            ("jpg", vec!["images", "image", "pictures", "photos", "jpg", "jpeg", "png"]),
            ("mp4", vec!["videos", "video", "mp4", "movie", "movies"]),
        ];

        for (ext, patterns) in type_patterns {
            for pattern in &patterns {
                if query_lower.contains(pattern) {
                    if !file_types.contains(&ext.to_string()) {
                        file_types.push(ext.to_string());
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

        if !file_types.is_empty() {
            Some((file_types, cleaned_query))
        } else {
            None
        }
    }

    /// Extract folder path filters from query
    fn extract_folder_paths(query: &str) -> Option<(Vec<String>, String)> {
        let query_lower = query.to_lowercase();
        let mut cleaned_query = query.to_string();
        let mut folder_paths = Vec::new();

        // Folder name patterns
        let folder_patterns: Vec<(&str, Vec<&str>)> = vec![
            ("Downloads", vec!["downloads", "download", "from downloads", "in downloads"]),
            ("Desktop", vec!["desktop", "from desktop", "in desktop"]),
            ("Documents", vec!["documents", "document", "from documents", "in documents", "documents folder"]),
        ];

        for (folder_name, patterns) in folder_patterns {
            for pattern in &patterns {
                if query_lower.contains(pattern) {
                    if !folder_paths.contains(&folder_name.to_string()) {
                        folder_paths.push(folder_name.to_string());
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

        if !folder_paths.is_empty() {
            Some((folder_paths, cleaned_query))
        } else {
            None
        }
    }
}
