use std::sync::Arc;
use std::time::Instant;
use clap::Parser;
use anyhow::Result;
use std::fs;
use serde::Serialize;

use nlp_file_explorer_backend::{
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
    /// Directory to index
    #[arg(short, long)]
    directory: String,

    /// Number of files to benchmark (default: all files)
    #[arg(short, long)]
    count: Option<usize>,

    /// Output format: json or csv (default: human-readable)
    #[arg(short, long, default_value = "human")]
    format: String,

    /// Optional file with search queries to test
    #[arg(short, long)]
    search_queries: Option<String>,
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

    println!("=== NLP File Explorer Benchmark ===");
    println!("Directory: {}", args.directory);
    println!("Format: {}", args.format);
    if let Some(count) = args.count {
        println!("File limit: {}", count);
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

    // Benchmark indexing
    let indexing_start = Instant::now();
    println!("Starting indexing benchmark...");
    let indexed_count = indexer.index_directory(&args.directory).await?;
    let indexing_duration = indexing_start.elapsed();
    let total_time_secs = indexing_duration.as_secs_f64();

    // Get all embeddings
    let embeddings = storage.get_all_embeddings().await?;

    println!("\n=== Indexing Results ===");
    println!("Total files indexed: {}", indexed_count);
    if indexed_count > 0 {
        println!("Total time: {:.2} seconds ({:.2} minutes)", total_time_secs, total_time_secs / 60.0);
        println!("Average time per file: {:.3} seconds", total_time_secs / indexed_count as f64);
        println!("Files per second: {:.2}", indexed_count as f64 / total_time_secs);
    } else {
        println!("No files indexed");
    }

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

            // Benchmark search if queries provided
            let mut search_results = Vec::new();
            if let Some(queries_file) = &args.search_queries {
                println!("\n=== Search Benchmark ===");
                if let Ok(queries_content) = fs::read_to_string(queries_file) {
                    let queries: Vec<String> = queries_content
                        .lines()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    
                    for query in queries {
                        let search_start = Instant::now();
                        // Generate embedding for query
                        let query_embedding = embedding_service.generate_embedding(&query).await?;
                        let search_result = hnsw_index.search(query_embedding, 10)?;
                        let search_duration = search_start.elapsed();
                        
                        search_results.push(SearchBenchmark {
                            query: query.clone(),
                            results_count: search_result.len(),
                            search_time_ms: search_duration.as_secs_f64() * 1000.0,
                        });
                        
                        println!("Query: '{}' -> {} results in {:.2}ms", 
                                query, search_result.len(), search_duration.as_secs_f64() * 1000.0);
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
