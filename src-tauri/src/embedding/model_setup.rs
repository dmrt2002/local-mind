use anyhow::Result;
use std::path::PathBuf;

/// Find the local model directory and set it up for fastembed to use directly
/// Returns the path to the model directory
pub fn get_local_model_path() -> Result<PathBuf> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Try multiple locations
    let search_paths = vec![
        cwd.join("src-tauri/models/embeddings"),
        cwd.join("models/embeddings"),
        PathBuf::from("src-tauri/models/embeddings"),
    ];

    for path in &search_paths {
        // Fastembed requires ONNX format, check for model.onnx first
        let onnx_file = path.join("model.onnx");
        if onnx_file.exists() {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
            // Don't log here - logger might not be initialized yet
            return Ok(canonical);
        }
        // Fallback: check for PyTorch (will need conversion, but allow it for now)
        let pytorch_file = path.join("pytorch_model.bin");
        if pytorch_file.exists() {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
            // Don't log here - logger might not be initialized yet
            return Ok(canonical);
        }
    }

    Err(anyhow::anyhow!(
        "Local model not found. Expected ONNX model at src-tauri/models/embeddings/model.onnx\n\
        Fastembed requires ONNX format, not PyTorch. Run download_embedding_model_onnx.sh"
    ))
}
