use anyhow::{Context, Result};
use log;
use std::path::Path;
use std::sync::Arc;

use crate::db::sqlite;
use crate::inference::global_llm;
use crate::inference::summarization::{self, ContentType};
use crate::job_queue::{PersistentJobQueue, Priority};
use crate::processing::{ocr, vision, text_processing::TextProcessor};
use crate::settings;

/// Process a screenshot: OCR + Caption + Embedding
pub async fn process_screenshot(
    snippet_id: i64,
    image_path: &Path,
    job_queue: Arc<PersistentJobQueue>,
) -> Result<()> {
    log::info!("🔄 Processing screenshot {}: {}", snippet_id, image_path.display());

    let pool = sqlite::get_pool().await?;
    let settings = settings::load_settings(&pool).await?;

    // Step 1: OCR with entity extraction (if enabled)
    let (ocr_text, entities) = if settings.screenshot_ocr_enabled {
        match ocr::extract_text_with_entities(image_path).await {
            Ok((text, ent)) if !text.is_empty() => {
                log::info!("   ✓ OCR extracted {} characters", text.len());
                log::debug!("   ✓ Entities: {} URLs, {} emails, {} commands",
                    ent.urls.len(), ent.emails.len(), ent.commands.len());
                (Some(text), Some(ent))
            }
            Ok(_) => {
                log::debug!("   OCR returned empty text");
                (None, None)
            }
            Err(e) => {
                log::warn!("   ⚠ OCR failed: {}", e);
                (None, None)
            }
        }
    } else {
        log::debug!("   OCR disabled");
        (None, None)
    };

    // Step 2: Caption (if enabled)
    let caption = if settings.screenshot_caption_enabled {
        log::info!("🎨 [SCREENSHOT] Caption generation enabled, calling vision model...");
        match vision::generate_caption(image_path).await {
            Ok(text) => {
                if text.starts_with("Screenshot captured at") {
                    log::warn!("⚠️  [SCREENSHOT] Caption generation returned fallback caption (vision model may have failed)");
                } else {
                    log::info!("✅ [SCREENSHOT] Caption generated successfully: {}", text);
                }
                Some(text)
            }
            Err(e) => {
                log::error!("❌ [SCREENSHOT] Caption generation failed with error: {}", e);
                log::error!("❌ [SCREENSHOT] Error details: {:?}", e);
                None
            }
        }
    } else {
        log::debug!("⏭️  [SCREENSHOT] Caption generation disabled in settings");
        None
    };

    // Step 3: Detect content type - prioritize caption, then OCR text, then General
    let content_type = detect_content_type_from_caption_or_ocr(caption.as_deref(), ocr_text.as_deref());

    // Step 4: Combine text for embedding
    let combined_text = combine_screenshot_text(caption.as_deref(), ocr_text.as_deref());

    if combined_text.is_empty() {
        log::warn!("   No text extracted from screenshot, using placeholder");
    }

    log::info!("   Combined text: {} characters", combined_text.len());

    // Step 5: Generate intelligent summary using LLM (if available) or fallback to rule-based
    let summary = generate_summary(
        ocr_text.as_deref(),
        caption.as_deref(),
        entities.as_ref(),
        content_type,
    ).await;

    log::info!("   ✓ Summary generated: {}", summary);

    // Step 6: Update snippet content and summary
    update_snippet_content(snippet_id, &combined_text, Some(&summary)).await?;

    // Step 7: Queue for embedding using existing job queue
    job_queue
        .push(
            snippet_id,
            combined_text.clone(),
            Some(summary), // Use intelligent summary
            Priority::Normal,
        )
        .await?;

    log::info!("✅ Screenshot {} processed and queued for embedding", snippet_id);

    Ok(())
}

/// Generate intelligent summary using LLM or rule-based fallback
/// Uses only the caption (vision model output) if available, falls back to OCR text only if caption is not available
async fn generate_summary(
    ocr_text: Option<&str>,
    caption: Option<&str>,
    entities: Option<&crate::processing::text_processing::ExtractedEntities>,
    content_type: ContentType,
) -> String {
    // Priority: Use caption (vision model) if available, otherwise fall back to OCR text
    let (text, use_caption) = if let Some(cap) = caption {
        if !cap.is_empty() && !cap.starts_with("Screenshot") {
            log::info!("   Using caption (vision model) for summary generation");
            (cap, true)
        } else if let Some(ocr) = ocr_text {
            if !ocr.is_empty() {
                log::info!("   Caption is placeholder, falling back to OCR text");
                (ocr, false)
            } else {
                return "Screenshot".to_string();
            }
        } else {
            return "Screenshot".to_string();
        }
    } else if let Some(ocr) = ocr_text {
        if !ocr.is_empty() {
            log::info!("   No caption available, using OCR text for summary generation");
            (ocr, false)
        } else {
            return "Screenshot".to_string();
        }
    } else {
        return "Screenshot".to_string();
    };

    // Try LLM-based summarization first (if model is available)
    if let Some(llm_manager) = global_llm::get_global_llm() {
        if let Ok(model) = llm_manager.get_model() {
            // Only pass caption if we're using it, otherwise pass None
            let caption_for_llm = if use_caption { caption } else { None };
            match summarization::summarize_with_llm(
                &model,
                text,
                caption_for_llm,
                entities,
                content_type.clone(),
            )
            .await
            {
                Ok(result) => {
                    log::debug!("   ✓ LLM summary: {}", result.summary);
                    return result.summary;
                }
                Err(e) => {
                    log::warn!("   ⚠ LLM summarization failed: {}, using fallback", e);
                }
            }
        }
    }

    // Fallback to rule-based summarization
    let entities = entities.cloned().unwrap_or_else(|| {
        let processor = TextProcessor::new();
        processor.extract_entities(text)
    });

    // Only pass caption if we're using it, otherwise pass None
    let caption_for_rule = if use_caption { caption } else { None };
    let result = summarization::generate_rule_based_summary(
        text,
        caption_for_rule,
        &entities,
        content_type,
    );

    result.summary
}

/// Detect content type from caption or OCR text
/// Priority: Caption → OCR text → General
fn detect_content_type_from_caption_or_ocr(caption: Option<&str>, ocr_text: Option<&str>) -> ContentType {
    // First priority: Try to detect from caption
    if let Some(cap) = caption {
        if !cap.is_empty() && !cap.starts_with("Screenshot") {
            let content_type = parse_content_type_from_caption(cap);
            if content_type != ContentType::General {
                log::info!("   Content type detected from caption: {:?}", content_type);
                return content_type;
            }
            log::debug!("   Caption available but no content type prefix found, falling back to OCR");
        }
    }

    // Second priority: Detect from OCR text structure
    if let Some(text) = ocr_text {
        if !text.is_empty() {
            let processor = TextProcessor::new();
            let structure = processor.detect_structure(text);
            log::info!("   Detected structure from OCR: {:?}", structure);
            let ct = ContentType::from(structure);
            log::info!("   Content type from OCR: {:?}", ct);
            return ct;
        }
    }

    // Third priority: Default to General
    log::info!("   No caption or OCR text available, using General content type");
    ContentType::General
}

/// Parse content type from caption format
/// Handles patterns like "Code:", "Terminal:", "Web:", "Document:", etc.
fn parse_content_type_from_caption(caption: &str) -> ContentType {
    let caption_lower = caption.to_lowercase();
    
    // Check for explicit prefixes (e.g., "Code: Deploy workflows to coolify")
    if caption_lower.starts_with("code:") {
        return ContentType::Code;
    }
    if caption_lower.starts_with("terminal:") || caption_lower.starts_with("shell:") || caption_lower.starts_with("command:") {
        return ContentType::Terminal;
    }
    if caption_lower.starts_with("web:") || caption_lower.starts_with("webpage:") || caption_lower.starts_with("browser:") {
        return ContentType::WebPage;
    }
    if caption_lower.starts_with("document:") || caption_lower.starts_with("pdf:") || caption_lower.starts_with("text:") {
        return ContentType::Document;
    }
    if caption_lower.starts_with("design:") || caption_lower.starts_with("ui:") || caption_lower.starts_with("mockup:") {
        return ContentType::Design;
    }
    if caption_lower.starts_with("chat:") || caption_lower.starts_with("message:") || caption_lower.starts_with("conversation:") {
        return ContentType::Chat;
    }

    // Analyze caption text for keywords if no prefix found
    if caption_lower.contains("code") || caption_lower.contains("programming") || caption_lower.contains("function") || 
       caption_lower.contains("variable") || caption_lower.contains("class") || caption_lower.contains("import") {
        return ContentType::Code;
    }
    if caption_lower.contains("terminal") || caption_lower.contains("command line") || caption_lower.contains("shell") ||
       caption_lower.contains("prompt") || caption_lower.contains("$") || caption_lower.contains("#") {
        return ContentType::Terminal;
    }
    if caption_lower.contains("webpage") || caption_lower.contains("browser") || caption_lower.contains("url") ||
       caption_lower.contains("website") || caption_lower.contains("http") {
        return ContentType::WebPage;
    }
    if caption_lower.contains("document") || caption_lower.contains("pdf") || caption_lower.contains("text document") {
        return ContentType::Document;
    }
    if caption_lower.contains("design") || caption_lower.contains("ui") || caption_lower.contains("mockup") ||
       caption_lower.contains("wireframe") || caption_lower.contains("prototype") {
        return ContentType::Design;
    }
    if caption_lower.contains("chat") || caption_lower.contains("message") || caption_lower.contains("conversation") ||
       caption_lower.contains("discord") || caption_lower.contains("slack") {
        return ContentType::Chat;
    }

    // Default to General if no pattern matches
    ContentType::General
}

/// Combine caption and OCR text into searchable content
fn combine_screenshot_text(caption: Option<&str>, ocr_text: Option<&str>) -> String {
    let mut parts = Vec::new();

    if let Some(cap) = caption {
        if !cap.is_empty() {
            parts.push(format!("[Caption: {}]", cap));
        }
    }

    if let Some(ocr) = ocr_text {
        if !ocr.is_empty() {
            parts.push(format!("[Text: {}]", ocr));
        }
    }

    if parts.is_empty() {
        // Fallback if no text was extracted
        return "[Screenshot]".to_string();
    }

    parts.join(" ")
}

/// Update snippet content in database
async fn update_snippet_content(
    snippet_id: i64,
    content: &str,
    summary: Option<&str>,
) -> Result<()> {
    let pool = sqlite::get_pool().await?;

    sqlx::query(
        r#"
        UPDATE snippets
        SET content = ?, summary = ?, updated_at = datetime('now')
        WHERE id = ?
        "#,
    )
    .bind(content)
    .bind(summary)
    .bind(snippet_id)
    .execute(&pool)
    .await
    .context("Failed to update snippet content")?;

    log::debug!("Updated snippet {} with processed text", snippet_id);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_combine_screenshot_text() {
        let result = combine_screenshot_text(
            Some("A screenshot of code"),
            Some("fn main() { println!(\"hello\"); }"),
        );
        assert!(result.contains("[Caption: A screenshot of code]"));
        assert!(result.contains("[Text: fn main()"));

        let result_caption_only = combine_screenshot_text(Some("Just a caption"), None);
        assert_eq!(result_caption_only, "[Caption: Just a caption]");

        let result_ocr_only = combine_screenshot_text(None, Some("Just OCR text"));
        assert_eq!(result_ocr_only, "[Text: Just OCR text]");

        let result_empty = combine_screenshot_text(None, None);
        assert_eq!(result_empty, "[Screenshot]");
    }
}
