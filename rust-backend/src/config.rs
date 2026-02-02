use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::Result;
use dirs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub performance_mode: PerformanceMode,
    pub embedding_model: String,
    pub indexed_directories: Vec<String>,
    pub file_type_filters: FileTypeFilters,
    pub chunk_size: usize,
    pub auto_index: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PerformanceMode {
    Lightweight,
    Normal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTypeFilters {
    pub include_pdf: bool,
    pub include_docx: bool,
    pub include_text: bool,
    pub include_xlsx: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            performance_mode: PerformanceMode::Normal,
            embedding_model: "embeddinggemma".to_string(),
            indexed_directories: Vec::new(),
            file_type_filters: FileTypeFilters {
                include_pdf: true,
                include_docx: true,
                include_text: true,
                include_xlsx: true,
            },
            chunk_size: 512,
            auto_index: true,
        }
    }
}

impl AppConfig {
    pub fn config_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".nlpfileexplorer")
    }

    pub fn config_file() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    pub fn data_dir() -> PathBuf {
        Self::config_dir().join("data")
    }

    pub async fn load_or_default() -> Result<Self> {
        let config_file = Self::config_file();
        
        if config_file.exists() {
            let content = tokio::fs::read_to_string(&config_file).await?;
            let mut config: AppConfig = serde_json::from_str(&content)?;
            
            // Ensure model matches performance mode
            config.update_model_for_mode();
            
            Ok(config)
        } else {
            let config = Self::default();
            config.save().await?;
            Ok(config)
        }
    }

    pub async fn save(&self) -> Result<()> {
        let config_dir = Self::config_dir();
        tokio::fs::create_dir_all(&config_dir).await?;
        
        let config_file = Self::config_file();
        let content = serde_json::to_string_pretty(self)?;
        tokio::fs::write(&config_file, content).await?;
        
        Ok(())
    }

    pub fn update_model_for_mode(&mut self) {
        self.embedding_model = match self.performance_mode {
            PerformanceMode::Lightweight => "all-minilm".to_string(),
            PerformanceMode::Normal => "embeddinggemma".to_string(),
        };
    }

    pub fn set_performance_mode(&mut self, mode: PerformanceMode) {
        self.performance_mode = mode;
        self.update_model_for_mode();
    }
}
