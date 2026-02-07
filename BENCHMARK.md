# NLP File Explorer Benchmark

The benchmark tool measures indexing performance and HNSW index build time for the NLP File Explorer. Use it to profile indexing speed, validate the vector index, and optionally benchmark search latency.

## Current Benchmark Results

*Last run: Desktop directory, mixed content (PDF, DOCX, Excel, images, configs)*

| Metric | Value |
|--------|-------|
| **Total files indexed** | 2,735 |
| **Total time** | 1,332.92 s (22.22 min) |
| **Average time per file** | 0.487 s |
| **Files per second** | 2.05 |
| **HNSW items** | 1,125 |
| **HNSW dimensions** | 768 |
| **HNSW build time** | <0.01 s |
| **HNSW ready** | Yes |
| **HNSW valid** | Yes |

*First 1,000 files: 789.48 s (13.16 min), 1.27 files/s*

## Prerequisites

- **Ollama** must be running with your embedding model pulled:
  ```bash
  ollama serve
  ollama pull embeddinggemma   # or all-minilm for lightweight mode
  ```
- **Rust** toolchain installed
- The benchmark uses your app config (`~/.nlpfileexplorer/config.json` or `%APPDATA%\nlpfileexplorer\config.json` on Windows)

## Running the Benchmark

From the project root:

```bash
cd rust-backend
cargo run --release --bin benchmark -- --directory <path-to-index>
```

Use `--release` for realistic performance; debug builds are much slower.

### Arguments

| Argument | Short | Required | Default | Description |
|----------|-------|----------|---------|-------------|
| `--directory` | `-d` | Yes* | — | Directory to index (*required unless `--search-only`) |
| `--search-only` | — | No | false | Skip indexing; use existing embeddings from database |
| `--count` | `-c` | No | all | Limit number of files (if supported) |
| `--format` | `-f` | No | `human` | Output format: `human`, `json`, or `csv` |
| `--search_queries` | `-s` | No | — | Path to text file with one search query per line |
| `--show-top` | — | No | 0 | Show top N results per query for accuracy verification (0 = off) |

### Examples

```bash
# Basic run: index a directory and measure performance
cargo run --release --bin benchmark -- -d C:\Users\kande\Desktop

# Search-only: benchmark search latency using existing index (no re-indexing)
# Requires a previously indexed database; -s recommended for meaningful results
cargo run --release --bin benchmark -- --search-only -s queries.txt

# Show top 3 results per query to verify search accuracy
cargo run --release --bin benchmark -- --search-only -s queries.txt --show-top 3

# JSON output (for scripting or CI)
cargo run --release --bin benchmark -- -d ./docs -f json

# Include search latency benchmarks
# Create queries.txt with one query per line, e.g.:
#   summary of Carpaal NDA
#   geology homework
#   meeting notes
cargo run --release --bin benchmark -- -d ./docs -s queries.txt
```

## Understanding the Output

### Indexing Results

| Metric | Description |
|--------|-------------|
| **Total files indexed** | Number of files processed (content-indexed + metadata-only) |
| **Total time** | Wall-clock time for indexing |
| **Average time per file** | Total time ÷ files indexed |
| **Files per second** | Throughput |

Indexing speed is dominated by parsing (PDF, DOCX, Excel) and embedding generation (Ollama API calls). Expect roughly 1–3 files/second for mixed content on typical hardware.

### HNSW Index Build

| Metric | Description |
|--------|-------------|
| **Build time** | Time to construct the HNSW index from embeddings |
| **Items** | Number of vectors in the index |
| **Dimensions** | Embedding dimension (e.g., 768 for `embeddinggemma`) |
| **Ready / Valid** | Index health flags |

### Why Total Files ≠ HNSW Items?

**Total files indexed** includes both:

1. **Content-indexed files** (PDF, DOCX, XLSX, TXT, etc.) — each produces one or more embeddings → included in HNSW
2. **Metadata-only files** (images, configs, binaries, logs) — stored in the DB by filename only, **no embedding** → not in HNSW

Example: 2,735 files indexed, 1,125 HNSW items → ~1,610 files were metadata-only (images, `.json`, `.yaml`, `.exe`, `.log`, etc.). Metadata-only files appear in the metadata store for filename search but do not participate in vector/semantic search.

## Example Output

```
=== NLP File Explorer Benchmark ===
Directory: C:\Users\kande\Desktop
Format: human

Starting indexing benchmark...

=== Indexing Results ===
Total files indexed: 2735
Total time: 1332.92 seconds (22.22 minutes)
Average time per file: 0.487 seconds
Files per second: 2.05

=== HNSW Index Build ===
[HNSW] Rebuilding index with 1125 embeddings
[HNSW] Index rebuilt successfully with 1125 items
HNSW index built successfully
Build time: 0.00 seconds
Items: 1125
Dimensions: 768
Ready: true
Valid: true
```

## Tips

- **Use `--release`** — Debug builds can be 10x+ slower
- **Warm Ollama** — First embedding calls may be slower; subsequent runs reflect steady-state performance
- **Mixed content** — Realistic benchmarks use directories with PDFs, DOCX, images, and configs
- **Search benchmark** — Provide `-s queries.txt` to measure query embedding + HNSW search latency. Results use the **same scoring pipeline as the app** (hybrid vector+filename, penalties for short filenames, etc.), so `--show-top` reflects what users actually see.
