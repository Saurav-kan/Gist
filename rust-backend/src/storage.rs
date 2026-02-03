use anyhow::Result;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::task;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub id: i64,
    pub file_path: String,
    pub file_name: String,
    pub file_size: i64,
    pub modified_time: i64,
    pub file_type: String,
    pub embedding_offset: i64,
    pub embedding_length: i64,
}

pub struct Storage {
    db_path: PathBuf,
    embeddings_path: PathBuf,
}

impl Storage {
    pub async fn new(data_dir: &PathBuf) -> Result<Self> {
        std::fs::create_dir_all(data_dir)?;
        
        let db_path = data_dir.join("metadata.db");
        
        // Initialize database in blocking thread
        let db_path_clone = db_path.clone();
        task::spawn_blocking(move || -> Result<()> {
            let conn = Connection::open(&db_path_clone)?;
            
            // Create tables
            conn.execute(
                "CREATE TABLE IF NOT EXISTS files (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    file_path TEXT NOT NULL UNIQUE,
                    file_name TEXT NOT NULL,
                    file_size INTEGER NOT NULL,
                    modified_time INTEGER NOT NULL,
                    file_type TEXT NOT NULL,
                    embedding_offset INTEGER NOT NULL,
                    embedding_length INTEGER NOT NULL
                )",
                [],
            )?;
            
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_file_path ON files(file_path)",
                [],
            )?;
            
            Ok(())
        }).await??;
        
        let embeddings_path = data_dir.join("embeddings.bin");
        
        Ok(Self {
            db_path,
            embeddings_path,
        })
    }

    pub async fn add_file(&self, metadata: &FileMetadata, embedding: &[f32]) -> Result<()> {
        // Check if file already exists in index
        let existing_metadata = self.get_file_metadata(&metadata.file_path).await?;
        
        let (offset, length) = if let Some(existing) = existing_metadata {
            // File exists - check if it has changed
            if existing.modified_time == metadata.modified_time 
                && existing.file_size == metadata.file_size 
                && existing.embedding_length == (bincode::serialize(embedding)?.len() as i64) {
                // File hasn't changed, reuse existing embedding
                (existing.embedding_offset, existing.embedding_length)
            } else {
                // File has changed, need new embedding
                // Get current file size for offset (append to end)
                let new_offset = if self.embeddings_path.exists() {
                    std::fs::metadata(&self.embeddings_path)?.len() as i64
                } else {
                    0
                };
                
                // Serialize and append new embedding
                let serialized = bincode::serialize(embedding)?;
                let new_length = serialized.len() as i64;
                
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .write(true)
                    .open(&self.embeddings_path)?;
                
                file.write_all(&serialized)?;
                file.flush()?;
                
                (new_offset, new_length)
            }
        } else {
            // New file, append embedding
            let new_offset = if self.embeddings_path.exists() {
                std::fs::metadata(&self.embeddings_path)?.len() as i64
            } else {
                0
            };
            
            // Serialize embedding
            let serialized = bincode::serialize(embedding)?;
            let new_length = serialized.len() as i64;
            
            // Append embedding to binary file
            use std::io::Write;
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .write(true)
                .open(&self.embeddings_path)?;
            
            file.write_all(&serialized)?;
            file.flush()?;
            
            (new_offset, new_length)
        };
        
        // Update metadata in database
        let db_path = self.db_path.clone();
        let metadata_clone = metadata.clone();
        task::spawn_blocking(move || {
            let conn = Connection::open(&db_path)?;
            conn.execute(
                "INSERT OR REPLACE INTO files 
                 (file_path, file_name, file_size, modified_time, file_type, embedding_offset, embedding_length)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    metadata_clone.file_path,
                    metadata_clone.file_name,
                    metadata_clone.file_size,
                    metadata_clone.modified_time,
                    metadata_clone.file_type,
                    offset,
                    length
                ],
            )?;
            Ok::<(), anyhow::Error>(())
        }).await??;
        
        Ok(())
    }

    pub async fn get_file_metadata(&self, file_path: &str) -> Result<Option<FileMetadata>> {
        let db_path = self.db_path.clone();
        let file_path = file_path.to_string();
        
        task::spawn_blocking(move || {
            let conn = Connection::open(&db_path)?;
            let mut stmt = conn.prepare(
                "SELECT id, file_path, file_name, file_size, modified_time, file_type, 
                        embedding_offset, embedding_length
                 FROM files WHERE file_path = ?1"
            )?;
            
            let result = stmt.query_row(params![file_path], |row| {
                Ok(FileMetadata {
                    id: row.get(0)?,
                    file_path: row.get(1)?,
                    file_name: row.get(2)?,
                    file_size: row.get(3)?,
                    modified_time: row.get(4)?,
                    file_type: row.get(5)?,
                    embedding_offset: row.get(6)?,
                    embedding_length: row.get(7)?,
                })
            });
            
            match result {
                Ok(metadata) => Ok(Some(metadata)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e.into()),
            }
        }).await?
    }

    pub async fn get_all_files(&self) -> Result<Vec<FileMetadata>> {
        let db_path = self.db_path.clone();
        
        task::spawn_blocking(move || {
            let conn = Connection::open(&db_path)?;
            let mut stmt = conn.prepare(
                "SELECT id, file_path, file_name, file_size, modified_time, file_type,
                        embedding_offset, embedding_length
                 FROM files"
            )?;
            
            let rows = stmt.query_map([], |row| {
                Ok(FileMetadata {
                    id: row.get(0)?,
                    file_path: row.get(1)?,
                    file_name: row.get(2)?,
                    file_size: row.get(3)?,
                    modified_time: row.get(4)?,
                    file_type: row.get(5)?,
                    embedding_offset: row.get(6)?,
                    embedding_length: row.get(7)?,
                })
            })?;
            
            let mut files = Vec::new();
            for row in rows {
                files.push(row?);
            }
            
            Ok::<Vec<FileMetadata>, anyhow::Error>(files)
        }).await?
    }

    pub async fn get_embedding(&self, metadata: &FileMetadata) -> Result<Vec<f32>> {
        use std::io::{Seek, Read};
        
        // Retry logic for Windows file locking issues
        let mut retries = 5;
        let mut last_error = None;
        
        while retries > 0 {
            match std::fs::File::open(&self.embeddings_path) {
                Ok(mut file) => {
                    match file.seek(std::io::SeekFrom::Start(metadata.embedding_offset as u64)) {
                        Ok(_) => {
                            let mut buffer = vec![0u8; metadata.embedding_length as usize];
                            match file.read_exact(&mut buffer) {
                                Ok(_) => {
                                    match bincode::deserialize(&buffer) {
                                        Ok(embedding) => return Ok(embedding),
                                        Err(e) => {
                                            last_error = Some(e.into());
                                            break; // Deserialization error, don't retry
                                        }
                                    }
                                }
                                Err(e) => {
                                    last_error = Some(e.into());
                                    retries -= 1;
                                    if retries > 0 {
                                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                                        continue;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            last_error = Some(e.into());
                            retries -= 1;
                            if retries > 0 {
                                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                                continue;
                            }
                        }
                    }
                }
                Err(e) => {
                    last_error = Some(e.into());
                    retries -= 1;
                    if retries > 0 {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        continue;
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Failed to read embedding after retries")))
    }

    pub async fn get_all_embeddings(&self) -> Result<Vec<(FileMetadata, Vec<f32>)>> {
        let files = self.get_all_files().await?;
        let mut result = Vec::new();
        let mut errors = Vec::new();
        
        for file in files {
            match self.get_embedding(&file).await {
                Ok(embedding) => {
                    result.push((file, embedding));
                }
                Err(e) => {
                    eprintln!("Warning: Failed to get embedding for {}: {}", file.file_path, e);
                    errors.push((file.file_path.clone(), e));
                    // Continue with other files even if one fails
                }
            }
        }
        
        if result.is_empty() && !errors.is_empty() {
            return Err(anyhow::anyhow!("Failed to read any embeddings. First error: {}", errors[0].1));
        }
        
        Ok(result)
    }

    pub async fn delete_file(&self, file_path: &str) -> Result<()> {
        let db_path = self.db_path.clone();
        let file_path = file_path.to_string();
        
        task::spawn_blocking(move || {
            let conn = Connection::open(&db_path)?;
            conn.execute("DELETE FROM files WHERE file_path = ?1", params![file_path])?;
            Ok::<(), anyhow::Error>(())
        }).await?
    }

    pub fn embeddings_path(&self) -> &PathBuf {
        &self.embeddings_path
    }

    pub async fn clear_all(&self) -> Result<()> {
        // Delete all records from database
        let db_path = self.db_path.clone();
        task::spawn_blocking(move || {
            let conn = Connection::open(&db_path)?;
            conn.execute("DELETE FROM files", [])?;
            Ok::<(), anyhow::Error>(())
        }).await??;

        // Delete embeddings file
        if self.embeddings_path.exists() {
            std::fs::remove_file(&self.embeddings_path)?;
        }

        Ok(())
    }
}
