// LLM manager for lazy loading and caching with auto-unload
use anyhow::{Context, Result};
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::inference::llama::LlamaModel;

/// Idle timeout before unloading LLM model (3 minutes)
/// Aggressive memory management - model unloads after brief idle period
const LLM_IDLE_TIMEOUT: Duration = Duration::from_secs(180);

/// Global LLM manager for lazy loading with auto-unload
pub struct LlmManager {
    model_path: PathBuf,
    model: Arc<RwLock<Option<Arc<LlamaModel>>>>,
    last_used: Arc<RwLock<Option<Instant>>>,
}

impl LlmManager {
    /// Create new LLM manager with model path
    pub fn new(model_path: PathBuf) -> Self {
        Self {
            model_path,
            model: Arc::new(RwLock::new(None)),
            last_used: Arc::new(RwLock::new(None)),
        }
    }

    /// Get model, loading it if not already loaded
    pub fn get_model(&self) -> Result<Arc<LlamaModel>> {
        // Try read lock first (fast path)
        {
            let model_option = self.model.read();
            if let Some(model) = model_option.as_ref() {
                // Update last used timestamp
                *self.last_used.write() = Some(Instant::now());
                return Ok(Arc::clone(model));
            }
        }

        // Need to load - acquire write lock
        let mut model_option = self.model.write();

        // Double-check in case another thread loaded it
        if let Some(model) = model_option.as_ref() {
            // Update last used timestamp
            *self.last_used.write() = Some(Instant::now());
            return Ok(Arc::clone(model));
        }

        // Load the model
        log::info!("🤖 Loading LLM model from: {:?}", self.model_path);
        log::info!("⏳ This may take a few seconds...");

        let model = LlamaModel::new(
            self.model_path
                .to_str()
                .context("Invalid model path")?,
        )?;

        let model_arc = Arc::new(model);
        *model_option = Some(Arc::clone(&model_arc));

        // Set last used timestamp
        *self.last_used.write() = Some(Instant::now());

        log::info!("✅ LLM model loaded successfully (940 MB)");
        Ok(model_arc)
    }

    /// Unload model from memory if idle
    pub fn unload_if_idle(&self) -> bool {
        let last_used = self.last_used.read();

        if let Some(timestamp) = *last_used {
            let elapsed = timestamp.elapsed();

            if elapsed > LLM_IDLE_TIMEOUT {
                drop(last_used); // Release read lock before acquiring write lock

                let mut model_option = self.model.write();
                if model_option.is_some() {
                    *model_option = None;
                    *self.last_used.write() = None;

                    log::info!("💾 Unloaded LLM model after {} seconds idle (freed ~940 MB)", elapsed.as_secs());
                    return true;
                }
            }
        }

        false
    }

    /// Manually unload model from memory
    pub fn unload_model(&self) {
        let mut model_option = self.model.write();
        if model_option.is_some() {
            *model_option = None;
            *self.last_used.write() = None;
            log::info!("💾 Manually unloaded LLM model (freed ~940 MB)");
        }
    }

    /// Check if model is loaded
    pub fn is_loaded(&self) -> bool {
        self.model.read().is_some()
    }

    /// Get idle time in seconds (None if not loaded)
    pub fn idle_time_secs(&self) -> Option<u64> {
        self.last_used.read().map(|t| t.elapsed().as_secs())
    }

    /// Check if model file exists
    pub fn model_exists(&self) -> bool {
        self.model_path.exists()
    }

    /// Get model path
    pub fn model_path(&self) -> &PathBuf {
        &self.model_path
    }
}

impl Clone for LlmManager {
    fn clone(&self) -> Self {
        Self {
            model_path: self.model_path.clone(),
            model: Arc::clone(&self.model),
            last_used: Arc::clone(&self.last_used),
        }
    }
}

/// Get default model path based on platform
pub fn get_default_model_path() -> Result<PathBuf> {
    // Get the directory where the binary is located
    let exe_path = std::env::current_exe().context("Failed to get executable path")?;
    let exe_dir = exe_path
        .parent()
        .context("Failed to get executable directory")?;

    // In development: src-tauri/target/debug/
    // In production: LocalMind.app/Contents/MacOS/ or similar
    // We want to go up to find the models directory

    // Try development path first
    let dev_model_path = exe_dir
        .join("../../models/llm/qwen2.5-1.5b-instruct-q3_k_m.gguf")
        .canonicalize();

    if let Ok(path) = dev_model_path {
        if path.exists() {
            return Ok(path);
        }
    }

    // Try production path (models bundled with app)
    let prod_model_path = exe_dir
        .join("../Resources/models/llm/qwen2.5-1.5b-instruct-q3_k_m.gguf");

    if prod_model_path.exists() {
        return Ok(prod_model_path.canonicalize()?);
    }

    // Fallback: relative to working directory
    let fallback_path = PathBuf::from("src-tauri/models/llm/qwen2.5-1.5b-instruct-q3_k_m.gguf");

    if fallback_path.exists() {
        Ok(fallback_path.canonicalize()?)
    } else {
        // Return the path anyway - will fail when trying to load
        Ok(fallback_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_manager_creation() {
        let model_path = PathBuf::from("test_model.gguf");
        let manager = LlmManager::new(model_path.clone());
        assert_eq!(manager.model_path(), &model_path);
        assert!(!manager.is_loaded());
    }

    #[test]
    fn test_model_exists_check() {
        let model_path = PathBuf::from("/nonexistent/model.gguf");
        let manager = LlmManager::new(model_path);
        assert!(!manager.model_exists());
    }
}
