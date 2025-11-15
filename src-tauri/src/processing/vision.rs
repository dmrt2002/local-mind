use anyhow::{Context, Result};
use log;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Check if Florence-2 model is downloaded and ready to use
///
/// Returns true if the model directory exists and contains the caption.py script
pub fn is_florence2_downloaded() -> bool {
    match get_florence2_model_path() {
        Ok(model_dir) => {
            let script_path = model_dir.join("caption.py");
            model_dir.exists() && script_path.exists()
        }
        Err(_) => false,
    }
}

/// Get the path where Florence-2 model should be stored
pub fn get_florence2_model_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME environment variable not set")?;
    Ok(PathBuf::from(home)
        .join(".localmind")
        .join("models")
        .join("florence2"))
}

/// Generate a caption for an image using Florence-2
///
/// Uses Python script with transformers library for Florence-2 inference.
/// Falls back to generic caption if model not available.
pub async fn generate_caption(image_path: &Path) -> Result<String> {
    log::info!("🎨 [VISION] Starting caption generation for: {}", image_path.display());

    // Try to use Python Florence-2 if available
    if is_florence2_downloaded() {
        log::info!("✅ [VISION] Florence-2 model detected, attempting caption generation");
        match call_python_florence2(image_path).await {
            Ok(caption) => {
                if !caption.is_empty() && !caption.starts_with("ERROR:") {
                    log::info!("✅ [VISION] Successfully generated Florence-2 caption: {}", caption);
                    return Ok(caption);
                } else {
                    log::error!("❌ [VISION] Florence-2 returned error/invalid caption: {}", caption);
                    // Fall through to placeholder
                }
            }
            Err(e) => {
                log::error!("❌ [VISION] Failed to generate caption with Florence-2: {}", e);
                log::error!("❌ [VISION] Error details: {:?}", e);
                // Fall through to placeholder
            }
        }
    } else {
        let model_path = get_florence2_model_path().unwrap_or_else(|_| PathBuf::from("unknown"));
        log::warn!("⚠️  [VISION] Florence-2 not available - checking model path: {}", model_path.display());
        let script_path = model_path.join("caption.py");
        if !model_path.exists() {
            log::warn!("⚠️  [VISION] Model directory does not exist: {}", model_path.display());
        } else if !script_path.exists() {
            log::warn!("⚠️  [VISION] Caption script not found: {}", script_path.display());
        } else {
            log::warn!("⚠️  [VISION] Model directory exists but is_florence2_downloaded() returned false");
        }
    }

    // Fallback to generic caption if model not available or failed
    let caption = format!(
        "Screenshot captured at {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );

    log::warn!("⚠️  [VISION] Using fallback caption (vision model unavailable/failed): {}", caption);

    Ok(caption)
}

/// Call Python script for Florence-2 inference (helper for MVP)
async fn call_python_florence2(image_path: &Path) -> Result<String> {
    let python_script = get_florence2_model_path()?.join("caption.py");

    log::info!("🔍 [VISION] Checking Python script at: {}", python_script.display());

    if !python_script.exists() {
        let error = format!(
            "Florence-2 Python script not found at: {}. Run download_florence2_model() first.",
            python_script.display()
        );
        log::error!("❌ [VISION] {}", error);
        return Err(anyhow::anyhow!(error));
    }

    // Verify image exists
    if !image_path.exists() {
        let error = format!("Image file not found: {}", image_path.display());
        log::error!("❌ [VISION] {}", error);
        return Err(anyhow::anyhow!(error));
    }

    log::info!("🚀 [VISION] Executing Python script: python3 {} {}", 
               python_script.display(), 
               image_path.display());

    let start_time = std::time::Instant::now();
    let output = Command::new("python3")
        .arg(&python_script)
        .arg(image_path)
        .output()
        .context("Failed to run Florence-2 Python script. Make sure Python 3 is installed.")?;

    let duration = start_time.elapsed();
    log::info!("⏱️  [VISION] Python script execution took: {:.2}s", duration.as_secs_f64());

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let exit_code = output.status.code().unwrap_or(-1);
        
        log::error!("❌ [VISION] Python script failed with exit code: {}", exit_code);
        log::error!("❌ [VISION] STDERR: {}", stderr);
        if !stdout.is_empty() {
            log::error!("❌ [VISION] STDOUT: {}", stdout);
        }
        
        let error_msg = if !stderr.is_empty() {
            format!("Florence-2 script error (exit {}): {}", exit_code, stderr)
        } else if !stdout.is_empty() {
            format!("Florence-2 script output (exit {}): {}", exit_code, stdout)
        } else {
            format!("Florence-2 script failed with exit code {} and no error message", exit_code)
        };
        return Err(anyhow::anyhow!("{}", error_msg));
    }

    let caption = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string();

    if caption.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::error!("❌ [VISION] Script returned empty caption");
        if !stderr.is_empty() {
            log::error!("❌ [VISION] STDERR (may contain error): {}", stderr);
        }
        return Err(anyhow::anyhow!("Florence-2 script returned empty caption"));
    }

    log::info!("✅ [VISION] Caption generated successfully (length: {} chars)", caption.len());

    Ok(caption)
}

/// Download Florence-2 model
pub async fn download_florence2_model() -> Result<()> {
    let model_dir = get_florence2_model_path()?;

    // Create model directory
    std::fs::create_dir_all(&model_dir)
        .context("Failed to create model directory")?;

    log::info!("📥 Setting up Florence-2 model at {}", model_dir.display());

    // Create the Python caption script
    let caption_script = model_dir.join("caption.py");
    let python_code = r#"#!/usr/bin/env python3
"""
Florence-2 Image Captioning Script for LocalMind
Generates descriptive captions for screenshots using Microsoft's Florence-2 model.
"""

import sys
import os
from pathlib import Path

try:
    from transformers import AutoProcessor, AutoModelForCausalLM
    from PIL import Image
    import torch
except ImportError:
    print("ERROR: Required packages not installed", file=sys.stderr)
    print("Please run: pip install transformers torch pillow", file=sys.stderr)
    sys.exit(1)

def generate_caption(image_path: str) -> str:
    """Generate a descriptive caption for an image using Florence-2."""
    try:
        # Load model and processor (cached after first use)
        model_name = "microsoft/Florence-2-base"
        device = "cuda" if torch.cuda.is_available() else "cpu"

        processor = AutoProcessor.from_pretrained(model_name, trust_remote_code=True)
        model = AutoModelForCausalLM.from_pretrained(
            model_name,
            trust_remote_code=True,
            torch_dtype=torch.float16 if device == "cuda" else torch.float32
        ).to(device)

        # Load and process image
        image = Image.open(image_path).convert("RGB")

        # Generate caption
        prompt = "<CAPTION>"
        inputs = processor(text=prompt, images=image, return_tensors="pt").to(device)

        generated_ids = model.generate(
            input_ids=inputs["input_ids"],
            pixel_values=inputs["pixel_values"],
            max_new_tokens=1024,
            num_beams=3,
        )

        generated_text = processor.batch_decode(generated_ids, skip_special_tokens=False)[0]

        # Parse the caption from the generated text
        caption = generated_text.replace(prompt, "").strip()

        return caption

    except Exception as e:
        print(f"ERROR: {str(e)}", file=sys.stderr)
        return f"Screenshot of {Path(image_path).stem}"

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: caption.py <image_path>", file=sys.stderr)
        sys.exit(1)

    image_path = sys.argv[1]

    if not os.path.exists(image_path):
        print(f"ERROR: Image file not found: {image_path}", file=sys.stderr)
        sys.exit(1)

    caption = generate_caption(image_path)
    print(caption)
"#;

    std::fs::write(&caption_script, python_code)
        .context("Failed to write caption.py script")?;

    // Make script executable on Unix-like systems
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&caption_script)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&caption_script, perms)?;
    }

    // Create setup instructions file
    let readme_file = model_dir.join("README.txt");
    std::fs::write(
        &readme_file,
        "Florence-2 Image Captioning Setup\n\
         ===================================\n\
         \n\
         The caption.py script has been created.\n\
         \n\
         To enable AI-powered screenshot captions:\n\
         1. Install Python dependencies:\n\
         \n\
         pip install transformers torch pillow\n\
         \n\
         2. The Florence-2 model (~800MB) will auto-download on first use\n\
         3. Captions will be generated automatically for new screenshots\n\
         \n\
         Note: First run may take 30-60 seconds as model downloads.\n\
         Subsequent captions generate in 2-3 seconds.\n\
         \n\
         To test manually:\n\
         python3 caption.py /path/to/screenshot.png\n",
    )?;

    log::info!("✅ Created Florence-2 caption script");
    log::info!("   Script: {}", caption_script.display());
    log::info!("   Setup: {}", readme_file.display());
    log::info!("   Run: pip install transformers torch pillow");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_florence2_model_path() {
        let path = get_florence2_model_path();
        assert!(path.is_ok());
        println!("Model path: {}", path.unwrap().display());
    }

    #[test]
    fn test_is_florence2_downloaded() {
        let downloaded = is_florence2_downloaded();
        println!("Florence-2 downloaded: {}", downloaded);
    }
}
