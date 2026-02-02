use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::AppConfig;
use crate::embedding::EmbeddingService;
use crate::parsers::ParserRegistry;
use crate::storage::{Storage, FileMetadata};

pub struct Indexer {
    storage: Arc<Storage>,
    embedding_service: Arc<EmbeddingService>,
    parser_registry: Arc<ParserRegistry>,
    config: Arc<AppConfig>,
    is_indexing: Arc<RwLock<bool>>,
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
        }
    }

    pub async fn index_directory(&self, directory: &str) -> Result<usize> {
        let mut indexing = self.is_indexing.write().await;
        if *indexing {
            return Err(anyhow::anyhow!("Indexing already in progress"));
        }
        *indexing = true;
        drop(indexing);

        let mut count = 0;
        let dir_path = PathBuf::from(directory);

        for entry in walkdir::WalkDir::new(&dir_path)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                let file_path = entry.path().to_string_lossy().to_string();
                
                if self.parser_registry.can_parse(&file_path) {
                    if let Err(e) = self.index_file(&file_path).await {
                        eprintln!("Error indexing {}: {}", file_path, e);
                    } else {
                        count += 1;
                    }
                }
            }
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
}
