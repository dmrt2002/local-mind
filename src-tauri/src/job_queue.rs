use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::{SqlitePool, SqlitePoolOptions}, Row};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use crate::embedding::engine::EmbeddingEngine;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: Option<i64>,
    pub snippet_id: i64,
    pub content: String,
    pub summary: Option<String>,
    pub priority: Priority,
    pub status: JobStatus,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Priority {
    Low = 0,
    Normal = 1,
    High = 2,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
}

fn get_job_queue_db_path() -> Result<PathBuf> {
    // For now, always use local data directory (works in both dev and prod)
    let mut path = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("data")
        .join("local-mind");

    // Ensure directory exists
    std::fs::create_dir_all(&path)
        .context(format!("Failed to create data directory at {:?}", path))?;

    path.push("job_queue.db");
    Ok(path)
}

pub struct PersistentJobQueue {
    pool: SqlitePool,
    worker_tx: Option<mpsc::Sender<Job>>,
}

impl PersistentJobQueue {
    pub async fn new() -> Result<Self> {
        let db_path = get_job_queue_db_path()?;

        // Ensure the database file exists (sqlx needs it for absolute paths)
        if !db_path.exists() {
            std::fs::File::create(&db_path).context(format!(
                "Failed to create job queue database file at {:?}",
                db_path
            ))?;
        }

        // Get absolute path for sqlx
        let db_path_abs = db_path.canonicalize().unwrap_or_else(|_| {
            let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            if db_path.is_absolute() {
                db_path
            } else {
                current_dir.join(&db_path)
            }
        });

        let path_str = db_path_abs.display().to_string().replace('\\', "/");
        let db_url = format!("sqlite:///{}", path_str);

        let pool = SqlitePoolOptions::new()
            .max_connections(3)  // Reduced from default 10 for memory optimization
            .min_connections(1)
            .connect(&db_url)
            .await
            .with_context(|| format!("Failed to connect to job queue database at {}", db_url))?;

        // Create job queue table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS job_queue (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                snippet_id INTEGER NOT NULL,
                content TEXT NOT NULL,
                priority INTEGER NOT NULL,
                status TEXT NOT NULL,
                error_message TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT
            )
            "#,
        )
        .execute(&pool)
        .await
        .context("Failed to create job_queue table")?;

        // Create index for faster queries
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_job_queue_status 
            ON job_queue(status)
            "#,
        )
        .execute(&pool)
        .await
        .context("Failed to create index")?;

        Ok(Self {
            pool,
            worker_tx: None,
        })
    }

    /// Start the background worker
    pub fn start_worker(&mut self) -> Result<()> {
        let (tx, rx) = mpsc::channel(100);
        self.worker_tx = Some(tx);

        let pool = self.pool.clone();
        tokio::spawn(async move {
            worker_loop(pool, rx).await;
        });

        Ok(())
    }

    /// Push a new job to the queue
    pub async fn push(&self, snippet_id: i64, content: String, summary: Option<String>, priority: Priority) -> Result<()> {
        // Check if embedding already exists before queueing
        match crate::db::lancedb::has_embedding(snippet_id).await {
            Ok(true) => {
                log::info!("⏭️  Embedding already exists for snippet {}, skipping job queue", snippet_id);
                // Mark any existing pending job as completed
                let _ = sqlx::query(
                    r#"
                    UPDATE job_queue
                    SET status = 'completed', updated_at = ?
                    WHERE snippet_id = ? AND status = 'pending'
                    "#,
                )
                .bind(Utc::now().to_rfc3339())
                .bind(snippet_id)
                .execute(&self.pool)
                .await;
                return Ok(()); // Skip queueing
            }
            Ok(false) => {
                // Embedding doesn't exist, proceed with queueing
            }
            Err(e) => {
                log::warn!("⚠️  Failed to check embedding existence for snippet {}: {}, queueing anyway", snippet_id, e);
                // Continue queueing even if check failed
            }
        }

        let now = Utc::now().to_rfc3339();

        // Check if job already exists for this snippet
        let existing_job: Option<i64> = sqlx::query_scalar(
            r#"
            SELECT id FROM job_queue
            WHERE snippet_id = ? AND status IN ('pending', 'running')
            LIMIT 1
            "#,
        )
        .bind(snippet_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to check for existing job")?;

        if existing_job.is_some() {
            log::info!("⏭️  Job already exists for snippet {}, skipping duplicate", snippet_id);
            return Ok(()); // Skip creating duplicate job
        }

        // Save to database immediately (for crash recovery)
        sqlx::query(
            r#"
            INSERT INTO job_queue (snippet_id, content, priority, status, created_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(snippet_id)
        .bind(&content)
        .bind(priority.clone() as i32)
        .bind(JobStatus::Pending.to_string())
        .bind(&now)
        .execute(&self.pool)
        .await
        .context("Failed to insert job")?;

        // Send to worker if available
        if let Some(ref tx) = self.worker_tx {
            let job = Job {
                id: None,
                snippet_id,
                content,
                summary,
                priority,
                status: JobStatus::Pending,
                created_at: Some(now),
            };
            let _ = tx.send(job).await;
        }

        Ok(())
    }

    /// Recover incomplete jobs on startup
    /// This function never fails - it logs errors but always returns Ok(())
    pub async fn recover_on_startup(&mut self) -> Result<()> {
        log::info!("🔄 [JOB_QUEUE] Starting job recovery on startup...");

        // Reset 'running' jobs to 'pending' (they were interrupted by crash)
        match sqlx::query(
            r#"
            UPDATE job_queue 
            SET status = 'pending', updated_at = ?
            WHERE status = 'running'
            "#,
        )
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await
        {
            Ok(result) => {
                let count = result.rows_affected();
                if count > 0 {
                    log::info!("🔄 [JOB_QUEUE] Reset {} running jobs to pending", count);
                }
            }
            Err(e) => {
                log::warn!("⚠️  [JOB_QUEUE] Failed to reset running jobs: {}", e);
                // Continue anyway - this is not critical
            }
        }

        // Load all pending jobs and reschedule them
        // Try to get summaries from main database if available, but don't fail if it's not
        let rows_result: Result<Vec<_>, _> = async {
            match crate::db::sqlite::get_db_path() {
                Ok(main_db_path) => {
                    // Check if main database exists
                    if !main_db_path.exists() {
                        log::warn!("⚠️  [JOB_QUEUE] Main database not found at: {}, fetching jobs without summaries", main_db_path.display());
                        // Fetch jobs without summaries
                        sqlx::query(
                            r#"
                            SELECT id, snippet_id, content, priority, status, created_at, NULL as summary
                            FROM job_queue
                            WHERE status = 'pending'
                            ORDER BY priority DESC, created_at ASC
                            "#,
                        )
                        .fetch_all(&self.pool)
                        .await
                        .context("Failed to fetch pending jobs")
                    } else {
                        // Try to attach main database and get summaries
                        let main_db_path_abs = main_db_path.canonicalize()
                            .unwrap_or_else(|_| main_db_path.clone());
                        let attach_sql = format!("ATTACH DATABASE '{}' AS main_db", main_db_path_abs.display().to_string().replace('\\', "/"));
                        
                        match sqlx::query(&attach_sql).execute(&self.pool).await {
                            Ok(_) => {
                                log::info!("✅ [JOB_QUEUE] Attached main database for job recovery");
                                
                                // Check if snippets table exists in main_db before querying
                                let table_exists: Option<i64> = sqlx::query_scalar(
                                    r#"
                                    SELECT COUNT(*) FROM main_db.sqlite_master 
                                    WHERE type='table' AND name='snippets'
                                    "#,
                                )
                                .fetch_optional(&self.pool)
                                .await
                                .ok()
                                .flatten();
                                
                                let result = if table_exists == Some(1) {
                                    // Table exists, try to get summaries
                                    sqlx::query(
                                        r#"
                                        SELECT jq.id, jq.snippet_id, jq.content, jq.priority, jq.status, jq.created_at, s.summary
                                        FROM job_queue jq
                                        LEFT JOIN main_db.snippets s ON jq.snippet_id = s.id
                                        WHERE jq.status = 'pending'
                                        ORDER BY jq.priority DESC, jq.created_at ASC
                                        "#,
                                    )
                                    .fetch_all(&self.pool)
                                    .await
                                } else {
                                    log::warn!("⚠️  [JOB_QUEUE] snippets table not found in main_db, fetching jobs without summaries");
                                    // Table doesn't exist, fetch without summaries
                                    sqlx::query(
                                        r#"
                                        SELECT id, snippet_id, content, priority, status, created_at, NULL as summary
                                        FROM job_queue
                                        WHERE status = 'pending'
                                        ORDER BY priority DESC, created_at ASC
                                        "#,
                                    )
                                    .fetch_all(&self.pool)
                                    .await
                                };
                                
                                // Always try to detach, even if query failed
                                let _ = sqlx::query("DETACH DATABASE main_db")
                                    .execute(&self.pool)
                                    .await;
                                
                                result.context("Failed to fetch pending jobs")
                            }
                            Err(e) => {
                                log::warn!("⚠️  [JOB_QUEUE] Failed to attach main database: {}, fetching jobs without summaries", e);
                                // Fetch jobs without summaries as fallback
                                sqlx::query(
                                    r#"
                                    SELECT id, snippet_id, content, priority, status, created_at, NULL as summary
                                    FROM job_queue
                                    WHERE status = 'pending'
                                    ORDER BY priority DESC, created_at ASC
                                    "#,
                                )
                                .fetch_all(&self.pool)
                                .await
                                .context("Failed to fetch pending jobs")
                            }
                        }
                    }
                }
                Err(e) => {
                    log::warn!("⚠️  [JOB_QUEUE] Failed to get main database path: {}, fetching jobs without summaries", e);
                    // Fetch jobs without summaries as fallback
                    sqlx::query(
                        r#"
                        SELECT id, snippet_id, content, priority, status, created_at, NULL as summary
                        FROM job_queue
                        WHERE status = 'pending'
                        ORDER BY priority DESC, created_at ASC
                        "#,
                    )
                    .fetch_all(&self.pool)
                    .await
                    .context("Failed to fetch pending jobs")
                }
            }
        }.await;

        let rows = match rows_result {
            Ok(rows) => rows,
            Err(e) => {
                log::error!("❌ [JOB_QUEUE] Failed to fetch pending jobs: {}", e);
                log::error!("❌ [JOB_QUEUE] Error details: {:?}", e);
                log::warn!("⚠️  [JOB_QUEUE] Continuing without job recovery - this is non-critical");
                // Return empty vec - no jobs to recover
                Vec::new()
            }
        };

        let job_count = rows.len();
        if job_count > 0 {
            log::info!("📋 [JOB_QUEUE] Found {} pending jobs to recover", job_count);
            
            if self.worker_tx.is_none() {
                if let Err(e) = self.start_worker() {
                    log::error!("❌ [JOB_QUEUE] Failed to start worker: {}", e);
                    log::warn!("⚠️  [JOB_QUEUE] Jobs will not be processed until worker is started");
                    return Ok(()); // Don't fail, just return
                }
            }

            for row in rows {
                let job = match (|| -> Result<Job> {
                    Ok(Job {
                        id: Some(row.get(0)),
                        snippet_id: row.get(1),
                        content: row.get(2),
                        priority: match row.get::<i32, _>(3) {
                            0 => Priority::Low,
                            1 => Priority::Normal,
                            _ => Priority::High,
                        },
                        status: JobStatus::Pending,
                        created_at: row.get(5),
                        summary: row.get(6),
                    })
                })() {
                    Ok(job) => job,
                    Err(e) => {
                        log::error!("❌ [JOB_QUEUE] Failed to parse job row: {}", e);
                        continue; // Skip this job
                    }
                };

                // Check if embedding already exists before re-queuing
                match crate::db::lancedb::has_embedding(job.snippet_id).await {
                    Ok(true) => {
                        log::info!("⏭️  [JOB_QUEUE] Embedding already exists for snippet {}, marking job as completed", job.snippet_id);
                        // Mark job as completed since embedding already exists
                        let _ = sqlx::query(
                            r#"
                            UPDATE job_queue
                            SET status = 'completed', updated_at = ?
                            WHERE snippet_id = ?
                            "#,
                        )
                        .bind(Utc::now().to_rfc3339())
                        .bind(job.snippet_id)
                        .execute(&self.pool)
                        .await;
                        continue; // Skip to next job
                    }
                    Ok(false) => {
                        // Embedding doesn't exist, proceed with re-queuing
                    }
                    Err(e) => {
                        log::warn!("⚠️  [JOB_QUEUE] Failed to check embedding for snippet {}: {}, re-queuing anyway", job.snippet_id, e);
                        // Continue re-queuing even if check failed
                    }
                }

                if let Some(ref tx) = self.worker_tx {
                    if let Err(e) = tx.send(job).await {
                        log::error!("❌ [JOB_QUEUE] Failed to send job to worker: {}", e);
                        // Continue with other jobs
                    }
                }
            }
            
            log::info!("✅ [JOB_QUEUE] Successfully recovered {} jobs", job_count);
        } else {
            log::info!("✅ [JOB_QUEUE] No pending jobs to recover");
        }

        Ok(())
    }

    pub fn get_pending_count(&self) -> usize {
        // This would need to be async, so for now return 0
        // In production, use a channel to query the worker
        0
    }

    /// Clean up completed jobs older than the specified number of days
    pub async fn cleanup_old_jobs(&self, days_old: i64) -> Result<usize> {
        let cutoff_date = Utc::now() - chrono::Duration::days(days_old);
        let cutoff_str = cutoff_date.to_rfc3339();

        let result = sqlx::query(
            r#"
            DELETE FROM job_queue
            WHERE (status = 'completed' OR status LIKE 'failed:%')
              AND updated_at < ?
            "#,
        )
        .bind(&cutoff_str)
        .execute(&self.pool)
        .await
        .context("Failed to cleanup old jobs")?;

        let deleted = result.rows_affected() as usize;

        if deleted > 0 {
            log::info!("Cleaned up {} old jobs (older than {} days)", deleted, days_old);
            println!("🧹 Cleaned up {} old jobs", deleted);
        }

        Ok(deleted)
    }
}

impl JobStatus {
    fn to_string(&self) -> String {
        match self {
            JobStatus::Pending => "pending".to_string(),
            JobStatus::Running => "running".to_string(),
            JobStatus::Completed => "completed".to_string(),
            JobStatus::Failed(msg) => format!("failed:{}", msg),
        }
    }
}


/// Background worker that processes embedding jobs with batching
async fn worker_loop(pool: SqlitePool, mut rx: mpsc::Receiver<Job>) {
    use std::time::Duration;

    let embedding_engine = Arc::new(EmbeddingEngine::new());
    let mut batch: Vec<Job> = Vec::new();
    const BATCH_SIZE: usize = 10;
    const BATCH_TIMEOUT_MS: u64 = 100;

    loop {
        // Collect jobs into batch (up to BATCH_SIZE or timeout)
        let timeout = tokio::time::sleep(Duration::from_millis(BATCH_TIMEOUT_MS));
        tokio::pin!(timeout);

        tokio::select! {
            Some(job) = rx.recv() => {
                batch.push(job);

                // Process batch when full
                if batch.len() >= BATCH_SIZE {
                    process_batch(&embedding_engine, &pool, &mut batch).await;
                }
            }
            _ = &mut timeout, if !batch.is_empty() => {
                // Timeout: process accumulated batch
                process_batch(&embedding_engine, &pool, &mut batch).await;
            }
            else => {
                // Channel closed and no pending jobs
                if batch.is_empty() {
                    break;
                } else {
                    // Process final batch
                    process_batch(&embedding_engine, &pool, &mut batch).await;
                    break;
                }
            }
        }
    }
}

/// Process a batch of embedding jobs
async fn process_batch(
    embedding_engine: &Arc<EmbeddingEngine>,
    pool: &SqlitePool,
    batch: &mut Vec<Job>,
) {
    if batch.is_empty() {
        return;
    }

    let batch_size = batch.len();
    println!("🔄 Processing batch of {} embedding jobs", batch_size);

    for mut job in batch.drain(..) {
        // Check if embedding already exists before processing
        match crate::db::lancedb::has_embedding(job.snippet_id).await {
            Ok(true) => {
                log::info!("⏭️  Embedding already exists for snippet {}, skipping job", job.snippet_id);
                println!("⏭️  Embedding already exists for snippet {}, marking as completed", job.snippet_id);
                
                // Mark job as completed since embedding already exists
                let _ = sqlx::query(
                    r#"
                    UPDATE job_queue
                    SET status = 'completed', updated_at = ?
                    WHERE snippet_id = ?
                    "#,
                )
                .bind(Utc::now().to_rfc3339())
                .bind(job.snippet_id)
                .execute(pool)
                .await;
                
                continue; // Skip to next job
            }
            Ok(false) => {
                // Embedding doesn't exist, proceed with processing
            }
            Err(e) => {
                log::warn!("⚠️  Failed to check embedding existence for snippet {}: {}, proceeding anyway", job.snippet_id, e);
                // Continue processing even if check failed
            }
        }

        // Mark job as running
        let _ = sqlx::query(
            r#"
            UPDATE job_queue
            SET status = 'running', updated_at = ?
            WHERE snippet_id = ?
            "#,
        )
        .bind(Utc::now().to_rfc3339())
        .bind(job.snippet_id)
        .execute(pool)
        .await;

        // Combine content and summary for embedding
        // This allows semantic search to match on both full content and summary
        let embed_text = if let Some(ref summary) = job.summary {
            format!("{}\n\n{}", summary, job.content)
        } else {
            job.content.clone()
        };

        // Process embedding
        match embedding_engine.embed(&embed_text).await {
            Ok(embedding) => {
                // Save embedding to LanceDB
                if let Err(e) = crate::db::lancedb::save_embedding(job.snippet_id, embedding).await
                {
                    log::error!(
                        "Failed to save embedding for snippet {}: {}",
                        job.snippet_id,
                        e
                    );
                    job.status = JobStatus::Failed(e.to_string());
                } else {
                    job.status = JobStatus::Completed;
                    println!("✅ Embedding saved for snippet {}", job.snippet_id);

                    // Auto-categorize the snippet after successful embedding
                    println!("🏷️  Auto-categorizing snippet {}", job.snippet_id);
                    log::info!("🏷️  Auto-categorizing snippet {}", job.snippet_id);

                    // CRITICAL: Check snippet type FIRST to determine categorization strategy
                    let snippet_type: Option<String> = sqlx::query_scalar(
                        "SELECT type FROM snippets WHERE id = ?"
                    )
                    .bind(job.snippet_id)
                    .fetch_optional(pool)
                    .await
                    .unwrap_or(None);

                    // For COMMANDS: Use pattern matching ONLY, skip embedding categorization entirely
                    // This prevents commands from matching to bad existing categories like "Docker Build", "Docker Compose Build", etc.
                    if snippet_type.as_deref() == Some("command") {
                        println!("⚡ Command detected - using pattern matching for categorization (skipping embedding)");
                        log::info!("⚡ Command detected for snippet {} - using pattern matching only", job.snippet_id);

                        use crate::inference::categorization;
                        let (category_name, emoji) = if let Some(canonical) = categorization::get_canonical_category_info(&job.content) {
                            println!("🎯 Pattern-matched canonical category: {} {}", canonical.emoji, canonical.name);
                            log::info!("Pattern-matched canonical category '{}' for command snippet {}", canonical.name, job.snippet_id);
                            (canonical.name.to_string(), canonical.emoji.to_string())
                        } else {
                            // For non-canonical commands, use generic "Commands" category
                            println!("🎯 Using generic 'Commands' category for non-canonical command");
                            log::info!("Non-canonical command, using generic 'Commands' category for snippet {}", job.snippet_id);
                            ("Commands".to_string(), "⚡".to_string())
                        };

                        // Jump directly to category creation (skip embedding categorization)
                        // We'll handle this inline here
                        println!("🆕 Finding or creating category: {} {}", emoji, category_name);

                        // Check if category already exists GLOBALLY
                        match crate::db::sqlite::get_categories(None, None).await {
                            Ok(existing_categories) => {
                                let normalize_name = |name: &str| -> String {
                                    name.to_lowercase()
                                        .replace("_", " ")
                                        .replace("-", " ")
                                        .split_whitespace()
                                        .collect::<Vec<_>>()
                                        .join(" ")
                                        .trim()
                                        .to_string()
                                };

                                let normalized_search = normalize_name(&category_name);
                                let existing = existing_categories.iter().find(|c|
                                    normalize_name(&c.name) == normalized_search
                                );

                                let category_id = if let Some(cat) = existing {
                                    println!("✅ Using existing category: {} (ID: {})", cat.name, cat.id);
                                    cat.id
                                } else {
                                    // Create new category (no parent_id for commands to keep them at root level)
                                    match crate::db::sqlite::create_category(
                                        category_name.clone(),
                                        None, // No parent for command categories
                                        Some(emoji.clone())
                                    ).await {
                                        Ok(new_id) => {
                                            println!("✅ Created new category: {} {} (ID: {})", emoji, category_name, new_id);
                                            new_id
                                        }
                                        Err(e) => {
                                            println!("⚠️  Failed to create category: {}", e);
                                            log::error!("Failed to create category: {}", e);
                                            -1
                                        }
                                    }
                                };

                                // Assign snippet to category
                                if category_id > 0 {
                                    if let Err(e) = crate::db::sqlite::assign_snippet_to_category_with_method(
                                        job.snippet_id,
                                        category_id,
                                        0.95, // High confidence for pattern-matched categories
                                        false,
                                        "pattern_match",
                                        None,
                                    ).await {
                                        println!("⚠️  Failed to assign snippet {} to category {}: {}", job.snippet_id, category_id, e);
                                    } else {
                                        println!("✅ Snippet {} assigned to category {} {} via pattern matching", job.snippet_id, emoji, category_name);
                                    }
                                }
                            }
                            Err(e) => {
                                println!("⚠️  Failed to get existing categories: {}", e);
                                log::error!("Failed to get existing categories: {}", e);
                            }
                        }
                    } else {
                        // For NON-COMMANDS: Use embedding-based categorization (faster, more accurate)
                        // For screenshots, use a lower confidence threshold (0.65) since they have good summaries
                        let min_confidence = if snippet_type.as_deref() == Some("screenshot") {
                            log::info!("📸 Using lower confidence threshold (0.65) for screenshot categorization");
                            0.65
                        } else {
                            0.75
                        };

                        println!("🔍 Using embedding similarity categorization (min_confidence: {:.2})...", min_confidence);
                        let categorization_result = crate::embedding::categorization::categorize_snippet(job.snippet_id, min_confidence).await;

                        match categorization_result {
                        Ok(Some((category_id, confidence))) => {
                            println!(
                                "✅ Snippet {} auto-categorized to category {} (confidence: {:.2})",
                                job.snippet_id,
                                category_id,
                                confidence
                            );
                            log::info!(
                                "✅ Snippet {} auto-categorized to category {} via embedding similarity (confidence: {:.2})",
                                job.snippet_id,
                                category_id,
                                confidence
                            );
                            // Assign the snippet to the category using new function
                            if let Err(e) = crate::db::sqlite::assign_snippet_to_category_with_method(
                                job.snippet_id,
                                category_id,
                                confidence as f64,
                                false, // not manual
                                "embedding",
                                None,
                            ).await {
                                println!(
                                    "⚠️  Failed to assign snippet {} to category {}: {}",
                                    job.snippet_id,
                                    category_id,
                                    e
                                );
                                log::warn!(
                                    "Failed to assign snippet {} to category {}: {}",
                                    job.snippet_id,
                                    category_id,
                                    e
                                );
                            } else {
                                println!("✅ Snippet {} assigned to category {} via embedding similarity", job.snippet_id, category_id);
                            }
                        }
                        Ok(None) => {
                            println!(
                                "ℹ️  No suitable category found for snippet {} (confidence < {:.2}) - creating new category from content",
                                job.snippet_id,
                                min_confidence
                            );
                            log::info!(
                                "No suitable category found for snippet {} (confidence < {:.2}) - creating new category",
                                job.snippet_id,
                                min_confidence
                            );

                            // IMPORTANT: For commands, try pattern matching FIRST before keyword suggestion
                            // snippet_type was already queried above, reuse it

                            // For screenshots, prefer using summary (cleaned and meaningful) over raw content
                            // The summary is already processed and contains the actual topic (e.g., "Deploy workflow to Coolify")
                            let content_for_suggestion = if snippet_type.as_deref() == Some("screenshot") {
                                if let Some(ref summary) = job.summary {
                                    // Use summary if it's meaningful (not just "Screenshot" or too short)
                                    if !summary.is_empty() 
                                        && summary.len() > 10 
                                        && !summary.eq_ignore_ascii_case("screenshot")
                                        && !summary.starts_with("Screenshot") {
                                        log::info!("📸 Using summary for screenshot category suggestion: '{}'", summary);
                                        summary.as_str()
                                    } else {
                                        log::debug!("📸 Summary too generic, using content for screenshot category suggestion");
                                        &job.content
                                    }
                                } else {
                                    log::debug!("📸 No summary available, using content for screenshot category suggestion");
                                    &job.content
                                }
                            } else {
                                &job.content
                            };

                            let (category_name, emoji) = if snippet_type.as_deref() == Some("command") {
                                // CRITICAL: Commands MUST use pattern matching ONLY
                                // DO NOT fall back to keyword suggestion or LLM for commands
                                // This prevents hallucination and ensures canonical categories
                                use crate::inference::categorization;
                                if let Some(canonical) = categorization::get_canonical_category_info(&job.content) {
                                    println!("🎯 Pattern-matched canonical category: {} {}", canonical.emoji, canonical.name);
                                    log::info!("Pattern-matched canonical category '{}' for command snippet {}", canonical.name, job.snippet_id);
                                    (canonical.name.to_string(), canonical.emoji.to_string())
                                } else {
                                    // For non-canonical commands, use generic "Commands" category
                                    // This prevents creating fragmented categories like "ls Commands", "cat Commands", etc.
                                    println!("🎯 Using generic 'Commands' category for non-canonical command");
                                    log::info!("Non-canonical command, using generic 'Commands' category for snippet {}", job.snippet_id);
                                    ("Commands".to_string(), "⚡".to_string())
                                }
                            } else {
                                // For non-command content (including screenshots), try LLM first, then fall back to keywords
                                // This prevents generic "Uncategorized" or "Notes" names for novel content
                                // For screenshots with good summaries, LLM will work much better

                                // Try LLM category name suggestion (especially important for screenshots)
                                let llm_result = {
                                    use crate::inference::{categorization, global_llm};

                                    if let Some(llm_manager) = global_llm::get_global_llm() {
                                        if llm_manager.model_exists() {
                                            match llm_manager.get_model() {
                                                Ok(model) => {
                                                    if snippet_type.as_deref() == Some("screenshot") {
                                                        println!("🤖 Using LLM to suggest category name from screenshot summary...");
                                                        log::info!("🤖 Using LLM for screenshot category suggestion (summary: '{}')", content_for_suggestion);
                                                    } else {
                                                        println!("🤖 Using LLM to suggest category name...");
                                                    }
                                                    categorization::suggest_category_name_with_llm(&model, content_for_suggestion).await.ok()
                                                }
                                                Err(e) => {
                                                    log::debug!("Failed to load LLM model: {}", e);
                                                    None
                                                }
                                            }
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                };

                                if let Some((name, emoji)) = llm_result {
                                    println!("✅ LLM suggested category: {} {}", emoji, name);
                                    (name, emoji)
                                } else {
                                    // Fall back to keyword extraction
                                    if snippet_type.as_deref() == Some("screenshot") {
                                        println!("📝 Falling back to keyword extraction for screenshot category name (from summary)");
                                        log::info!("📝 Using keyword extraction for screenshot with content: '{}'", content_for_suggestion);
                                    } else {
                                        println!("📝 Falling back to keyword extraction for category name");
                                    }
                                    use crate::embedding::category_suggestion;
                                    category_suggestion::suggest_category_from_content(content_for_suggestion)
                                }
                            };

                            println!("🆕 Creating new category: {} {}", emoji, category_name);

                            // Get snippet's source_app to determine parent folder
                            let source_app: Option<String> = sqlx::query_scalar(
                                "SELECT source_app FROM snippets WHERE id = ?"
                            )
                            .bind(job.snippet_id)
                            .fetch_optional(pool)
                            .await
                            .ok()
                            .flatten();

                            // Determine parent_id based on app hierarchy
                            let parent_id = if let Some(ref app) = source_app {
                                // Validate app name is not empty before creating folder
                                if app.trim().is_empty() {
                                    log::warn!("Source app is empty, skipping app folder creation");
                                    None
                                } else {
                                    // Get or create app folder
                                    match crate::db::sqlite::get_or_create_app_folder(app).await {
                                        Ok(folder_id) => {
                                            println!("📁 Using app folder '{}' (ID: {})", app, folder_id);
                                            Some(folder_id)
                                        }
                                        Err(e) => {
                                            println!("⚠️  Failed to get/create app folder: {}", e);
                                            log::warn!("Failed to get/create app folder: {}", e);
                                            None // Fall back to root level
                                        }
                                    }
                                }
                            } else {
                                None // No source app, create at root level
                            };

                            // CRITICAL FIX: Check if category already exists GLOBALLY (not just under parent)
                            // This prevents creating duplicate "Docker Commands" under different app folders
                            match crate::db::sqlite::get_categories(None, None).await {
                                Ok(existing_categories) => {
                                    // Normalize category names for comparison (case-insensitive, trim spaces/underscores)
                                    let normalize_name = |name: &str| -> String {
                                        name.to_lowercase()
                                            .replace("_", " ")
                                            .replace("-", " ")
                                            .split_whitespace()
                                            .collect::<Vec<_>>()
                                            .join(" ")
                                            .trim()
                                            .to_string()
                                    };

                                    let normalized_search = normalize_name(&category_name);
                                    let existing = existing_categories.iter().find(|c|
                                        normalize_name(&c.name) == normalized_search
                                    );

                                    let category_id = if let Some(cat) = existing {
                                        println!("✅ Using existing category: {} (ID: {})", cat.name, cat.id);
                                        cat.id
                                    } else {
                                        // RACE CONDITION FIX: Use transaction to prevent duplicate creation
                                        // Between checking and creating, another job might create the same category
                                        // CRITICAL: Use main database pool, not job_queue pool
                                        match crate::db::sqlite::get_pool().await {
                                            Ok(main_pool) => {
                                                match main_pool.begin().await {
                                                    Ok(mut tx) => {
                                                        // Re-check inside transaction to prevent race condition
                                                        let recheck_result: Result<Option<i64>, sqlx::Error> = sqlx::query_scalar(
                                                            "SELECT id FROM categories WHERE LOWER(TRIM(name)) = LOWER(TRIM(?))"
                                                        )
                                                        .bind(&category_name)
                                                        .fetch_optional(&mut *tx)
                                                        .await;

                                                        match recheck_result {
                                                            Ok(Some(existing_id)) => {
                                                                // Category was created by another job, use it
                                                                let _ = tx.commit().await;
                                                                println!("✅ Using category created by concurrent job: {} (ID: {})", category_name, existing_id);
                                                                existing_id
                                                            }
                                                            Ok(None) => {
                                                                // Still doesn't exist, create it inside transaction
                                                                let insert_result: Result<i64, sqlx::Error> = sqlx::query_scalar(
                                                                    "INSERT INTO categories (name, parent_id, emoji) VALUES (?, ?, ?) RETURNING id"
                                                                )
                                                                .bind(&category_name)
                                                                .bind(parent_id)
                                                                .bind(&emoji)
                                                                .fetch_one(&mut *tx)
                                                                .await;

                                                                match insert_result {
                                                                    Ok(new_id) => {
                                                                        match tx.commit().await {
                                                                            Ok(_) => {
                                                                                if let Some(app) = source_app {
                                                                                    println!("✅ Created new category: {} / {} {} (ID: {})", app, emoji, category_name, new_id);
                                                                                } else {
                                                                                    println!("✅ Created new category: {} {} (ID: {})", emoji, category_name, new_id);
                                                                                }
                                                                                new_id
                                                                            }
                                                                            Err(e) => {
                                                                                println!("⚠️  Failed to commit category creation: {}", e);
                                                                                log::error!("Failed to commit category creation: {}", e);
                                                                                -1
                                                                            }
                                                                        }
                                                                    }
                                                                    Err(e) => {
                                                                        let _ = tx.rollback().await;
                                                                        println!("⚠️  Failed to insert category: {}", e);
                                                                        log::error!("Failed to insert category: {}", e);
                                                                        -1
                                                                    }
                                                                }
                                                            }
                                                            Err(e) => {
                                                                let _ = tx.rollback().await;
                                                                println!("⚠️  Failed to check for category in transaction: {}", e);
                                                                log::error!("Failed to check for category in transaction: {}", e);
                                                                -1
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        println!("⚠️  Failed to begin transaction: {}", e);
                                                        log::error!("Failed to begin transaction: {}", e);
                                                        -1
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                println!("⚠️  Failed to get main database pool: {}", e);
                                                log::error!("Failed to get main database pool: {}", e);
                                                -1
                                            }
                                        }
                                    };

                                    // Assign snippet to category if we have a valid ID
                                    if category_id > 0 {
                                        if let Err(e) = crate::db::sqlite::assign_snippet_to_category_with_method(
                                            job.snippet_id,
                                            category_id,
                                            0.9, // High confidence for auto-created categories
                                            false,
                                            "keyword",
                                            None,
                                        ).await {
                                            println!("⚠️  Failed to assign snippet {} to category {}: {}", job.snippet_id, category_id, e);
                                        } else {
                                            println!("✅ Snippet {} auto-assigned to new category {} {}", job.snippet_id, emoji, category_name);
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!("⚠️  Failed to get existing categories: {}", e);
                                    log::error!("Failed to get existing categories: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            println!(
                                "⚠️  Failed to auto-categorize snippet {}: {}",
                                job.snippet_id,
                                e
                            );
                            log::warn!(
                                "Failed to auto-categorize snippet {}: {}",
                                job.snippet_id,
                                e
                            );
                        }
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to embed snippet {}: {}", job.snippet_id, e);
                log::error!("Error details: {:#}", e);
                job.status = JobStatus::Failed(format!("Embedding failed: {}", e));
            }
        }

        // Update job status in database
        let status_str = job.status.to_string();
        let error_msg = match &job.status {
            JobStatus::Failed(msg) => Some(msg.as_str()),
            _ => None,
        };

        let _ = sqlx::query(
            r#"
            UPDATE job_queue
            SET status = ?, error_message = ?, updated_at = ?
            WHERE snippet_id = ?
            "#,
        )
        .bind(&status_str)
        .bind(error_msg)
        .bind(Utc::now().to_rfc3339())
        .bind(job.snippet_id)
        .execute(pool)
        .await;
    }

    println!("✅ Batch of {} jobs completed", batch_size);
}
