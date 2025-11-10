use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub id: String,
    pub suggestion_type: SuggestionType,
    pub title: String,
    pub description: String,
    pub action: SuggestionAction,
    pub priority: SuggestionPriority,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum SuggestionType {
    Duplicate,
    AutoTag,
    RelatedSnippets,
    Archive,
    SmartCollection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", content = "params")]
pub enum SuggestionAction {
    MergeDuplicates { snippet_ids: Vec<i64> },
    AddTag { snippet_id: i64, tag: String },
    ViewRelated { snippet_id: i64, related_ids: Vec<i64> },
    ArchiveSnippets { snippet_ids: Vec<i64> },
    CreateCollection { name: String, snippet_ids: Vec<i64> },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum SuggestionPriority {
    Low,
    Medium,
    High,
}

/// Get all active suggestions
pub async fn get_all_suggestions(pool: &SqlitePool) -> Result<Vec<Suggestion>> {
    let mut suggestions = Vec::new();

    // Add duplicate suggestions
    if let Ok(mut dups) = get_duplicate_suggestions(pool).await {
        suggestions.append(&mut dups);
    }

    // Add archive suggestions
    if let Ok(mut archives) = get_archive_suggestions(pool).await {
        suggestions.append(&mut archives);
    }

    // Add auto-tag suggestions
    if let Ok(mut tags) = get_auto_tag_suggestions(pool).await {
        suggestions.append(&mut tags);
    }

    // Sort by priority
    suggestions.sort_by(|a, b| b.priority.cmp(&a.priority));

    Ok(suggestions)
}

/// Suggest duplicates using embeddings (semantic similarity)
pub async fn get_duplicate_suggestions(pool: &SqlitePool) -> Result<Vec<Suggestion>> {
    use crate::db::lancedb;
    use crate::dedup;

    let mut suggestions = Vec::new();

    // First, get hash-based exact duplicates
    let duplicate_groups = dedup::find_all_duplicates(pool).await?;

    for group in duplicate_groups.iter().take(5) {
        // Limit to top 5
        let snippet_ids: Vec<i64> = group.snippets.iter().map(|s| s.id).collect();

        suggestions.push(Suggestion {
            id: format!("dup-hash-{}", group.hash),
            suggestion_type: SuggestionType::Duplicate,
            title: format!("{} exact duplicates found", group.count),
            description: format!(
                "Multiple snippets with identical content. Merge to keep database clean."
            ),
            action: SuggestionAction::MergeDuplicates { snippet_ids },
            priority: if group.count > 5 {
                SuggestionPriority::High
            } else {
                SuggestionPriority::Medium
            },
            metadata: Some(serde_json::json!({ "hash": group.hash })),
        });
    }

    // TODO: Add semantic duplicate detection using embeddings
    // This would find snippets that are similar but not exactly the same

    Ok(suggestions)
}

/// Suggest archiving old, unused snippets
pub async fn get_archive_suggestions(pool: &SqlitePool) -> Result<Vec<Suggestion>> {
    let cutoff = Utc::now() - chrono::Duration::days(90);
    let cutoff_str = cutoff.to_rfc3339();

    // Find snippets that are old and haven't been accessed recently
    let old_snippets: Vec<i64> = sqlx::query_scalar(
        r#"
        SELECT id FROM snippets
        WHERE created_at < ?
        AND id NOT IN (
            SELECT DISTINCT snippet_id FROM snippet_categories
            WHERE is_manual = 1
        )
        ORDER BY created_at
        LIMIT 10
        "#,
    )
    .bind(&cutoff_str)
    .fetch_all(pool)
    .await?;

    if old_snippets.is_empty() {
        return Ok(Vec::new());
    }

    let suggestion = Suggestion {
        id: "archive-old".to_string(),
        suggestion_type: SuggestionType::Archive,
        title: format!("{} old snippets unused for 90+ days", old_snippets.len()),
        description: "These snippets haven't been categorized or accessed recently. Consider archiving them.".to_string(),
        action: SuggestionAction::ArchiveSnippets {
            snippet_ids: old_snippets,
        },
        priority: SuggestionPriority::Low,
        metadata: None,
    };

    Ok(vec![suggestion])
}

/// Suggest tags based on content analysis
pub async fn get_auto_tag_suggestions(pool: &SqlitePool) -> Result<Vec<Suggestion>> {
    let mut suggestions = Vec::new();

    // Get uncategorized snippets
    let snippets: Vec<(i64, String)> = sqlx::query_as(
        r#"
        SELECT id, content FROM snippets
        WHERE id NOT IN (SELECT snippet_id FROM snippet_categories)
        LIMIT 5
        "#,
    )
    .fetch_all(pool)
    .await?;

    for (snippet_id, content) in snippets {
        // Simple keyword-based tag suggestions
        let suggested_tags = suggest_tags_from_content(&content);

        if !suggested_tags.is_empty() {
            for tag in suggested_tags {
                suggestions.push(Suggestion {
                    id: format!("tag-{}-{}", snippet_id, tag),
                    suggestion_type: SuggestionType::AutoTag,
                    title: format!("Add #{} tag", tag),
                    description: format!("This snippet looks like {} content", tag),
                    action: SuggestionAction::AddTag {
                        snippet_id,
                        tag: tag.clone(),
                    },
                    priority: SuggestionPriority::Medium,
                    metadata: Some(serde_json::json!({ "confidence": 0.8 })),
                });
            }
        }
    }

    Ok(suggestions)
}

/// Simple keyword-based tag suggestion
fn suggest_tags_from_content(content: &str) -> Vec<String> {
    let content_lower = content.to_lowercase();
    let mut tags = Vec::new();

    // Code-related tags
    if content_lower.contains("function") || content_lower.contains("def ") || content_lower.contains("const ") {
        tags.push("code".to_string());
    }

    // Language-specific tags
    if content_lower.contains("import react") || content_lower.contains("usestate") || content_lower.contains("jsx") {
        tags.push("react".to_string());
    }

    if content_lower.contains("fn ") || content_lower.contains("impl ") || content_lower.contains("struct ") {
        tags.push("rust".to_string());
    }

    if content_lower.contains("def ") || content_lower.contains("import ") && content_lower.contains("python") {
        tags.push("python".to_string());
    }

    if content_lower.contains("async") || content_lower.contains("await") {
        tags.push("async".to_string());
    }

    // Documentation/notes
    if content_lower.contains("todo") || content_lower.contains("note:") {
        tags.push("note".to_string());
    }

    // Limit to 2 tags
    tags.truncate(2);
    tags
}

/// Find related snippets using semantic similarity
pub async fn find_related_snippets(
    _pool: &SqlitePool,
    snippet_id: i64,
    limit: usize,
) -> Result<Vec<i64>> {
    use crate::db::lancedb;

    // Get the snippet's embedding
    let embedding = match lancedb::get_embedding(snippet_id).await {
        Ok(Some(emb)) => emb,
        _ => return Ok(Vec::new()),
    };

    // Find similar snippets
    let similar = lancedb::search_semantic(embedding, limit + 1).await?;

    // Filter out the original snippet
    let related: Vec<i64> = similar
        .into_iter()
        .map(|(id, _score)| id)
        .filter(|&id| id != snippet_id)
        .take(limit)
        .collect();

    Ok(related)
}

/// Dismiss a suggestion
pub async fn dismiss_suggestion(suggestion_id: &str) -> Result<()> {
    // Store dismissed suggestions in localStorage on frontend
    // Or create a dismissed_suggestions table if needed
    log::info!("Dismissed suggestion: {}", suggestion_id);
    Ok(())
}
