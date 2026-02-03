# GIST Vector Search

A modern desktop application for semantic file search using AI-powered vector embeddings. Built with Electron and Rust, powered by Ollama for local, privacy-focused embedding generation.

![Version](https://img.shields.io/badge/version-1.0.0-blue)
![License](https://img.shields.io/badge/license-MIT-green)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey)

## ✨ Features

- **🔍 Semantic Search**: Find files using natural language queries instead of exact filename matching
- **🏠 Local Processing**: All embeddings generated locally using Ollama - your data never leaves your machine
- **📄 Multi-Format Support**: Indexes and searches through:
  - Text files (`.txt`, `.md`, `.js`, `.ts`, `.py`, `.rs`, etc.)
  - PDF documents (`.pdf`)
  - Word documents (`.docx`)
  - Excel spreadsheets (`.xlsx`)
- **⚡ Performance Modes**: 
  - **Lightweight Mode**: `all-minilm` model (384 dims, ~86MB) - Perfect for systems with 4GB+ RAM
  - **Normal Mode**: `embeddinggemma` model (1024 dims, ~200-500MB) - Better accuracy for systems with 8GB+ RAM
- **🔄 Auto-Indexing**: Automatically watches and indexes new/modified files in real-time
- **🎨 Modern UI**: Clean, intuitive interface with dark sidebar and light content area
- **📊 System Monitoring**: Real-time system status and resource usage display

## 🚀 Quick Start

### Prerequisites

1. **Ollama** - Install from [https://ollama.ai](https://ollama.ai)
2. **Node.js** - Version 18 or higher ([Download](https://nodejs.org/))
3. **Rust** - Latest stable version ([Install](https://www.rust-lang.org/tools/install))
4. **Ollama Models** - Pull the required embedding models:
   ```bash
   ollama pull all-minilm      # For lightweight mode
   ollama pull embeddinggemma # For normal mode (recommended)
   ```

### Installation

#### 1. Clone the Repository

```bash
git clone https://github.com/Saurav-kan/Gist.git
cd NLPFileExplorer
```

#### 2. Build the Backend

```bash
cd rust-backend
cargo build --release
```

The compiled binary will be in `rust-backend/target/release/`

#### 3. Install Frontend Dependencies

```bash
cd ../electron
npm install
```

## 🏃 Running the Application

### Development Mode

1. **Start Ollama** (if not already running):
   ```bash
   ollama serve
   ```

2. **Start the Rust Backend**:
   ```bash
   cd rust-backend
   cargo run --release
   ```
   The backend API will be available at `http://localhost:8080`

3. **Start the Electron Frontend** (in a new terminal):
   ```bash
   cd electron
   npm start
   ```
   Or with DevTools:
   ```bash
   npm run dev
   ```

### Production Build

Build a distributable application:

```bash
cd electron
npm run build
```

The built application will be in the `dist/` directory.

## 📖 Usage Guide

### Initial Setup

1. **Configure Performance Mode**:
   - Open the app and navigate to **Settings** (gear icon in sidebar)
   - Select your preferred mode:
     - **Lightweight**: Faster indexing, lower RAM usage, good for general search
     - **Normal**: Better semantic understanding, more accurate results

2. **Add Directories to Index**:
   - In Settings, click **Add Folder**
   - Select the directory you want to index
   - The app will automatically start indexing files

3. **Wait for Indexing**:
   - Large directories may take a few minutes to index
   - Check the system status in the sidebar footer for progress

### Searching Files

1. **Enter Your Query**:
   - Use natural language to describe what you're looking for
   - Examples:
     - "invoices from last month"
     - "meeting notes about project X"
     - "code related to authentication"
     - "documents mentioning budget"

2. **Adjust Similarity Threshold**:
   - Use the slider to control how closely results must match your query
   - Lower threshold = more results (may include less relevant files)
   - Higher threshold = fewer results (only highly relevant files)

3. **Open Files**:
   - Click on any search result to open it with your default application
   - The file path is shown at the bottom of each result card

### Managing Index

- **Clear Index**: Removes all indexed files and embeddings (files remain untouched)
- **Remove Directory**: Stops watching a directory and removes its files from the index
- **Re-index**: Remove and re-add a directory to refresh its index

## 🏗️ Project Structure

```
NLPFileExplorer/
├── electron/                    # Electron frontend application
│   ├── src/
│   │   ├── main.js             # Main Electron process
│   │   ├── preload.js           # Preload script (IPC bridge)
│   │   └── renderer/            # Renderer process (UI)
│   │       ├── index.html       # Main UI structure
│   │       ├── styles.css       # Application styles
│   │       └── app.js           # Frontend logic
│   └── package.json             # Node.js dependencies
│
├── rust-backend/                # Rust backend service
│   ├── src/
│   │   ├── main.rs              # HTTP server entry point
│   │   ├── config.rs            # Configuration management
│   │   ├── embedding.rs         # Ollama embedding client
│   │   ├── storage.rs           # Vector storage (SQLite + binary)
│   │   ├── indexer.rs          # File indexing logic
│   │   ├── search.rs            # Vector similarity search
│   │   ├── parsers.rs           # Document parsers (PDF, DOCX, XLSX)
│   │   ├── file_watcher.rs     # File system watcher
│   │   └── api/                 # HTTP API routes
│   │       ├── search.rs        # Search endpoint
│   │       ├── index.rs         # Indexing endpoints
│   │       ├── settings.rs      # Settings management
│   │       ├── files.rs         # File listing
│   │       └── system_info.rs   # System information
│   └── Cargo.toml               # Rust dependencies
│
└── README.md                    # This file
```

## ⚙️ Configuration

Configuration is stored in `~/.nlpfileexplorer/config.json` (or `%APPDATA%/nlpfileexplorer/config.json` on Windows).

Example configuration:

```json
{
  "performance_mode": "normal",
  "embedding_model": "embeddinggemma",
  "indexed_directories": [
    "C:/Users/YourName/Documents",
    "C:/Users/YourName/Projects"
  ],
  "file_type_filters": {
    "include_pdf": true,
    "include_docx": true,
    "include_text": true,
    "include_xlsx": true
  },
  "chunk_size": 1000,
  "auto_index": true
}
```

You can modify settings through the UI or edit the config file directly.

## 💻 System Requirements

### Lightweight Mode
- **RAM**: 4GB minimum (8GB recommended)
- **CPU**: Dual-core processor (2+ GHz)
- **Storage**: 1-2GB free space for embeddings
- **OS**: Windows 10+, macOS 10.15+, or Linux (Ubuntu 20.04+)

### Normal Mode (Recommended)
- **RAM**: 8GB minimum (16GB recommended)
- **CPU**: Modern multi-core processor (4+ cores recommended)
- **Storage**: 2-5GB free space for embeddings
- **GPU**: Optional but recommended (40% faster inference with CUDA/ROCm)
- **OS**: Windows 10+, macOS 10.15+, or Linux (Ubuntu 20.04+)

## 🔧 Troubleshooting

### Backend Won't Start

- **Check Ollama**: Ensure Ollama is running (`ollama serve`)
- **Check Port**: Ensure port 8080 is not in use
- **Check Models**: Verify models are downloaded (`ollama list`)

### Search Returns No Results

- **Check Index**: Ensure directories are indexed (check Settings)
- **Lower Threshold**: Try reducing the similarity threshold
- **Re-index**: Clear and re-add directories if needed

### Slow Indexing

- **Switch Mode**: Try lightweight mode if normal mode is too slow
- **Reduce Scope**: Index smaller directories first
- **Check Resources**: Ensure sufficient RAM and CPU available

### Files Not Being Indexed

- **Check File Types**: Verify file types are enabled in settings
- **Check Permissions**: Ensure the app has read access to directories
- **Check Logs**: Check console output for error messages

## 🛠️ Development

### Backend Development

```bash
cd rust-backend
cargo run                    # Debug build
cargo run --release          # Release build
cargo test                   # Run tests
cargo check                  # Check without building
```

### Frontend Development

```bash
cd electron
npm start                    # Run app
npm run dev                  # Run with DevTools
npm run build                # Build distributable
```

### API Endpoints

The backend exposes a REST API at `http://localhost:8080`:

- `GET /api/health` - Health check
- `GET /api/settings` - Get current settings
- `PUT /api/settings` - Update settings
- `POST /api/search` - Perform semantic search
- `POST /api/index/start` - Start indexing a directory
- `POST /api/index/clear` - Clear all indexes
- `GET /api/files` - List indexed files
- `GET /api/system-info` - Get system information

## 📝 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [Ollama](https://ollama.ai) for local LLM inference
- [Electron](https://www.electronjs.org/) for cross-platform desktop apps
- [Axum](https://github.com/tokio-rs/axum) for the async web framework
- All the open-source libraries that made this project possible

## 📧 Support

For issues, questions, or contributions, please open an issue on [GitHub](https://github.com/Saurav-kan/Gist/issues).

---

**Made with ❤️ using Rust and Electron**
