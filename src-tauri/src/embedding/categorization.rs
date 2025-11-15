use anyhow::Result;
use log;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::db::{lancedb, sqlite};

/// Maximum number of category centroids to cache (limits memory to ~150 KB)
/// Each centroid is ~1.5 KB (384 floats * 4 bytes)
const MAX_CENTROID_CACHE_SIZE: usize = 100;

/// Category centroid (average embedding vector for all snippets in a category)
#[derive(Debug, Clone)]
pub struct CategoryCentroid {
    pub category_id: i64,
    pub centroid: Vec<f32>,
    pub snippet_count: usize,
}

/// Cache for category centroids (in-memory, limited to MAX_CENTROID_CACHE_SIZE)
static CENTROID_CACHE: Lazy<Arc<Mutex<HashMap<i64, CategoryCentroid>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

/// Calculate centroid for a category (average of all snippet embeddings)
pub async fn calculate_category_centroid(category_id: i64) -> Result<Option<CategoryCentroid>> {
    // Get all snippets in this category (all content types)
    let snippets = sqlite::get_snippets_by_category(category_id, 10000, 0, None).await?;

    if snippets.is_empty() {
        return Ok(None);
    }

    // Get snippet IDs
    let snippet_ids: Vec<i64> = snippets.iter().map(|s| s.id).collect();

    // Get embeddings for these snippets
    let embeddings = lancedb::get_embeddings(&snippet_ids).await?;

    if embeddings.is_empty() {
        return Ok(None);
    }

    // Calculate centroid (average of all embeddings)
    let embedding_dim = embeddings[0].1.len();
    let mut centroid = vec![0.0; embedding_dim];

    for (_, embedding) in &embeddings {
        for (i, val) in embedding.iter().enumerate() {
            centroid[i] += val;
        }
    }

    // Normalize by count
    let count = embeddings.len() as f32;
    for val in &mut centroid {
        *val /= count;
    }

    // Normalize centroid to unit vector (for cosine similarity)
    let norm: f32 = centroid.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for val in &mut centroid {
            *val /= norm;
        }
    }

    log::debug!(
        "Calculated centroid for category {} from {} snippets",
        category_id,
        embeddings.len()
    );

    Ok(Some(CategoryCentroid {
        category_id,
        centroid,
        snippet_count: embeddings.len(),
    }))
}

/// Get category centroid from cache or calculate it
pub async fn get_category_centroid(category_id: i64) -> Result<Option<CategoryCentroid>> {
    let cache = CENTROID_CACHE.lock().await;

    if let Some(centroid) = cache.get(&category_id) {
        return Ok(Some(centroid.clone()));
    }

    drop(cache);

    // Not in cache, calculate it
    if let Some(centroid) = calculate_category_centroid(category_id).await? {
        // Store in cache with size limit
        let mut cache = CENTROID_CACHE.lock().await;

        // If cache is at capacity, remove oldest entries (simple eviction)
        if cache.len() >= MAX_CENTROID_CACHE_SIZE {
            // Clear half the cache to make room (simple FIFO-ish eviction)
            let keys_to_remove: Vec<i64> = cache.keys().take(MAX_CENTROID_CACHE_SIZE / 2).copied().collect();
            for key in keys_to_remove {
                cache.remove(&key);
            }
            log::debug!("Centroid cache at capacity ({}), evicted {} entries", MAX_CENTROID_CACHE_SIZE, MAX_CENTROID_CACHE_SIZE / 2);
        }

        cache.insert(category_id, centroid.clone());
        log::debug!("Cached centroid for category {} (cache size: {}/{})", category_id, cache.len(), MAX_CENTROID_CACHE_SIZE);
        Ok(Some(centroid))
    } else {
        Ok(None)
    }
}

/// Invalidate centroid cache for a category (call after category changes)
pub async fn invalidate_centroid_cache(category_id: i64) {
    let mut cache = CENTROID_CACHE.lock().await;
    cache.remove(&category_id);
    log::debug!("Invalidated centroid cache for category {}", category_id);
}

/// Clear all centroid cache (call on startup or when needed)
pub async fn clear_centroid_cache() {
    let mut cache = CENTROID_CACHE.lock().await;
    cache.clear();
    log::debug!("Cleared all centroid cache");
}

/// Categorize a snippet based on embedding similarity to category centroids
/// Returns the best matching category ID and confidence score, or None if no good match
pub async fn categorize_snippet(
    snippet_id: i64,
    min_confidence: f32,
) -> Result<Option<(i64, f32)>> {
    // Get snippet embedding
    let embedding = match lancedb::get_embedding(snippet_id).await? {
        Some(emb) => emb,
        None => {
            log::debug!("Snippet {} has no embedding, cannot categorize", snippet_id);
            return Ok(None);
        }
    };

    // Normalize embedding to unit vector
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    let normalized_embedding: Vec<f32> = if norm > 0.0 {
        embedding.iter().map(|x| x / norm).collect()
    } else {
        embedding
    };

    // Get all categories (root level and children)
    let all_categories = sqlite::get_categories(None, None).await?;

    if all_categories.is_empty() {
        log::debug!("No categories exist yet");
        return Ok(None);
    }

    // Calculate similarity to each category centroid
    let mut best_match: Option<(i64, f32)> = None;
    let min_category_size = 2; // Require at least 2 snippets in category for reliable matching

    log::debug!("🔍 Categorizing snippet {} - checking {} categories", snippet_id, all_categories.len());

    for category in all_categories {
        if let Some(centroid) = get_category_centroid(category.id).await? {
            // Skip categories with too few snippets (unstable centroids)
            if centroid.snippet_count < min_category_size {
                log::debug!(
                    "⏭️  Skipping category {} - only {} snippets (need at least {})",
                    category.id,
                    centroid.snippet_count,
                    min_category_size
                );
                continue;
            }

            let similarity = cosine_similarity(&normalized_embedding, &centroid.centroid);

            log::debug!(
                "📊 Category {} (size: {}) - similarity: {:.3}",
                category.id,
                centroid.snippet_count,
                similarity
            );

            if similarity >= min_confidence {
                if let Some((_, best_score)) = best_match {
                    if similarity > best_score {
                        best_match = Some((category.id, similarity));
                    }
                } else {
                    best_match = Some((category.id, similarity));
                }
            }
        }
    }

    if let Some((cat_id, score)) = best_match {
        log::debug!(
            "✅ Snippet {} best matches category {} with confidence {:.3}",
            snippet_id,
            cat_id,
            score
        );
    } else {
        log::debug!(
            "❌ Snippet {} has no category match above confidence threshold {:.2}",
            snippet_id,
            min_confidence
        );
    }

    Ok(best_match)
}

/// Batch categorize uncategorized snippets
/// Returns number of snippets categorized
pub async fn categorize_uncategorized_snippets(
    limit: i64,
    min_confidence: f32,
) -> Result<usize> {
    log::info!("Starting batch categorization (limit: {}, min_confidence: {:.2})", limit, min_confidence);

    // Get all snippet IDs that have embeddings
    let embedded_ids = lancedb::get_embedded_snippet_ids().await?;

    if embedded_ids.is_empty() {
        log::info!("No snippets with embeddings to categorize");
        return Ok(0);
    }

    // Filter out already categorized snippets
    let mut uncategorized: Vec<i64> = Vec::new();

    for snippet_id in embedded_ids {
        if sqlite::get_category_for_snippet(snippet_id).await?.is_none() {
            uncategorized.push(snippet_id);

            if uncategorized.len() >= limit as usize {
                break;
            }
        }
    }

    if uncategorized.is_empty() {
        log::info!("All snippets are already categorized");
        return Ok(0);
    }

    log::info!("Found {} uncategorized snippets to process", uncategorized.len());

    // Categorize each snippet
    let mut categorized_count = 0;
    let total_to_process = uncategorized.len();

    for snippet_id in &uncategorized {
        if let Some((category_id, confidence)) = categorize_snippet(*snippet_id, min_confidence).await? {
            // Assign snippet to category
            sqlite::assign_snippet_to_category(*snippet_id, category_id, confidence as f64, false).await?;
            categorized_count += 1;

            log::debug!(
                "Assigned snippet {} to category {} (confidence: {:.2})",
                snippet_id,
                category_id,
                confidence
            );
        }
    }

    log::info!(
        "Batch categorization complete: {} snippets categorized out of {} processed",
        categorized_count,
        total_to_process
    );

    Ok(categorized_count)
}

/// Calculate cosine similarity between two normalized vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    // Since vectors are already normalized, cosine similarity is just dot product
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        // Identical vectors
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        // Orthogonal vectors
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 0.0).abs() < 0.001);

        // Opposite vectors
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) + 1.0).abs() < 0.001);
    }
}
