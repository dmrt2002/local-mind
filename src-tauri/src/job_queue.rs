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
        let now = Utc::now().to_rfc3339();

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
    pub async fn recover_on_startup(&mut self) -> Result<()> {
        // Reset 'running' jobs to 'pending' (they were interrupted by crash)
        sqlx::query(
            r#"
            UPDATE job_queue 
            SET status = 'pending', updated_at = ?
            WHERE status = 'running'
            "#,
        )
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await
        .context("Failed to reset running jobs")?;

        // Load all pending jobs and reschedule them
        // We need to attach the main database to access summaries
        let main_db_path = crate::db::sqlite::get_db_path()
            .context("Failed to get main database path")?;

        // Attach main database
        sqlx::query(&format!("ATTACH DATABASE '{}' AS main_db", main_db_path.display()))
            .execute(&self.pool)
            .await
            .context("Failed to attach main database")?;

        let rows = sqlx::query(
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
        .context("Failed to fetch pending jobs")?;

        // Detach main database
        sqlx::query("DETACH DATABASE main_db")
            .execute(&self.pool)
            .await
            .context("Failed to detach main database")?;

        if !rows.is_empty() && self.worker_tx.is_none() {
            self.start_worker()?;
        }

        for row in rows {
            let job = Job {
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
            };

            if let Some(ref tx) = self.worker_tx {
                let _ = tx.send(job).await;
            }
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

/// Try to categorize snippet using LLM, returns (category_id, confidence, method_name, reasoning) if successful
async fn try_llm_categorization(snippet_id: i64, content: &str, pool: &SqlitePool) -> Option<(i64, f32, String, String)> {
    use crate::inference::{categorization, global_llm};

    // Get global LLM manager
    let llm_manager = match global_llm::get_global_llm() {
        Some(manager) => manager,
        None => {
            log::debug!("LLM not available for categorization");
            return None;
        }
    };

    // Check if model exists
    if !llm_manager.model_exists() {
        log::debug!("LLM model file not found");
        return None;
    }

    // Get existing categories (pass None to get all categories)
    let categories = match crate::db::sqlite::get_categories(None).await {
        Ok(cats) => cats,
        Err(e) => {
            log::warn!("Failed to get categories for LLM: {}", e);
            return None;
        }
    };

    // Get LLM model (lazy loading)
    let model = match llm_manager.get_model() {
        Ok(m) => m,
        Err(e) => {
            log::warn!("Failed to load LLM model: {}", e);
            return None;
        }
    };

    println!("🤖 Using LLM for smart categorization...");
    log::info!("🤖 Using LLM for categorization of snippet {}", snippet_id);

    // Call LLM categorization
    let decision = match categorization::categorize_with_llm(&model, content, &categories).await {
        Ok(d) => d,
        Err(e) => {
            log::warn!("LLM categorization failed: {}", e);
            println!("⚠️  LLM categorization failed: {}, falling back to semantic search", e);
            return None;
        }
    };

    println!("🤖 LLM decision: {:?} category '{}' (confidence: {:.2})",
             decision.action, decision.category_name, decision.confidence);
    log::info!("LLM categorization result: {:?}", decision);

    let reasoning = decision.reasoning.clone();

    // Handle LLM decision
    match decision.action {
        categorization::CategoryAction::UseExisting => {
            // Find the category by name (case-insensitive)
            let category_name_lower = decision.category_name.to_lowercase();
            if let Some(category) = categories.iter().find(|c| c.name.to_lowercase() == category_name_lower) {
                println!("✅ LLM matched existing category: {} (ID: {})", category.name, category.id);
                Some((category.id, decision.confidence, "llm".to_string(), reasoning))
            } else {
                log::warn!("LLM suggested category '{}' but it doesn't exist", decision.category_name);
                None
            }
        }
        categorization::CategoryAction::CreateNew => {
            // Create new category with LLM-suggested name and emoji
            println!("🆕 LLM suggests creating new category: {} {}",
                     decision.emoji.as_deref().unwrap_or("📁"), decision.category_name);

            // Get snippet's source_app for hierarchy
            let source_app: Option<String> = sqlx::query_scalar(
                "SELECT source_app FROM snippets WHERE id = ?"
            )
            .bind(snippet_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();

            // Get or create app folder
            let parent_id = if let Some(ref app) = source_app {
                match crate::db::sqlite::get_or_create_app_folder(app).await {
                    Ok(folder_id) => Some(folder_id),
                    Err(e) => {
                        log::warn!("Failed to get/create app folder: {}", e);
                        None
                    }
                }
            } else {
                None
            };

            // Check if category with this name already exists (case-insensitive)
            match crate::db::sqlite::get_categories(Some(parent_id)).await {
                Ok(existing) => {
                    let category_name_lower = decision.category_name.to_lowercase();
                    if let Some(cat) = existing.iter().find(|c| c.name.to_lowercase() == category_name_lower) {
                        println!("✅ Category already exists: {} (ID: {})", cat.name, cat.id);
                        log::info!("LLM wanted to create '{}' but '{}' already exists (case-insensitive match)",
                                   decision.category_name, cat.name);
                        Some((cat.id, decision.confidence, "llm".to_string(), reasoning))
                    } else {
                        // Create new category
                        match crate::db::sqlite::create_category(
                            decision.category_name.clone(),
                            parent_id,
                            decision.emoji.clone()
                        ).await {
                            Ok(new_id) => {
                                println!("✅ Created new category (LLM): {} {} (ID: {})",
                                         decision.emoji.as_deref().unwrap_or("📁"),
                                         decision.category_name,
                                         new_id);
                                Some((new_id, decision.confidence, "llm".to_string(), reasoning))
                            }
                            Err(e) => {
                                log::error!("Failed to create LLM-suggested category: {}", e);
                                None
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("Failed to check existing categories: {}", e);
                    None
                }
            }
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

                    // Try LLM categorization first
                    let llm_result = try_llm_categorization(job.snippet_id, &job.content, &pool).await;

                    // Check if LLM categorization succeeded
                    if let Some((category_id, confidence, method, reasoning)) = llm_result {
                        println!("✅ LLM categorization successful: category {} (confidence: {:.2})", category_id, confidence);

                        // Use new function that tracks method and reasoning
                        if let Err(e) = crate::db::sqlite::assign_snippet_to_category_with_method(
                            job.snippet_id,
                            category_id,
                            confidence as f64,
                            false, // not manual
                            &method,
                            Some(reasoning),
                        ).await {
                            println!("⚠️  Failed to assign snippet {} to category {}: {}", job.snippet_id, category_id, e);
                            log::warn!("Failed to assign snippet {} to category {}: {}", job.snippet_id, category_id, e);
                        } else {
                            println!("✅ Snippet {} assigned to category {} via LLM", job.snippet_id, category_id);
                        }
                    } else {
                        // Fall back to semantic similarity
                        println!("🔍 Falling back to semantic similarity categorization...");
                        let categorization_result = crate::embedding::categorization::categorize_snippet(job.snippet_id, 0.75).await;

                        match categorization_result {
                            Ok(Some((category_id, confidence))) => {
                                println!(
                                    "✅ Snippet {} auto-categorized to category {} (confidence: {:.2})",
                                    job.snippet_id,
                                    category_id,
                                    confidence
                                );
                                log::info!(
                                    "✅ Snippet {} auto-categorized to category {} (confidence: {:.2})",
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
                                println!("✅ Snippet {} assigned to category {}", job.snippet_id, category_id);
                            }
                        }
                        Ok(None) => {
                            println!(
                                "ℹ️  No suitable category found for snippet {} - creating new category from content",
                                job.snippet_id
                            );
                            log::info!(
                                "No suitable category found for snippet {} - creating new category",
                                job.snippet_id
                            );

                            // Auto-create category based on content
                            use crate::embedding::category_suggestion;
                            let (category_name, emoji) = category_suggestion::suggest_category_from_content(&job.content);

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
                            } else {
                                None // No source app, create at root level
                            };

                            // Check if category already exists with this name under the same parent
                            match crate::db::sqlite::get_categories(Some(parent_id)).await {
                                Ok(existing_categories) => {
                                    let existing = existing_categories.iter().find(|c| c.name == category_name);

                                    let category_id = if let Some(cat) = existing {
                                        println!("✅ Using existing category: {} (ID: {})", category_name, cat.id);
                                        cat.id
                                    } else {
                                        // Create new category under app folder
                                        match crate::db::sqlite::create_category(
                                            category_name.clone(),
                                            parent_id,
                                            Some(emoji.clone())
                                        ).await {
                                            Ok(new_id) => {
                                                if let Some(app) = source_app {
                                                    println!("✅ Created new category: {} / {} {} (ID: {})", app, emoji, category_name, new_id);
                                                } else {
                                                    println!("✅ Created new category: {} {} (ID: {})", emoji, category_name, new_id);
                                                }
                                                new_id
                                            }
                                            Err(e) => {
                                                println!("⚠️  Failed to create category: {}", e);
                                                log::error!("Failed to create category: {}", e);
                                                -1 // Invalid ID, will skip assignment
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
