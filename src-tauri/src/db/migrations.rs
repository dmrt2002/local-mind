use anyhow::{Context, Result};
use log;
use sqlx::{sqlite::SqlitePool, Row};

/// Current database schema version
const CURRENT_VERSION: i32 = 20;

/// Migration definitions
#[derive(Debug)]
struct Migration {
    version: i32,
    name: &'static str,
    up: &'static str,
}

/// All migrations in order
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_schema",
        up: r#"
            CREATE TABLE IF NOT EXISTS snippets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL,
                source_app TEXT,
                metadata TEXT
            );
            
            CREATE VIRTUAL TABLE IF NOT EXISTS snippets_fts USING fts5(
                content,
                tokenize='porter'
            );
            
            CREATE TRIGGER IF NOT EXISTS snippets_fts_insert AFTER INSERT ON snippets BEGIN
                INSERT INTO snippets_fts(rowid, content) VALUES (new.id, new.content);
            END;
            
            CREATE TRIGGER IF NOT EXISTS snippets_fts_delete AFTER DELETE ON snippets BEGIN
                DELETE FROM snippets_fts WHERE rowid = old.id;
            END;
            
            CREATE TRIGGER IF NOT EXISTS snippets_fts_update AFTER UPDATE ON snippets BEGIN
                DELETE FROM snippets_fts WHERE rowid = old.id;
                INSERT INTO snippets_fts(rowid, content) VALUES (new.id, new.content);
            END;
        "#,
    },
    Migration {
        version: 2,
        name: "fix_fts5_triggers",
        up: r#"
            -- Drop old triggers if they exist
            DROP TRIGGER IF EXISTS snippets_fts_insert;
            DROP TRIGGER IF EXISTS snippets_fts_delete;
            DROP TRIGGER IF EXISTS snippets_fts_update;
            
            -- Drop old FTS5 table if it has incorrect schema
            -- Note: We check if it exists with external content and drop it
            DROP TABLE IF EXISTS snippets_fts;
            
            -- Recreate FTS5 table with correct schema (no external content)
            CREATE VIRTUAL TABLE IF NOT EXISTS snippets_fts USING fts5(
                content,
                tokenize='porter'
            );
            
            -- Migrate existing data
            INSERT INTO snippets_fts(rowid, content) 
            SELECT id, content FROM snippets
            WHERE NOT EXISTS (SELECT 1 FROM snippets_fts WHERE rowid = snippets.id);
            
            -- Recreate triggers
            CREATE TRIGGER IF NOT EXISTS snippets_fts_insert AFTER INSERT ON snippets BEGIN
                INSERT INTO snippets_fts(rowid, content) VALUES (new.id, new.content);
            END;
            
            CREATE TRIGGER IF NOT EXISTS snippets_fts_delete AFTER DELETE ON snippets BEGIN
                DELETE FROM snippets_fts WHERE rowid = old.id;
            END;
            
            CREATE TRIGGER IF NOT EXISTS snippets_fts_update AFTER UPDATE ON snippets BEGIN
                DELETE FROM snippets_fts WHERE rowid = old.id;
                INSERT INTO snippets_fts(rowid, content) VALUES (new.id, new.content);
            END;
        "#,
    },
    Migration {
        version: 3,
        name: "add_categorization_tables",
        up: r#"
            -- Categories table for organizing snippets
            CREATE TABLE IF NOT EXISTS categories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                parent_id INTEGER NULL,
                emoji TEXT DEFAULT '📁',
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (parent_id) REFERENCES categories(id) ON DELETE CASCADE
            );

            -- Index for faster category hierarchy queries
            CREATE INDEX IF NOT EXISTS idx_categories_parent
                ON categories(parent_id);

            -- Snippet to category mapping (one snippet = one category)
            CREATE TABLE IF NOT EXISTS snippet_categories (
                snippet_id INTEGER PRIMARY KEY,
                category_id INTEGER NOT NULL,
                confidence REAL DEFAULT 1.0,
                is_manual BOOLEAN DEFAULT 0,
                assigned_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (snippet_id) REFERENCES snippets(id) ON DELETE CASCADE,
                FOREIGN KEY (category_id) REFERENCES categories(id) ON DELETE SET NULL
            );

            -- Index for faster category->snippets queries
            CREATE INDEX IF NOT EXISTS idx_snippet_categories_category
                ON snippet_categories(category_id);

            -- Index for filtering by assignment type (manual vs auto)
            CREATE INDEX IF NOT EXISTS idx_snippet_categories_manual
                ON snippet_categories(is_manual);
        "#,
    },
    Migration {
        version: 4,
        name: "add_snippet_summary",
        up: r#"
            -- Add summary field to snippets table
            ALTER TABLE snippets ADD COLUMN summary TEXT;

            -- Generate summaries for existing snippets (first 50 chars)
            UPDATE snippets
            SET summary = SUBSTR(
                REPLACE(REPLACE(REPLACE(content, CHAR(10), ' '), CHAR(13), ' '), '  ', ' '),
                1,
                50
            )
            WHERE summary IS NULL;
        "#,
    },
    Migration {
        version: 5,
        name: "add_app_hierarchy",
        up: r#"
            -- Add is_app_folder flag to distinguish app folders from regular categories
            ALTER TABLE categories ADD COLUMN is_app_folder BOOLEAN DEFAULT 0;

            -- Add app_name to track which app this folder represents
            ALTER TABLE categories ADD COLUMN app_name TEXT;

            -- Create unique index on app folders (only one folder per app)
            CREATE UNIQUE INDEX IF NOT EXISTS idx_categories_app_folder
                ON categories(app_name) WHERE is_app_folder = 1 AND app_name IS NOT NULL;
        "#,
    },
    Migration {
        version: 6,
        name: "optimize_category_queries",
        up: r#"
            -- Drop old single-column index on category_id (will be replaced by composite index)
            DROP INDEX IF EXISTS idx_snippet_categories_category;

            -- Create composite index for efficient category filtering + sorting
            -- This enables the database to use a single index for both WHERE and ORDER BY
            CREATE INDEX IF NOT EXISTS idx_snippet_categories_category_time
                ON snippet_categories(category_id, assigned_at DESC);

            -- Add index on snippet_id for faster joins (if not already present)
            CREATE INDEX IF NOT EXISTS idx_snippet_categories_snippet
                ON snippet_categories(snippet_id);
        "#,
    },
    Migration {
        version: 7,
        name: "add_summary_to_fts",
        up: r#"
            -- Drop old triggers
            DROP TRIGGER IF EXISTS snippets_fts_insert;
            DROP TRIGGER IF EXISTS snippets_fts_delete;
            DROP TRIGGER IF EXISTS snippets_fts_update;

            -- Drop old FTS5 table
            DROP TABLE IF EXISTS snippets_fts;

            -- Recreate FTS5 table with both content and summary columns
            -- This allows searching both the full content and the summary
            CREATE VIRTUAL TABLE IF NOT EXISTS snippets_fts USING fts5(
                content,
                summary,
                tokenize='porter'
            );

            -- Migrate existing data - include both content and summary
            INSERT INTO snippets_fts(rowid, content, summary)
            SELECT id, content, COALESCE(summary, '') FROM snippets;

            -- Recreate triggers to maintain both content and summary
            CREATE TRIGGER IF NOT EXISTS snippets_fts_insert AFTER INSERT ON snippets BEGIN
                INSERT INTO snippets_fts(rowid, content, summary)
                VALUES (new.id, new.content, COALESCE(new.summary, ''));
            END;

            CREATE TRIGGER IF NOT EXISTS snippets_fts_delete AFTER DELETE ON snippets BEGIN
                DELETE FROM snippets_fts WHERE rowid = old.id;
            END;

            CREATE TRIGGER IF NOT EXISTS snippets_fts_update AFTER UPDATE ON snippets BEGIN
                DELETE FROM snippets_fts WHERE rowid = old.id;
                INSERT INTO snippets_fts(rowid, content, summary)
                VALUES (new.id, new.content, COALESCE(new.summary, ''));
            END;
        "#,
    },
    Migration {
        version: 8,
        name: "add_settings_table",
        up: r#"
            -- Settings table for app configuration
            CREATE TABLE IF NOT EXISTS settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                theme TEXT NOT NULL DEFAULT 'light',
                enable_semantic_search BOOLEAN NOT NULL DEFAULT 1,
                enable_search_analytics BOOLEAN NOT NULL DEFAULT 1,
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            -- Insert default settings
            INSERT OR IGNORE INTO settings (id, theme, enable_semantic_search, enable_search_analytics)
            VALUES (1, 'light', 1, 1);
        "#,
    },
    Migration {
        version: 9,
        name: "add_snippet_editing_support",
        up: r#"
            -- Add updated_at column to snippets table for tracking edits
            ALTER TABLE snippets ADD COLUMN updated_at TEXT;

            -- Set updated_at to created_at for existing snippets
            UPDATE snippets SET updated_at = created_at WHERE updated_at IS NULL;

            -- Create version history table for snippet edits
            CREATE TABLE IF NOT EXISTS snippet_versions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                snippet_id INTEGER NOT NULL,
                version_number INTEGER NOT NULL,
                content TEXT NOT NULL,
                summary TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (snippet_id) REFERENCES snippets(id) ON DELETE CASCADE,
                UNIQUE(snippet_id, version_number)
            );

            -- Index for faster version queries
            CREATE INDEX IF NOT EXISTS idx_snippet_versions_snippet
                ON snippet_versions(snippet_id, version_number DESC);
        "#,
    },
    Migration {
        version: 10,
        name: "add_export_history",
        up: r#"
            -- Track export operations for audit and recovery
            CREATE TABLE IF NOT EXISTS export_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                format TEXT NOT NULL, -- 'json' or 'markdown'
                file_path TEXT NOT NULL,
                item_count INTEGER NOT NULL,
                file_size_bytes INTEGER,
                exported_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            -- Index for faster queries by date
            CREATE INDEX IF NOT EXISTS idx_export_history_date
                ON export_history(exported_at DESC);
        "#,
    },
    Migration {
        version: 11,
        name: "add_categorization_method_tracking",
        up: r#"
            -- Add categorization_method to track how category was assigned
            -- Values: 'llm', 'embedding', 'keyword', 'manual'
            ALTER TABLE snippet_categories ADD COLUMN categorization_method TEXT DEFAULT 'manual';

            -- Add optional llm_reasoning to store LLM's decision reasoning
            ALTER TABLE snippet_categories ADD COLUMN llm_reasoning TEXT;

            -- Update existing auto-categorized snippets (is_manual = 0) to 'embedding' method
            UPDATE snippet_categories
            SET categorization_method = 'embedding'
            WHERE is_manual = 0;

            -- Create index for querying by categorization method
            CREATE INDEX IF NOT EXISTS idx_snippet_categories_method
                ON snippet_categories(categorization_method);
        "#,
    },
    Migration {
        version: 12,
        name: "add_keyboard_shortcuts_and_background_mode",
        up: r#"
            -- Add keyboard shortcut settings and background mode preference
            ALTER TABLE settings ADD COLUMN shortcut_save TEXT DEFAULT 'Alt+Shift+C';
            ALTER TABLE settings ADD COLUMN shortcut_search TEXT DEFAULT 'Alt+Shift+F';
            ALTER TABLE settings ADD COLUMN run_in_background BOOLEAN DEFAULT 1;
        "#,
    },
    Migration {
        version: 13,
        name: "add_content_types_and_monitoring_support",
        up: r#"
            -- Add content type system to snippets table
            ALTER TABLE snippets ADD COLUMN type TEXT NOT NULL DEFAULT 'text';
            -- Values: 'text' (clipboard snippets) | 'command' (terminal) | 'screenshot'

            -- Add file path for screenshots
            ALTER TABLE snippets ADD COLUMN file_path TEXT NULL;

            -- Add working directory for commands
            ALTER TABLE snippets ADD COLUMN working_directory TEXT NULL;

            -- Add exit code for commands (0=success, non-zero=error)
            ALTER TABLE snippets ADD COLUMN exit_code INTEGER NULL;

            -- Add website URL for screenshots captured from browsers
            ALTER TABLE snippets ADD COLUMN website_url TEXT NULL;

            -- Add website title for screenshots captured from browsers
            ALTER TABLE snippets ADD COLUMN website_title TEXT NULL;

            -- Performance indices for new columns
            CREATE INDEX IF NOT EXISTS idx_snippets_type ON snippets(type);
            CREATE INDEX IF NOT EXISTS idx_snippets_working_directory ON snippets(working_directory);
            CREATE INDEX IF NOT EXISTS idx_snippets_website_url ON snippets(website_url);

            -- Add terminal monitoring settings
            ALTER TABLE settings ADD COLUMN terminal_monitoring_enabled BOOLEAN DEFAULT 0;
            ALTER TABLE settings ADD COLUMN terminal_blocklist TEXT DEFAULT 'ls,cd,pwd,clear,exit,history,echo,cat,which,type';
            ALTER TABLE settings ADD COLUMN terminal_allowlist TEXT DEFAULT 'docker,git,kubectl,npm,cargo,python,ffmpeg,curl,aws,gcloud,az,terraform,ansible,ssh,scp,rsync';
            ALTER TABLE settings ADD COLUMN terminal_min_length INTEGER DEFAULT 60;
            ALTER TABLE settings ADD COLUMN shell_type TEXT DEFAULT 'zsh';

            -- Add screenshot monitoring settings
            ALTER TABLE settings ADD COLUMN screenshot_monitoring_enabled BOOLEAN DEFAULT 0;
            ALTER TABLE settings ADD COLUMN screenshot_directory TEXT DEFAULT '';
            ALTER TABLE settings ADD COLUMN screenshot_ocr_enabled BOOLEAN DEFAULT 1;
            ALTER TABLE settings ADD COLUMN screenshot_caption_enabled BOOLEAN DEFAULT 1;
            ALTER TABLE settings ADD COLUMN visual_search_enabled BOOLEAN DEFAULT 0;
        "#,
    },
    Migration {
        version: 14,
        name: "add_screenshot_deduplication_index",
        up: r#"
            -- Create unique index on content_hash for screenshots to prevent duplicates
            -- This allows multiple text snippets with same hash but only one screenshot per hash
            CREATE UNIQUE INDEX IF NOT EXISTS idx_screenshot_content_hash
            ON snippets(content_hash)
            WHERE type = 'screenshot' AND content_hash IS NOT NULL;
        "#,
    },
    Migration {
        version: 15,
        name: "add_command_deduplication_index",
        up: r#"
            -- This migration needs to be handled in Rust code due to hash calculation
            -- See run_migrations() for the actual implementation
            -- We'll use a placeholder here and do the work in apply_migration_15()
            SELECT 1;
        "#,
    },
    Migration {
        version: 16,
        name: "add_ocr_settings",
        up: r#"
            -- Add OCR engine settings to settings table
            ALTER TABLE settings ADD COLUMN ocr_engine TEXT DEFAULT 'auto';
            ALTER TABLE settings ADD COLUMN ocr_recognition_level TEXT DEFAULT 'accurate';
            ALTER TABLE settings ADD COLUMN ocr_cleaning_level TEXT DEFAULT 'balanced';
            ALTER TABLE settings ADD COLUMN tesseract_psm_mode INTEGER DEFAULT 3;
        "#,
    },
    Migration {
        version: 17,
        name: "add_category_unique_constraint",
        up: r#"
            -- Add unique constraint on (name, parent_id) to prevent duplicate categories
            -- This prevents race conditions where multiple jobs try to create the same category
            CREATE UNIQUE INDEX IF NOT EXISTS idx_categories_name_parent_unique 
            ON categories(name, parent_id);
        "#,
    },
    Migration {
        version: 18,
        name: "add_content_hash_unique_constraint",
        up: r#"
            -- Add unique constraint on content_hash for ALL snippet types to prevent duplicates
            -- This prevents duplicates at database level even if application logic fails
            -- Only applies where content_hash IS NOT NULL (allows NULL for legacy data)
            CREATE UNIQUE INDEX IF NOT EXISTS idx_snippets_content_hash_unique
            ON snippets(content_hash)
            WHERE content_hash IS NOT NULL;
        "#,
    },
    Migration {
        version: 19,
        name: "update_category_unique_constraint_global",
        up: r#"
            -- Drop old unique constraint that was scoped to parent_id
            DROP INDEX IF EXISTS idx_categories_name_parent_unique;

            -- Add NEW unique constraint on normalized category name GLOBALLY (ignoring parent_id)
            -- This prevents duplicate categories across different app folders
            -- Uses LOWER(TRIM(name)) to handle case-insensitive and whitespace variations
            -- CRITICAL: This enforces canonical categories like "Docker Commands" across all folders
            CREATE UNIQUE INDEX IF NOT EXISTS idx_categories_name_global_unique
            ON categories(LOWER(REPLACE(REPLACE(TRIM(name), '_', ' '), '-', ' ')));
        "#,
    },
    Migration {
        version: 20,
        name: "add_command_picker_settings",
        up: r#"
            -- Add command picker settings for terminal autocomplete feature
            ALTER TABLE settings ADD COLUMN command_picker_enabled BOOLEAN DEFAULT 1;
            ALTER TABLE settings ADD COLUMN command_picker_shortcut TEXT DEFAULT 'Ctrl+R';
        "#,
    },
];

/// Create migrations table if it doesn't exist
async fn create_migrations_table(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        )
        "#,
    )
    .execute(pool)
    .await
    .context("Failed to create migrations table")?;

    Ok(())
}

/// Get current database version
async fn get_current_version(pool: &SqlitePool) -> Result<i32> {
    let row = sqlx::query("SELECT MAX(version) as version FROM schema_migrations")
        .fetch_optional(pool)
        .await
        .context("Failed to query migrations table")?;

    Ok(row
        .and_then(|r| r.try_get::<Option<i32>, _>(0).ok())
        .flatten()
        .unwrap_or(0))
}

/// Check if a migration has been applied
async fn is_migration_applied(pool: &SqlitePool, version: i32) -> Result<bool> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM schema_migrations WHERE version = ?")
        .bind(version)
        .fetch_one(pool)
        .await
        .context("Failed to check migration status")?;

    Ok(count > 0)
}

/// Record that a migration was applied
async fn record_migration(pool: &SqlitePool, version: i32, name: &str) -> Result<()> {
    sqlx::query("INSERT INTO schema_migrations (version, name) VALUES (?, ?)")
        .bind(version)
        .bind(name)
        .execute(pool)
        .await
        .context("Failed to record migration")?;

    Ok(())
}

/// Apply a single migration
async fn apply_migration(pool: &SqlitePool, migration: &Migration) -> Result<()> {
    log::info!(
        "Applying migration {}: {}",
        migration.version,
        migration.name
    );

    // Special handling for migration 15 (command deduplication)
    if migration.version == 15 {
        apply_migration_15(pool).await?;
    } else {
        // Execute migration SQL
        sqlx::query(migration.up)
            .execute(pool)
            .await
            .with_context(|| {
                format!(
                    "Failed to execute migration {}: {}",
                    migration.version, migration.name
                )
            })?;
    }

    // Record migration
    record_migration(pool, migration.version, migration.name).await?;

    log::info!("✓ Migration {} applied successfully", migration.version);

    Ok(())
}

/// Special migration 15: Add command deduplication with hash recalculation and cleanup
async fn apply_migration_15(pool: &SqlitePool) -> Result<()> {
    log::info!("Step 1: Recalculating command hashes...");

    // Recalculate all command hashes using the dedup module
    let count = crate::dedup::recalculate_command_hashes(pool).await?;
    log::info!("✓ Recalculated {} command hashes", count);

    log::info!("Step 2: Removing duplicate commands...");

    // Find duplicates: commands with same content_hash
    let duplicates: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT content_hash, COUNT(*) as count
        FROM snippets
        WHERE type = 'command' AND content_hash IS NOT NULL
        GROUP BY content_hash
        HAVING COUNT(*) > 1
        "#
    )
    .fetch_all(pool)
    .await
    .context("Failed to find duplicate commands")?;

    let mut total_deleted = 0;

    for (hash, _count) in duplicates {
        // Get all IDs for this hash, ordered by created_at (oldest first)
        let ids: Vec<i64> = sqlx::query_scalar(
            "SELECT id FROM snippets WHERE content_hash = ? AND type = 'command' ORDER BY created_at ASC"
        )
        .bind(&hash)
        .fetch_all(pool)
        .await?;

        // Delete all except the first (oldest)
        if ids.len() > 1 {
            for id in &ids[1..] {
                sqlx::query("DELETE FROM snippets WHERE id = ?")
                    .bind(id)
                    .execute(pool)
                    .await?;
                total_deleted += 1;
            }
        }
    }

    log::info!("✓ Deleted {} duplicate commands", total_deleted);

    log::info!("Step 3: Creating unique index...");

    // Now create the unique index
    sqlx::query(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS idx_command_content_hash
        ON snippets(content_hash)
        WHERE type = 'command' AND content_hash IS NOT NULL
        "#
    )
    .execute(pool)
    .await
    .context("Failed to create unique index")?;

    log::info!("✓ Created unique index on command content_hash");

    Ok(())
}

/// Run all pending migrations
pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    log::info!("Checking database migrations...");

    // Create migrations table
    create_migrations_table(pool).await?;

    // Get current version
    let current_version = get_current_version(pool).await?;
    log::info!("Current database version: {}", current_version);

    if current_version >= CURRENT_VERSION {
        log::info!("Database is up to date (version {})", current_version);
        return Ok(());
    }

    // Find pending migrations
    let pending: Vec<&Migration> = MIGRATIONS
        .iter()
        .filter(|m| m.version > current_version)
        .collect();

    if pending.is_empty() {
        log::info!("No pending migrations");
        return Ok(());
    }

    log::info!("Found {} pending migration(s)", pending.len());

    // Apply each pending migration
    for migration in pending {
        apply_migration(pool, migration).await?;
    }

    let new_version = get_current_version(pool).await?;
    log::info!(
        "✓ Migrations complete! Database upgraded from version {} to {}",
        current_version,
        new_version
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_migrations_table() {
        // This would require a test database setup
        // For now, just verify the migration list is valid
        assert!(!MIGRATIONS.is_empty());
        assert_eq!(MIGRATIONS[0].version, 1);
        assert!(
            MIGRATIONS[0].version < CURRENT_VERSION || MIGRATIONS[0].version == CURRENT_VERSION
        );
    }
}
