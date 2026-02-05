# GIST Performance Benchmarks (Baseline)

*Date: 2026-02-05*
*Architecture: Rust (Axum) + SQLite + Custom Binary Vector Storage*
*Hardware: Windows Desktop (Multi-core CPU)*

## Test Environment
- **Dataset Size:** 1,094 indexed documents
- **Vector Dimensions:** 768 (Ollama Embedding Model)
- **Iteration Count:** 50 searches per test

## Search Latency Metrics
We compared Single-Threaded Linear Search vs. Multi-Threaded Chunked Search (Tokio).

| Test Mode | Avg Latency | Throughput | improvement |
|:---|:---|:---|:---|
| **Single-Threaded** | 43.71 ms | 25.03 items/ms | 1.0x (Base) |
| **Multi-Threaded** | 16.79 ms | 65.17 items/ms | **2.60x** |

## Scaling Projections (Linear Search)
Based on current linear search performance, we project the following latencies as the file index grows:

| Index Size | Projected Latency (Linear) | User Experience |
|:---|:---|:---|
| 1,000 items | ~44 ms | Instant ✨ |
| 10,000 items | ~400 ms | Noticeable Lag ⏱️ |
| 50,000 items | ~2.0 seconds | Frustrating 🐌 |
| 100,000 items | ~4.0 seconds | Unusable 🚫 |

## Conclusions
- **Current State:** The system is highly efficient for personal use-cases (1k-5k files).
- **The Bottleneck:** Similarity calculation and sorting scales linearly ($O(N)$). At 100k items, the CPU overhead becomes the primary bottleneck.
- **Next Steps:** Implementing **HNSW (Hierarchical Navigable Small World)** indexing will reduce search complexity to $O(\log N)$, enabling sub-10ms searches even at 100k+ items.

---
*Note: Run these benchmarks anytime using `cargo run --bin benchmark` in the `rust-backend` directory.*
