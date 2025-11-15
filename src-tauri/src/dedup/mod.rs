use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateGroup {
    pub hash: String,
    pub snippets: Vec<DuplicateSnippet>,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateSnippet {
    pub id: i64,
    pub content: String,
    pub created_at: String,
    pub source_app: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DuplicateStats {
    pub total_snippets: i64,
    pub unique_snippets: i64,
    pub duplicate_snippets: i64,
    pub duplicate_groups: i64,
    pub duplicate_rate: f64,
}

/// Calculate SHA-256 hash of content
pub fn calculate_content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.trim().as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Calculate SHA-256 hash of image file bytes
pub fn calculate_image_hash(image_bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(image_bytes);
    format!("{:x}", hasher.finalize())
}

/// Add content_hash column to snippets table if it doesn't exist
pub async fn migrate_add_content_hash(pool: &SqlitePool) -> Result<()> {
    // Check if column exists
    let column_exists: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM pragma_table_info('snippets') WHERE name = 'content_hash'"
    )
    .fetch_one(pool)
    .await?;

    if !column_exists {
        log::info!("Adding content_hash column to snippets table");
        sqlx::query("ALTER TABLE snippets ADD COLUMN content_hash TEXT")
            .execute(pool)
            .await
            .context("Failed to add content_hash column")?;

        // Create index
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_content_hash ON snippets(content_hash)")
            .execute(pool)
            .await
            .context("Failed to create content_hash index")?;

        log::info!("✅ Added content_hash column and index");
    }

    Ok(())
}

/// Calculate and update hash for a single snippet
pub async fn update_snippet_hash(pool: &SqlitePool, snippet_id: i64, content: &str) -> Result<()> {
    let hash = calculate_content_hash(content);

    sqlx::query("UPDATE snippets SET content_hash = ? WHERE id = ?")
        .bind(&hash)
        .bind(snippet_id)
        .execute(pool)
        .await
        .context("Failed to update snippet hash")?;

    Ok(())
}

/// Calculate and update hashes for all snippets that don't have one
pub async fn update_all_hashes(pool: &SqlitePool) -> Result<usize> {
    // Ensure column exists
    migrate_add_content_hash(pool).await?;

    // Fetch all snippets without hashes, including type and working_directory
    let snippets: Vec<(i64, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, content, type, working_directory FROM snippets WHERE content_hash IS NULL"
    )
    .fetch_all(pool)
    .await
    .context("Failed to fetch snippets without hashes")?;

    let count = snippets.len();

    for (id, content, snippet_type, working_directory) in snippets {
        // Calculate hash based on snippet type
        let hash = match snippet_type.as_deref() {
            Some("command") => {
                // For commands, hash content + working directory
                let unique_content = format!("{}:{}", content, working_directory.unwrap_or_default());
                calculate_content_hash(&unique_content)
            }
            Some("screenshot") => {
                // Screenshots use image hash, skip here
                continue;
            }
            _ => {
                // For text and other types, hash just the content
                calculate_content_hash(&content)
            }
        };

        sqlx::query("UPDATE snippets SET content_hash = ? WHERE id = ?")
            .bind(&hash)
            .bind(id)
            .execute(pool)
            .await?;
    }

    log::info!("Updated hashes for {} snippets", count);
    Ok(count)
}

/// Recalculate and fix ALL hashes (including existing ones) for commands
/// This is needed to fix corrupted hashes from the old update_all_hashes function
pub async fn recalculate_command_hashes(pool: &SqlitePool) -> Result<usize> {
    log::info!("Recalculating command hashes to fix corrupted data...");

    // Fetch all commands
    let commands: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT id, content, working_directory FROM snippets WHERE type = 'command'"
    )
    .fetch_all(pool)
    .await
    .context("Failed to fetch commands")?;

    let count = commands.len();

    for (id, content, working_directory) in commands {
        // Calculate correct hash: content + working_directory
        let unique_content = format!("{}:{}", content, working_directory);
        let hash = calculate_content_hash(&unique_content);

        sqlx::query("UPDATE snippets SET content_hash = ? WHERE id = ?")
            .bind(&hash)
            .bind(id)
            .execute(pool)
            .await?;
    }

    log::info!("✅ Recalculated hashes for {} commands", count);
    Ok(count)
}

/// Check if a snippet with the same content hash exists
pub async fn find_duplicate(pool: &SqlitePool, content: &str) -> Result<Option<DuplicateSnippet>> {
    let hash = calculate_content_hash(content);

    let result: Option<(i64, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, content, created_at, source_app FROM snippets WHERE content_hash = ? LIMIT 1"
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await
    .context("Failed to check for duplicate")?;

    Ok(result.map(|(id, content, created_at, source_app)| DuplicateSnippet {
        id,
        content,
        created_at,
        source_app,
    }))
}

/// Check if a screenshot with the same hash exists
pub async fn find_duplicate_screenshot(pool: &SqlitePool, hash: &str) -> Result<Option<i64>> {
    let result: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM snippets WHERE type = 'screenshot' AND content_hash = ? LIMIT 1"
    )
    .bind(hash)
    .fetch_optional(pool)
    .await
    .context("Failed to check for duplicate screenshot")?;

    Ok(result)
}

/// Check if a command with the same content hash exists
/// For commands, the hash is based on content + working_directory
pub async fn find_duplicate_command(pool: &SqlitePool, content: &str, working_dir: &str) -> Result<Option<i64>> {
    let unique_content = format!("{}:{}", content, working_dir);
    let hash = calculate_content_hash(&unique_content);

    let result: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM snippets WHERE type = 'command' AND content_hash = ? LIMIT 1"
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await
    .context("Failed to check for duplicate command")?;

    Ok(result)
}

/// Find all duplicate groups (snippets with same content hash)
pub async fn find_all_duplicates(pool: &SqlitePool) -> Result<Vec<DuplicateGroup>> {
    // Ensure all snippets have hashes
    update_all_hashes(pool).await?;

    // Find all hashes that have more than one snippet
    let duplicate_hashes: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT content_hash, COUNT(*) as count
        FROM snippets
        WHERE content_hash IS NOT NULL
        GROUP BY content_hash
        HAVING COUNT(*) > 1
        ORDER BY count DESC
        "#
    )
    .fetch_all(pool)
    .await
    .context("Failed to find duplicate hashes")?;

    let mut groups = Vec::new();

    for (hash, count) in duplicate_hashes {
        let snippets: Vec<(i64, String, String, Option<String>)> = sqlx::query_as(
            "SELECT id, content, created_at, source_app FROM snippets WHERE content_hash = ? ORDER BY created_at"
        )
        .bind(&hash)
        .fetch_all(pool)
        .await?;

        let duplicate_snippets: Vec<DuplicateSnippet> = snippets
            .into_iter()
            .map(|(id, content, created_at, source_app)| DuplicateSnippet {
                id,
                content,
                created_at,
                source_app,
            })
            .collect();

        groups.push(DuplicateGroup {
            hash,
            snippets: duplicate_snippets,
            count: count as usize,
        });
    }

    log::info!("Found {} duplicate groups", groups.len());
    Ok(groups)
}

/// Get duplicate statistics
pub async fn get_duplicate_stats(pool: &SqlitePool) -> Result<DuplicateStats> {
    // Ensure all snippets have hashes
    update_all_hashes(pool).await?;

    let total_snippets: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM snippets")
        .fetch_one(pool)
        .await?;

    let unique_snippets: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT content_hash) FROM snippets WHERE content_hash IS NOT NULL"
    )
    .fetch_one(pool)
    .await?;

    let duplicate_groups: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM (
            SELECT content_hash
            FROM snippets
            WHERE content_hash IS NOT NULL
            GROUP BY content_hash
            HAVING COUNT(*) > 1
        )
        "#
    )
    .fetch_one(pool)
    .await?;

    let duplicate_snippets = total_snippets - unique_snippets;
    let duplicate_rate = if total_snippets > 0 {
        (duplicate_snippets as f64) / (total_snippets as f64)
    } else {
        0.0
    };

    Ok(DuplicateStats {
        total_snippets,
        unique_snippets,
        duplicate_snippets,
        duplicate_groups,
        duplicate_rate,
    })
}

/// Delete a snippet (used for deduplication)
pub async fn delete_duplicate(pool: &SqlitePool, snippet_id: i64) -> Result<bool> {
    let result = sqlx::query("DELETE FROM snippets WHERE id = ?")
        .bind(snippet_id)
        .execute(pool)
        .await
        .context("Failed to delete duplicate snippet")?;

    Ok(result.rows_affected() > 0)
}

/// Merge duplicates - keep the oldest one, delete the rest
pub async fn merge_duplicates(pool: &SqlitePool, hash: &str) -> Result<usize> {
    // Get all snippets with this hash, ordered by creation date (oldest first)
    let snippets: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM snippets WHERE content_hash = ? ORDER BY created_at ASC"
    )
    .bind(hash)
    .fetch_all(pool)
    .await?;

    if snippets.len() <= 1 {
        return Ok(0);
    }

    // Keep the first (oldest), delete the rest
    let to_delete = &snippets[1..];
    let mut deleted = 0;

    for &id in to_delete {
        if delete_duplicate(pool, id).await? {
            deleted += 1;
        }
    }

    log::info!("Merged {} duplicates for hash {}", deleted, hash);
    Ok(deleted)
}
