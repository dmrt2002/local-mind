// Progressive model loading for Phase 2

use anyhow::Result;
use std::path::Path;
use tokio::sync::mpsc;
use crate::inference::llama::LlamaModel;

#[derive(Debug, Clone)]
pub struct LoadProgress {
    pub percent: u64,
    pub stage: String,
}

/// Load LLM model with progress reporting
pub async fn load_llm_with_progress(
    model_path: &Path,
    progress_tx: mpsc::Sender<LoadProgress>,
) -> Result<LlamaModel> {
    use std::fs;

    let file_size = fs::metadata(model_path)?.len();

    // Send initial progress
    let _ = progress_tx
        .send(LoadProgress {
            percent: 0,
            stage: "Initializing...".to_string(),
        })
        .await;

    // TODO: Implement actual progressive loading with mmap
    // For now, load model and simulate progress
    let model_path_str = model_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid model path"))?;
    
    let mut loaded_mut = 0u64;
    let model = LlamaModel::load_with_callback(model_path_str, |bytes_loaded| {
        loaded_mut += bytes_loaded as u64;
        let percent = if file_size > 0 {
            (loaded_mut * 100) / file_size
        } else {
            0
        };
        
        // Try to send progress (non-blocking)
        let _ = progress_tx.try_send(LoadProgress {
            percent,
            stage: "Loading model...".to_string(),
        });
    })?;

    // Send completion
    let _ = progress_tx
        .send(LoadProgress {
            percent: 100,
            stage: "Ready!".to_string(),
        })
        .await;

    Ok(model)
}
