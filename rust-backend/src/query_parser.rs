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
    fn should_try_llm(query: &str) -> bool {
        let query_lower = query.to_lowercase();
        let words: Vec<&str> = query_lower.split_whitespace().collect();
        
        // Skip LLM for very short queries (1-2 words)
        if words.len() <= 2 {
            return false;
        }
        
        // Skip LLM if query is just common search terms without filter indicators
        let filter_indicators = [
            "from", "in", "last", "this", "yesterday", "week", "month", "year",
            "pdf", "word", "excel", "image", "video", "document",
            "downloads", "desktop", "documents", "folder"
        ];
        
        // Only try LLM if query contains potential filter indicators
        filter_indicators.iter().any(|indicator| query_lower.contains(indicator))
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
                    date_range.month = Some(month_num);
                    
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
                    
                    date_range.year = year;
                    
                    // Calculate start and end timestamps for the month
                    let year_val = year.unwrap_or(current_year);
                    if let Some(start_date) = NaiveDate::from_ymd_opt(year_val, month_num, 1) {
                        if let Some(start_naive) = start_date.and_hms_opt(0, 0, 0) {
                            if let Some(start_dt) = Local.from_local_datetime(&start_naive).single() {
                                date_range.start = Some(start_dt.timestamp());
                            }
                        }

                        // End of month - get last day
                        let next_month = if month_num == 12 {
                            NaiveDate::from_ymd_opt(year_val + 1, 1, 1)
                        } else {
                            NaiveDate::from_ymd_opt(year_val, month_num + 1, 1)
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

        // Enhanced file type mappings with more variations
        let type_patterns: Vec<(&str, Vec<&str>)> = vec![
            ("pdf", vec!["pdf", "pdf files", "pdf documents", "pdfs", ".pdf"]),
            ("docx", vec!["word", "word documents", "docx", "doc files", "documents", "microsoft word", "ms word", ".docx", ".doc"]),
            ("xlsx", vec!["excel", "spreadsheet", "spreadsheets", "xlsx", "xls files", "microsoft excel", "ms excel", ".xlsx", ".xls"]),
            ("txt", vec!["text files", "text", "txt files", "plain text", ".txt"]),
            ("jpg", vec!["images", "image", "pictures", "photos", "jpg", "jpeg", "png", "gif", "bmp", ".jpg", ".jpeg", ".png"]),
            ("mp4", vec!["videos", "video", "mp4", "movie", "movies", "avi", "mov", ".mp4", ".avi", ".mov"]),
            ("zip", vec!["zip", "zip files", "archives", "compressed", ".zip", ".rar", ".7z"]),
            ("mp3", vec!["audio", "music", "songs", "mp3", "sound", ".mp3", ".wav", ".flac"]),
            ("pptx", vec!["powerpoint", "presentation", "ppt", "pptx", ".pptx", ".ppt"]),
            ("csv", vec!["csv", "csv files", "comma separated", ".csv"]),
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

        for (ext, patterns) in type_patterns {
            for pattern in &patterns {
                // Use word boundaries to avoid partial matches
                let pattern_re = regex::Regex::new(&format!(r"\b{}\b", regex::escape(pattern))).ok();
                let matched = if let Some(ref re) = pattern_re {
                    re.is_match(&query_lower)
                } else {
                    query_lower.contains(pattern)
                };
                
                if matched {
                    if !file_types.contains(&ext.to_string()) {
                        file_types.push(ext.to_string());
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
