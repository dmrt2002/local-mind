use anyhow::{Context, Result};
use log;
use std::path::Path;
use std::process::Command;

use super::text_processing::{TextProcessor, ExtractedEntities};
use super::image_preprocessing::{preprocess_for_ocr, cleanup_temp_files};
use super::ocr_apple::{extract_text_apple_vision, is_apple_vision_available};
use crate::settings::get_cached_settings;

/// Check if Tesseract is installed on the system
pub fn is_tesseract_installed() -> bool {
    Command::new("tesseract")
        .arg("--version")
        .output()
        .is_ok()
}

/// Extract text from an image using smart OCR engine selection
/// This is the main entry point that automatically chooses the best OCR engine
pub async fn extract_text_from_image(image_path: &Path) -> Result<String> {
    extract_text_smart(image_path).await
}

/// Extract text from an image and also return extracted entities
/// Uses intelligent engine selection: Apple Vision (macOS) or Tesseract (fallback)
pub async fn extract_text_with_entities(image_path: &Path) -> Result<(String, ExtractedEntities)> {
    let cleaned_text = extract_text_smart(image_path).await?;

    let processor = TextProcessor::new();
    let entities = processor.extract_entities(&cleaned_text);

    log::debug!(
        "Extracted entities: {} URLs, {} emails, {} file paths, {} code snippets, {} commands",
        entities.urls.len(),
        entities.emails.len(),
        entities.file_paths.len(),
        entities.code_snippets.len(),
        entities.commands.len()
    );

    Ok((cleaned_text, entities))
}

/// Smart OCR engine selection based on platform and settings
/// Priority: User preference > Apple Vision (macOS) > Tesseract (fallback)
async fn extract_text_smart(image_path: &Path) -> Result<String> {
    let settings = get_cached_settings().unwrap_or_default();

    // Determine which engine to use
    let use_apple_vision = match settings.ocr_engine.as_str() {
        "apple_vision" => {
            // User explicitly requested Apple Vision
            if !cfg!(target_os = "macos") {
                log::warn!("Apple Vision requested but not on macOS, falling back to Tesseract");
                false
            } else if !is_apple_vision_available() {
                log::warn!("Apple Vision requested but not available (install: pip install ocrmac), falling back to Tesseract");
                false
            } else {
                true
            }
        },
        "tesseract" => {
            // User explicitly requested Tesseract
            false
        },
        "auto" | _ => {
            // Auto-detect: use Apple Vision if available on macOS
            cfg!(target_os = "macos") && is_apple_vision_available()
        }
    };

    if use_apple_vision {
        // Try Apple Vision first
        log::info!("Using Apple Vision Framework for OCR (recognition level: {})", settings.ocr_recognition_level);

        match extract_text_apple_vision(image_path, &settings.ocr_recognition_level).await {
            Ok(result) => {
                log::info!(
                    "Apple Vision extracted {} words in {:.2}s",
                    result.word_count,
                    result.processing_time
                );

                // Apply text cleaning based on settings
                let cleaned = apply_text_cleaning(&result.full_text, &settings.ocr_cleaning_level);
                return Ok(cleaned);
            },
            Err(e) => {
                log::warn!("Apple Vision failed: {}, falling back to Tesseract", e);
                // Fall through to Tesseract
            }
        }
    }

    // Use Tesseract (either by choice or as fallback)
    log::info!("Using Tesseract OCR (PSM mode: {})", settings.tesseract_psm_mode);
    extract_text_tesseract(image_path, &settings.tesseract_psm_mode, &settings.ocr_cleaning_level).await
}

/// Extract text using Tesseract with configurable settings
async fn extract_text_tesseract(
    image_path: &Path,
    psm_mode: &i32,
    cleaning_level: &str,
) -> Result<String> {
    // Check if tesseract is installed
    if !is_tesseract_installed() {
        return Err(anyhow::anyhow!(
            "Tesseract is not installed. Please install it:\n\
             macOS: brew install tesseract\n\
             Linux: sudo apt-get install tesseract-ocr\n\
             Windows: Download from GitHub releases"
        ));
    }

    log::debug!("Running Tesseract OCR on: {}", image_path.display());

    // Preprocess image for better OCR quality (optional based on cleaning level)
    let image_to_process = if cleaning_level == "aggressive" {
        log::debug!("Preprocessing image for OCR (aggressive mode)...");
        preprocess_for_ocr(image_path).await?
    } else {
        // For minimal/balanced, skip preprocessing to preserve digital text quality
        image_path.to_path_buf()
    };

    // Run tesseract with configurable PSM mode
    let output = Command::new("tesseract")
        .arg(&image_to_process)
        .arg("stdout")
        .arg("--dpi")
        .arg("300")
        .arg("--psm")
        .arg(psm_mode.to_string()) // Use configurable PSM mode
        .arg("--oem")
        .arg("1") // LSTM only
        .arg("-c")
        .arg("tessedit_char_blacklist=|©®™")
        .arg("-c")
        .arg("preserve_interword_spaces=1")
        .output()
        .context("Failed to execute tesseract command")?;

    // Clean up temporary preprocessed image if we created one
    if cleaning_level == "aggressive" && image_to_process != image_path {
        if let Err(e) = cleanup_temp_files(&image_to_process) {
            log::warn!("Failed to cleanup temp file: {}", e);
        }
    }

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Tesseract failed: {}", error));
    }

    let text = String::from_utf8_lossy(&output.stdout).to_string();

    // Apply text cleaning based on settings
    let cleaned = apply_text_cleaning(&text, cleaning_level);

    log::debug!(
        "Tesseract extracted {} characters (cleaned from {})",
        cleaned.len(),
        text.len()
    );

    Ok(cleaned)
}

/// Apply text cleaning based on configured level
fn apply_text_cleaning(text: &str, level: &str) -> String {
    let processor = TextProcessor::new();

    match level {
        "minimal" => {
            // Minimal cleaning: just remove null bytes and excessive whitespace
            text.replace('\0', "")
                .lines()
                .filter(|line| !line.trim().is_empty())
                .collect::<Vec<_>>()
                .join("\n")
        },
        "balanced" => {
            // Balanced: use standard OCR cleaning
            processor.clean_ocr_text(text)
        },
        "aggressive" => {
            // Aggressive: use screenshot-specific cleaning
            processor.clean_screenshot_ocr(text)
        },
        _ => processor.clean_ocr_text(text), // Default to balanced
    }
}

/// Clean up OCR text using comprehensive text processing
fn cleanup_ocr_text(text: &str) -> String {
    let processor = TextProcessor::new();
    processor.clean_ocr_text(text)
}

/// Extract text with confidence scores (if available)
pub async fn extract_text_with_confidence(image_path: &Path) -> Result<(String, Option<f32>)> {
    // For now, just extract text without confidence
    // In the future, we could parse TSV output for confidence scores
    let text = extract_text_from_image(image_path).await?;
    Ok((text, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cleanup_ocr_text() {
        let input = "  Line 1  \n\n  Line 2  \n  \n  Line 3  ";
        let expected = "Line 1\nLine 2\nLine 3";
        assert_eq!(cleanup_ocr_text(input), expected);
    }

    #[test]
    fn test_is_tesseract_installed() {
        // This test will pass if tesseract is installed
        let installed = is_tesseract_installed();
        if installed {
            println!("✓ Tesseract is installed");
        } else {
            println!("✗ Tesseract is not installed");
        }
    }
}
