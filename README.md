# NLP File Explorer

A desktop application for semantic file search using vector embeddings. Built with Electron (frontend) and Rust (backend), powered by Ollama for local embedding generation.

## Features

- **Semantic Search**: Search files using natural language queries
- **Local Processing**: All embeddings generated locally using Ollama (no external API calls)
- **Multiple File Types**: Supports text files, PDFs, Word documents, and Excel files
- **Performance Modes**: 
  - **Lightweight Mode**: Uses `all-minilm` model (384 dims, ~86MB) for systems with 4GB+ RAM
  - **Normal Mode**: Uses `embeddinggemma` model (1024 dims, ~200-500MB) for systems with 8GB+ RAM
- **Auto-indexing**: Automatically indexes new and modified files

## Prerequisites

1. **Ollama**: Install from [https://ollama.ai](https://ollama.ai)
2. **Node.js**: Version 18 or higher
3. **Rust**: Latest stable version
4. **Ollama Models**: Pull the required embedding models:
   ```bash
   ollama pull all-minilm      # For lightweight mode
   ollama pull embeddinggemma # For normal mode
   ```

## Installation

### Backend (Rust)

```bash
cd rust-backend
cargo build --release
```

### Frontend (Electron)

```bash
cd electron
npm install
```

## Running

1. **Start Ollama** (if not already running):
   ```bash
   ollama serve
   ```

2. **Start the Rust backend**:
   ```bash
   cd rust-backend
   cargo run --release
   ```
   The backend will run on `http://localhost:8080`

3. **Start the Electron frontend**:
   ```bash
   cd electron
   npm start
   ```

## Usage

1. **Configure Settings**: Go to the Settings tab and select your preferred performance mode
2. **Index Directories**: Add directories to index in the Settings tab
3. **Search**: Use the search bar to find files using natural language queries

## Project Structure

```
NLPFileExplorer/
├── electron/          # Electron frontend
│   └── src/
│       ├── main.js    # Main process
│       ├── preload.js # Preload script
│       └── renderer/  # UI code
├── rust-backend/      # Rust backend service
│   └── src/
│       ├── main.rs    # HTTP server
│       ├── api/       # API routes
│       ├── config/    # Configuration
│       ├── embedding/ # Ollama integration
│       ├── indexer/   # File indexing
│       ├── search/    # Vector search
│       ├── storage/   # Vector storage
│       └── parsers/   # Document parsers
└── README.md
```

## Configuration

Configuration is stored in `~/.nlpfileexplorer/config.json`. You can modify settings through the UI or directly edit the config file.

## Hardware Requirements

### Lightweight Mode
- **RAM**: 4GB minimum
- **CPU**: Dual-core processor
- **Storage**: 1-2GB free space

### Normal Mode
- **RAM**: 8GB minimum
- **CPU**: Modern multi-core processor
- **Storage**: 2-5GB free space
- **GPU**: Optional but recommended (40% faster inference)

## License

MIT
