use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::Result;
use crate::embedding::engine::EmbeddingEngine;
use crate::db::sqlite::Snippet;

/// Streaming embedder that can be interrupted
pub struct StreamingEmbedder {
    model: EmbeddingEngine,
    pause_flag: Arc<AtomicBool>,
}

impl StreamingEmbedder {
    pub fn new() -> Self {
        Self {
            model: EmbeddingEngine::new(),
            pause_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Embed multiple snippets with ability to pause
    pub async fn embed_interruptible(
        &self,
        snippets: Vec<Snippet>,
    ) -> Result<()> {
        for (idx, snippet) in snippets.iter().enumerate() {
            // Check if pause requested
            if self.pause_flag.load(Ordering::Relaxed) {
                log::info!("Embedding paused by user");
                self.pause_and_unload().await?;
                return Ok(());
            }

            // Generate embedding
            match self.model.embed(&snippet.content).await {
                Ok(embedding) => {
                    // Save to LanceDB
                    if let Err(e) = crate::db::lancedb::save_embedding(
                        snippet.id,
                        embedding,
                    ).await {
                        log::error!("Failed to save embedding for snippet {}: {}", snippet.id, e);
                    }
                }
                Err(e) => {
                    log::error!("Failed to embed snippet {}: {}", snippet.id, e);
                }
            }

            // Yield every N embeddings to let other tasks run
            if idx % 3 == 0 {
                tokio::task::yield_now().await;
            }
        }

        Ok(())
    }

    /// Pause embedding and unload model
    pub async fn pause_and_unload(&self) -> Result<()> {
        self.pause_flag.store(true, Ordering::Relaxed);
        self.model.unload();
        Ok(())
    }

    /// Resume embedding
    pub fn resume(&self) {
        self.pause_flag.store(false, Ordering::Relaxed);
    }

    /// Check if paused
    pub fn is_paused(&self) -> bool {
        self.pause_flag.load(Ordering::Relaxed)
    }
}
