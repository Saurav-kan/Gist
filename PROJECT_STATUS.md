# GIST (NLP File Explorer) - Development Roadmap & Status

This document tracks the engineering progress, technical decisions, and future milestones for the GIST semantic search engine.

## 📍 Current State: Baseline Performance
**Status:** MVP (Minimum Viable Product) focus with high-performance Rust core and Electron UI.
- **Backend:** Axum-based REST API with parallelized indexing and search.
- **Storage:** Hybrid architecture using SQLite for metadata and a custom binary flat-file for vector storage.
- **Search Strategy:** Parallelized linear search with heuristic similarity refinement.

### 📊 Current Benchmarks (Baseline - Normal Mode)
*Measured on **1,094** vectors (**768** dimensions each)*
- **Loading Latency:** 371.52 ms
- **Single-Threaded Latency:** 43.71 ms (25.03 items/ms)
- **Multi-Threaded Latency:** **16.79 ms** (2.60x improvement)

#### Scaling Projections (Current Architecture)
| Dataset Size | Est. Latency (Linear Search) |
| :--- | :--- |
| 10,000 items | ~399.50 ms |
| 50,000 items | ~1,997.51 ms |
| 100,000 items | ~3,995.02 ms |


---

## 🛠️ Feature List

### 🟢 Implemented Features
- **Semantic Search:** Natural language querying vs. exact keyword matching.
- **Local LLM Integration:** Powered by Ollama (`all-minilm`, `embeddinggemma`).
- **Multi-Format Parsing:** Support for `.pdf`, `.docx`, `.xlsx`, and standard text-based formats.
- **Real-time Indexing:** File-system watcher automatically updates vectors on file changes.
- **System Monitoring:** Built-in resource tracking for CPU/RAM usage.
- **Heuristic Quality Control:** Custom weighting to reduce false positives for short filenames and small files.
- **Developer Suite:** Sub-1ms API health checks and modular `lib/bin` architecture for benchmarking.

---

## 🚀 Incoming Roadmap (Next Steps)

### 1. HNSW Index Integration (High Priority)
- **Goal:** Shift from $O(N)$ linear search to $O(\log N)$ graph-based search.
- **Impact:** Sub-10ms search latency on datasets exceeding 100,000 documents.
- **Tech:** Implementing `hnsw-rs` or custom neighbor graph logic.

### 2. Hybrid Search (Semantic + Keyword)
- **Goal:** Combine the "meaning" of semantic search with the "precision" of keyword search.
- **Implementation:** Integrate SQLite FTS5 (Full-Text Search) to ensure exact matches for serial numbers, IDs, and specific technical terms.

### 3. Active RAG (Retrieval-Augmented Generation)
- **Goal:** Move from "Search" to "Answer."
- **Implementation:** Add a "Summarize results" feature that feeds the top search hits into an LLM (Ollama) to provide a direct answer to the user's question.

### 4. Advanced Parsing & OCR
- **Goal:** Index scanned documents and images.
- **Implementation:** Optional Tesseract OCR integration for image-only PDFs.

---

## 📝 Design Philosophy
1. **Privacy First:** All data remains local; zero telemetry.
2. **Systems Performance:** Leverage Rust for CPU-intensive vector operations.
3. **UX Simplicity:** Hide the complexity of vector math behind a clean, intuitive Electron interface.
