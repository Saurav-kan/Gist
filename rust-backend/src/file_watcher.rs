use anyhow::Result;
use notify::{Watcher, RecursiveMode, Event, EventKind};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::indexer::Indexer;
use crate::storage::Storage;

pub struct FileWatcher {
    _indexer: Arc<Indexer>, // Kept for handle_event closure
    storage: Arc<Storage>, // Needed for removing deleted files
    watcher: notify::RecommendedWatcher,
    _tx: mpsc::UnboundedSender<()>, // Keep the channel alive
}

impl FileWatcher {
    pub fn new(indexer: Arc<Indexer>, storage: Arc<Storage>, directories: Vec<String>) -> Result<Self> {
        let (tx, _rx) = mpsc::unbounded_channel();
        
        let indexer_clone = indexer.clone();
        let storage_clone = storage.clone();
        let (watcher_tx, mut watcher_rx) = mpsc::unbounded_channel();
        
        // Spawn task to handle file events
        tokio::spawn(async move {
            while let Some(event) = watcher_rx.recv().await {
                Self::handle_event(&indexer_clone, &storage_clone, event).await;
            }
        });
        
        let mut watcher = notify::recommended_watcher(move |res| {
            if let Ok(event) = res {
                let _ = watcher_tx.send(event);
            }
        })?;
        
        // Watch all directories
        for dir in directories {
            if let Err(e) = watcher.watch(PathBuf::from(&dir).as_path(), RecursiveMode::Recursive) {
                eprintln!("Warning: Failed to watch directory {}: {}", dir, e);
            }
        }
        
        Ok(Self {
            _indexer: indexer,
            storage,
            watcher,
            _tx: tx,
        })
    }

    async fn handle_event(indexer: &Indexer, storage: &Storage, event: Event) {
        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                for path in event.paths {
                    if path.is_file() {
                        if let Some(path_str) = path.to_str() {
                            // Skip files that tend to give false positives
                            if Indexer::should_exclude_file(path_str) {
                                continue;
                            }
                            
                            if let Err(e) = indexer.index_file(path_str).await {
                                eprintln!("Error auto-indexing {}: {}", path_str, e);
                            }
                        }
                    }
                }
            }
            EventKind::Remove(_) => {
                // Remove deleted files from index
                for path in event.paths {
                    if let Some(path_str) = path.to_str() {
                        // Check if it's a file (not a directory)
                        if path.is_file() {
                            if let Err(e) = storage.delete_file(path_str).await {
                                eprintln!("Error removing file {} from index: {}", path_str, e);
                            } else {
                                println!("Removed file from index: {}", path_str);
                            }
                        } else {
                            // If it's a directory, remove all files in that directory from index
                            // Get all files that start with this path
                            if let Ok(all_files) = storage.get_all_files().await {
                                for file in all_files {
                                    if file.file_path.starts_with(path_str) {
                                        if let Err(e) = storage.delete_file(&file.file_path).await {
                                            eprintln!("Error removing file {} from index: {}", file.file_path, e);
                                        }
                                    }
                                }
                                println!("Removed directory and its files from index: {}", path_str);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    pub fn add_directory(&mut self, directory: &str) -> Result<()> {
        self.watcher.watch(PathBuf::from(directory).as_path(), RecursiveMode::Recursive)?;
        Ok(())
    }

    pub fn remove_directory(&mut self, directory: &str) -> Result<()> {
        self.watcher.unwatch(PathBuf::from(directory).as_path())?;
        Ok(())
    }
}
