use crate::analytics::search_analytics::{SearchAnalytics, SearchMetrics, SearchType};
use crate::db::sqlite::{self, SearchResult};
use crate::job_queue::PersistentJobQueue;
use crate::search::{build_fts5_query, parse_query};
use crate::export;
use crate::dedup;
use crate::suggestions;
use anyhow::Result;
use chrono::DateTime;
use log;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::time::Instant;
use tauri::{Manager, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetId {
    pub id: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResults {
    pub keyword_results: Vec<SearchResultJson>,
    pub semantic_results: Vec<SearchResultJson>,
    pub combined: Vec<SearchResultJson>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultJson {
    pub id: i64,
    pub content: String,
    pub created_at: String,
    pub source_app: Option<String>,
    pub rank: f64,
    pub match_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SnippetJson {
    pub id: i64,
    pub content: String,
    pub summary: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
    pub source_app: Option<String>,
    pub metadata: Option<String>,
    #[serde(rename = "type")]
    pub content_type: Option<String>,
    pub file_path: Option<String>,
    pub working_directory: Option<String>,
    pub exit_code: Option<i32>,
    pub website_url: Option<String>,
    pub website_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchFilters {
    pub date_from: Option<String>,  // ISO 8601 format
    pub date_to: Option<String>,    // ISO 8601 format
    pub source_app: Option<String>,
    pub has_embedding: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryJson {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub emoji: String,
    pub created_at: String,
    pub snippet_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCategoryRequest {
    pub name: String,
    pub parent_id: Option<i64>,
    pub emoji: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCategoryRequest {
    pub id: i64,
    pub name: Option<String>,
    pub emoji: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignSnippetRequest {
    pub snippet_id: i64,
    pub category_id: i64,
    pub is_manual: bool,
}

/// Save a snippet from clipboard
#[tauri::command]
pub async fn save_snippet(
    content: String,
    source_app: Option<String>,
    metadata: Option<serde_json::Value>,
    job_queue: State<'_, PersistentJobQueue>,
) -> Result<SnippetId, String> {
    // Save to SQLite immediately (instant keyword search)
    let id = sqlite::save_snippet(content.clone(), source_app, metadata)
        .await
        .map_err(|e| format!("Failed to save snippet: {}", e))?;

    // Get the snippet to retrieve its summary
    let snippet = sqlite::get_snippet(id)
        .await
        .map_err(|e| format!("Failed to retrieve snippet: {}", e))?
        .ok_or_else(|| format!("Snippet {} not found after saving", id))?;

    // Queue embedding job (background processing) with summary
    job_queue
        .push(id, content, snippet.summary, crate::job_queue::Priority::Normal)
        .await
        .map_err(|e| format!("Failed to queue embedding job: {}", e))?;

    Ok(SnippetId { id })
}

/// Search snippets using both keyword and semantic search
#[tauri::command]
pub async fn search(
    query: String,
    filters: Option<SearchFilters>,
    analytics: State<'_, SearchAnalytics>,
) -> Result<SearchResults, String> {
    let start_time = Instant::now();
    log::info!("🔍 Search called with query: '{}'", query);

    // Parse query for advanced features (phrases, boolean, proximity)
    let parsed = parse_query(&query);
    log::info!("Parsed query type: {:?}", parsed.query_type);
    log::info!("Original query: '{}'", query);

    // Use the parsed query to build FTS5 query
    let enhanced_query = build_fts5_query(&parsed);
    log::info!("Enhanced FTS5 query: '{}'", enhanced_query);

    // Run keyword search immediately (instant, doesn't depend on embeddings)
    let keyword_results = match search_keyword_enhanced(&query, &enhanced_query).await {
        Ok(results) => {
            log::info!(
                "Keyword search '{}' returned {} results",
                query,
                results.len()
            );
            results
        }
        Err(e) => {
            log::error!("Keyword search failed: {}", e);
            // Try fallback: search without enhanced query
            log::info!("Trying fallback search without enhanced query");
            match sqlite::search_fts5_with_original(&query, &query, 10).await {
                Ok(results) => {
                    log::info!("Fallback search returned {} results", results.len());
                    results
                }
                Err(e2) => {
                    log::error!("Fallback search also failed: {}", e2);
                    vec![]
                }
            }
        }
    };

    let mut keyword_json: Vec<SearchResultJson> =
        keyword_results.into_iter().map(|r| r.into()).collect();

    // Apply filters to keyword results if provided
    if let Some(ref filters) = filters {
        keyword_json = apply_filters(keyword_json, filters).await;
    }

    // Semantic search runs in background - non-blocking with timeout
    // If embedding model is downloading or fails, just return empty results
    let semantic_query = query.clone();
    let semantic_task = tokio::spawn(async move {
        match search_semantic(&semantic_query).await {
            Ok(results) => results,
            Err(e) => {
                log::debug!("Semantic search skipped (non-blocking): {}", e);
                vec![]
            }
        }
    });

    // Wait briefly for semantic results, but don't block keyword results
    let semantic_results: Vec<SearchResult> = match tokio::time::timeout(
        std::time::Duration::from_millis(500), // 500ms max wait
        semantic_task,
    )
    .await
    {
        Ok(Ok(results)) => results,
        Ok(Err(_)) => vec![],
        Err(_) => {
            log::debug!("Semantic search timed out (non-blocking)");
            vec![]
        }
    };

    let mut semantic_json: Vec<SearchResultJson> =
        semantic_results.into_iter().map(|r| r.into()).collect();

    // Apply filters to semantic results if provided
    if let Some(ref filters) = filters {
        semantic_json = apply_filters(semantic_json, filters).await;
    }

    log::info!(
        "🔍 Search '{}' - keyword: {}, semantic: {}",
        query,
        keyword_json.len(),
        semantic_json.len()
    );

    // Use Reciprocal Rank Fusion (RRF) for intelligent hybrid ranking
    let combined = reciprocal_rank_fusion(&keyword_json, &semantic_json, 60.0);

    log::info!(
        "🔍 Search '{}' RRF combined results: {} (keyword: {}, semantic: {})",
        query,
        combined.len(),
        keyword_json.len(),
        semantic_json.len()
    );
    println!(
        "🎯 RRF merged {} keyword + {} semantic = {} total results",
        keyword_json.len(),
        semantic_json.len(),
        combined.len()
    );

    // Log search analytics
    let latency_ms = start_time.elapsed().as_millis() as u64;
    let metrics = SearchMetrics {
        query: query.clone(),
        search_type: if keyword_json.is_empty() && !semantic_json.is_empty() {
            SearchType::Semantic
        } else if !keyword_json.is_empty() && semantic_json.is_empty() {
            SearchType::Keyword
        } else {
            SearchType::Hybrid
        },
        keyword_results: keyword_json.len(),
        semantic_results: semantic_json.len(),
        total_results: combined.len(),
        latency_ms,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    if let Err(e) = analytics.log_search(metrics).await {
        log::warn!("Failed to log search analytics: {}", e);
    }

    println!("⏱️  Search completed in {}ms", latency_ms);

    Ok(SearchResults {
        keyword_results: keyword_json,
        semantic_results: semantic_json,
        combined,
    })
}

/// Get snippet by ID
#[tauri::command]
pub async fn get_snippet(id: i64) -> Result<SnippetJson, String> {
    let snippet = sqlite::get_snippet(id)
        .await
        .map_err(|e| format!("Failed to get snippet: {}", e))?
        .ok_or_else(|| format!("Snippet {} not found", id))?;

    Ok(SnippetJson {
        id: snippet.id,
        content: snippet.content,
        summary: snippet.summary,
        created_at: snippet.created_at.to_rfc3339(),
        updated_at: snippet.updated_at.map(|dt| dt.to_rfc3339()),
        source_app: snippet.source_app,
        metadata: snippet.metadata,
        content_type: snippet.content_type,
        file_path: snippet.file_path,
        working_directory: snippet.working_directory,
        exit_code: snippet.exit_code,
        website_url: snippet.website_url,
        website_title: snippet.website_title,
    })
}

/// Edit snippet content
#[tauri::command]
pub async fn edit_snippet(
    id: i64,
    new_content: String,
    job_queue: State<'_, PersistentJobQueue>,
) -> Result<(), String> {
    // Edit the snippet (saves version history)
    sqlite::edit_snippet(id, new_content.clone(), true)
        .await
        .map_err(|e| format!("Failed to edit snippet: {}", e))?;

    // Get the updated snippet to retrieve its summary
    let snippet = sqlite::get_snippet(id)
        .await
        .map_err(|e| format!("Failed to retrieve updated snippet: {}", e))?
        .ok_or_else(|| format!("Snippet {} not found after editing", id))?;

    // Queue re-embedding job with updated content and summary
    use crate::job_queue::Priority;
    job_queue
        .push(id, snippet.content, snippet.summary, Priority::High)
        .await
        .map_err(|e| format!("Failed to queue re-embedding job: {}", e))?;

    log::info!("✅ Edited snippet {} and queued re-embedding", id);
    Ok(())
}

/// Get version history for a snippet
#[derive(Debug, Serialize, Deserialize)]
pub struct SnippetVersionJson {
    pub id: i64,
    pub snippet_id: i64,
    pub version_number: i64,
    pub content: String,
    pub summary: Option<String>,
    pub created_at: String,
}

#[tauri::command]
pub async fn get_snippet_versions(snippet_id: i64) -> Result<Vec<SnippetVersionJson>, String> {
    let versions = sqlite::get_snippet_versions(snippet_id)
        .await
        .map_err(|e| format!("Failed to get snippet versions: {}", e))?;

    Ok(versions
        .into_iter()
        .map(|v| SnippetVersionJson {
            id: v.id,
            snippet_id: v.snippet_id,
            version_number: v.version_number,
            content: v.content,
            summary: v.summary,
            created_at: v.created_at.to_rfc3339(),
        })
        .collect())
}

/// Delete snippet by ID
#[tauri::command]
pub async fn delete_snippet(id: i64, app: tauri::AppHandle) -> Result<bool, String> {
    use crate::db::lancedb;

    log::debug!("Delete snippet command called with id: {}", id);
    println!("🗑️  Deleting snippet {}", id);

    match sqlite::delete_snippet(id).await {
        Ok(result) => {
            if result {
                log::info!("Snippet {} deleted successfully", id);
                println!("✅ Snippet {} deleted successfully", id);

                // Delete embedding from LanceDB
                if let Err(e) = lancedb::delete_embedding(id).await {
                    log::warn!("Failed to delete embedding for snippet {}: {}", id, e);
                    println!("⚠️  Failed to delete embedding for snippet {}: {}", id, e);
                    // Continue anyway - snippet is already deleted from main DB
                }

                // Emit event to frontend to update UI
                if let Some(window) = app.get_window("main") {
                    let _ = window.emit("snippet-deleted", id);
                    println!("📡 Emitted snippet-deleted event for id: {}", id);
                }

                Ok(true)
            } else {
                log::warn!("Snippet {} not found", id);
                println!("⚠️  Snippet {} not found", id);
                Ok(false)
            }
        }
        Err(e) => {
            log::error!("Failed to delete snippet {}: {}", id, e);
            println!("❌ Failed to delete snippet {}: {}", id, e);
            Err(e.to_string())
        }
    }
}

/// Reciprocal Rank Fusion (RRF) - Industry standard for merging search results
/// Combines keyword and semantic search results with proper ranking
/// Formula: score(d) = Σ 1 / (k + rank(d))
/// where k is a constant (typically 60), rank is position in result list
fn reciprocal_rank_fusion(
    keyword_results: &[SearchResultJson],
    semantic_results: &[SearchResultJson],
    k: f64,
) -> Vec<SearchResultJson> {
    use std::collections::HashMap;

    let mut rrf_scores: HashMap<i64, f64> = HashMap::new();
    let mut all_results: HashMap<i64, SearchResultJson> = HashMap::new();

    // Add keyword RRF scores (position-based)
    for (rank, result) in keyword_results.iter().enumerate() {
        let score = 1.0 / (k + rank as f64 + 1.0);
        *rrf_scores.entry(result.id).or_insert(0.0) += score;
        all_results.insert(result.id, result.clone());
    }

    // Add semantic RRF scores (position-based)
    for (rank, result) in semantic_results.iter().enumerate() {
        let score = 1.0 / (k + rank as f64 + 1.0);
        *rrf_scores.entry(result.id).or_insert(0.0) += score;
        all_results.insert(result.id, result.clone());
    }

    // Create final ranked list with RRF scores
    let mut ranked: Vec<SearchResultJson> = rrf_scores
        .iter()
        .map(|(id, rrf_score)| {
            let mut result = all_results.get(id).unwrap().clone();
            result.rank = *rrf_score; // Use RRF score for ranking
            result
        })
        .collect();

    // Sort by RRF score (descending)
    ranked.sort_by(|a, b| b.rank.partial_cmp(&a.rank).unwrap_or(std::cmp::Ordering::Equal));

    println!("🔀 RRF Top 5 results:");
    for (i, result) in ranked.iter().take(5).enumerate() {
        println!(
            "  {}. ID {}: RRF score {:.6} ({})",
            i + 1,
            result.id,
            result.rank,
            result.match_type
        );
    }

    ranked
}

async fn search_keyword_enhanced(
    original_query: &str,
    enhanced_query: &str,
) -> Result<Vec<SearchResult>, String> {
    // Always use enhanced query if available (it's properly formatted for FTS5)
    let search_query = if !enhanced_query.is_empty() {
        enhanced_query
    } else {
        original_query
    };

    log::debug!(
        "Searching with query: '{}' (original: '{}')",
        search_query,
        original_query
    );

    // Pass both the formatted query (for FTS5) and original query (for validation)
    sqlite::search_fts5_with_original(search_query, original_query, 10)
        .await
        .map_err(|e| format!("Keyword search failed: {}", e))
}

/// Get search analytics stats
#[tauri::command]
pub async fn get_search_stats(days: i64, analytics: State<'_, SearchAnalytics>) -> Result<serde_json::Value, String> {
    match analytics.get_stats(days).await {
        Ok(stats) => Ok(serde_json::to_value(stats).unwrap()),
        Err(e) => Err(format!("Failed to get search stats: {}", e)),
    }
}

/// Get all unique source apps (for filter dropdown)
#[tauri::command]
pub async fn get_source_apps() -> Result<Vec<String>, String> {
    sqlite::get_unique_source_apps()
        .await
        .map_err(|e| format!("Failed to get source apps: {}", e))
}

// ============================================================================
// Category Management Commands
// ============================================================================

/// Create a new category
#[tauri::command]
pub async fn create_category(request: CreateCategoryRequest) -> Result<i64, String> {
    sqlite::create_category(request.name, request.parent_id, request.emoji)
        .await
        .map_err(|e| format!("Failed to create category: {}", e))
}

/// Get all categories or filter by parent_id
/// If include_counts is true, includes snippet count for each category
#[tauri::command]
pub async fn get_categories(
    parent_id: Option<i64>,
    include_counts: bool,
    content_type: Option<String>,
) -> Result<Vec<CategoryJson>, String> {
    // If parent_id is provided, get children of that parent
    // If parent_id is None, get root categories (parent_id IS NULL)
    let filter = if parent_id.is_some() {
        Some(parent_id)
    } else {
        Some(None) // Get root categories
    };

    // Convert "all" to None for backend processing
    let filter_type = content_type.and_then(|t| {
        if t == "all" {
            None
        } else {
            Some(t)
        }
    });

    let categories = sqlite::get_categories(filter, filter_type.clone())
        .await
        .map_err(|e| format!("Failed to get categories: {}", e))?;

    let mut result = Vec::new();
    for cat in categories {
        let snippet_count = if include_counts {
            sqlite::get_category_snippet_count(cat.id, filter_type.clone())
                .await
                .ok()
        } else {
            None
        };

        result.push(CategoryJson {
            id: cat.id,
            name: cat.name,
            parent_id: cat.parent_id,
            emoji: cat.emoji,
            created_at: cat.created_at.to_rfc3339(),
            snippet_count,
        });
    }

    Ok(result)
}

/// Get all root categories (for initial tree load)
#[tauri::command]
pub async fn get_root_categories(include_counts: bool, content_type: Option<String>) -> Result<Vec<CategoryJson>, String> {
    get_categories(None, include_counts, content_type).await
}

/// Get child categories of a parent
#[tauri::command]
pub async fn get_child_categories(
    parent_id: i64,
    include_counts: bool,
    content_type: Option<String>,
) -> Result<Vec<CategoryJson>, String> {
    get_categories(Some(parent_id), include_counts, content_type).await
}

/// Update category name and/or emoji
#[tauri::command]
pub async fn update_category(request: UpdateCategoryRequest) -> Result<(), String> {
    sqlite::update_category(request.id, request.name, request.emoji)
        .await
        .map_err(|e| format!("Failed to update category: {}", e))
}

/// Delete a category (cascades to children and assignments)
#[tauri::command]
pub async fn delete_category(id: i64) -> Result<(), String> {
    sqlite::delete_category(id)
        .await
        .map_err(|e| format!("Failed to delete category: {}", e))
}

/// Delete all data (snippets, commands, screenshots, categories, embeddings)
#[tauri::command]
pub async fn delete_all_data(app: tauri::AppHandle) -> Result<(), String> {
    sqlite::delete_all_data()
        .await
        .map_err(|e| format!("Failed to delete all data: {}", e))?;

    // After deleting all data, check if screenshot monitoring is enabled
    // and trigger a rescan to pick up existing screenshot files
    log::info!("🔄 Checking for orphaned screenshots after data deletion...");

    let pool = sqlite::get_pool()
        .await
        .map_err(|e| format!("Failed to get database pool: {}", e))?;

    let settings = crate::settings::load_settings(&pool)
        .await
        .map_err(|e| format!("Failed to load settings: {}", e))?;

    if settings.screenshot_monitoring_enabled && settings.screenshot_ocr_enabled {
        log::info!("📸 Screenshot monitoring is enabled, triggering rescan...");

        // Get job queue and clone it before spawning
        let job_queue = app.state::<std::sync::Arc<crate::job_queue::PersistentJobQueue>>();
        let job_queue_clone = job_queue.inner().clone();

        tokio::spawn(async move {
            use crate::monitors::screenshots;

            // Wait 2 seconds for database operations to settle
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            match screenshots::rescan_existing_screenshots(job_queue_clone).await {
                Ok((total, processed, skipped)) => {
                    log::info!(
                        "✅ Auto-rescan after deletion: {} total files, {} processed, {} skipped",
                        total, processed, skipped
                    );
                }
                Err(e) => {
                    log::error!("❌ Auto-rescan after deletion failed: {}", e);
                }
            }
        });
    }

    Ok(())
}

/// Assign a snippet to a category (manual or automatic)
#[tauri::command]
pub async fn assign_snippet_to_category(request: AssignSnippetRequest) -> Result<(), String> {
    let confidence = if request.is_manual { 1.0 } else { 0.0 };

    sqlite::assign_snippet_to_category(
        request.snippet_id,
        request.category_id,
        confidence,
        request.is_manual,
    )
    .await
    .map_err(|e| format!("Failed to assign snippet to category: {}", e))
}

/// Get all snippets in a category with pagination
#[tauri::command]
pub async fn get_snippets_by_category(
    category_id: i64,
    limit: i64,
    offset: i64,
    content_type: Option<String>,
) -> Result<Vec<SnippetJson>, String> {
    let snippets = sqlite::get_snippets_by_category(category_id, limit, offset, content_type)
        .await
        .map_err(|e| format!("Failed to get snippets for category: {}", e))?;

    Ok(snippets
        .into_iter()
        .map(|s| SnippetJson {
            id: s.id,
            content: s.content,
            summary: s.summary,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.map(|dt| dt.to_rfc3339()),
            source_app: s.source_app,
            metadata: s.metadata,
            content_type: s.content_type,
            file_path: s.file_path,
            working_directory: s.working_directory,
            exit_code: s.exit_code,
            website_url: s.website_url,
            website_title: s.website_title,
        })
        .collect())
}

/// Get the category of a snippet
#[tauri::command]
pub async fn get_snippet_category(snippet_id: i64) -> Result<Option<i64>, String> {
    match sqlite::get_category_for_snippet(snippet_id).await {
        Ok(Some(sc)) => Ok(Some(sc.category_id)),
        Ok(None) => Ok(None),
        Err(e) => Err(format!("Failed to get snippet category: {}", e)),
    }
}

/// Get categorization reasoning for a snippet
#[tauri::command]
pub async fn get_categorization_reasoning(snippet_id: i64) -> Result<Option<sqlite::CategorizationReasoning>, String> {
    sqlite::get_categorization_reasoning(snippet_id)
        .await
        .map_err(|e| format!("Failed to get categorization reasoning: {}", e))
}

// ============================================================================
// Auto-Categorization Commands
// ============================================================================

/// Automatically categorize a single snippet based on embedding similarity
#[tauri::command]
pub async fn auto_categorize_snippet(
    snippet_id: i64,
    min_confidence: f32,
) -> Result<Option<(i64, f32)>, String> {
    use crate::embedding::categorization;

    categorization::categorize_snippet(snippet_id, min_confidence)
        .await
        .map_err(|e| format!("Failed to categorize snippet: {}", e))
}

/// Batch categorize uncategorized snippets
#[tauri::command]
pub async fn auto_categorize_batch(
    limit: i64,
    min_confidence: f32,
) -> Result<usize, String> {
    use crate::embedding::categorization;

    categorization::categorize_uncategorized_snippets(limit, min_confidence)
        .await
        .map_err(|e| format!("Failed to batch categorize: {}", e))
}

/// Calculate and cache centroid for a category
#[tauri::command]
pub async fn calculate_category_centroid(category_id: i64) -> Result<bool, String> {
    use crate::embedding::categorization;

    match categorization::calculate_category_centroid(category_id).await {
        Ok(Some(_)) => Ok(true),
        Ok(None) => Ok(false),
        Err(e) => Err(format!("Failed to calculate centroid: {}", e)),
    }
}

/// Clear centroid cache (call when categories change significantly)
#[tauri::command]
pub async fn clear_centroid_cache() -> Result<(), String> {
    use crate::embedding::categorization;

    categorization::clear_centroid_cache().await;
    Ok(())
}

async fn search_semantic(query: &str) -> Result<Vec<SearchResult>, String> {
    use crate::db::lancedb;
    use crate::db::sqlite::get_snippets_by_ids;
    use crate::embedding::cache::QueryCache;

    // Skip semantic search if query is too short
    if query.trim().len() < 3 {
        log::debug!("Semantic search skipped - query too short: '{}'", query);
        println!("🔍 Semantic search skipped - query too short: '{}'", query);
        return Ok(vec![]);
    }

    println!("🔍 Starting semantic search for query: '{}'", query);

    // Get or embed query
    let cache = QueryCache::new();
    let query_embedding = match cache.get_or_embed(query).await {
        Ok(emb) => {
            log::debug!(
                "Semantic search - embedded query '{}' ({} dimensions)",
                query,
                emb.len()
            );
            println!("✅ Query embedded successfully: {} dimensions", emb.len());
            emb
        }
        Err(e) => {
            log::warn!("Failed to embed query '{}': {}", query, e);
            println!("❌ Failed to embed query '{}': {}", query, e);
            return Ok(vec![]);
        }
    };

    // Vector search - now returns (id, score) tuples
    let snippet_scores = match lancedb::search_semantic(query_embedding, 10).await {
        Ok(scores) => {
            log::debug!(
                "Semantic search '{}' returned {} results with scores",
                query,
                scores.len()
            );
            println!("🎯 Semantic search found {} matches", scores.len());
            scores
        }
        Err(e) => {
            log::warn!("Semantic search failed: {}", e);
            return Ok(vec![]);
        }
    };

    if snippet_scores.is_empty() {
        log::debug!("Semantic search '{}' - no matching snippet IDs", query);
        return Ok(vec![]);
    }

    // Extract IDs and create score lookup map
    let snippet_ids: Vec<i64> = snippet_scores.iter().map(|(id, _)| *id).collect();
    let scores: std::collections::HashMap<i64, f32> = snippet_scores.into_iter().collect();

    // Fetch snippet details
    let snippets = get_snippets_by_ids(&snippet_ids)
        .await
        .map_err(|e| format!("Failed to fetch snippets: {}", e))?;

    // Convert to SearchResult - use actual similarity scores for ranking
    let results: Vec<SearchResult> = snippets
        .into_iter()
        .map(|snippet| {
            let similarity_score = scores.get(&snippet.id).copied().unwrap_or(0.0);
            SearchResult {
                snippet,
                rank: similarity_score as f64, // ✅ Use actual similarity score!
                match_type: crate::db::sqlite::MatchType::Semantic,
            }
        })
        .collect();

    log::debug!(
        "Semantic search '{}' returned {} results with scores",
        query,
        results.len()
    );
    Ok(results)
}

/// Apply filters to search results
async fn apply_filters(
    results: Vec<SearchResultJson>,
    filters: &SearchFilters,
) -> Vec<SearchResultJson> {
    // Get embedded snippet IDs if has_embedding filter is set
    let embedded_ids: Option<std::collections::HashSet<i64>> = if filters.has_embedding.is_some() {
        match crate::db::lancedb::get_embedded_snippet_ids().await {
            Ok(ids) => Some(ids.into_iter().collect()),
            Err(e) => {
                log::warn!("Failed to get embedded snippet IDs: {}", e);
                None
            }
        }
    } else {
        None
    };

    results
        .into_iter()
        .filter(|result| {
            // Filter by date range
            if let Some(ref date_from) = filters.date_from {
                if let Ok(from_dt) = DateTime::parse_from_rfc3339(date_from) {
                    if let Ok(result_dt) = DateTime::parse_from_rfc3339(&result.created_at) {
                        if result_dt < from_dt {
                            return false;
                        }
                    }
                }
            }

            if let Some(ref date_to) = filters.date_to {
                if let Ok(to_dt) = DateTime::parse_from_rfc3339(date_to) {
                    if let Ok(result_dt) = DateTime::parse_from_rfc3339(&result.created_at) {
                        if result_dt > to_dt {
                            return false;
                        }
                    }
                }
            }

            // Filter by source app
            if let Some(ref app_filter) = filters.source_app {
                if !app_filter.is_empty() {
                    match &result.source_app {
                        Some(app) if app.to_lowercase().contains(&app_filter.to_lowercase()) => {}
                        _ => return false,
                    }
                }
            }

            // Filter by embedding status
            if let Some(has_embedding_required) = filters.has_embedding {
                if let Some(ref ids) = embedded_ids {
                    let has_embedding = ids.contains(&result.id);
                    if has_embedding_required != has_embedding {
                        return false;
                    }
                }
            }

            true
        })
        .collect()
}

impl From<SearchResult> for SearchResultJson {
    fn from(result: SearchResult) -> Self {
        Self {
            id: result.snippet.id,
            content: result.snippet.content,
            created_at: result.snippet.created_at.to_rfc3339(),
            source_app: result.snippet.source_app,
            rank: result.rank,
            match_type: match result.match_type {
                crate::db::sqlite::MatchType::Keyword => "keyword".to_string(),
                crate::db::sqlite::MatchType::Semantic => "semantic".to_string(),
            },
        }
    }
}

// Utility Commands

#[tauri::command]
pub async fn regenerate_all_summaries() -> Result<usize, String> {
    log::info!("Starting regeneration of all snippet summaries");

    match sqlite::regenerate_all_summaries().await {
        Ok(count) => {
            log::info!("Successfully regenerated {} summaries", count);
            Ok(count)
        }
        Err(e) => {
            log::error!("Failed to regenerate summaries: {}", e);
            Err(format!("Failed to regenerate summaries: {}", e))
        }
    }
}

// Settings Commands

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageStats {
    pub snippets_db_size: u64,
    pub job_queue_db_size: u64,
    pub vectors_db_size: u64,
    pub total_size: u64,
    pub snippets_count: i64,
    pub categories_count: i64,
    pub embeddings_count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryStats {
    pub rss_mb: f64,
    pub virtual_mb: f64,
    pub cpu_percent: f32,
}

#[tauri::command]
pub async fn get_settings() -> Result<crate::settings::Settings, String> {
    let pool = sqlite::get_pool()
        .await
        .map_err(|e| format!("Failed to get database pool: {}", e))?;

    crate::settings::load_settings(&pool)
        .await
        .map_err(|e| format!("Failed to load settings: {}", e))
}

#[tauri::command]
pub async fn update_settings(settings: crate::settings::Settings, app: tauri::AppHandle) -> Result<(), String> {
    let pool = sqlite::get_pool()
        .await
        .map_err(|e| format!("Failed to get database pool: {}", e))?;

    crate::settings::save_settings(&pool, &settings)
        .await
        .map_err(|e| format!("Failed to save settings: {}", e))?;

    // Re-register shortcuts with new settings
    crate::shortcuts::register_shortcuts(&app)
        .map_err(|e| format!("Failed to re-register shortcuts: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn get_storage_stats() -> Result<StorageStats, String> {
    use std::fs;

    let data_dir = sqlite::get_db_path()
        .map_err(|e| format!("Failed to get database path: {}", e))?
        .parent()
        .ok_or("Invalid database path")?
        .to_path_buf();

    let snippets_db = data_dir.join("snippets.db");
    let job_queue_db = data_dir.join("job_queue.db");
    let vectors_db = data_dir.join("vectors.lance");

    let snippets_db_size = fs::metadata(&snippets_db).map(|m| m.len()).unwrap_or(0);
    let job_queue_db_size = fs::metadata(&job_queue_db).map(|m| m.len()).unwrap_or(0);
    let vectors_db_size = fs::metadata(&vectors_db).map(|m| m.len()).unwrap_or(0);

    // Get counts from database
    let pool = sqlite::get_pool()
        .await
        .map_err(|e| format!("Failed to get database pool: {}", e))?;

    let snippets_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM snippets")
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("Failed to count snippets: {}", e))?;

    let categories_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM categories")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

    // Count snippets with embeddings by checking vector store
    let embeddings_count = crate::db::lancedb::count_vectors()
        .await
        .unwrap_or(0);

    Ok(StorageStats {
        snippets_db_size,
        job_queue_db_size,
        vectors_db_size,
        total_size: snippets_db_size + job_queue_db_size + vectors_db_size,
        snippets_count,
        categories_count,
        embeddings_count,
    })
}

#[tauri::command]
pub async fn get_app_memory_usage() -> Result<MemoryStats, String> {
    use sysinfo::{System, Pid};

    let mut sys = System::new_all();
    sys.refresh_all();

    let pid = Pid::from_u32(std::process::id());

    if let Some(process) = sys.process(pid) {
        let rss_mb = process.memory() as f64 / 1024.0 / 1024.0;
        let virtual_mb = process.virtual_memory() as f64 / 1024.0 / 1024.0;
        let cpu_percent = process.cpu_usage();

        Ok(MemoryStats {
            rss_mb,
            virtual_mb,
            cpu_percent,
        })
    } else {
        Err("Process not found".to_string())
    }
}

// Export/Import Commands

#[tauri::command]
pub async fn export_to_json_file(file_path: String, include_versions: bool) -> Result<usize, String> {
    let pool = sqlite::get_pool()
        .await
        .map_err(|e| format!("Failed to get database pool: {}", e))?;

    export::export_to_json(&pool, &file_path, include_versions)
        .await
        .map_err(|e| format!("Failed to export to JSON: {}", e))
}

#[tauri::command]
pub async fn export_to_markdown_file(file_path: String) -> Result<usize, String> {
    let pool = sqlite::get_pool()
        .await
        .map_err(|e| format!("Failed to get database pool: {}", e))?;

    export::export_to_markdown(&pool, &file_path)
        .await
        .map_err(|e| format!("Failed to export to Markdown: {}", e))
}

#[tauri::command]
pub async fn import_from_json_file(file_path: String) -> Result<usize, String> {
    let pool = sqlite::get_pool()
        .await
        .map_err(|e| format!("Failed to get database pool: {}", e))?;

    export::import_from_json(&pool, &file_path)
        .await
        .map_err(|e| format!("Failed to import from JSON: {}", e))
}

#[tauri::command]
pub async fn get_export_history(limit: i64) -> Result<Vec<export::ExportRecord>, String> {
    let pool = sqlite::get_pool()
        .await
        .map_err(|e| format!("Failed to get database pool: {}", e))?;

    export::get_export_history(&pool, limit)
        .await
        .map_err(|e| format!("Failed to get export history: {}", e))
}

// Duplicate Detection Commands

#[tauri::command]
pub async fn check_duplicate(content: String) -> Result<Option<dedup::DuplicateSnippet>, String> {
    let pool = sqlite::get_pool()
        .await
        .map_err(|e| format!("Failed to get database pool: {}", e))?;

    dedup::find_duplicate(&pool, &content)
        .await
        .map_err(|e| format!("Failed to check for duplicate: {}", e))
}

#[tauri::command]
pub async fn find_all_duplicates() -> Result<Vec<dedup::DuplicateGroup>, String> {
    let pool = sqlite::get_pool()
        .await
        .map_err(|e| format!("Failed to get database pool: {}", e))?;

    dedup::find_all_duplicates(&pool)
        .await
        .map_err(|e| format!("Failed to find duplicates: {}", e))
}

#[tauri::command]
pub async fn get_duplicate_stats() -> Result<dedup::DuplicateStats, String> {
    let pool = sqlite::get_pool()
        .await
        .map_err(|e| format!("Failed to get database pool: {}", e))?;

    dedup::get_duplicate_stats(&pool)
        .await
        .map_err(|e| format!("Failed to get duplicate stats: {}", e))
}

#[tauri::command]
pub async fn merge_duplicate_group(hash: String) -> Result<usize, String> {
    let pool = sqlite::get_pool()
        .await
        .map_err(|e| format!("Failed to get database pool: {}", e))?;

    dedup::merge_duplicates(&pool, &hash)
        .await
        .map_err(|e| format!("Failed to merge duplicates: {}", e))
}

#[tauri::command]
pub async fn update_all_content_hashes() -> Result<usize, String> {
    let pool = sqlite::get_pool()
        .await
        .map_err(|e| format!("Failed to get database pool: {}", e))?;

    dedup::update_all_hashes(&pool)
        .await
        .map_err(|e| format!("Failed to update hashes: {}", e))
}

#[tauri::command]
pub async fn recalculate_command_hashes() -> Result<usize, String> {
    let pool = sqlite::get_pool()
        .await
        .map_err(|e| format!("Failed to get database pool: {}", e))?;

    dedup::recalculate_command_hashes(&pool)
        .await
        .map_err(|e| format!("Failed to recalculate command hashes: {}", e))
}

// Smart Suggestions Commands

#[tauri::command]
pub async fn get_suggestions() -> Result<Vec<suggestions::Suggestion>, String> {
    let pool = sqlite::get_pool()
        .await
        .map_err(|e| format!("Failed to get database pool: {}", e))?;

    suggestions::get_all_suggestions(&pool)
        .await
        .map_err(|e| format!("Failed to get suggestions: {}", e))
}

#[tauri::command]
pub async fn find_related_snippets(snippet_id: i64, limit: usize) -> Result<Vec<i64>, String> {
    let pool = sqlite::get_pool()
        .await
        .map_err(|e| format!("Failed to get database pool: {}", e))?;

    suggestions::find_related_snippets(&pool, snippet_id, limit)
        .await
        .map_err(|e| format!("Failed to find related snippets: {}", e))
}

#[tauri::command]
pub async fn dismiss_suggestion(suggestion_id: String) -> Result<(), String> {
    suggestions::dismiss_suggestion(&suggestion_id)
        .await
        .map_err(|e| format!("Failed to dismiss suggestion: {}", e))
}

// Terminal Monitoring Commands

use crate::monitors::shell_hook;

#[tauri::command]
pub fn detect_shell() -> Result<String, String> {
    let shell = shell_hook::detect_shell()
        .map_err(|e| format!("Failed to detect shell: {}", e))?;
    Ok(shell.as_str().to_string())
}

#[tauri::command]
pub fn install_shell_hooks() -> Result<String, String> {
    let shell = shell_hook::detect_shell()
        .map_err(|e| format!("Failed to detect shell: {}", e))?;

    shell_hook::install_hooks(shell)
        .map_err(|e| format!("Failed to install hooks: {}", e))
}

#[tauri::command]
pub fn uninstall_shell_hooks() -> Result<String, String> {
    let shell = shell_hook::detect_shell()
        .map_err(|e| format!("Failed to detect shell: {}", e))?;

    shell_hook::uninstall_hooks(shell)
        .map_err(|e| format!("Failed to uninstall hooks: {}", e))
}

#[tauri::command]
pub fn are_shell_hooks_installed() -> Result<bool, String> {
    let shell = shell_hook::detect_shell()
        .map_err(|e| format!("Failed to detect shell: {}", e))?;

    shell_hook::are_hooks_installed(shell)
        .map_err(|e| format!("Failed to check hooks: {}", e))
}

#[tauri::command]
pub fn get_terminal_log_path() -> Result<String, String> {
    shell_hook::get_terminal_log_path()
        .map(|p| p.display().to_string())
        .map_err(|e| format!("Failed to get log path: {}", e))
}

// Screenshot Monitoring Commands

use crate::monitors::screenshots;
use crate::processing::{ocr, vision};

#[tauri::command]
pub fn get_default_screenshot_dir() -> Result<String, String> {
    screenshots::get_default_screenshot_dir()
        .map(|p| p.display().to_string())
        .map_err(|e| format!("Failed to get screenshot directory: {}", e))
}

#[tauri::command]
pub fn is_tesseract_installed() -> bool {
    ocr::is_tesseract_installed()
}

#[tauri::command]
pub fn is_apple_vision_available() -> bool {
    use crate::processing::ocr_apple;
    ocr_apple::is_apple_vision_available()
}

#[tauri::command]
pub fn get_apple_architecture() -> String {
    use crate::processing::ocr_apple;
    ocr_apple::get_architecture_info()
}

#[tauri::command]
pub fn is_florence2_downloaded() -> bool {
    vision::is_florence2_downloaded()
}

#[tauri::command]
pub async fn download_florence2_model() -> Result<String, String> {
    vision::download_florence2_model()
        .await
        .map(|_| "Florence-2 model directory created. See instructions for manual download.".to_string())
        .map_err(|e| format!("Failed to setup Florence-2 model: {}", e))
}

#[tauri::command]
pub async fn search_text_in_database(search_text: String) -> Result<serde_json::Value, String> {
    use crate::db::sqlite;
    
    let pool = sqlite::get_pool().await
        .map_err(|e| format!("Failed to get database pool: {}", e))?;
    
    // Search in summary field
    let summary_results = sqlx::query(
        r#"
        SELECT id, summary, content, type, file_path
        FROM snippets
        WHERE summary LIKE ? OR summary = ?
        LIMIT 10
        "#
    )
    .bind(format!("%{}%", search_text))
    .bind(&search_text)
    .fetch_all(&pool)
    .await
    .map_err(|e| format!("Failed to search summary: {}", e))?;
    
    // Search in content field (check if in [Caption: ...] section)
    let content_results = sqlx::query(
        r#"
        SELECT id, summary, content, type, file_path
        FROM snippets
        WHERE content LIKE ? OR content LIKE ?
        LIMIT 10
        "#
    )
    .bind(format!("%[Caption: {}%", search_text))
    .bind(format!("%{}%", search_text))
    .fetch_all(&pool)
    .await
    .map_err(|e| format!("Failed to search content: {}", e))?;
    
    // Check if caption and ocr_text columns exist by trying to query them
    let has_caption_column = sqlx::query(
        r#"
        SELECT name FROM pragma_table_info('snippets') WHERE name = 'caption'
        "#
    )
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten()
    .is_some();
    
    let has_ocr_text_column = sqlx::query(
        r#"
        SELECT name FROM pragma_table_info('snippets') WHERE name = 'ocr_text'
        "#
    )
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten()
    .is_some();
    
    // If caption column exists, search in it
    let caption_results = if has_caption_column {
        sqlx::query(
            r#"
            SELECT id, summary, content, type, file_path, caption
            FROM snippets
            WHERE caption LIKE ? OR caption = ?
            LIMIT 10
            "#
        )
        .bind(format!("%{}%", search_text))
        .bind(&search_text)
        .fetch_all(&pool)
        .await
        .ok()
        .unwrap_or_default()
    } else {
        Vec::new()
    };
    
    // If ocr_text column exists, search in it
    let ocr_results = if has_ocr_text_column {
        sqlx::query(
            r#"
            SELECT id, summary, content, type, file_path, ocr_text
            FROM snippets
            WHERE ocr_text LIKE ? OR ocr_text = ?
            LIMIT 10
            "#
        )
        .bind(format!("%{}%", search_text))
        .bind(&search_text)
        .fetch_all(&pool)
        .await
        .ok()
        .unwrap_or_default()
    } else {
        Vec::new()
    };
    
    // Format results
    let mut summary_matches = Vec::new();
    for row in summary_results {
        summary_matches.push(serde_json::json!({
            "id": row.get::<i64, _>(0),
            "summary": row.get::<Option<String>, _>(1),
            "content": row.get::<String, _>(2),
            "type": row.get::<Option<String>, _>(3),
            "file_path": row.get::<Option<String>, _>(4),
            "field": "summary"
        }));
    }
    
    let mut content_matches = Vec::new();
    for row in content_results {
        let content: String = row.get(2);
        let is_in_caption = content.contains(&format!("[Caption: {}", search_text)) || 
                           content.contains(&format!("[Caption: {}]", search_text));
        content_matches.push(serde_json::json!({
            "id": row.get::<i64, _>(0),
            "summary": row.get::<Option<String>, _>(1),
            "content": row.get::<String, _>(2),
            "type": row.get::<Option<String>, _>(3),
            "file_path": row.get::<Option<String>, _>(4),
            "field": "content",
            "is_in_caption_section": is_in_caption
        }));
    }
    
    let mut caption_matches = Vec::new();
    for row in caption_results {
        caption_matches.push(serde_json::json!({
            "id": row.get::<i64, _>(0),
            "summary": row.get::<Option<String>, _>(1),
            "content": row.get::<String, _>(2),
            "type": row.get::<Option<String>, _>(3),
            "file_path": row.get::<Option<String>, _>(4),
            "caption": row.get::<Option<String>, _>(5),
            "field": "caption"
        }));
    }
    
    let mut ocr_matches = Vec::new();
    for row in ocr_results {
        ocr_matches.push(serde_json::json!({
            "id": row.get::<i64, _>(0),
            "summary": row.get::<Option<String>, _>(1),
            "content": row.get::<String, _>(2),
            "type": row.get::<Option<String>, _>(3),
            "file_path": row.get::<Option<String>, _>(4),
            "ocr_text": row.get::<Option<String>, _>(5),
            "field": "ocr_text"
        }));
    }
    
    Ok(serde_json::json!({
        "search_text": search_text,
        "schema_info": {
            "has_caption_column": has_caption_column,
            "has_ocr_text_column": has_ocr_text_column
        },
        "results": {
            "summary": summary_matches,
            "content": content_matches,
            "caption": caption_matches,
            "ocr_text": ocr_matches
        }
    }))
}

#[tauri::command]
pub async fn rescan_screenshots(app: tauri::AppHandle) -> Result<String, String> {
    use crate::monitors::screenshots;

    // Get job queue from app state
    let job_queue = app.state::<std::sync::Arc<crate::job_queue::PersistentJobQueue>>();

    match screenshots::rescan_existing_screenshots(job_queue.inner().clone()).await {
        Ok((total, processed, skipped)) => {
            Ok(format!(
                "Rescan complete: {} total files, {} processed, {} skipped",
                total, processed, skipped
            ))
        }
        Err(e) => Err(format!("Failed to rescan screenshots: {}", e))
    }
}
