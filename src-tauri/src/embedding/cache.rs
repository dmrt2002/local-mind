use lru::LruCache;
use std::num::NonZeroUsize;
use std::time::{SystemTime, Duration};
use std::sync::Arc;
use tokio::sync::Mutex;
use anyhow::Result;
use crate::embedding::engine::EmbeddingEngine;

/// Cache for query embeddings to avoid re-embedding similar queries
pub struct QueryCache {
    cache: Arc<Mutex<LruCache<String, (Vec<f32>, SystemTime)>>>,
    engine: EmbeddingEngine,
    ttl: Duration,
}

impl QueryCache {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(
                LruCache::new(NonZeroUsize::new(50).unwrap())
            )),
            engine: EmbeddingEngine::new(),
            ttl: Duration::from_secs(3600), // 1 hour
        }
    }

    /// Get embedding from cache or generate new one
    pub async fn get_or_embed(&self, query: &str) -> Result<Vec<f32>> {
        // Normalize query
        let normalized = Self::normalize(query);

        // Check cache
        let mut cache_guard = self.cache.lock().await;
        
        if let Some((embedding, timestamp)) = cache_guard.get(&normalized) {
            if timestamp.elapsed().unwrap_or(Duration::MAX) < self.ttl {
                log::debug!("Cache hit for query: {}", normalized);
                return Ok(embedding.clone());
            } else {
                // Cache expired
                cache_guard.pop(&normalized);
            }
        }

        drop(cache_guard);

        // Cache miss - generate embedding
        log::debug!("Cache miss for query: {}", normalized);
        let embedding = self.engine.embed(query).await?;

        // Store in cache
        let mut cache_guard = self.cache.lock().await;
        cache_guard.put(
            normalized,
            (embedding.clone(), SystemTime::now()),
        );

        Ok(embedding)
    }

    /// Normalize query for caching
    fn normalize(query: &str) -> String {
        query
            .to_lowercase()
            .trim()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Clear cache
    pub async fn clear(&self) {
        let mut cache_guard = self.cache.lock().await;
        cache_guard.clear();
    }
}
