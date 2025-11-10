use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::fs;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportData {
    pub version: String,
    pub exported_at: String,
    pub snippets: Vec<ExportSnippet>,
    pub categories: Vec<ExportCategory>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportSnippet {
    pub id: i64,
    pub content: String,
    pub summary: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
    pub source_app: Option<String>,
    pub metadata: Option<String>,
    pub category_id: Option<i64>,
    pub versions: Option<Vec<ExportVersion>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportVersion {
    pub version_number: i64,
    pub content: String,
    pub summary: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportCategory {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub emoji: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportRecord {
    pub id: i64,
    pub format: String,
    pub file_path: String,
    pub item_count: i64,
    pub file_size_bytes: Option<i64>,
    pub exported_at: String,
}

/// Export all data to JSON format
pub async fn export_to_json(
    pool: &SqlitePool,
    file_path: &str,
    include_versions: bool,
) -> Result<usize> {
    log::info!("Exporting to JSON: {}", file_path);

    // Fetch all snippets
    let snippets = sqlx::query_as::<_, (i64, String, Option<String>, String, Option<String>, Option<String>, Option<String>)>(
        "SELECT id, content, summary, created_at, updated_at, source_app, metadata FROM snippets ORDER BY created_at DESC"
    )
    .fetch_all(pool)
    .await
    .context("Failed to fetch snippets")?;

    // Fetch snippet categories
    let snippet_categories = sqlx::query_as::<_, (i64, i64)>(
        "SELECT snippet_id, category_id FROM snippet_categories"
    )
    .fetch_all(pool)
    .await
    .context("Failed to fetch snippet categories")?;

    let category_map: std::collections::HashMap<i64, i64> = snippet_categories.into_iter().collect();

    let mut export_snippets = Vec::new();
    for (id, content, summary, created_at, updated_at, source_app, metadata) in snippets {
        let mut versions = None;

        if include_versions {
            let snippet_versions = sqlx::query_as::<_, (i64, String, Option<String>, String)>(
                "SELECT version_number, content, summary, created_at FROM snippet_versions WHERE snippet_id = ? ORDER BY version_number DESC"
            )
            .bind(id)
            .fetch_all(pool)
            .await
            .context("Failed to fetch snippet versions")?;

            if !snippet_versions.is_empty() {
                versions = Some(
                    snippet_versions
                        .into_iter()
                        .map(|(version_number, content, summary, created_at)| ExportVersion {
                            version_number,
                            content,
                            summary,
                            created_at,
                        })
                        .collect(),
                );
            }
        }

        export_snippets.push(ExportSnippet {
            id,
            content,
            summary,
            created_at,
            updated_at,
            source_app,
            metadata,
            category_id: category_map.get(&id).copied(),
            versions,
        });
    }

    // Fetch all categories
    let categories = sqlx::query_as::<_, (i64, String, Option<i64>, String, String)>(
        "SELECT id, name, parent_id, emoji, created_at FROM categories ORDER BY created_at"
    )
    .fetch_all(pool)
    .await
    .context("Failed to fetch categories")?;

    let export_categories = categories
        .into_iter()
        .map(|(id, name, parent_id, emoji, created_at)| ExportCategory {
            id,
            name,
            parent_id,
            emoji,
            created_at,
        })
        .collect();

    let export_data = ExportData {
        version: "1.0.0".to_string(),
        exported_at: Utc::now().to_rfc3339(),
        snippets: export_snippets,
        categories: export_categories,
    };

    let item_count = export_data.snippets.len();

    // Write to file
    let json = serde_json::to_string_pretty(&export_data)
        .context("Failed to serialize export data")?;

    fs::write(file_path, &json)
        .context("Failed to write export file")?;

    let file_size = fs::metadata(file_path)?.len() as i64;

    // Record export in history
    sqlx::query(
        "INSERT INTO export_history (format, file_path, item_count, file_size_bytes) VALUES (?, ?, ?, ?)"
    )
    .bind("json")
    .bind(file_path)
    .bind(item_count as i64)
    .bind(file_size)
    .execute(pool)
    .await
    .context("Failed to record export history")?;

    log::info!("✅ Exported {} snippets to {}", item_count, file_path);
    Ok(item_count)
}

/// Export all data to Markdown format
pub async fn export_to_markdown(pool: &SqlitePool, file_path: &str) -> Result<usize> {
    log::info!("Exporting to Markdown: {}", file_path);

    // Fetch all snippets with categories
    let snippets = sqlx::query_as::<_, (i64, String, Option<String>, String, Option<String>, Option<String>, Option<i64>)>(
        r#"
        SELECT s.id, s.content, s.summary, s.created_at, s.source_app, s.metadata, sc.category_id
        FROM snippets s
        LEFT JOIN snippet_categories sc ON s.id = sc.snippet_id
        ORDER BY s.created_at DESC
        "#
    )
    .fetch_all(pool)
    .await
    .context("Failed to fetch snippets")?;

    // Fetch categories for lookup
    let categories = sqlx::query_as::<_, (i64, String, String)>(
        "SELECT id, name, emoji FROM categories"
    )
    .fetch_all(pool)
    .await
    .context("Failed to fetch categories")?;

    let category_map: std::collections::HashMap<i64, (String, String)> = categories
        .into_iter()
        .map(|(id, name, emoji)| (id, (name, emoji)))
        .collect();

    let mut markdown = String::new();
    markdown.push_str("# Local Mind Export\n\n");
    markdown.push_str(&format!("Exported: {}\n\n", Utc::now().format("%Y-%m-%d %H:%M:%S")));
    markdown.push_str(&format!("Total Snippets: {}\n\n", snippets.len()));
    markdown.push_str("---\n\n");

    for (id, content, summary, created_at, source_app, _metadata, category_id) in &snippets {
        markdown.push_str(&format!("## Snippet #{}\n\n", id));

        if let Some(summary) = summary {
            if !summary.is_empty() {
                markdown.push_str(&format!("**Summary:** {}\n\n", summary));
            }
        }

        markdown.push_str(&format!("**Created:** {}\n\n", created_at));

        if let Some(app) = source_app {
            markdown.push_str(&format!("**Source:** {}\n\n", app));
        }

        if let Some(cat_id) = category_id {
            if let Some((name, emoji)) = category_map.get(cat_id) {
                markdown.push_str(&format!("**Category:** {} {}\n\n", emoji, name));
            }
        }

        markdown.push_str("**Content:**\n\n");
        markdown.push_str("```\n");
        markdown.push_str(content);
        markdown.push_str("\n```\n\n");
        markdown.push_str("---\n\n");
    }

    let item_count = snippets.len();

    // Write to file
    fs::write(file_path, &markdown)
        .context("Failed to write markdown file")?;

    let file_size = fs::metadata(file_path)?.len() as i64;

    // Record export in history
    sqlx::query(
        "INSERT INTO export_history (format, file_path, item_count, file_size_bytes) VALUES (?, ?, ?, ?)"
    )
    .bind("markdown")
    .bind(file_path)
    .bind(item_count as i64)
    .bind(file_size)
    .execute(pool)
    .await
    .context("Failed to record export history")?;

    log::info!("✅ Exported {} snippets to {}", item_count, file_path);
    Ok(item_count)
}

/// Import data from JSON file
pub async fn import_from_json(pool: &SqlitePool, file_path: &str) -> Result<usize> {
    log::info!("Importing from JSON: {}", file_path);

    if !Path::new(file_path).exists() {
        anyhow::bail!("File not found: {}", file_path);
    }

    let json = fs::read_to_string(file_path)
        .context("Failed to read import file")?;

    let import_data: ExportData = serde_json::from_str(&json)
        .context("Failed to parse JSON import file")?;

    log::info!("Import data version: {}", import_data.version);
    log::info!("Importing {} snippets", import_data.snippets.len());

    let mut imported_count = 0;

    // Import categories first
    for category in &import_data.categories {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM categories WHERE id = ?)"
        )
        .bind(category.id)
        .fetch_one(pool)
        .await?;

        if !exists {
            sqlx::query(
                "INSERT INTO categories (id, name, parent_id, emoji, created_at) VALUES (?, ?, ?, ?, ?)"
            )
            .bind(category.id)
            .bind(&category.name)
            .bind(category.parent_id)
            .bind(&category.emoji)
            .bind(&category.created_at)
            .execute(pool)
            .await
            .context("Failed to import category")?;
        }
    }

    // Import snippets
    for snippet in &import_data.snippets {
        // Check if snippet already exists
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM snippets WHERE id = ?)"
        )
        .bind(snippet.id)
        .fetch_one(pool)
        .await?;

        if exists {
            log::warn!("Snippet {} already exists, skipping", snippet.id);
            continue;
        }

        // Insert snippet
        sqlx::query(
            "INSERT INTO snippets (id, content, summary, created_at, updated_at, source_app, metadata) VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(snippet.id)
        .bind(&snippet.content)
        .bind(&snippet.summary)
        .bind(&snippet.created_at)
        .bind(&snippet.updated_at)
        .bind(&snippet.source_app)
        .bind(&snippet.metadata)
        .execute(pool)
        .await
        .context("Failed to import snippet")?;

        // Assign to category if provided
        if let Some(category_id) = snippet.category_id {
            sqlx::query(
                "INSERT OR IGNORE INTO snippet_categories (snippet_id, category_id, is_manual) VALUES (?, ?, ?)"
            )
            .bind(snippet.id)
            .bind(category_id)
            .bind(1)
            .execute(pool)
            .await
            .context("Failed to assign snippet to category")?;
        }

        // Import versions if provided
        if let Some(versions) = &snippet.versions {
            for version in versions {
                sqlx::query(
                    "INSERT OR IGNORE INTO snippet_versions (snippet_id, version_number, content, summary, created_at) VALUES (?, ?, ?, ?, ?)"
                )
                .bind(snippet.id)
                .bind(version.version_number)
                .bind(&version.content)
                .bind(&version.summary)
                .bind(&version.created_at)
                .execute(pool)
                .await
                .context("Failed to import snippet version")?;
            }
        }

        imported_count += 1;
    }

    log::info!("✅ Imported {} snippets from {}", imported_count, file_path);
    Ok(imported_count)
}

/// Get export history
pub async fn get_export_history(pool: &SqlitePool, limit: i64) -> Result<Vec<ExportRecord>> {
    let records = sqlx::query_as::<_, (i64, String, String, i64, Option<i64>, String)>(
        "SELECT id, format, file_path, item_count, file_size_bytes, exported_at FROM export_history ORDER BY exported_at DESC LIMIT ?"
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .context("Failed to fetch export history")?;

    Ok(records
        .into_iter()
        .map(|(id, format, file_path, item_count, file_size_bytes, exported_at)| ExportRecord {
            id,
            format,
            file_path,
            item_count,
            file_size_bytes,
            exported_at,
        })
        .collect())
}
