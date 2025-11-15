use anyhow::{Context, Result};
use log;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use crate::clipboard::get_source_app;
use crate::db::sqlite;
use crate::job_queue::PersistentJobQueue;
use crate::processing::screenshot_processor;

#[derive(Debug, Clone)]
pub struct ScreenshotMetadata {
    pub source_app: Option<String>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub width: u32,
    pub height: u32,
    pub file_size: u64,
}

pub struct ScreenshotMonitor {
    screenshot_dir: PathBuf,
    job_queue: Arc<PersistentJobQueue>,
    processing_files: Arc<Mutex<HashMap<PathBuf, Instant>>>,
}

impl ScreenshotMonitor {
    pub fn new(screenshot_dir: PathBuf, job_queue: Arc<PersistentJobQueue>) -> Self {
        Self {
            screenshot_dir,
            job_queue,
            processing_files: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Start monitoring the screenshot directory
    pub async fn start_monitoring(&self) -> Result<()> {
        log::info!("📸 Starting screenshot monitoring");
        log::info!("   Directory: {}", self.screenshot_dir.display());

        // Ensure directory exists
        if !self.screenshot_dir.exists() {
            log::warn!(
                "Screenshot directory does not exist: {}",
                self.screenshot_dir.display()
            );
            return Ok(());
        }

        // For now, we'll implement polling-based monitoring
        // In production, we'd use notify crate for file system events
        let screenshot_dir = self.screenshot_dir.clone();
        let job_queue = self.job_queue.clone();
        let processing_files = self.processing_files.clone();

        tokio::spawn(async move {
            let mut known_files: HashMap<PathBuf, u64> = HashMap::new();

            // Pre-populate known_files with existing screenshots from database
            // This prevents re-processing files that were already saved
            if let Ok(pool) = sqlite::get_pool().await {
                match sqlx::query_as::<_, (String,)>(
                    "SELECT file_path FROM snippets WHERE type = 'screenshot' AND file_path IS NOT NULL"
                )
                .fetch_all(&pool)
                .await
                {
                    Ok(existing_paths) => {
                        for (path_str,) in existing_paths {
                            let path = PathBuf::from(path_str);
                            // Get file modification time if file still exists
                            if let Ok(metadata) = std::fs::metadata(&path) {
                                let modified = metadata
                                    .modified()
                                    .ok()
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_secs())
                                    .unwrap_or(0);
                                known_files.insert(path, modified);
                            }
                        }
                        log::info!("📸 Pre-loaded {} existing screenshots from database", known_files.len());
                    }
                    Err(e) => {
                        log::warn!("Failed to load existing screenshots: {}", e);
                    }
                }
            }

            loop {
                // Scan directory for new image files
                if let Ok(entries) = std::fs::read_dir(&screenshot_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();

                        if !Self::is_image_file(&path) {
                            continue;
                        }

                        // Check if this is a new file
                        if let Ok(metadata) = entry.metadata() {
                            let modified = metadata
                                .modified()
                                .ok()
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs())
                                .unwrap_or(0);

                            if let Some(&last_modified) = known_files.get(&path) {
                                if last_modified >= modified {
                                    continue; // Already processed
                                }
                            }

                            known_files.insert(path.clone(), modified);

                            // Check if already processing
                            let mut processing = processing_files.lock().await;
                            if processing.contains_key(&path) {
                                continue;
                            }
                            processing.insert(path.clone(), Instant::now());
                            drop(processing);

                            // Process new screenshot
                            let job_queue_clone = job_queue.clone();
                            let processing_files_clone = processing_files.clone();
                            let path_clone = path.clone();

                            tokio::spawn(async move {
                                // Wait 2 seconds to ensure file write is complete
                                tokio::time::sleep(Duration::from_secs(2)).await;

                                if let Err(e) =
                                    Self::handle_new_screenshot(&path_clone, &job_queue_clone).await
                                {
                                    log::error!(
                                        "Failed to process screenshot {}: {}",
                                        path_clone.display(),
                                        e
                                    );
                                }

                                // Remove from processing
                                let mut processing = processing_files_clone.lock().await;
                                processing.remove(&path_clone);
                            });
                        }
                    }
                }

                // Clean up old processing entries (older than 1 minute)
                let mut processing = processing_files.lock().await;
                processing.retain(|_, instant| instant.elapsed().as_secs() < 60);
                drop(processing);

                // Check every 2 seconds
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        });

        Ok(())
    }

    fn is_image_file(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| matches!(ext.to_lowercase().as_str(), "png" | "jpg" | "jpeg" | "webp"))
            .unwrap_or(false)
    }

    async fn handle_new_screenshot(
        path: &Path,
        job_queue: &Arc<PersistentJobQueue>,
    ) -> Result<()> {
        log::info!("📸 New screenshot detected: {}", path.display());

        // Read image file and calculate hash for deduplication
        let image_bytes = std::fs::read(path).context("Failed to read screenshot file")?;
        let image_hash = crate::dedup::calculate_image_hash(&image_bytes);

        // Check if this screenshot already exists
        let pool = sqlite::get_pool().await?;
        if let Some(existing_id) = crate::dedup::find_duplicate_screenshot(&pool, &image_hash).await? {
            log::info!("⏭️  Screenshot already exists (ID: {}), skipping duplicate", existing_id);
            return Ok(());
        }

        // Copy screenshot to LocalMind managed directory with normalized filename
        // This ensures we have full control over the filename and avoid Unicode issues
        let managed_path = Self::copy_to_managed_dir(path, &image_hash).await?;

        // Extract metadata
        let metadata = Self::extract_metadata(path).await?;

        // Save to database with hash (using managed path)
        let snippet_id = Self::save_screenshot(&managed_path, &metadata, &image_hash).await?;

        log::info!(
            "✅ Saved screenshot {} from {}",
            snippet_id,
            metadata.source_app.as_deref().unwrap_or("unknown")
        );

        // Process screenshot: OCR + Caption + Embedding
        screenshot_processor::process_screenshot(snippet_id, &managed_path, job_queue.clone()).await?;

        Ok(())
    }

    /// Copy screenshot to LocalMind managed directory with normalized, hash-based filename
    async fn copy_to_managed_dir(source_path: &Path, content_hash: &str) -> Result<PathBuf> {
        // Get managed screenshots directory
        let mut managed_dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("data")
            .join("local-mind")
            .join("screenshots");

        // Create directory if it doesn't exist
        std::fs::create_dir_all(&managed_dir)
            .context("Failed to create managed screenshots directory")?;

        // Create filename from hash + original extension
        let extension = source_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("png");
        let filename = format!("{}.{}", content_hash, extension);
        managed_dir.push(&filename);

        // Copy file
        std::fs::copy(source_path, &managed_dir)
            .context("Failed to copy screenshot to managed directory")?;

        log::debug!("📁 Copied screenshot to managed directory: {}", managed_dir.display());

        Ok(managed_dir)
    }

    async fn extract_metadata(path: &Path) -> Result<ScreenshotMetadata> {
        use crate::clipboard::get_browser_metadata;

        // Get image dimensions
        let (width, height) = Self::get_image_dimensions(path)?;

        // Get file size
        let file_size = std::fs::metadata(path)?.len();

        // Get source app
        let source_app = get_source_app();

        // If from browser, get URL and title
        let mut url = None;
        let mut title = None;

        if let Some(ref app) = source_app {
            if Self::is_browser(app) {
                if let Some(browser_data) = get_browser_metadata(app) {
                    url = browser_data
                        .get("url")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    title = browser_data
                        .get("title")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                }
            }
        }

        Ok(ScreenshotMetadata {
            source_app,
            url,
            title,
            width,
            height,
            file_size,
        })
    }

    fn get_image_dimensions(path: &Path) -> Result<(u32, u32)> {
        let img = image::open(path).context("Failed to open image")?;
        Ok((img.width(), img.height()))
    }

    fn is_browser(app_name: &str) -> bool {
        let app_lower = app_name.to_lowercase();
        app_lower.contains("chrome")
            || app_lower.contains("firefox")
            || app_lower.contains("safari")
            || app_lower.contains("edge")
            || app_lower.contains("brave")
            || app_lower.contains("opera")
    }

    async fn save_screenshot(path: &Path, metadata: &ScreenshotMetadata, content_hash: &str) -> Result<i64> {
        use chrono::Utc;

        let pool = sqlite::get_pool().await?;
        let now = Utc::now();

        // Create a descriptive content string
        let content = format!(
            "Screenshot: {}",
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
        );

        // Create metadata JSON
        let metadata_json = serde_json::json!({
            "width": metadata.width,
            "height": metadata.height,
            "file_size": metadata.file_size,
        });

        // Save to snippets table with content_hash
        // Use INSERT OR IGNORE to handle race conditions where multiple threads
        // try to insert the same screenshot simultaneously
        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO snippets (
                content, type, file_path, source_app,
                website_url, website_title, metadata, content_hash,
                created_at, updated_at
            )
            VALUES (?, 'screenshot', ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&content)
        .bind(path.to_str())
        .bind(&metadata.source_app)
        .bind(&metadata.url)
        .bind(&metadata.title)
        .bind(metadata_json.to_string())
        .bind(content_hash)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&pool)
        .await
        .context("Failed to insert screenshot snippet")?;

        // If no rows were affected, the screenshot already existed (race condition)
        if result.rows_affected() == 0 {
            // Query for the existing screenshot ID
            let existing_id = sqlx::query_scalar::<_, i64>(
                "SELECT id FROM snippets WHERE content_hash = ? AND type = 'screenshot'"
            )
            .bind(content_hash)
            .fetch_one(&pool)
            .await
            .context("Failed to fetch existing screenshot ID")?;

            log::debug!(
                "Screenshot already exists (race condition), using existing ID: {}",
                existing_id
            );
            return Ok(existing_id);
        }

        // Get the ID of the newly inserted row
        let snippet_id = result.last_insert_rowid();

        Ok(snippet_id)
    }
}

/// Get default screenshot directory for the current OS
pub fn get_default_screenshot_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME environment variable not set")?;

    #[cfg(target_os = "macos")]
    {
        // macOS default: ~/Desktop
        Ok(PathBuf::from(home).join("Desktop"))
    }

    #[cfg(target_os = "linux")]
    {
        // Linux: Try Pictures/Screenshots first, fallback to Desktop
        let pictures_screenshots = PathBuf::from(&home).join("Pictures/Screenshots");
        if pictures_screenshots.exists() {
            Ok(pictures_screenshots)
        } else {
            Ok(PathBuf::from(home).join("Desktop"))
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows: %USERPROFILE%\Pictures\Screenshots
        Ok(PathBuf::from(home).join("Pictures\\Screenshots"))
    }
}

/// Rescan existing screenshots and process any that aren't in the database
/// Returns (total_found, processed, skipped)
pub async fn rescan_existing_screenshots(job_queue: Arc<PersistentJobQueue>) -> Result<(usize, usize, usize)> {
    log::info!("🔄 Starting rescan of existing screenshots...");

    // Get managed screenshots directory
    let managed_dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("data")
        .join("local-mind")
        .join("screenshots");

    if !managed_dir.exists() {
        log::warn!("Managed screenshots directory does not exist: {}", managed_dir.display());
        return Ok((0, 0, 0));
    }

    // Get all existing screenshots from database
    let pool = sqlite::get_pool().await?;
    let existing_paths: std::collections::HashSet<PathBuf> = sqlx::query_as::<_, (String,)>(
        "SELECT file_path FROM snippets WHERE type = 'screenshot' AND file_path IS NOT NULL"
    )
    .fetch_all(&pool)
    .await
    .context("Failed to query existing screenshots")?
    .into_iter()
    .map(|(path_str,)| PathBuf::from(path_str))
    .collect();

    log::info!("📊 Found {} screenshots in database", existing_paths.len());

    // Scan directory for all image files
    let mut total_found = 0;
    let mut processed = 0;
    let mut skipped = 0;

    if let Ok(entries) = std::fs::read_dir(&managed_dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            if !ScreenshotMonitor::is_image_file(&path) {
                continue;
            }

            total_found += 1;

            // Check if this file is in the database
            if existing_paths.contains(&path) {
                skipped += 1;
                continue;
            }

            // File exists on disk but not in database - process it!
            log::info!("📸 Found orphaned screenshot: {}", path.display());

            match process_orphaned_screenshot(&path, &job_queue).await {
                Ok(()) => {
                    processed += 1;
                    log::info!("✅ Successfully processed orphaned screenshot");
                }
                Err(e) => {
                    log::error!("❌ Failed to process orphaned screenshot {}: {}", path.display(), e);
                }
            }
        }
    }

    log::info!(
        "✅ Rescan complete: {} total files, {} processed, {} skipped",
        total_found,
        processed,
        skipped
    );

    Ok((total_found, processed, skipped))
}

/// Process an orphaned screenshot (exists on disk but not in database)
async fn process_orphaned_screenshot(
    path: &Path,
    job_queue: &Arc<PersistentJobQueue>,
) -> Result<()> {
    // Read image file and calculate hash
    let image_bytes = std::fs::read(path).context("Failed to read screenshot file")?;
    let image_hash = crate::dedup::calculate_image_hash(&image_bytes);

    // Double-check it doesn't exist (dedup check)
    let pool = sqlite::get_pool().await?;
    if let Some(existing_id) = crate::dedup::find_duplicate_screenshot(&pool, &image_hash).await? {
        log::info!("⏭️  Screenshot already exists (ID: {}), skipping", existing_id);
        return Ok(());
    }

    // Extract metadata
    let metadata = ScreenshotMonitor::extract_metadata(path).await?;

    // Save to database (use existing path - it's already in managed dir)
    let snippet_id = ScreenshotMonitor::save_screenshot(path, &metadata, &image_hash).await?;

    log::info!(
        "✅ Saved orphaned screenshot {} from {}",
        snippet_id,
        metadata.source_app.as_deref().unwrap_or("unknown")
    );

    // Process screenshot: OCR + Caption + Embedding + Categorization
    screenshot_processor::process_screenshot(snippet_id, path, job_queue.clone()).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_image_file() {
        assert!(ScreenshotMonitor::is_image_file(Path::new("test.png")));
        assert!(ScreenshotMonitor::is_image_file(Path::new("test.jpg")));
        assert!(ScreenshotMonitor::is_image_file(Path::new("test.jpeg")));
        assert!(ScreenshotMonitor::is_image_file(Path::new("test.webp")));
        assert!(!ScreenshotMonitor::is_image_file(Path::new("test.txt")));
        assert!(!ScreenshotMonitor::is_image_file(Path::new("test.pdf")));
    }

    #[test]
    fn test_is_browser() {
        assert!(ScreenshotMonitor::is_browser("Google Chrome"));
        assert!(ScreenshotMonitor::is_browser("Firefox"));
        assert!(ScreenshotMonitor::is_browser("Safari"));
        assert!(ScreenshotMonitor::is_browser("Microsoft Edge"));
        assert!(!ScreenshotMonitor::is_browser("Terminal"));
        assert!(!ScreenshotMonitor::is_browser("VS Code"));
    }

    #[test]
    fn test_get_default_screenshot_dir() {
        let dir = get_default_screenshot_dir();
        assert!(dir.is_ok());
    }
}
