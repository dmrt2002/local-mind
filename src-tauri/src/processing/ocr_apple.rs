use anyhow::{Context, Result};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextAnnotation {
    pub text: String,
    pub confidence: f64,
    pub bbox: BoundingBox,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppleVisionResult {
    pub full_text: String,
    pub annotations: Vec<TextAnnotation>,
    pub processing_time: f64,
    pub word_count: usize,
    pub char_count: usize,
    pub error: Option<String>,
}

/// Extract text from image using Apple Vision Framework
///
/// Uses the ocrmac Python wrapper to access native macOS Vision API.
/// This provides excellent accuracy for screenshot and UI text.
///
/// # Arguments
/// * `image_path` - Path to the image file
/// * `recognition_level` - "fast" or "accurate" (default: "accurate")
///
/// # Returns
/// * `AppleVisionResult` with extracted text and metadata
pub async fn extract_text_apple_vision(
    image_path: &Path,
    recognition_level: &str,
) -> Result<AppleVisionResult> {
    debug!(
        "Starting Apple Vision OCR for: {} (level: {})",
        image_path.display(),
        recognition_level
    );

    // Get the Python script path
    let script_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("processing")
        .join("ocr_apple.py");

    if !script_path.exists() {
        anyhow::bail!(
            "Apple Vision OCR script not found at: {}",
            script_path.display()
        );
    }

    // Execute Python script
    let output = Command::new("python3")
        .arg(&script_path)
        .arg(image_path.as_os_str())
        .arg(recognition_level)
        .output()
        .context("Failed to execute Apple Vision OCR script")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Apple Vision OCR failed: {}", stderr);
    }

    // Parse JSON output
    let result: AppleVisionResult = serde_json::from_slice(&output.stdout).context(
        "Failed to parse Apple Vision OCR output. Make sure ocrmac is installed (pip install ocrmac)",
    )?;

    // Check for errors from Python side
    if let Some(ref error) = result.error {
        warn!("Apple Vision OCR error: {}", error);
        anyhow::bail!("Apple Vision OCR error: {}", error);
    }

    info!(
        "Apple Vision OCR completed: {} words in {:.2}s",
        result.word_count, result.processing_time
    );

    Ok(result)
}

/// Detect if running on Apple Silicon (ARM64) architecture
///
/// Returns true if the system architecture is aarch64 (Apple Silicon)
pub fn is_apple_silicon() -> bool {
    cfg!(target_arch = "aarch64")
}

/// Get architecture information for display purposes
///
/// Returns a string describing the architecture (e.g., "Apple Silicon", "Intel")
pub fn get_architecture_info() -> String {
    if !cfg!(target_os = "macos") {
        return String::new();
    }

    if is_apple_silicon() {
        "Apple Silicon".to_string()
    } else {
        "Intel".to_string()
    }
}

/// Check if Apple Vision Framework is available on this system
///
/// Returns true on macOS systems where ocrmac is installed.
/// Apple Vision Framework works on both Intel and Apple Silicon Macs.
pub fn is_apple_vision_available() -> bool {
    // Check if we're on macOS
    if !cfg!(target_os = "macos") {
        debug!("Apple Vision not available: not running on macOS");
        return false;
    }

    // Check if Python and ocrmac are available
    let output = Command::new("python3")
        .arg("-c")
        .arg("import ocrmac; print('OK')")
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let available = stdout.trim() == "OK";
            
            if available {
                let arch = get_architecture_info();
                info!("Apple Vision available on {}", arch);
            } else {
                debug!("Apple Vision not available: ocrmac package not installed");
            }
            
            available
        }
        Err(e) => {
            debug!("Apple Vision not available: failed to check ocrmac - {}", e);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apple_vision_availability() {
        let available = is_apple_vision_available();
        #[cfg(target_os = "macos")]
        {
            // On macOS, log whether it's available
            println!("Apple Vision available: {}", available);
        }
        #[cfg(not(target_os = "macos"))]
        {
            // On other platforms, should always be false
            assert!(!available);
        }
    }

    #[tokio::test]
    #[cfg(target_os = "macos")]
    async fn test_extract_text_apple_vision() {
        // Skip if ocrmac not installed
        if !is_apple_vision_available() {
            println!("Skipping test: ocrmac not installed");
            return;
        }

        // Create a test image (requires test_screenshot.png)
        let test_image = Path::new("test_screenshot.png");
        if !test_image.exists() {
            println!("Skipping test: test_screenshot.png not found");
            return;
        }

        let result = extract_text_apple_vision(test_image, "fast").await;
        assert!(result.is_ok());

        let ocr_result = result.unwrap();
        assert!(ocr_result.word_count > 0);
        assert!(!ocr_result.full_text.is_empty());
        println!("Extracted {} words", ocr_result.word_count);
    }
}
