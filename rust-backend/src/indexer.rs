use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
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
                if !Self::should_exclude_file(&file_path) && self.parser_registry.can_parse(&file_path) {
                    total_files += 1;
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
                
                if self.parser_registry.can_parse(&file_path) {
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
                    
                    let result = indexer.index_file(&file_path).await;
                    (file_path, result)
                }));
            }
            
            // Wait for all tasks in batch to complete
            for task in tasks {
                match task.await {
                    Ok((file_path, Ok(_))) => {
                        count += 1;
                        current += 1;
                        
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
        // Extract text
        let text = self.parser_registry.extract_text(file_path)?;
        
        if text.trim().is_empty() {
            return Ok(());
        }

        // Chunk text if needed
        let chunks = self.chunk_text(&text);
        
        // Generate embedding for first chunk (or combine chunks)
        let combined_text = if chunks.len() > 1 {
            chunks.join("\n\n")
        } else {
            chunks.into_iter().next().unwrap_or_default()
        };

        // Generate embedding
        let embedding = self.embedding_service.generate_embedding(&combined_text).await?;

        // Get file metadata
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

        let file_metadata = FileMetadata {
            id: 0,
            file_path: file_path.to_string(),
            file_name,
            file_size: metadata.len() as i64,
            modified_time: metadata.modified()?
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64,
            file_type,
            embedding_offset: 0,
            embedding_length: 0,
        };

        // Store
        self.storage.add_file(&file_metadata, &embedding).await?;

        Ok(())
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

    pub async fn is_indexing(&self) -> bool {
        *self.is_indexing.read().await
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
        
        excluded_patterns.iter().any(|pattern| {
            file_name == pattern.to_lowercase()
        })
    }
}
