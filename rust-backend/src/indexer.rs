use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;

use crate::config::AppConfig;
use crate::embedding::EmbeddingService;
use crate::parsers::ParserRegistry;
use crate::storage::{Storage, FileMetadata};

#[derive(Clone)]
pub struct IndexingProgress {
    pub is_indexing: bool,
    pub current: usize,
    pub total: usize,
    pub current_file: String,
    pub directory: String,
}

#[derive(Clone)]
pub struct Indexer {
    storage: Arc<Storage>,
    embedding_service: Arc<EmbeddingService>,
    parser_registry: Arc<ParserRegistry>,
    config: Arc<AppConfig>,
    is_indexing: Arc<RwLock<bool>>,
    progress: Option<Arc<tokio::sync::RwLock<Option<IndexingProgress>>>>,
}

impl Indexer {
    pub fn new(
        storage: Arc<Storage>,
        embedding_service: Arc<EmbeddingService>,
        parser_registry: Arc<ParserRegistry>,
        config: Arc<AppConfig>,
    ) -> Self {
        Self {
            storage,
            embedding_service,
            parser_registry,
            config,
            is_indexing: Arc::new(RwLock::new(false)),
            progress: None,
        }
    }
    
    pub fn with_progress_tracker(mut self, progress: Arc<tokio::sync::RwLock<Option<IndexingProgress>>>) -> Self {
        self.progress = Some(progress);
        self
    }

    pub async fn index_directory(&self, directory: &str) -> Result<usize> {
        let mut indexing = self.is_indexing.write().await;
        if *indexing {
            return Err(anyhow::anyhow!("Indexing already in progress"));
        }
        *indexing = true;
        drop(indexing);

        // First pass: count total files to index
        let dir_path = PathBuf::from(directory);
        let mut total_files = 0;
        for entry in walkdir::WalkDir::new(&dir_path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                let file_path = entry.path().to_string_lossy().to_string();
                if !Self::should_exclude_file(&file_path) {
                    // Count files that will be indexed (either metadata-only or content-indexed)
                    if Self::should_index_metadata_only(&file_path) || self.parser_registry.can_parse(&file_path) {
                        total_files += 1;
                    }
                }
            }
        }

        // Initialize progress
        if let Some(ref progress_tracker) = self.progress {
            let mut progress = progress_tracker.write().await;
            *progress = Some(IndexingProgress {
                is_indexing: true,
                current: 0,
                total: total_files,
                current_file: String::new(),
                directory: directory.to_string(),
            });
        }

        // Benchmark tracking: start timer for first 1000 files
        let start_time = std::time::Instant::now();
        let mut benchmark_1000_logged = false;
        
        let mut count = 0;
        let mut current = 0;

        // Collect all files to index
        let mut files_to_index = Vec::new();
        for entry in walkdir::WalkDir::new(&dir_path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                let file_path = entry.path().to_string_lossy().to_string();
                
                // Skip files that tend to give false positives
                if Self::should_exclude_file(&file_path) {
                    continue;
                }
                
                // Check if this file should be metadata-only or content-indexed
                if Self::should_index_metadata_only(&file_path) || self.parser_registry.can_parse(&file_path) {
                    files_to_index.push(file_path);
                }
            }
        }

        // Process files in batches for parallel embedding generation
        const BATCH_SIZE: usize = 5; // Process 5 files concurrently
        for batch in files_to_index.chunks(BATCH_SIZE) {
            // Create tasks for parallel processing
            let mut tasks = Vec::new();
            for file_path in batch {
                let file_path = file_path.clone();
                let indexer = self.clone();
                let progress_tracker = self.progress.clone();
                
                    tasks.push(tokio::spawn(async move {
                    // Update progress before starting
                    if let Some(ref tracker) = progress_tracker {
                        let mut progress = tracker.write().await;
                        if let Some(ref mut p) = *progress {
                            p.current_file = file_path.clone();
                        }
                    }
                    
                    // Route to appropriate indexing method
                    let result = if Self::should_index_metadata_only(&file_path) {
                        indexer.index_file_metadata_only(&file_path).await
                    } else {
                        indexer.index_file(&file_path).await
                    };
                    (file_path, result)
                }));
            }
            
            // Wait for all tasks in batch to complete
            for task in tasks {
                match task.await {
                    Ok((file_path, Ok(_))) => {
                        count += 1;
                        current += 1;
                        
                        // Benchmark: Log time for first 1000 files
                        if count == 1000 && !benchmark_1000_logged {
                            let elapsed = start_time.elapsed();
                            let elapsed_secs = elapsed.as_secs_f64();
                            let files_per_sec = 1000.0 / elapsed_secs;
                            
                            eprintln!("═══════════════════════════════════════════════════════════");
                            eprintln!("[BENCHMARK] First 1000 files processed!");
                            eprintln!("[BENCHMARK] Time elapsed: {:.2} seconds ({:.2} minutes)", elapsed_secs, elapsed_secs / 60.0);
                            eprintln!("[BENCHMARK] Processing rate: {:.2} files/second", files_per_sec);
                            eprintln!("[BENCHMARK] Average time per file: {:.3} seconds", elapsed_secs / 1000.0);
                            eprintln!("═══════════════════════════════════════════════════════════");
                            
                            benchmark_1000_logged = true;
                        }
                        
                        // Update progress
                        if let Some(ref progress_tracker) = self.progress {
                            let mut progress = progress_tracker.write().await;
                            if let Some(ref mut p) = *progress {
                                p.current = current;
                                p.current_file = file_path;
                            }
                        }
                    }
                    Ok((file_path, Err(e))) => {
                        eprintln!("Error indexing {}: {}", file_path, e);
                        current += 1;
                        
                        // Update progress even on error
                        if let Some(ref progress_tracker) = self.progress {
                            let mut progress = progress_tracker.write().await;
                            if let Some(ref mut p) = *progress {
                                p.current = current;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Task error: {}", e);
                        current += 1;
                    }
                }
            }
        }

        // Log final benchmark if we processed at least 1000 files
        if count >= 1000 && benchmark_1000_logged {
            let total_elapsed = start_time.elapsed();
            let total_elapsed_secs = total_elapsed.as_secs_f64();
            eprintln!("[BENCHMARK] Total files indexed: {}", count);
            eprintln!("[BENCHMARK] Total time: {:.2} seconds ({:.2} minutes)", total_elapsed_secs, total_elapsed_secs / 60.0);
        } else if count < 1000 {
            let elapsed = start_time.elapsed();
            eprintln!("[BENCHMARK] Indexed {} files in {:.2} seconds (less than 1000 files, no 1k benchmark)", count, elapsed.as_secs_f64());
        }

        // Clear progress
        if let Some(ref progress_tracker) = self.progress {
            let mut progress = progress_tracker.write().await;
            *progress = None;
        }

        let mut indexing = self.is_indexing.write().await;
        *indexing = false;

        Ok(count)
    }

    pub async fn index_file(&self, file_path: &str) -> Result<()> {
        // Check if this file should be metadata-only
        if Self::should_index_metadata_only(file_path) {
            return self.index_file_metadata_only(file_path).await;
        }
        
        // Extract text - on failure, store metadata-only so we don't reindex every run
        let text = match self.parser_registry.extract_text(file_path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("[INDEXING] Text extraction failed for {}: {}. Indexing metadata only.", file_path, e);
                return self.index_file_metadata_only(file_path).await;
            }
        };
        
        if text.trim().is_empty() {
            // No extractable text - store metadata-only so we don't reindex every run
            return self.index_file_metadata_only(file_path).await;
        }

        // Chunk text if needed
        let chunks = self.chunk_text(&text);
        
        // Get file metadata (needed for both single and multiple embeddings)
        let metadata = std::fs::metadata(file_path)?;
        let file_name = PathBuf::from(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        let file_type = PathBuf::from(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("unknown")
            .to_string();

        let modified_time = metadata.modified()?
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64;
        let file_size = metadata.len() as i64;

        // Estimate total tokens (rough: 1 token ≈ 4 characters)
        let total_estimated_tokens: usize = chunks.iter()
            .map(|c| c.len() / 4)
            .sum();
        
        let max_context = self.config.max_context_tokens;
        let multiple_embedding_threshold = max_context * 4; // 4x threshold for multiple embeddings

        // Decide strategy based on file size
        if total_estimated_tokens <= max_context {
            // File fits in context - use all chunks
            let combined_text = chunks.join("\n\n");
            
            // Double check length just in case
            let final_text = if combined_text.len() > max_context * 4 {
                let max_chars = max_context * 4;
                 combined_text.chars().take(max_chars).collect()
            } else {
                combined_text
            };

            let embedding = self.generate_safe_embedding(&final_text, &file_name).await?;
            
            let file_metadata = FileMetadata {
                id: 0,
                file_path: file_path.to_string(),
                file_name: file_name.clone(),
                file_size,
                modified_time,
                file_type: file_type.clone(),
                embedding_offset: 0,
                embedding_length: 0,
            };
            
            self.storage.add_file(&file_metadata, Some(&embedding)).await?;
        } else if total_estimated_tokens <= multiple_embedding_threshold {
            // File is 1x-4x context size - use intelligent sampling
            let sampled_text = Self::intelligent_chunk_sampling(&chunks, max_context);
            let embedding = self.generate_safe_embedding(&sampled_text, &file_name).await?;
            
            eprintln!("[INDEXING] Large file '{}' ({:.1}K tokens) - used intelligent sampling", 
                file_name, total_estimated_tokens as f64 / 1000.0);
            
            let file_metadata = FileMetadata {
                id: 0,
                file_path: file_path.to_string(),
                file_name: file_name.clone(),
                file_size,
                modified_time,
                file_type: file_type.clone(),
                embedding_offset: 0,
                embedding_length: 0,
            };
            
            self.storage.add_file(&file_metadata, Some(&embedding)).await?;
        } else {
            // File is >4x context size - generate multiple embeddings
            eprintln!("[INDEXING] Very large file '{}' ({:.1}K tokens) - generating multiple embeddings", 
                file_name, total_estimated_tokens as f64 / 1000.0);
            
            let embedding_sections = Self::create_multiple_embedding_sections(&chunks, max_context);
            
            for (section_idx, section_text) in embedding_sections.iter().enumerate() {
                let embedding = self.generate_safe_embedding(section_text, &file_name).await?;
                
                // Create unique file path for this embedding (for storage)
                let section_path = if section_idx == 0 {
                    file_path.to_string()
                } else {
                    format!("{}#section{}", file_path, section_idx + 1)
                };
                
                let section_file_name = if section_idx == 0 {
                    file_name.clone()
                } else {
                    format!("{} (section {})", file_name, section_idx + 1)
                };
                
                let file_metadata = FileMetadata {
                    id: 0,
                    file_path: section_path,
                    file_name: section_file_name,
                    file_size,
                    modified_time,
                    file_type: file_type.clone(),
                    embedding_offset: 0,
                    embedding_length: 0,
                };
                
                self.storage.add_file(&file_metadata, Some(&embedding)).await?;
            }
            
            eprintln!("[INDEXING] Generated {} embeddings for '{}'", embedding_sections.len(), file_name);
        }

        Ok(())
    }

    /// Wrapper for generating embeddings with retry logic for context length errors
    async fn generate_safe_embedding(&self, text: &str, file_name: &str) -> Result<Vec<f32>> {
        match self.embedding_service.generate_embedding(text).await {
            Ok(emb) => Ok(emb),
            Err(e) => {
                let error_msg = e.to_string();
                // Match "500 Internal" strictly, or "context length"
                if error_msg.contains("500") || error_msg.contains("context length") || error_msg.contains("Internal Server Error") {
                    println!("[INDEXING] Context length error for '{}', trying 50% truncation ({} chars)", file_name, text.len() / 2);
                    
                    let half_len = text.len() / 2;
                    // Ensure we don't slice in the middle of a char
                    let truncated: String = text.chars().take(half_len).collect();
                    
                    if half_len < 10 {
                         return Err(e); // Stop if too small
                    }

                     match self.embedding_service.generate_embedding(&truncated).await {
                         Ok(emb) => Ok(emb),
                         Err(e2) => {
                             // Try one more time at 25%?
                             println!("[INDEXING] Context length error again for '{}', trying 25% truncation...", file_name);
                             let quarter_len = text.len() / 4;
                             let truncated_q: String = text.chars().take(quarter_len).collect();
                             self.embedding_service.generate_embedding(&truncated_q).await
                         }
                     }
                } else {
                    Err(e)
                }
            }
        }
    }

    fn chunk_text(&self, text: &str) -> Vec<String> {
        let chunk_size = self.config.chunk_size;
        let mut chunks = Vec::new();
        
        let words: Vec<&str> = text.split_whitespace().collect();
        
        for chunk in words.chunks(chunk_size) {
            chunks.push(chunk.join(" "));
        }
        
        if chunks.is_empty() {
            chunks.push(text.to_string());
        }
        
        chunks
    }

    /// Intelligent chunk sampling: takes beginning, middle samples, and end
    /// This preserves information from different parts of the document
    fn intelligent_chunk_sampling(chunks: &[String], max_tokens: usize) -> String {
        if chunks.is_empty() {
            return String::new();
        }
        
        let num_chunks = chunks.len();
        let mut selected_chunks = Vec::new();
        
        // Always include beginning (title, intro, important context)
        if num_chunks > 0 {
            selected_chunks.push(chunks[0].clone());
        }
        
        // Sample from middle (distributed sampling)
        if num_chunks > 2 {
            let middle_start = num_chunks / 4;
            let middle_end = (num_chunks * 3) / 4;
            
            // Sample 2-3 chunks from middle section
            let num_middle_samples = 3.min(middle_end - middle_start);
            if num_middle_samples > 0 {
                let step = ((middle_end - middle_start) / num_middle_samples.max(1)).max(1);
                
                for i in (middle_start..middle_end).step_by(step) {
                    if i < chunks.len() && selected_chunks.len() < 4 {
                        selected_chunks.push(chunks[i].clone());
                    }
                }
            }
        }
        
        // Always include end (conclusions, summary)
        if num_chunks > 1 {
            selected_chunks.push(chunks[num_chunks - 1].clone());
        }
        
        // Combine selected chunks
        let combined = selected_chunks.join("\n\n");
        
        // Estimate tokens and truncate if still too large
        let estimated_tokens = combined.len() / 4;
        
        // Use a much safer margin (75% of max) to avoid edge cases with tokenization
        // Some models have very different token/char ratios for code/special chars
        let safe_limit = (max_tokens as f64 * 0.75) as usize;
        
        if estimated_tokens > safe_limit {
            // Truncate to safe limit
            let max_chars = safe_limit * 4;
            
            if combined.len() > max_chars {
                 combined.chars().take(max_chars).collect()
            } else {
                combined
            }
        } else {
            combined
        }
    }

    /// Create multiple embedding sections for very large files (>4x context size)
    /// Uses logarithmic scaling (log2(ratio + 1)) to prevent excessive embeddings for extremely large files
    fn create_multiple_embedding_sections(chunks: &[String], max_tokens: usize) -> Vec<String> {
        if chunks.is_empty() {
            return vec![String::new()];
        }
        
        // Calculate total tokens and ratio
        let total_tokens: usize = chunks.iter().map(|c| c.len() / 4).sum();
        let ratio = total_tokens as f64 / max_tokens as f64;
        
        // Logarithmic scaling: log2(ratio + 1)
        // This prevents excessive embeddings for extremely large files
        let num_sections = ((ratio + 1.0).log2().ceil() as usize).max(1);
        
        // If file fits in one section, return single section
        if num_sections == 1 {
            return vec![chunks.join("\n\n")];
        }
        
        // Divide chunks into sections with overlap
        let mut sections = Vec::new();
        let chunks_per_section = chunks.len() / num_sections;
        let overlap_chunks = (chunks_per_section as f64 * 0.2) as usize; // 20% overlap
        
        for i in 0..num_sections {
            let start_idx = if i == 0 {
                0
            } else {
                (i * chunks_per_section).saturating_sub(overlap_chunks)
            };
            
            let end_idx = if i == num_sections - 1 {
                chunks.len()
            } else {
                ((i + 1) * chunks_per_section).min(chunks.len())
            };
            
            if start_idx < end_idx && start_idx < chunks.len() {
                let section_chunks: Vec<String> = chunks[start_idx..end_idx].to_vec();
                sections.push(section_chunks.join("\n\n"));
            }
        }
        
        // Ensure we have at least one section
        if sections.is_empty() {
            sections.push(chunks.join("\n\n"));
        }
        
        sections
    }

    pub async fn is_indexing(&self) -> bool {
        *self.is_indexing.read().await
    }

    /// Check if a file should be indexed with metadata only (filename only, no content)
    pub fn should_index_metadata_only(file_path: &str) -> bool {
        let path = PathBuf::from(file_path);
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        // Config file extensions
        let config_extensions = [
            "json", "yaml", "yml", "toml", "ini", "cfg", 
            "conf", "properties", "config"
        ];
        
        // Binary/executable extensions
        let binary_extensions = [
            "exe", "dll", "jar", "so", "dylib", "dll.a", 
            "dat", "mca", "rrf", "igt", "class"
        ];
        
    // Log file extensions
    let log_extensions = ["log"];
    
    // Image extensions - index by filename only, not content
    // This prevents random images from appearing in semantic search results
    let image_extensions = [
        "jpg", "jpeg", "png", "gif", "bmp", "webp", 
        "svg", "ico", "tiff", "tif"
    ];
    
    config_extensions.contains(&ext.as_str()) ||
    binary_extensions.contains(&ext.as_str()) ||
    log_extensions.contains(&ext.as_str()) ||
    image_extensions.contains(&ext.as_str())
    }

    /// Index a file with metadata only (filename only, no content)
    async fn index_file_metadata_only(&self, file_path: &str) -> Result<()> {
        // Get file metadata
        let metadata = std::fs::metadata(file_path)?;
        let file_name = PathBuf::from(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        // Create metadata record without embedding
        
        // Store with metadata
        let file_metadata = FileMetadata {
            id: 0,
            file_path: file_path.to_string(),
            file_name,
            file_size: metadata.len() as i64,
            modified_time: metadata.modified()?
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64,
            file_type: PathBuf::from(file_path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("unknown")
                .to_string(),
            embedding_offset: 0,
            embedding_length: 0,
        };
        
        self.storage.add_file(&file_metadata, None).await?;
        Ok(())
    }

    /// Check if a file should be excluded from indexing due to high false positive rates
    pub fn should_exclude_file(file_path: &str) -> bool {
        let path = PathBuf::from(file_path);
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        // Exclude common config/boilerplate files that cause false positives
        let excluded_patterns = [
            "config.js",
            "index.html",
            "aca.conf.ini",
        ];
        
        if excluded_patterns.iter().any(|pattern| {
            file_name == pattern.to_lowercase()
        }) {
            return true;
        }
        
        // Exclude temporary/incomplete file extensions
        let excluded_extensions = [
            ".tmp",           // Temporary files
            ".crdownload",    // Chrome download files (incomplete)
            ".part",          // Firefox download files (incomplete)
            ".download",      // Generic download files (incomplete)
            ".partial",       // Partial download files
            ".lock",          // Lock files
            ".swp",           // Vim swap files
            ".~",             // Backup files
        ];
        
        excluded_extensions.iter().any(|ext| {
            file_name.ends_with(ext)
        })
    }
    pub async fn perform_startup_scan(&self) -> Result<()> {
        if !self.config.auto_index || self.config.indexed_directories.is_empty() {
            return Ok(());
        }

        println!("[STARTUP] Starting file synchronization...");
        
        let mut indexing = self.is_indexing.write().await;
        if *indexing {
            return Ok(());
        }
        *indexing = true;
        drop(indexing);

        // Get all files currently in the database
        let db_files = self.storage.get_all_files().await?;
        let mut db_files_map: HashMap<String, FileMetadata> = db_files
            .into_iter()
            .map(|f| (f.file_path.clone(), f))
            .collect();
            
        println!("[STARTUP] Database contains {} files. Scanning disk...", db_files_map.len());

        // Collect files to index (new or modified)
        let mut files_to_index = Vec::new();

        println!("[STARTUP] Configured to scan {} directories:", self.config.indexed_directories.len());
        for dir in &self.config.indexed_directories {
            println!("[STARTUP] - {}", dir);
             if !std::path::Path::new(dir).exists() {
                println!("[STARTUP]   (Directory does not exist, skipping)");
                continue;
            }
            
            for entry in walkdir::WalkDir::new(dir)
                .into_iter()
                .filter_map(|e| e.ok()) 
            {
                if entry.file_type().is_file() {
                     let file_path = entry.path().to_string_lossy().to_string();
                     
                     // Diagnostic logging for EVERY file to debug detection
                     // println!("[STARTUP] Checking: {}", file_path); 
                     
                     if Self::should_exclude_file(&file_path) {
                         // println!("[STARTUP] Excluded: {}", file_path);
                         continue;
                     }
                     
                     // Check if file exists in DB
                     if let Some(metadata) = db_files_map.remove(&file_path) {
                         // File exists in DB - only reindex if modified
                         if let Ok(fs_metadata) = std::fs::metadata(&file_path) {
                             let modified = fs_metadata.modified()
                                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs() as i64;
                            let size = fs_metadata.len() as i64;
                            
                             if modified != metadata.modified_time || size != metadata.file_size {
                                 println!("[STARTUP] File changed: {} (Time: {} vs {}, Size: {} vs {})", 
                                     file_path, modified, metadata.modified_time, size, metadata.file_size);
                                 if Self::should_index_metadata_only(&file_path) || self.parser_registry.can_parse(&file_path) {
                                     files_to_index.push(file_path.clone());
                                 } else {
                                     println!("[STARTUP] Skipping changed file (unsupported type): {}", file_path);
                                 }
                             }
                         }
                     } else {
                         // File NOT in DB - it's a new file
                         if Self::should_index_metadata_only(&file_path) || self.parser_registry.can_parse(&file_path) {
                             println!("[STARTUP] New file found: {}", file_path);
                             files_to_index.push(file_path.clone());
                         }
                     }
                }
            }
        }
        
        // Remove deleted files (those remaining in db_files_map)
        if !db_files_map.is_empty() {
            println!("[STARTUP] Found {} deleted files. Removing from index...", db_files_map.len());
            for (path, _) in db_files_map {
                if let Err(e) = self.storage.delete_file(&path).await {
                    eprintln!("Failed to delete file from index: {}: {}", path, e);
                }
            }
        }
        
        println!("[STARTUP] Found {} new/modified files to index.", files_to_index.len());
        
        // Index new/modified files
        // We can reuse the logic from index_directory but it takes a directory path.
        // It's better to iterate and call index_file directly or create a batch processor.
        // For simplicity reusing the logic similar to index_directory but for a specific list.
        
        if !files_to_index.is_empty() {
             // Initialize progress if tracker exists (optional for startup scan but good for UI)
             // For now just process them.
             
            for file_path in files_to_index {
                // Determine if metadata only
                let result = if Self::should_index_metadata_only(&file_path) {
                    println!("[STARTUP] Indexing metadata: {}", file_path);
                    self.index_file_metadata_only(&file_path).await
                } else {
                    println!("[STARTUP] Indexing content: {}", file_path);
                    
                    // IMPORTANT: We need to use index_file here, but index_file checks filtering again.
                    // It's safe to call.
                    self.index_file(&file_path).await
                };
                
                if let Err(e) = result {
                    eprintln!("Error indexing {}: {}", file_path, e);
                } else {
                    println!("[STARTUP] Successfully indexed: {}", file_path);
                }
            }
        }
        
        let mut indexing = self.is_indexing.write().await;
        *indexing = false;
        
        println!("[STARTUP] Sync complete.");
        Ok(())
    }
}

