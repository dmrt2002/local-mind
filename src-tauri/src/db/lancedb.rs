use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Simple file-based vector store using JSON format
/// This is a temporary implementation until LanceDB Rust API is stable
#[derive(Clone)]
pub struct VectorStore {
    path: PathBuf,
    vectors: Arc<Mutex<Vec<(i64, Vec<f32>)>>>,
}

static VECTOR_STORE: Lazy<Arc<Mutex<Option<VectorStore>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

/// Get LanceDB directory path
fn get_lancedb_path() -> Result<PathBuf> {
    // For now, always use local data directory (works in both dev and prod)
    let mut path = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("data")
        .join("local-mind");

    // Ensure directory exists
    std::fs::create_dir_all(&path)
        .context(format!("Failed to create data directory at {:?}", path))?;

    path.push("vectors.lance");
    Ok(path)
}

/// Initialize vector store
pub async fn init_lancedb() -> Result<()> {
    let path = get_lancedb_path()?;
    std::fs::create_dir_all(path.parent().unwrap())?;

    let store = VectorStore {
        path: path.clone(),
        vectors: Arc::new(Mutex::new(Vec::new())),
    };

    // Try to load existing vectors from disk
    if path.exists() {
        if let Err(e) = load_vectors_from_disk(&store).await {
            log::warn!("Failed to load existing vectors: {}", e);
        }
    }

    *VECTOR_STORE.lock().await = Some(store);
    Ok(())
}

/// Load vectors from disk (simple JSON format for now)
async fn load_vectors_from_disk(store: &VectorStore) -> Result<()> {
    use serde_json;
    use std::fs;

    let data = fs::read_to_string(&store.path)?;
    let vectors: Vec<(i64, Vec<f32>)> = serde_json::from_str(&data)?;

    let mut vecs = store.vectors.lock().await;
    *vecs = vectors;

    Ok(())
}

/// Save vectors to disk
async fn save_vectors_to_disk(store: &VectorStore) -> Result<()> {
    use serde_json;
    use std::fs;

    let vecs = store.vectors.lock().await;
    let data = serde_json::to_string(&*vecs)?;
    fs::write(&store.path, data)?;

    Ok(())
}

/// Get vector store instance
async fn get_store() -> Result<Arc<VectorStore>> {
    let store = VECTOR_STORE
        .lock()
        .await
        .clone()
        .context("Vector store not initialized")?;
    Ok(Arc::new(store))
}

/// Save embedding vector
pub async fn save_embedding(snippet_id: i64, embedding: Vec<f32>) -> Result<()> {
    let store = get_store().await?;

    let mut vecs = store.vectors.lock().await;

    // Remove existing entry if present
    vecs.retain(|(id, _)| *id != snippet_id);

    // Add new entry
    vecs.push((snippet_id, embedding));

    // Save to disk
    drop(vecs);
    save_vectors_to_disk(&store).await?;

    Ok(())
}

/// Delete embedding for a snippet
pub async fn delete_embedding(snippet_id: i64) -> Result<()> {
    let store = get_store().await?;

    let mut vecs = store.vectors.lock().await;
    let before_count = vecs.len();

    // Remove the entry
    vecs.retain(|(id, _)| *id != snippet_id);

    let after_count = vecs.len();
    let deleted = before_count - after_count;

    // Save to disk
    drop(vecs);
    save_vectors_to_disk(&store).await?;

    if deleted > 0 {
        log::info!("Deleted embedding for snippet {}", snippet_id);
        println!("🗑️  Deleted embedding for snippet {}", snippet_id);
    } else {
        log::debug!("No embedding found for snippet {}", snippet_id);
    }

    Ok(())
}

/// Check if a snippet has an embedding
pub async fn has_embedding(snippet_id: i64) -> Result<bool> {
    let store = get_store().await?;
    let vecs = store.vectors.lock().await;
    Ok(vecs.iter().any(|(id, _)| *id == snippet_id))
}

/// Get all snippet IDs that have embeddings
pub async fn get_embedded_snippet_ids() -> Result<Vec<i64>> {
    let store = get_store().await?;
    let vecs = store.vectors.lock().await;
    Ok(vecs.iter().map(|(id, _)| *id).collect())
}

/// Count total number of embeddings
pub async fn count_vectors() -> Result<i64> {
    let store = get_store().await?;
    let vecs = store.vectors.lock().await;
    Ok(vecs.len() as i64)
}

/// Clean up orphaned embeddings (embeddings for deleted snippets)
/// This should be called on startup to remove stale embeddings
pub async fn cleanup_orphaned_embeddings() -> Result<usize> {
    let store = get_store().await?;

    let mut vecs = store.vectors.lock().await;
    let before_count = vecs.len();

    // Get all valid snippet IDs from database using sqlite module
    let valid_ids: Vec<i64> = match crate::db::sqlite::get_all_snippet_ids().await {
        Ok(ids) => ids,
        Err(e) => {
            log::warn!("Failed to fetch snippet IDs for cleanup: {}", e);
            return Ok(0);
        }
    };

    let valid_ids_set: std::collections::HashSet<i64> = valid_ids.into_iter().collect();

    // Remove embeddings for snippets that don't exist
    vecs.retain(|(id, _)| valid_ids_set.contains(id));

    let after_count = vecs.len();
    let cleaned = before_count - after_count;

    if cleaned > 0 {
        // Save to disk
        drop(vecs);
        save_vectors_to_disk(&store).await?;
        log::info!("Cleaned up {} orphaned embeddings", cleaned);
        println!("🧹 Cleaned up {} orphaned embeddings", cleaned);
    }

    Ok(cleaned)
}

/// Calculate cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot_product / (norm_a * norm_b)
}

/// Search for similar embeddings (semantic search)
/// Returns Vec<(snippet_id, similarity_score)> sorted by similarity (descending)
pub async fn search_semantic(query_embedding: Vec<f32>, limit: usize) -> Result<Vec<(i64, f32)>> {
    let store = get_store().await?;
    let vecs = store.vectors.lock().await;

    if vecs.is_empty() {
        log::debug!("No embeddings stored - semantic search returning empty");
        println!("⚠️  No embeddings stored - semantic search returning empty");
        return Ok(vec![]);
    }

    println!("🔍 Searching {} embeddings for matches...", vecs.len());

    // Calculate similarity for all vectors
    let mut results: Vec<(i64, f32)> = vecs
        .iter()
        .map(|(snippet_id, embedding)| {
            let similarity = cosine_similarity(&query_embedding, embedding);
            (*snippet_id, similarity)
        })
        .collect();

    // Sort by similarity (descending)
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Show top 5 similarity scores for debugging
    println!("Top 5 similarity scores:");
    for (i, (id, score)) in results.iter().take(5).enumerate() {
        println!("  {}. Snippet {}: {:.4}", i + 1, id, score);
    }

    // Filter out very low similarity scores (threshold: 0.3)
    // This ensures we only return semantically relevant results
    let threshold = 0.3;
    let filtered: Vec<(i64, f32)> = results
        .into_iter()
        .filter(|(_, similarity)| *similarity >= threshold)
        .collect();

    log::debug!(
        "Semantic search: {} embeddings checked, {} above threshold {}",
        vecs.len(),
        filtered.len(),
        threshold
    );
    println!(
        "✅ Found {} results above threshold {:.2}",
        filtered.len(),
        threshold
    );

    // Return top N snippet IDs WITH similarity scores
    Ok(filtered.into_iter().take(limit).collect())
}

/// Get embedding vector for a specific snippet
pub async fn get_embedding(snippet_id: i64) -> Result<Option<Vec<f32>>> {
    let store = get_store().await?;
    let vecs = store.vectors.lock().await;

    for (id, embedding) in vecs.iter() {
        if *id == snippet_id {
            return Ok(Some(embedding.clone()));
        }
    }

    Ok(None)
}

/// Get embeddings for multiple snippet IDs
pub async fn get_embeddings(snippet_ids: &[i64]) -> Result<Vec<(i64, Vec<f32>)>> {
    let store = get_store().await?;
    let vecs = store.vectors.lock().await;

    let id_set: std::collections::HashSet<i64> = snippet_ids.iter().copied().collect();
    let result: Vec<(i64, Vec<f32>)> = vecs
        .iter()
        .filter(|(id, _)| id_set.contains(id))
        .map(|(id, embedding)| (*id, embedding.clone()))
        .collect();

    Ok(result)
}
