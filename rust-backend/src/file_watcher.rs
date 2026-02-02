use anyhow::Result;
use notify::{Watcher, RecursiveMode, Event, EventKind};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::indexer::Indexer;

pub struct FileWatcher {
    _indexer: Arc<Indexer>, // Kept for handle_event closure
    watcher: notify::RecommendedWatcher,
    _tx: mpsc::UnboundedSender<()>, // Keep the channel alive
}

impl FileWatcher {
    pub fn new(indexer: Arc<Indexer>, directories: Vec<String>) -> Result<Self> {
        let (tx, _rx) = mpsc::unbounded_channel();
        
        let indexer_clone = indexer.clone();
        let (watcher_tx, mut watcher_rx) = mpsc::unbounded_channel();
        
        // Spawn task to handle file events
        tokio::spawn(async move {
            while let Some(event) = watcher_rx.recv().await {
                Self::handle_event(&indexer_clone, event).await;
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
            watcher,
            _tx: tx,
        })
    }

    async fn handle_event(indexer: &Indexer, event: Event) {
        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                for path in event.paths {
                    if path.is_file() {
                        if let Some(path_str) = path.to_str() {
                            if let Err(e) = indexer.index_file(path_str).await {
                                eprintln!("Error auto-indexing {}: {}", path_str, e);
                            }
                        }
                    }
                }
            }
            EventKind::Remove(_) => {
                // Could implement file deletion from index here
                for path in event.paths {
                    if let Some(path_str) = path.to_str() {
                        println!("File removed: {}", path_str);
                        // TODO: Remove from index
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
