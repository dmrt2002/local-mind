use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use parking_lot::{Mutex, RwLock};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Idle timeout before unloading embedding model (3 minutes)
/// Aggressive memory management - model unloads after brief idle period
const EMBEDDING_IDLE_TIMEOUT: Duration = Duration::from_secs(180);

/// Global singleton embedding engine - ensures model is only loaded once
static GLOBAL_EMBEDDING_ENGINE: Lazy<Arc<Mutex<Option<fastembed::TextEmbedding>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

/// Global last used timestamp for embedding engine
static GLOBAL_LAST_USED: Lazy<Arc<RwLock<Option<Instant>>>> =
    Lazy::new(|| Arc::new(RwLock::new(None)));

/// Embedding engine using fastembed
pub struct EmbeddingEngine {
    model: Arc<Mutex<Option<fastembed::TextEmbedding>>>,
    last_used: Arc<RwLock<Option<Instant>>>,
}

impl EmbeddingEngine {
    /// Create a new EmbeddingEngine that shares the global model instance
    pub fn new() -> Self {
        Self {
            model: GLOBAL_EMBEDDING_ENGINE.clone(),
            last_used: GLOBAL_LAST_USED.clone(),
        }
    }

    /// Load the embedding model lazily on first use
    /// Uses local model DIRECTLY - NO DOWNLOADS, NO CACHE in source directory
    async fn ensure_loaded(&self) -> Result<()> {
        // Fast path: check if already loaded
        {
            let model_guard = self.model.lock();
            if model_guard.is_some() {
                log::debug!("✅ Embedding model already loaded");
                return Ok(());
            }
        }

        {
            let model_guard = self.model.lock();
            if model_guard.is_some() {
                return Ok(());
            }
        }

        log::info!("🔧 [ENGINE] Loading LOCAL model - checking cache location...");

        // Verify XDG_CACHE_HOME is set and symlink exists
        let xdg_cache = std::env::var("XDG_CACHE_HOME").unwrap_or_default();
        log::info!("🔧 [ENGINE] XDG_CACHE_HOME = '{}'", xdg_cache);

        let cache_home = if !xdg_cache.is_empty() {
            std::path::PathBuf::from(xdg_cache)
        } else {
            std::env::var("HOME")
                .ok()
                .map(std::path::PathBuf::from)
                .map(|h| h.join(".cache"))
                .unwrap_or_else(|| std::path::PathBuf::from(".cache"))
        };

        let fastembed_cache = cache_home
            .join("fastembed")
            .join("sentence-transformers_all-MiniLM-L6-v2");

        log::info!(
            "🔧 [ENGINE] Fastembed cache location: {:?}",
            fastembed_cache
        );

        // Check if cache is symlink or directory
        #[cfg(unix)]
        {
            if let Ok(meta) = std::fs::symlink_metadata(&fastembed_cache) {
                if meta.file_type().is_symlink() {
                    if let Ok(target) = std::fs::read_link(&fastembed_cache) {
                        log::info!("✅ [ENGINE] Cache is symlink -> {:?}", target);
                    }
                } else {
                    log::warn!(
                        "⚠️  [ENGINE] Cache is a directory (not symlink) - this might cause issues"
                    );
                }
            }
        }

        // Verify ONNX model exists in cache
        let onnx_file = fastembed_cache.join("model.onnx");
        let tokenizer_file = fastembed_cache.join("tokenizer.json");

        log::info!("🔧 [ENGINE] Checking for model.onnx: {:?}", onnx_file);
        log::info!(
            "🔧 [ENGINE] Checking for tokenizer.json: {:?}",
            tokenizer_file
        );

        if !onnx_file.exists() {
            log::error!("❌ [ENGINE] ONNX model not found at: {:?}", onnx_file);
            log::error!(
                "❌ [ENGINE] Cache directory exists: {}",
                fastembed_cache.exists()
            );
            if fastembed_cache.exists() {
                log::error!("❌ [ENGINE] Listing cache directory contents:");
                if let Ok(entries) = std::fs::read_dir(&fastembed_cache) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        let name = path.file_name().unwrap_or_default();
                        log::error!(
                            "❌ [ENGINE]   - {} (exists: {})",
                            name.to_string_lossy(),
                            path.exists()
                        );
                    }
                }
            }
            return Err(anyhow::anyhow!(
                "ONNX model not found in cache. Expected at: {:?}",
                onnx_file
            ));
        }

        if !tokenizer_file.exists() {
            log::warn!("⚠️  [ENGINE] tokenizer.json not found (model.onnx exists, continuing)");
        }

        log::info!("✅ [ENGINE] ONNX model found in cache");

        // Verify file is readable
        if let Ok(metadata) = std::fs::metadata(&onnx_file) {
            log::info!("✅ [ENGINE] ONNX file size: {} bytes", metadata.len());
        }

        // CRITICAL: Set XDG_CACHE_HOME in the blocking thread context
        // fastembed checks this environment variable at initialization
        let xdg_cache_for_thread = cache_home.to_string_lossy().to_string();
        let cache_path_clone = fastembed_cache.clone();

        log::info!(
            "🔧 [ENGINE] Initializing fastembed with cache: {:?}",
            fastembed_cache
        );
        log::info!(
            "🔧 [ENGINE] XDG_CACHE_HOME for thread: {}",
            xdg_cache_for_thread
        );

        let model_result = tokio::task::spawn_blocking(move || {
            // Set environment variable in this thread
            std::env::set_var("XDG_CACHE_HOME", xdg_cache_for_thread.clone());

            // Verify files are accessible in blocking thread
            let onnx_check = cache_path_clone.join("model.onnx");
            if !onnx_check.exists() {
                log::error!(
                    "❌ [ENGINE] model.onnx NOT accessible in blocking thread at: {:?}",
                    onnx_check
                );
                return Err(anyhow::anyhow!(
                    "Model files not accessible - symlink may not be followed"
                ));
            }
            log::info!("✅ [ENGINE] model.onnx verified in blocking thread");

            // Use the new fastembed 5.2 API with explicit cache directory
            fastembed::TextEmbedding::try_new(
                fastembed::InitOptions::new(fastembed::EmbeddingModel::AllMiniLML6V2)
                    .with_show_download_progress(false)
                    .with_cache_dir(cache_path_clone.parent().unwrap_or(&cache_path_clone).to_path_buf())
            )
        })
        .await;

        let model = match model_result {
            Ok(Ok(model)) => {
                log::info!("✅ [ENGINE] Model loaded successfully from LOCAL cache!");
                println!("✅ [ENGINE] Model loaded successfully from LOCAL cache!");
                model
            }
            Ok(Err(e)) => {
                log::error!("❌ [ENGINE] Failed to load model: {}", e);
                log::error!("❌ [ENGINE] Error details: {:#}", e);
                println!("❌ [ENGINE] Failed to load model: {}", e);
                println!("❌ [ENGINE] Error details: {:#}", e);
                return Err(anyhow::anyhow!("Failed to load local model. Error: {}", e));
            }
            Err(e) => {
                println!("❌ [ENGINE] Task spawn error: {}", e);
                return Err(anyhow::anyhow!("Task spawn error: {}", e));
            }
        };

        let mut model_guard = self.model.lock();
        if model_guard.is_some() {
            return Ok(());
        }
        *model_guard = Some(model);
        log::info!("✅ [ENGINE] Embedding model ready - 100% LOCAL, NO network!");

        Ok(())
    }

    /// Generate embedding for text
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.ensure_loaded().await?;

        // Update last used timestamp
        *self.last_used.write() = Some(Instant::now());

        let mut model_guard = self.model.lock();
        let model = model_guard.as_mut().context("Model not loaded")?;

        let embeddings = model
            .embed(vec![text.to_string()], None)
            .context("Failed to generate embedding")?;

        drop(model_guard);

        embeddings
            .into_iter()
            .next()
            .context("No embedding generated")
    }

    /// Unload the model to free memory
    pub fn unload(&self) {
        let mut model_guard = self.model.lock();
        *model_guard = None;
        *self.last_used.write() = None;
        log::info!("Embedding model unloaded");
    }

    /// Unload model from memory if idle
    pub fn unload_if_idle(&self) -> bool {
        let last_used = self.last_used.read();

        if let Some(timestamp) = *last_used {
            let elapsed = timestamp.elapsed();

            if elapsed > EMBEDDING_IDLE_TIMEOUT {
                drop(last_used); // Release read lock before acquiring write lock

                let mut model_guard = self.model.lock();
                if model_guard.is_some() {
                    *model_guard = None;
                    *self.last_used.write() = None;

                    log::info!("💾 Unloaded embedding model after {} seconds idle (freed ~86 MB)", elapsed.as_secs());
                    return true;
                }
            }
        }

        false
    }

    /// Check if model is loaded
    pub fn is_loaded(&self) -> bool {
        self.model.lock().is_some()
    }

    /// Get idle time in seconds (None if not loaded)
    pub fn idle_time_secs(&self) -> Option<u64> {
        self.last_used.read().map(|t| t.elapsed().as_secs())
    }
}
