use std::time::{Instant, Duration};
use std::sync::Arc;
use nlp_file_explorer_backend::{
    config::AppConfig,
    storage::Storage,
    search::cosine_similarity,
};
use futures::future::join_all;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("==========================================");
    println!("       GIST PERFORMANCE BENCHMARK         ");
    println!("==========================================");
    
    let config = Arc::new(AppConfig::load_or_default().await?);
    let data_dir = AppConfig::data_dir();
    let storage = Arc::new(Storage::new(&data_dir).await?);
    
    println!("[1/3] Loading Data...");
    let start_load = Instant::now();
    let embeddings = storage.get_all_embeddings().await.unwrap_or_default();
    let load_duration = start_load.elapsed();
    
    if embeddings.is_empty() {
        println!("Error: No embeddings found in {:?}.", data_dir);
        println!("Please index some files using the application before running benchmarks.");
        return Ok(());
    }
    
    let count = embeddings.len();
    let dim = embeddings[0].1.len();
    println!("Successfully loaded {} embeddings ({} dimensions) in {:?}", count, dim, load_duration);
    
    // Create a dummy query vector
    let query_vector = vec![0.1f32; dim];
    
    println!("\n[2/3] Benchmarking Linear Search (Single-Threaded)...");
    let iterations = 50;
    let mut st_total = Duration::default();
    
    for _ in 0..iterations {
        let start = Instant::now();
        let mut results: Vec<_> = embeddings
            .iter()
            .map(|(meta, emb)| {
                let sim = cosine_similarity(&query_vector, emb);
                (meta, sim)
            })
            .collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        st_total += start.elapsed();
    }
    let st_avg = st_total / iterations;
    println!("   Average Latency: {:?}", st_avg);
    println!("   Throughput: {:.2} items/ms", count as f64 / st_avg.as_secs_f64() / 1000.0);

    println!("\n[3/3] Benchmarking Linear Search (Multi-Threaded / Parallel Chunks)...");
    let mut mt_total = Duration::default();
    let chunk_size = 100;
    
    for _ in 0..iterations {
        let start = Instant::now();
        let mut all_results = Vec::new();
        
        for chunk in embeddings.chunks(chunk_size) {
            let query = query_vector.clone();
            let chunk_data: Vec<_> = chunk.iter().map(|(m, e)| (m.clone(), e.clone())).collect();
            
            let chunk_tasks: Vec<_> = chunk_data.into_iter().map(|(meta, emb)| {
                let q = query.clone();
                tokio::spawn(async move {
                    let sim = cosine_similarity(&q, &emb);
                    (meta, sim)
                })
            }).collect();
            
            let chunk_results = join_all(chunk_tasks).await;
            for task_result in chunk_results {
                if let Ok(result) = task_result {
                    all_results.push(result);
                }
            }
        }
        all_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        mt_total += start.elapsed();
    }
    let mt_avg = mt_total / iterations;
    println!("   Average Latency (Chunked): {:?}", mt_avg);
    println!("   Improvement: {:.2}x", st_avg.as_secs_f64() / mt_avg.as_secs_f64());

    println!("\n==========================================");
    println!("             SCALING ESTIMATES            ");
    println!("==========================================");
    let ms_per_1k = st_avg.as_secs_f64() * 1000.0 * (1000.0 / count as f64);
    println!("Est. Latency for 10,000 items:  {:>7.2} ms", ms_per_1k * 10.0);
    println!("Est. Latency for 50,000 items:  {:>7.2} ms", ms_per_1k * 50.0);
    println!("Est. Latency for 100,000 items: {:>7.2} ms", ms_per_1k * 100.0);
    println!("\n[NOTE] Sub-10ms search on 100k items requires HNSW optimization.");
    println!("==========================================\n");

    Ok(())
}
