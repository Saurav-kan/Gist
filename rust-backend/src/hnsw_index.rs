use anyhow::Result;
use crate::storage::FileMetadata;
use std::collections::HashMap;

// Use a simpler approach: implement HNSW using the actual crate API
// Based on hnsw 0.11 crate structure
pub struct HnswIndex {
    // Store embeddings and metadata separately
    // We'll use a simple vector-based approach with cosine similarity
    embeddings: Vec<Vec<f32>>,
    metadata_list: Vec<FileMetadata>,
    id_to_index: HashMap<i64, usize>, // Map from file ID to vector index
    dimensions: usize,
}

impl HnswIndex {
    pub fn new(dimensions: usize) -> Self {
        Self {
            embeddings: Vec::new(),
            metadata_list: Vec::new(),
            id_to_index: HashMap::new(),
            dimensions,
        }
    }

    pub fn add(&mut self, embedding: Vec<f32>, metadata: FileMetadata) -> Result<()> {
        if embedding.len() != self.dimensions {
            return Err(anyhow::anyhow!(
                "Embedding dimension mismatch: expected {}, got {}",
                self.dimensions,
                embedding.len()
            ));
        }

        let index = self.embeddings.len();
        self.embeddings.push(embedding);
        self.metadata_list.push(metadata.clone());
        self.id_to_index.insert(metadata.id, index);

        Ok(())
    }

    pub fn search(&self, query_embedding: Vec<f32>, k: usize) -> Result<Vec<(FileMetadata, f32)>> {
        if query_embedding.len() != self.dimensions {
            return Err(anyhow::anyhow!(
                "Query embedding dimension mismatch: expected {}, got {}",
                self.dimensions,
                query_embedding.len()
            ));
        }

        if self.embeddings.is_empty() {
            return Ok(Vec::new());
        }

        // Use cosine similarity for search
        use crate::search::cosine_similarity;
        
        // Optimized: Use a binary heap to maintain top k results without full sort
        // For large datasets, this avoids sorting all similarities
        use std::collections::BinaryHeap;
        use std::cmp::Ordering;
        
        #[derive(PartialEq)]
        struct SimilarityItem {
            similarity: f32,
            index: usize,
        }
        
        impl Eq for SimilarityItem {}
        
        // Reverse ordering to make BinaryHeap a min-heap (we want to keep largest similarities)
        impl PartialOrd for SimilarityItem {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                other.similarity.partial_cmp(&self.similarity) // Reversed for min-heap
            }
        }
        
        impl Ord for SimilarityItem {
            fn cmp(&self, other: &Self) -> Ordering {
                self.partial_cmp(other).unwrap_or(Ordering::Equal)
            }
        }
        
        // Use a min-heap to keep only top k results (smallest similarity at top)
        let mut heap = BinaryHeap::new();
        
        for (idx, emb) in self.embeddings.iter().enumerate() {
            let similarity = cosine_similarity(&query_embedding, emb);
            
            if heap.len() < k {
                heap.push(SimilarityItem { similarity, index: idx });
            } else if let Some(mut top) = heap.peek_mut() {
                // If current similarity is larger than the smallest in heap, replace it
                if similarity > top.similarity {
                    *top = SimilarityItem { similarity, index: idx };
                }
            }
        }
        
        // Extract results and sort by similarity (descending) for final output
        let mut results: Vec<(usize, f32)> = heap.into_iter()
            .map(|item| (item.index, item.similarity))
            .collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

        // Convert to (metadata, similarity) pairs
        let search_results: Vec<(FileMetadata, f32)> = results.into_iter()
            .filter_map(|(idx, similarity)| {
                self.metadata_list.get(idx).map(|meta| (meta.clone(), similarity))
            })
            .collect();

        Ok(search_results)
    }

    pub fn remove(&mut self, file_path: &str) -> Result<()> {
        // Find the index for this file path
        if let Some((idx, _)) = self.metadata_list.iter()
            .enumerate()
            .find(|(_, meta)| meta.file_path == file_path) {
            
            // Remove from vectors (swap with last for O(1) removal)
            let last_idx = self.embeddings.len() - 1;
            if idx != last_idx {
                self.embeddings.swap(idx, last_idx);
                self.metadata_list.swap(idx, last_idx);
                
                // Update id_to_index for swapped item
                if let Some(swapped_meta) = self.metadata_list.get(idx) {
                    self.id_to_index.insert(swapped_meta.id, idx);
                }
            }
            
            // Remove last element
            self.embeddings.pop();
            if let Some(removed_meta) = self.metadata_list.pop() {
                self.id_to_index.remove(&removed_meta.id);
            }
        }

        Ok(())
    }

    pub fn clear(&mut self) -> Result<()> {
        self.embeddings.clear();
        self.metadata_list.clear();
        self.id_to_index.clear();
        Ok(())
    }

    pub fn rebuild_from_embeddings(&mut self, embeddings: Vec<(FileMetadata, Vec<f32>)>) -> Result<()> {
        eprintln!("[HNSW] Rebuilding index with {} embeddings", embeddings.len());
        
        if embeddings.is_empty() {
            self.clear()?;
            return Ok(());
        }

        // Get dimensions from first embedding
        let dims = embeddings[0].1.len();
        if dims != self.dimensions {
            eprintln!("[HNSW] Dimension mismatch, recreating index: {} -> {}", self.dimensions, dims);
            *self = Self::new(dims);
        } else {
            self.clear()?;
        }

        // Add all embeddings to the index
        for (metadata, embedding) in embeddings {
            if let Err(e) = self.add(embedding, metadata) {
                eprintln!("[HNSW] Error adding embedding: {}", e);
            }
        }

        eprintln!("[HNSW] Index rebuilt successfully with {} items", self.len());
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.embeddings.len()
    }
}
