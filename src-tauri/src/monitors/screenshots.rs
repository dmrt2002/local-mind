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
                                    log::debug!("⏭️  Screenshot {} already processed (last_modified: {}, current: {})", 
                                        path.display(), last_modified, modified);
                                    continue; // Already processed
                                } else {
                                    log::info!("🔄 Screenshot {} modified (last: {}, current: {}), will reprocess", 
                                        path.display(), last_modified, modified);
                                }
                            } else {
                                log::info!("📸 New screenshot detected: {} (modified: {})", 
                                    path.display(), modified);
                            }

                            known_files.insert(path.clone(), modified);

                            // Check if already processing
                            let mut processing = processing_files.lock().await;
                            if processing.contains_key(&path) {
                                log::debug!("⏳ Screenshot {} already being processed, skipping", path.display());
                                drop(processing);
                                continue;
                            }
                            processing.insert(path.clone(), Instant::now());
                            drop(processing);

                            log::info!("⏳ Queued screenshot {} for processing (waiting 2s for file write to complete)", 
                                path.display());

                            // Process new screenshot
                            let job_queue_clone = job_queue.clone();
                            let processing_files_clone = processing_files.clone();
                            let path_clone = path.clone();

                            tokio::spawn(async move {
                                // Wait 2 seconds to ensure file write is complete
                                tokio::time::sleep(Duration::from_secs(2)).await;

                                log::info!("🚀 Starting to process screenshot: {}", path_clone.display());
                                if let Err(e) =
                                    Self::handle_new_screenshot(&path_clone, &job_queue_clone).await
                                {
                                    log::error!(
                                        "❌ Failed to process screenshot {}: {}",
                                        path_clone.display(),
                                        e
                                    );
                                } else {
                                    log::info!("✅ Completed processing screenshot: {}", path_clone.display());
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

    pub async fn handle_new_screenshot(
        path: &Path,
        job_queue: &Arc<PersistentJobQueue>,
    ) -> Result<()> {
        log::info!("📸 Processing new screenshot: {}", path.display());

        // Read image file and calculate hash for deduplication
        log::debug!("📖 Reading screenshot file: {}", path.display());
        let image_bytes = std::fs::read(path).context("Failed to read screenshot file")?;
        log::debug!("📊 Calculating image hash (file size: {} bytes)", image_bytes.len());
        let image_hash = crate::dedup::calculate_image_hash(&image_bytes);
        log::debug!("🔑 Image hash: {}", image_hash);

        // Check if this screenshot already exists
        let pool = sqlite::get_pool().await?;
        log::debug!("🔍 Checking for duplicate screenshot (hash: {})", image_hash);
        if let Some(existing_id) = crate::dedup::find_duplicate_screenshot(&pool, &image_hash).await? {
            log::info!("⏭️  Screenshot already exists (ID: {}, hash: {}), skipping duplicate", existing_id, image_hash);
            return Ok(());
        }
        log::debug!("✅ No duplicate found, proceeding with save");

        // Copy screenshot to LocalMind managed directory with normalized filename
        // This ensures we have full control over the filename and avoid Unicode issues
        log::info!("📁 Copying screenshot to managed directory (hash: {})", image_hash);
        let managed_path = Self::copy_to_managed_dir(path, &image_hash).await?;
        log::info!("✅ Copied to: {}", managed_path.display());

        // Extract metadata
        log::debug!("📋 Extracting metadata from screenshot");
        let metadata = Self::extract_metadata(path).await?;
        log::debug!("📋 Metadata: {}x{}, {} bytes, source: {:?}", 
            metadata.width, metadata.height, metadata.file_size, metadata.source_app);

        // Save to database with hash (using managed path)
        log::info!("💾 Saving screenshot to database...");
        let snippet_id = Self::save_screenshot(&managed_path, &metadata, &image_hash).await?;

        log::info!(
            "✅ Saved screenshot {} from {} (hash: {})",
            snippet_id,
            metadata.source_app.as_deref().unwrap_or("unknown"),
            image_hash
        );

        // Process screenshot: OCR + Caption + Embedding
        log::info!("🔄 Starting screenshot processing pipeline (OCR + Caption + Embedding) for snippet {}", snippet_id);
        screenshot_processor::process_screenshot(snippet_id, &managed_path, job_queue.clone()).await?;
        log::info!("✅ Screenshot processing pipeline completed for snippet {}", snippet_id);

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
/// Scans both the managed directory and the Desktop directory for unprocessed screenshots
/// Returns (total_found, processed, skipped)
pub async fn rescan_existing_screenshots(job_queue: Arc<PersistentJobQueue>) -> Result<(usize, usize, usize)> {
    log::info!("🔄 Starting rescan of existing screenshots...");

    // Get managed screenshots directory (use same path resolution as database)
    let managed_dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("data")
        .join("local-mind")
        .join("screenshots");

    log::info!("📂 Scanning managed directory: {}", managed_dir.display());
    log::info!("📂 Current working directory: {:?}", std::env::current_dir());

    // Also get Desktop directory to scan for unprocessed screenshots
    let desktop_dir = get_default_screenshot_dir()
        .unwrap_or_else(|_| PathBuf::from("."));
    log::info!("📂 Also scanning Desktop directory: {}", desktop_dir.display());

    let mut total_found = 0;
    let mut processed = 0;
    let mut skipped = 0;

    // First, scan managed directory
    if managed_dir.exists() {
        let (found, proc, skip) = scan_directory(&managed_dir, &job_queue, true).await?;
        total_found += found;
        processed += proc;
        skipped += skip;
    } else {
        log::warn!("⚠️  Managed screenshots directory does not exist: {}", managed_dir.display());
    }

    // Then, scan Desktop directory for unprocessed screenshots
    if desktop_dir.exists() {
        log::info!("📂 Scanning Desktop directory for unprocessed screenshots...");
        let (found, proc, skip) = scan_directory(&desktop_dir, &job_queue, false).await?;
        total_found += found;
        processed += proc;
        skipped += skip;
    } else {
        log::warn!("⚠️  Desktop directory does not exist: {}", desktop_dir.display());
    }

    log::info!(
        "✅ Rescan complete: {} total files, {} processed, {} skipped",
        total_found,
        processed,
        skipped
    );

    Ok((total_found, processed, skipped))
}

/// Scan a directory for screenshots and process any that aren't in the database
/// If is_managed_dir is true, assumes files are already in managed directory format (hash-based filenames)
/// If false, treats files as source screenshots that need to be copied to managed directory
async fn scan_directory(
    dir: &Path,
    job_queue: &Arc<PersistentJobQueue>,
    is_managed_dir: bool,
) -> Result<(usize, usize, usize)> {
    // Get all existing screenshots from database
    // For managed directory, use filename-based comparison (hash-based filenames)
    // For Desktop directory, we'll use content hash comparison
    let pool = sqlite::get_pool().await?;
    
    let existing_filenames: std::collections::HashSet<String> = if is_managed_dir {
        // For managed directory, compare by filename (hash-based)
        sqlx::query_as::<_, (String,)>(
            "SELECT file_path FROM snippets WHERE type = 'screenshot' AND file_path IS NOT NULL"
        )
        .fetch_all(&pool)
        .await
        .context("Failed to query existing screenshots")?
        .into_iter()
        .filter_map(|(path_str,)| {
            // Extract filename from path (works with both absolute and relative paths)
            let path = PathBuf::from(&path_str);
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|s| s.to_string())
        })
        .collect()
    } else {
        // For Desktop directory, we'll check by content hash instead
        // Get all content hashes from database
        sqlx::query_as::<_, (Option<String>,)>(
            "SELECT content_hash FROM snippets WHERE type = 'screenshot' AND content_hash IS NOT NULL"
        )
        .fetch_all(&pool)
        .await
        .context("Failed to query existing screenshot hashes")?
        .into_iter()
        .filter_map(|(hash,)| hash)
        .collect()
    };

    log::info!("📊 Found {} existing screenshots in database", existing_filenames.len());

    // Scan directory for all image files
    let mut total_found = 0;
    let mut processed = 0;
    let mut skipped = 0;

    if let Ok(entries) = std::fs::read_dir(dir) {
        log::info!("📂 Scanning files in directory: {}", dir.display());
        for entry in entries.flatten() {
            let path = entry.path();

            if !ScreenshotMonitor::is_image_file(&path) {
                log::debug!("⏭️  Skipping non-image file: {}", path.display());
                continue;
            }

            total_found += 1;

            if is_managed_dir {
                // For managed directory, check by filename
                let filename = match path.file_name().and_then(|n| n.to_str()) {
                    Some(name) => name.to_string(),
                    None => {
                        log::warn!("⚠️  Cannot extract filename from path: {}, skipping", path.display());
                        skipped += 1;
                        continue;
                    }
                };

                log::info!("🔍 Checking file: {} (filename: {})", path.display(), filename);

                // Check if this filename exists in database
                if existing_filenames.contains(&filename) {
                    skipped += 1;
                    log::info!("⏭️  Skipping {} (filename '{}' already in database)", path.display(), filename);
                    continue;
                }

                // File exists on disk but not in database - process it!
                log::info!("📸 Found orphaned screenshot: {} (filename: '{}' not in database)", path.display(), filename);

                match process_orphaned_screenshot(&path, job_queue).await {
                    Ok(()) => {
                        processed += 1;
                        log::info!("✅ Successfully processed orphaned screenshot");
                    }
                    Err(e) => {
                        log::error!("❌ Failed to process orphaned screenshot {}: {}", path.display(), e);
                    }
                }
            } else {
                // For Desktop directory, check by content hash
                log::info!("🔍 Checking file: {}", path.display());

                // Read image and calculate hash
                match std::fs::read(&path) {
                    Ok(image_bytes) => {
                        let image_hash = crate::dedup::calculate_image_hash(&image_bytes);
                        
                        if existing_filenames.contains(&image_hash) {
                            skipped += 1;
                            log::info!("⏭️  Skipping {} (hash '{}' already in database)", path.display(), image_hash);
                            continue;
                        }

                        // New screenshot found - process it through normal pipeline
                        log::info!("📸 Found new screenshot: {} (hash: '{}' not in database)", path.display(), image_hash);
                        
                        match ScreenshotMonitor::handle_new_screenshot(&path, job_queue).await {
                            Ok(()) => {
                                processed += 1;
                                log::info!("✅ Successfully processed new screenshot");
                            }
                            Err(e) => {
                                log::error!("❌ Failed to process screenshot {}: {}", path.display(), e);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("❌ Failed to read file {}: {}", path.display(), e);
                        skipped += 1;
                    }
                }
            }
        }
    }

    log::info!("📊 Scan summary for {}:", dir.display());
    log::info!("   Total files found: {}", total_found);
    log::info!("   Files processed: {}", processed);
    log::info!("   Files skipped: {}", skipped);

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
