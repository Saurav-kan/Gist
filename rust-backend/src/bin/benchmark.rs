use std::sync::Arc;
use std::time::Instant;
use clap::Parser;
use anyhow::Result;
use std::fs;
use serde::Serialize;

use nlp_file_explorer_backend::{
    api::search::score_search_results,
    config::AppConfig,
    storage::Storage,
    indexer::Indexer,
    embedding::EmbeddingService,
    parsers::ParserRegistry,
    hnsw_index::HnswIndex,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Directory to index (required unless --search-only)
    #[arg(short, long)]
    directory: Option<String>,

    /// Skip indexing; use existing embeddings from database for search benchmark
    #[arg(long)]
    search_only: bool,

    /// Number of files to benchmark (default: all files)
    #[arg(short, long)]
    count: Option<usize>,

    /// Output format: json or csv (default: human-readable)
    #[arg(short, long, default_value = "human")]
    format: String,

    /// Optional file with search queries to test
    #[arg(short, long)]
    search_queries: Option<String>,

    /// Show top N search results per query for accuracy verification (0 = off)
    #[arg(long, default_value = "0")]
    show_top: usize,
}

#[derive(Serialize)]
struct BenchmarkResults {
    total_files: usize,
    indexed_files: usize,
    total_time_secs: f64,
    avg_time_per_file_ms: f64,
    embedding_time_secs: f64,
    avg_embedding_time_ms: f64,
    hnsw_build_time_secs: f64,
    search_results: Vec<SearchBenchmark>,
}

#[derive(Serialize)]
struct SearchBenchmark {
    query: String,
    results_count: usize,
    search_time_ms: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if !args.search_only && args.directory.is_none() {
        anyhow::bail!("--directory (-d) is required unless using --search-only");
    }

    println!("=== NLP File Explorer Benchmark ===");
    println!("Mode: {}", if args.search_only { "search-only (using existing index)" } else { "full (index + search)" });
    if let Some(ref dir) = args.directory {
        println!("Directory: {}", dir);
    }
    println!("Format: {}", args.format);
    if let Some(count) = args.count {
        println!("File limit: {}", count);
    }
    if args.show_top > 0 {
        println!("Show top results: {}", args.show_top);
    }
    println!();

    // Initialize components
    let config = AppConfig::load_or_default().await?;
    let storage = Arc::new(Storage::new(&AppConfig::data_dir()).await?);
    let embedding_service = Arc::new(EmbeddingService::new(config.embedding_model.clone()));
    let parser_registry = Arc::new(ParserRegistry::new(&config.file_type_filters));
    let indexer = Arc::new(Indexer::new(
        storage.clone(),
        embedding_service.clone(),
        parser_registry,
        Arc::new(config.clone()),
    ));

    let (indexed_count, total_time_secs, embeddings) = if args.search_only {
        println!("Skipping indexing (--search-only), loading embeddings from database...");
        let emb = storage.get_all_embeddings().await?;
        if emb.is_empty() {
            anyhow::bail!("No embeddings in database. Run a full benchmark with -d <directory> first to index files.");
        }
        println!("Loaded {} embeddings from existing index", emb.len());
        (emb.len(), 0.0, emb)
    } else {
        let dir = args.directory.as_ref().unwrap();
        let indexing_start = Instant::now();
        println!("Starting indexing benchmark...");
        let count = indexer.index_directory(dir).await?;
        let total = indexing_start.elapsed().as_secs_f64();

        println!("\n=== Indexing Results ===");
        println!("Total files indexed: {}", count);
        if count > 0 {
            println!("Total time: {:.2} seconds ({:.2} minutes)", total, total / 60.0);
            println!("Average time per file: {:.3} seconds", total / count as f64);
            println!("Files per second: {:.2}", count as f64 / total);
        } else {
            println!("No files indexed");
        }
        let emb = storage.get_all_embeddings().await?;
        (count, total, emb)
    };

    // Benchmark HNSW build
    println!("\n=== HNSW Index Build ===");
    let hnsw_build_start = Instant::now();
    if !embeddings.is_empty() {
        let dimensions = embeddings[0].1.len();
        let mut hnsw_index = HnswIndex::new(dimensions);
        if let Err(e) = hnsw_index.rebuild_from_embeddings(embeddings.clone()) {
            eprintln!("Error building HNSW index: {}", e);
        } else {
            let hnsw_build_duration = hnsw_build_start.elapsed();
            let hnsw_build_time_secs = hnsw_build_duration.as_secs_f64();
            
            let stats = hnsw_index.get_stats();
            let verification = hnsw_index.verify_index();
            
            println!("HNSW index built successfully");
            println!("Build time: {:.2} seconds", hnsw_build_time_secs);
            println!("Items: {}", stats.item_count);
            println!("Dimensions: {}", stats.dimensions);
            println!("Ready: {}", stats.is_ready);
            println!("Valid: {}", verification.is_valid);
            if !verification.errors.is_empty() {
                println!("Errors: {}", verification.errors.len());
            }
            if !verification.warnings.is_empty() {
                println!("Warnings: {}", verification.warnings.len());
            }

            // Benchmark search if queries provided (uses same scoring as main app)
            let mut search_results = Vec::new();
            if let Some(queries_file) = &args.search_queries {
                println!("\n=== Search Benchmark (app scoring: hybrid + penalties) ===");
                if let Ok(queries_content) = fs::read_to_string(queries_file) {
                    let queries: Vec<String> = queries_content
                        .lines()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    
                    let top_k = 10;
                    let candidate_count = top_k * 2; // Match main app: fetch 2x for re-ranking
                    
                    for query in queries {
                        let search_start = Instant::now();
                        // Generate embedding for query
                        let query_embedding = embedding_service.generate_embedding(&query).await?;
                        // Fetch more candidates, then apply same scoring pipeline as main search
                        let raw_results = hnsw_index.search(query_embedding, candidate_count)?;
                        let scored = score_search_results(&query, raw_results);
                        let final_results: Vec<_> = scored.into_iter().take(top_k).collect();
                        let search_duration = search_start.elapsed();
                        
                        search_results.push(SearchBenchmark {
                            query: query.clone(),
                            results_count: final_results.len(),
                            search_time_ms: search_duration.as_secs_f64() * 1000.0,
                        });
                        
                        println!("Query: '{}' -> {} results in {:.2}ms", 
                                query, final_results.len(), search_duration.as_secs_f64() * 1000.0);

                        if args.show_top > 0 && !final_results.is_empty() {
                            let n = args.show_top.min(final_results.len());
                            for (i, (meta, score)) in final_results.iter().take(n).enumerate() {
                                println!("    {}. {} ({:.1}%)", i + 1, meta.file_name, score * 100.0);
                            }
                        }
                    }
                } else {
                    eprintln!("Warning: Could not read queries file: {}", queries_file);
                }
            }

            // Compile results
            let results = BenchmarkResults {
                total_files: indexed_count,
                indexed_files: indexed_count,
                total_time_secs,
                avg_time_per_file_ms: if indexed_count > 0 { 
                    (total_time_secs / indexed_count as f64) * 1000.0 
                } else { 
                    0.0 
                },
                embedding_time_secs: 0.0, // Not separately measured
                avg_embedding_time_ms: 0.0, // Not separately measured
                hnsw_build_time_secs,
                search_results,
            };

            // Output results
            match args.format.as_str() {
                "json" => {
                    println!("\n=== JSON Output ===");
                    println!("{}", serde_json::to_string_pretty(&results)?);
                }
                "csv" => {
                    println!("\n=== CSV Output ===");
                    println!("metric,value");
                    println!("total_files,{}", results.total_files);
                    println!("total_time_secs,{}", results.total_time_secs);
                    println!("avg_time_per_file_ms,{}", results.avg_time_per_file_ms);
                    println!("embedding_time_secs,{}", results.embedding_time_secs);
                    println!("avg_embedding_time_ms,{}", results.avg_embedding_time_ms);
                    println!("hnsw_build_time_secs,{}", results.hnsw_build_time_secs);
                }
                _ => {
                    // Already printed above
                }
            }
        }
    } else {
        println!("No embeddings found, skipping HNSW build");
    }

    Ok(())
}
