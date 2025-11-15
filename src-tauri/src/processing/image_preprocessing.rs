use anyhow::{Context, Result};
use image::{DynamicImage, GrayImage, ImageBuffer, Luma};
use log;
use std::path::{Path, PathBuf};

/// Preprocess an image for optimal OCR quality
/// Returns path to preprocessed image (temporary file)
pub async fn preprocess_for_ocr(image_path: &Path) -> Result<PathBuf> {
    log::debug!("Preprocessing image for OCR: {}", image_path.display());

    // Load the image
    let img = image::open(image_path)
        .context(format!("Failed to open image: {}", image_path.display()))?;

    // Step 1: Convert to grayscale (reduces noise, improves contrast recognition)
    log::debug!("  Step 1: Converting to grayscale");
    let gray_img = img.to_luma8();

    // Step 2: Enhance contrast using histogram equalization
    log::debug!("  Step 2: Enhancing contrast");
    let enhanced = enhance_contrast(&gray_img);

    // Step 3: Apply adaptive binarization (convert to black & white)
    // This is crucial for OCR accuracy - separates text from background
    log::debug!("  Step 3: Applying adaptive binarization");
    let binarized = apply_adaptive_binarization(&enhanced);

    // Step 4: Save preprocessed image to temporary file
    let temp_path = create_temp_path(image_path)?;
    binarized.save(&temp_path)
        .context("Failed to save preprocessed image")?;

    log::debug!("  ✓ Preprocessed image saved to: {}", temp_path.display());

    Ok(temp_path)
}

/// Enhance contrast using histogram equalization (CLAHE-like approach)
fn enhance_contrast(img: &GrayImage) -> GrayImage {
    let (width, height) = img.dimensions();
    let mut enhanced = ImageBuffer::new(width, height);

    // Build histogram
    let mut histogram = [0u32; 256];
    for pixel in img.pixels() {
        histogram[pixel[0] as usize] += 1;
    }

    // Calculate cumulative distribution function (CDF)
    let mut cdf = [0u32; 256];
    cdf[0] = histogram[0];
    for i in 1..256 {
        cdf[i] = cdf[i - 1] + histogram[i];
    }

    // Normalize CDF to create equalization mapping
    let total_pixels = (width * height) as f32;
    let cdf_min = *cdf.iter().find(|&&x| x > 0).unwrap_or(&0) as f32;

    for (x, y, pixel) in enhanced.enumerate_pixels_mut() {
        let old_value = img.get_pixel(x, y)[0] as usize;
        let new_value = ((cdf[old_value] as f32 - cdf_min) / (total_pixels - cdf_min) * 255.0) as u8;
        *pixel = Luma([new_value]);
    }

    enhanced
}

/// Apply adaptive binarization using Otsu's method
/// This converts the image to pure black and white, making text crisp
fn apply_adaptive_binarization(img: &GrayImage) -> GrayImage {
    let (width, height) = img.dimensions();
    let mut binarized = ImageBuffer::new(width, height);

    // Calculate optimal threshold using Otsu's method
    let threshold = calculate_otsu_threshold(img);

    log::debug!("  Binarization threshold: {}", threshold);

    // Apply threshold - pixels above threshold become white, below become black
    for (x, y, pixel) in binarized.enumerate_pixels_mut() {
        let value = img.get_pixel(x, y)[0];
        *pixel = if value > threshold {
            Luma([255]) // White (text)
        } else {
            Luma([0])   // Black (background)
        };
    }

    binarized
}

/// Calculate optimal threshold using Otsu's method
/// This finds the threshold that best separates foreground (text) from background
fn calculate_otsu_threshold(img: &GrayImage) -> u8 {
    // Build histogram
    let mut histogram = [0u32; 256];
    for pixel in img.pixels() {
        histogram[pixel[0] as usize] += 1;
    }

    let total_pixels = img.width() * img.height();

    // Calculate optimal threshold using Otsu's method
    let mut sum_total = 0u64;
    for (i, &count) in histogram.iter().enumerate() {
        sum_total += (i as u64) * (count as u64);
    }

    let mut sum_background = 0u64;
    let mut weight_background = 0u32;
    let mut max_variance = 0.0;
    let mut threshold = 0u8;

    for t in 0..256 {
        weight_background += histogram[t];
        if weight_background == 0 {
            continue;
        }

        let weight_foreground = total_pixels - weight_background;
        if weight_foreground == 0 {
            break;
        }

        sum_background += (t as u64) * (histogram[t] as u64);

        let mean_background = sum_background as f64 / weight_background as f64;
        let mean_foreground =
            (sum_total - sum_background) as f64 / weight_foreground as f64;

        // Calculate inter-class variance
        let variance = (weight_background as f64) * (weight_foreground as f64)
            * (mean_background - mean_foreground).powi(2);

        if variance > max_variance {
            max_variance = variance;
            threshold = t as u8;
        }
    }

    threshold
}

/// Create temporary file path for preprocessed image
fn create_temp_path(original_path: &Path) -> Result<PathBuf> {
    let file_name = original_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid file name"))?;

    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join(format!("{}_preprocessed.png", file_name));

    Ok(temp_path)
}

/// Clean up temporary preprocessed images
pub fn cleanup_temp_files(temp_path: &Path) -> Result<()> {
    if temp_path.exists() {
        std::fs::remove_file(temp_path)
            .context("Failed to remove temporary preprocessed image")?;
        log::debug!("Cleaned up temp file: {}", temp_path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_otsu_threshold() {
        // Create a simple test image (black background, white text)
        let mut img = GrayImage::new(100, 100);

        // Fill top half with black (0), bottom half with white (255)
        for y in 0..50 {
            for x in 0..100 {
                img.put_pixel(x, y, Luma([0]));
            }
        }
        for y in 50..100 {
            for x in 0..100 {
                img.put_pixel(x, y, Luma([255]));
            }
        }

        let threshold = calculate_otsu_threshold(&img);
        // Threshold should be around 127 (middle of 0 and 255)
        assert!(threshold > 100 && threshold < 150, "Threshold was {}", threshold);
    }

    #[test]
    fn test_create_temp_path() {
        let original = PathBuf::from("/path/to/screenshot.png");
        let temp = create_temp_path(&original).unwrap();

        assert!(temp.to_str().unwrap().contains("screenshot_preprocessed.png"));
    }
}
