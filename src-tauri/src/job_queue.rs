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
    let categories = match crate::db::sqlite::get_categories(None, None).await {
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

    // Get snippet type and source information for content-aware categorization
    let row_result = sqlx::query(
        "SELECT type, website_url, website_title FROM snippets WHERE id = ?"
    )
    .bind(snippet_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    let (content_type, website_url, website_title) = if let Some(row) = row_result {
        (
            row.try_get::<Option<String>, _>(0).ok().flatten(),
            row.try_get::<Option<String>, _>(1).ok().flatten(),
            row.try_get::<Option<String>, _>(2).ok().flatten(),
        )
    } else {
        (None, None, None)
    };

    log::info!("📋 Snippet {} categorization context:", snippet_id);
    log::info!("   Content Type: {:?}", content_type);
    log::info!("   Website URL: {:?}", website_url);
    log::info!("   Website Title: {:?}", website_title);
    log::info!("   Content Preview: {}", content.chars().take(150).collect::<String>());

    // Call LLM categorization with content type and source information
    let decision = match categorization::categorize_with_llm(
        &model, 
        content, 
        &categories, 
        content_type.as_deref(),
        website_url.as_deref(),
        website_title.as_deref(),
    ).await {
        Ok(d) => d,
        Err(e) => {
            log::warn!("LLM categorization failed: {}", e);
            println!("⚠️  LLM categorization failed: {}, falling back to semantic search", e);
            return None;
        }
    };

    println!("🤖 LLM decision: {:?} category '{}' (confidence: {:.2})",
             decision.action, decision.category_name, decision.confidence);
    log::info!("✅ LLM categorization result: {:?}", decision);
    
    // CRITICAL: Final validation check - if category name is still invalid after validation, fall back
    let category_trimmed = decision.category_name.trim();
    let category_lower = category_trimmed.to_lowercase();
    let word_count = category_trimmed.split_whitespace().count();
    let has_letter = category_trimmed.chars().any(|c| c.is_alphabetic());
    let is_all_uppercase = category_trimmed.chars().all(|c| !c.is_lowercase());
    let is_alphanumeric_code = category_trimmed.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '-')
        && category_trimmed.chars().filter(|c| c.is_alphabetic()).count() <= 3
        && category_trimmed.chars().any(|c| c.is_numeric());
    
    let valid_single_words = [
        "documentation", "commands", "code", "notes", "links", "research",
        "news", "articles", "personal", "ideas", "shopping", "social",
    ];
    let is_valid_single_word = word_count == 1 && valid_single_words.contains(&category_lower.as_str());
    
    let is_still_invalid = category_trimmed.len() < 3
        || (is_all_uppercase && category_trimmed.len() < 5 && category_trimmed.len() >= 2)
        || (word_count < 2 && !is_valid_single_word)
        || is_alphanumeric_code
        || !has_letter;
    
    if is_still_invalid {
        log::error!(
            "❌ CRITICAL: LLM returned invalid category name '{}' even after validation - falling back to semantic similarity",
            decision.category_name
        );
        println!("⚠️  LLM returned invalid category name '{}' - falling back to semantic similarity", decision.category_name);
        return None;
    }
    
    // Special logging for "Documentation" category to help debug misclassifications
    let category_lower = decision.category_name.to_lowercase();
    if category_lower.contains("documentation") {
        log::warn!("📄 LLM chose 'Documentation' category - verifying this is correct:");
        log::warn!("   Content type: {:?}", content_type);
        log::warn!("   Website URL: {:?}", website_url);
        log::warn!("   Website Title: {:?}", website_title);
        log::warn!("   Reasoning: {}", decision.reasoning);
        log::warn!("   Content preview: {}", content.chars().take(200).collect::<String>());
        
        // Check for business keywords
        let content_lower = content.to_lowercase();
        let business_indicators = [
            ("case study", content_lower.contains("case study")),
            ("partnership", content_lower.contains("partnership")),
            ("harnesses", content_lower.contains("harnesses")),
            ("transformed", content_lower.contains("transformed")),
            ("discover", content_lower.contains("discover")),
            ("company name", content_lower.split_whitespace().any(|w| {
                w.len() > 2 && w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
            })),
        ];
        
        let found_indicators: Vec<_> = business_indicators.iter()
            .filter(|(_, found)| *found)
            .map(|(name, _)| name)
            .collect();
        
        if !found_indicators.is_empty() {
            log::warn!("   ⚠️  WARNING: Found business indicators: {:?}", found_indicators);
            log::warn!("   This may be incorrectly categorized as Documentation!");
        } else {
            log::info!("   ✓ No obvious business indicators found - Documentation category may be correct");
        }
    }

    // FINAL SAFETY CHECK: Verify UseExisting category actually exists
    // (This should have been caught by validate_category_decision, but double-check here)
    if matches!(decision.action, categorization::CategoryAction::UseExisting) {
        let category_name_lower = decision.category_name.to_lowercase();
        let category_exists = categories
            .iter()
            .any(|c| c.name.to_lowercase() == category_name_lower);
        
        if !category_exists {
            log::error!(
                "CRITICAL: LLM suggested UseExisting for '{}' but validation failed - category doesn't exist! Available categories: {}",
                decision.category_name,
                categories.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ")
            );
            // This should not happen if validate_category_decision worked correctly
            // But if it does, fall back to semantic similarity
            println!("⚠️  LLM validation error: category '{}' doesn't exist, falling back to semantic similarity", decision.category_name);
            return None;
        }
    }

    let reasoning = decision.reasoning.clone();

    // Handle LLM decision
    match decision.action {
        categorization::CategoryAction::UseExisting => {
            // Find the category by name (case-insensitive exact match first)
            let category_name_lower = decision.category_name.to_lowercase();
            if let Some(category) = categories.iter().find(|c| c.name.to_lowercase() == category_name_lower) {
                println!("✅ LLM matched existing category: {} (ID: {})", category.name, category.id);
                Some((category.id, decision.confidence, "llm".to_string(), reasoning))
            } else {
                // Try fuzzy matching when exact match fails
                let mut best_match: Option<(&crate::db::sqlite::Category, f32)> = None;
                for cat in categories.iter() {
                    let similarity = categorization::string_similarity(&decision.category_name, &cat.name);
                    if similarity > 0.80 {
                        // Found a similar category
                        if let Some((_, best_sim)) = best_match {
                            if similarity > best_sim {
                                best_match = Some((cat, similarity));
                            }
                        } else {
                            best_match = Some((cat, similarity));
                        }
                    }
                }
                
                if let Some((category, similarity)) = best_match {
                    println!("✅ LLM fuzzy-matched existing category: {} (ID: {}, similarity: {:.2})", 
                             category.name, category.id, similarity);
                    log::info!("LLM suggested '{}' but matched '{}' via fuzzy matching (similarity: {:.2})", 
                               decision.category_name, category.name, similarity);
                    Some((category.id, decision.confidence, "llm_fuzzy".to_string(), reasoning))
                } else {
                    log::warn!("LLM suggested category '{}' but it doesn't exist (no fuzzy match found)", decision.category_name);
                    None
                }
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

            // Check if category with this name already exists (case-insensitive, normalized)
            match crate::db::sqlite::get_categories(Some(parent_id), None).await {
                Ok(existing) => {
                    // Normalize category name for comparison (lowercase, normalize spaces/underscores)
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
                    
                    let normalized_new = normalize_name(&decision.category_name);
                    
                    // Check for exact match after normalization
                    if let Some(cat) = existing.iter().find(|c| normalize_name(&c.name) == normalized_new) {
                        println!("✅ Category already exists: {} (ID: {})", cat.name, cat.id);
                        log::warn!("⚠️  DUPLICATE PREVENTED: LLM wanted to create '{}' but '{}' already exists (normalized match: '{}')",
                                   decision.category_name, cat.name, normalized_new);
                        Some((cat.id, decision.confidence, "llm".to_string(), reasoning))
                    } else {
                        // Check for fuzzy match (similarity > 0.85)
                        let mut found_duplicate = None;
                        for cat in existing.iter() {
                            let normalized_existing = normalize_name(&cat.name);
                            let similarity = categorization::string_similarity(&normalized_new, &normalized_existing);
                            if similarity > 0.85 && similarity < 1.0 {
                                found_duplicate = Some((cat, similarity));
                                break;
                            }
                        }
                        
                        if let Some((cat, similarity)) = found_duplicate {
                            println!("✅ Similar category already exists: {} (ID: {}, similarity: {:.0}%)", 
                                     cat.name, cat.id, similarity * 100.0);
                            log::warn!("⚠️  SIMILAR CATEGORY PREVENTED: LLM wanted to create '{}' but '{}' already exists ({:.0}% similar)",
                                       decision.category_name, cat.name, similarity * 100.0);
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

                            // IMPORTANT: For commands, try pattern matching FIRST before keyword suggestion
                            // Get snippet type to check if it's a command
                            let snippet_type: Option<String> = sqlx::query_scalar(
                                "SELECT type FROM snippets WHERE id = ?"
                            )
                            .bind(job.snippet_id)
                            .fetch_optional(pool)
                            .await
                            .unwrap_or(None);

                            let (category_name, emoji) = if snippet_type.as_deref() == Some("command") {
                                // Try canonical pattern matching for commands
                                use crate::inference::categorization;
                                if let Some(canonical) = categorization::get_canonical_category_info(&job.content) {
                                    println!("🎯 Pattern-matched canonical category: {} {}", canonical.emoji, canonical.name);
                                    log::info!("Pattern-matched canonical category '{}' for command snippet {}", canonical.name, job.snippet_id);
                                    (canonical.name.to_string(), canonical.emoji.to_string())
                                } else {
                                    // Fall back to keyword suggestion for non-canonical commands
                                    use crate::embedding::category_suggestion;
                                    category_suggestion::suggest_category_from_content(&job.content)
                                }
                            } else {
                                // For non-command content, use keyword suggestion
                                use crate::embedding::category_suggestion;
                                category_suggestion::suggest_category_from_content(&job.content)
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

                            // Check if category already exists with this name under the same parent
                            match crate::db::sqlite::get_categories(Some(parent_id), None).await {
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
