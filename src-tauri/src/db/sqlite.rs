use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use log;
use once_cell::sync::Lazy;
use sqlx::{sqlite::{SqlitePool, SqlitePoolOptions}, Row};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::db::migrations;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Snippet {
    pub id: i64,
    pub content: String,
    pub summary: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub source_app: Option<String>,
    pub metadata: Option<String>,
    // New fields for content types
    #[serde(rename = "type")]
    pub content_type: Option<String>, // 'text' | 'command' | 'screenshot'
    pub file_path: Option<String>,
    pub working_directory: Option<String>,
    pub exit_code: Option<i32>,
    pub website_url: Option<String>,
    pub website_title: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub emoji: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnippetCategory {
    pub snippet_id: i64,
    pub category_id: i64,
    pub confidence: f64,
    pub is_manual: bool,
    pub assigned_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub snippet: Snippet,
    pub rank: f64,
    pub match_type: MatchType,
}

#[derive(Debug, Clone)]
pub enum MatchType {
    Keyword,
    Semantic,
}

static DB_POOL: Lazy<Arc<Mutex<Option<SqlitePool>>>> = Lazy::new(|| Arc::new(Mutex::new(None)));

/// Get database path in app data directory
pub fn get_db_path() -> Result<PathBuf> {
    // For now, always use local data directory (works in both dev and prod)
    // This ensures the path is always valid even before Tauri context is available
    let mut path = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("data")
        .join("local-mind");

    // Ensure directory exists
    std::fs::create_dir_all(&path)
        .context(format!("Failed to create data directory at {:?}", path))?;

    path.push("snippets.db");
    Ok(path)
}

/// Parse datetime string flexibly (supports both RFC3339 and SQLite formats)
#[inline]
fn parse_datetime_flexible(datetime_str: String) -> Result<DateTime<Utc>> {
    if datetime_str.contains('T') {
        // RFC3339 format (e.g., "2025-01-15T10:30:00Z")
        Ok(DateTime::parse_from_rfc3339(&datetime_str)
            .context("Failed to parse RFC3339 datetime")?
            .with_timezone(&Utc))
    } else {
        // SQLite format (e.g., "2025-01-15 10:30:00" or "2025-01-15 10:30:00.123")
        let naive = chrono::NaiveDateTime::parse_from_str(&datetime_str, "%Y-%m-%d %H:%M:%S")
            .or_else(|_| {
                chrono::NaiveDateTime::parse_from_str(&datetime_str, "%Y-%m-%d %H:%M:%S%.f")
            })
            .context("Failed to parse SQLite datetime")?;
        Ok(naive.and_utc())
    }
}

/// Initialize SQLite database with FTS5 table
pub async fn init_database() -> Result<()> {
    let db_path = get_db_path()?;

    // Ensure the database file exists (sqlx needs it for absolute paths)
    if !db_path.exists() {
        std::fs::File::create(&db_path)
            .context(format!("Failed to create database file at {:?}", db_path))?;
    }

    // Get absolute path for sqlx (requires sqlite:/// format with three slashes)
    let db_path_abs = db_path.canonicalize().unwrap_or_else(|_| {
        // Fallback: construct absolute path manually
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        if db_path.is_absolute() {
            db_path
        } else {
            current_dir.join(&db_path)
        }
    });

    // sqlx SQLite format: sqlite:/// for absolute paths on Unix, sqlite:///C:/ on Windows
    let path_str = db_path_abs.display().to_string().replace('\\', "/");
    let db_url = if cfg!(windows) && !path_str.starts_with('/') {
        format!("sqlite:///{}", path_str)
    } else {
        format!("sqlite:///{}", path_str)
    };

    log::info!("Connecting to database at: {}", db_url);
    let pool = SqlitePoolOptions::new()
        .max_connections(3)  // Reduced from default 10 for memory optimization
        .min_connections(1)
        .connect(&db_url)
        .await
        .with_context(|| format!("Failed to connect to SQLite database at {}", db_url))?;

    // Run migrations to set up or update schema
    log::info!("Running database migrations...");
    migrations::run_migrations(&pool).await?;

    // Store pool in static
    *DB_POOL.lock().await = Some(pool);

    Ok(())
}

/// Get database pool
pub async fn get_pool() -> Result<SqlitePool> {
    DB_POOL
        .lock()
        .await
        .clone()
        .context("Database not initialized")
}

/// Generate summary with LLM fallback for text content
async fn generate_summary_with_llm_fallback(content: &str) -> String {
    // Try LLM summarization first for longer text content (if available)
    if content.len() > 50 {
        if let Some(llm_manager) = crate::inference::global_llm::get_global_llm() {
            if llm_manager.model_exists() {
                if let Ok(model) = llm_manager.get_model() {
                    use crate::inference::summarization::ContentType;
                    use crate::processing::text_processing::TextProcessor;
                    
                    // Detect if this looks like text content (not command/code)
                    let processor = TextProcessor::new();
                    let structure = processor.detect_structure(content);
                    let content_type = match structure {
                        crate::processing::text_processing::StructureType::Code => ContentType::Code,
                        crate::processing::text_processing::StructureType::Terminal => ContentType::Terminal,
                        _ => ContentType::General,
                    };
                    
                    // Only use LLM for non-code, non-terminal content
                    if matches!(content_type, ContentType::General) {
                        let entities = processor.extract_entities(content);
                        match crate::inference::summarization::summarize_with_llm(
                            &model,
                            content,
                            None, // No caption for text content
                            Some(&entities),
                            content_type,
                        ).await {
                            Ok(result) => {
                                log::debug!("✓ LLM summary generated: {}", result.summary);
                                return result.summary;
                            }
                            Err(e) => {
                                log::debug!("LLM summarization failed: {}, using rule-based fallback", e);
                            }
                        }
                    }
                }
            }
        }
    }
    
    // Fall back to rule-based summary
    generate_summary(content)
}

/// Generate a smart summary from content (first meaningful line, max 50 chars)
pub fn generate_summary(content: &str) -> String {
    let trimmed = content.trim();

    if trimmed.is_empty() {
        return "Empty snippet".to_string();
    }

    // Detect error logs (timestamp + ERROR/WARN + module path)
    if let Some(error_summary) = extract_error_summary(trimmed) {
        return error_summary;
    }

    // Detect shell commands (starts with $ or common commands)
    if let Some(cmd_summary) = extract_command_summary(trimmed) {
        return cmd_summary;
    }

    // Detect code snippets (function definitions, imports, etc.)
    if let Some(code_summary) = extract_code_summary(trimmed) {
        return code_summary;
    }

    // Detect URLs
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        if let Some(url_summary) = extract_url_summary(trimmed) {
            return url_summary;
        }
    }

    // Detect build output
    if let Some(build_summary) = extract_build_output_summary(trimmed) {
        return build_summary;
    }

    // Detect log messages with emojis
    if let Some(log_summary) = extract_log_message_summary(trimmed) {
        return log_summary;
    }

    // Detect keyboard shortcuts/accelerators
    if trimmed.contains("Accelerator") && trimmed.contains("register") {
        return "Keyboard shortcuts".to_string();
    }

    // Try intelligent topic extraction for plain text
    if let Some(intelligent_summary) = extract_intelligent_summary(trimmed) {
        return intelligent_summary;
    }

    // Fallback: use cleaned first line with "Note:" prefix to ensure it's different
    let first_line = trimmed
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or(trimmed);

    let cleaned = clean_text(first_line);
    let truncated = truncate_string(&cleaned, 45);

    // Always add prefix to ensure summary differs from content
    format!("Note: {}", truncated)
}

/// Extract summary from build output
fn extract_build_output_summary(content: &str) -> Option<String> {

    // Check for compilation output
    if content.contains("Compiling") || content.contains("Building") {
        // Count compilations
        let compile_count = content.lines().filter(|l| l.contains("Compiling")).count();
        if compile_count > 0 {
            return Some(format!("Build: Compiling {} crates", compile_count));
        }
    }

    // Check for finished build
    if content.contains("Finished") && (content.contains("profile") || content.contains("target")) {
        if content.contains("unoptimized") {
            return Some("Build: Finished (dev)".to_string());
        } else {
            return Some("Build: Finished (release)".to_string());
        }
    }

    // Check for cargo/npm commands
    if content.contains("cargo") || content.contains("npm") || content.contains("yarn") {
        if content.contains("test") {
            return Some("Build: Test output".to_string());
        }
        if content.contains("build") {
            return Some("Build: Build output".to_string());
        }
    }

    None
}

/// Extract summary from log messages (emoji-based)
fn extract_log_message_summary(content: &str) -> Option<String> {
    let _first_lines: Vec<&str> = content.lines().take(5).collect();

    // Count different types of log emojis
    let processing_count = content.matches("🔄").count();
    let success_count = content.matches("✅").count();
    let error_count = content.matches("❌").count();
    let warning_count = content.matches("⚠️").count();

    // Prioritize errors and warnings
    if error_count > 0 {
        let first_error = content.lines().find(|l| l.contains("❌"))?;
        return Some(truncate_string(&format!("Error: {}", first_error.trim()), 45));
    }

    if warning_count > 0 {
        return Some(format!("Warning messages ({})", warning_count));
    }

    // Check for processing/batch logs
    if processing_count > 0 && content.contains("batch") {
        return Some("Log: Batch processing".to_string());
    }

    if processing_count > 0 && content.contains("embedding") {
        return Some("Log: Embedding jobs".to_string());
    }

    // Check for success logs
    if success_count > 0 {
        if content.contains("ENGINE") {
            return Some("Log: Model loaded".to_string());
        }
        if content.contains("completed") {
            return Some(format!("Log: {} tasks completed", success_count));
        }
    }

    None
}

/// Extract summary from error logs
fn extract_error_summary(content: &str) -> Option<String> {
    let first_line = content.lines().next()?;

    // Match patterns like: [2025-11-01T20:18:33Z ERROR module::path] message
    // or: ERROR: message
    if first_line.contains("ERROR") || first_line.contains("Error") {
        // Try to find the module/source
        if let Some(module_start) = first_line.find("ERROR ") {
            let after_error = &first_line[module_start + 6..];

            // Extract module path if present (e.g., local_mind::job_queue)
            if let Some(bracket_pos) = after_error.find(']') {
                let module_part = &after_error[..bracket_pos];
                if module_part.contains("::") {
                    let module = module_part.split("::").next().unwrap_or("unknown");
                    return Some(format!("Error in {}", module));
                }
            }

            // Extract error message
            let msg = after_error
                .split(':')
                .nth(1)
                .or_else(|| after_error.split(']').nth(1))
                .unwrap_or(after_error)
                .trim();

            return Some(truncate_string(&format!("Error: {}", msg), 50));
        }
    }

    // Match WARN logs
    if first_line.contains("WARN") || first_line.contains("Warn") {
        if let Some(warn_start) = first_line.find("WARN") {
            let after_warn = &first_line[warn_start + 4..].trim();
            return Some(truncate_string(&format!("Warning: {}", after_warn), 50));
        }
    }

    None
}

/// Extract summary from shell commands
fn extract_command_summary(content: &str) -> Option<String> {
    let first_line = content.lines().next()?.trim();

    // Remove leading $ or >
    let cmd = first_line.trim_start_matches('$').trim_start_matches('>').trim();

    // Check if it looks like a command (starts with common commands or has command-like structure)
    let common_cmds = ["ls", "cd", "git", "npm", "cargo", "docker", "kubectl", "cat", "grep",
                       "find", "curl", "wget", "ssh", "scp", "rsync", "mkdir", "rm", "mv", "cp"];

    let first_word = cmd.split_whitespace().next()?;

    if common_cmds.contains(&first_word) || first_word.starts_with("./") || first_word.starts_with('/') {
        return Some(truncate_string(&format!("$ {}", cmd), 50));
    }

    None
}

/// Extract summary from code snippets
fn extract_code_summary(content: &str) -> Option<String> {
    let first_line = content.lines().next()?.trim();

    // Function definitions
    if first_line.contains("fn ") || first_line.contains("function ") || first_line.contains("def ") {
        if let Some(name_start) = first_line.find(|c: char| c.is_alphanumeric() || c == '_') {
            let name = &first_line[name_start..]
                .split(|c: char| c == '(' || c == ' ' || c == '{')
                .next()?;
            return Some(format!("Function: {}", name));
        }
    }

    // Imports
    if first_line.starts_with("import ") || first_line.starts_with("from ") || first_line.starts_with("use ") {
        let module = first_line
            .split_whitespace()
            .nth(1)?
            .trim_end_matches(';')
            .split("::")
            .next()?;
        return Some(format!("Import: {}", module));
    }

    // Class definitions
    if first_line.contains("class ") || first_line.contains("struct ") || first_line.contains("interface ") {
        if let Some(name) = first_line.split_whitespace().nth(1) {
            let clean_name = name.trim_end_matches(|c: char| c == '{' || c == ':');
            return Some(format!("Type: {}", clean_name));
        }
    }

    None
}

/// Extract summary from URLs
fn extract_url_summary(content: &str) -> Option<String> {
    let first_line = content.lines().next()?.trim();

    // Parse URL and extract domain
    if let Some(domain_start) = first_line.find("://") {
        let after_protocol = &first_line[domain_start + 3..];
        let domain = after_protocol.split('/').next()?;
        let path = after_protocol.split('/').nth(1).unwrap_or("");

        if path.is_empty() {
            return Some(format!("Link: {}", domain));
        } else {
            return Some(truncate_string(&format!("Link: {}/{}", domain, path), 50));
        }
    }

    None
}

/// Helper to truncate string with ellipsis
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

/// Clean text by removing markdown and normalizing whitespace
fn clean_text(text: &str) -> String {
    text.replace('\n', " ")
        .replace('\r', " ")
        // Remove markdown formatting
        .replace("**", "")
        .replace("__", "")
        .replace("*", "")
        .replace("_", "")
        .replace("~~", "")
        .replace("`", "")
        .replace("#", "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Intelligently extract a summary that describes the content's topic/theme
fn extract_intelligent_summary(content: &str) -> Option<String> {
    let cleaned = clean_text(content);
    let words: Vec<&str> = cleaned.split_whitespace().collect();

    if words.len() < 3 {
        return None; // Too short for intelligent extraction
    }

    // Pattern: "The [X] is/are [Y]" -> Extract X as topic
    if cleaned.starts_with("The ") && words.len() > 3 {
        // Find position of "is", "are", "was", "were", "has", "have"
        let verb_pos = words.iter().position(|&w| {
            matches!(w, "is" | "are" | "was" | "were" | "has" | "have" | "contains" | "provides")
        });

        if let Some(pos) = verb_pos {
            if pos > 1 && pos <= 8 {
                // Extract the topic between "The" and the verb
                let topic = &words[1..pos].join(" ");

                // Detect if it's a review/analysis
                if cleaned.contains("implementation") || cleaned.contains("architecture") {
                    return Some(truncate_string(&format!("About: {}", topic), 50));
                } else if cleaned.contains("well-") || cleaned.contains("review") || cleaned.contains("analysis") {
                    return Some(truncate_string(&format!("Review: {}", topic), 50));
                } else {
                    return Some(truncate_string(&format!("About: {}", topic), 50));
                }
            }
        }
    }

    // Pattern: Review/Analysis content detection
    if cleaned.contains("implementation") && cleaned.contains("functional") {
        // Extract first 3-5 meaningful words as topic
        let topic_words: Vec<&str> = words.iter()
            .filter(|w| w.len() > 2 && !matches!(**w, "The" | "the" | "and" | "but" | "for" | "with"))
            .take(3)
            .copied()
            .collect();

        if !topic_words.is_empty() {
            return Some(truncate_string(&format!("Review: {}", topic_words.join(" ")), 50));
        }
    }

    // Pattern: Technical description with product/project names
    // Look for capitalized words that might be product names
    let cap_words: Vec<&str> = words.iter()
        .filter(|w| w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) && w.len() > 2)
        .take(2)
        .copied()
        .collect();

    if cap_words.len() >= 2 {
        // Check if followed by technical terms
        if cleaned.contains("implementation") || cleaned.contains("architecture")
           || cleaned.contains("system") || cleaned.contains("feature") {
            return Some(truncate_string(&format!("{} implementation", cap_words.join(" ")), 50));
        }
    }

    // Pattern: Sentences starting with action verbs (describing functionality)
    let first_word = words.first()?;
    if matches!(first_word.to_lowercase().as_str(), "provides" | "combines" | "implements" | "uses" | "creates" | "enables") {
        let topic = words.iter().take(5).copied().collect::<Vec<_>>().join(" ");
        return Some(truncate_string(&format!("Note: {}", topic), 50));
    }

    // Pattern: Marketing/product descriptions starting with action phrases
    // Examples: "Win More Work", "Get Better Results", "Improve Your Workflow"
    let first_word_lower = first_word.to_lowercase();
    if matches!(first_word_lower.as_str(), "win" | "get" | "improve" | "boost" | "increase" | "maximize" | "enhance" | "achieve" | "unlock" | "discover") {
        // Extract the key phrase (first 4-6 words that form a complete thought)
        let key_phrase = words.iter().take(6).copied().collect::<Vec<_>>().join(" ");
        return Some(truncate_string(&key_phrase, 50));
    }

    // Pattern: Product/platform descriptions starting with "As a" or "For [target audience]"
    if cleaned.starts_with("as a ") || cleaned.starts_with("for ") {
        // Extract the first sentence or first 8 words
        let summary = words.iter().take(8).copied().collect::<Vec<_>>().join(" ");
        return Some(truncate_string(&summary, 50));
    }

    // Pattern: Multi-line content with headline/title
    // Look for content that has a title/headline followed by description
    let lines: Vec<&str> = content.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    if lines.len() >= 2 {
        // First line is likely a title/headline
        let title = lines[0];
        let title_words: Vec<&str> = title.split_whitespace().collect();
        
        // If title is short (2-8 words) and second line provides context, create summary
        if title_words.len() >= 2 && title_words.len() <= 8 {
            // Extract key topic from title and first sentence of description
            let desc_first_sentence = lines[1]
                .split(|c: char| c == '.' || c == '!' || c == '?')
                .next()
                .unwrap_or(lines[1])
                .trim();
            
            // Create a descriptive summary combining title and key topic
            let desc_words: Vec<&str> = desc_first_sentence.split_whitespace().take(8).collect();
            if !desc_words.is_empty() {
                let summary = format!("{}: {}", title, desc_words.join(" "));
                return Some(truncate_string(&summary, 50));
            }
        }
    }

    // Pattern: Article/blog post style with title and subtitle
    // Look for content starting with capitalized title followed by description
    if lines.len() >= 1 {
        let first_line = lines[0];
        let first_line_words: Vec<&str> = first_line.split_whitespace().collect();
        
        // If first line looks like a title (mostly capitalized words, 3-10 words)
        let cap_count = first_line_words.iter()
            .filter(|w| w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false))
            .count();
        
        if first_line_words.len() >= 3 && first_line_words.len() <= 10 
           && cap_count >= first_line_words.len() / 2 {
            // This looks like a title - extract the main topic
            // Look for key nouns (capitalized words that aren't common words)
            let key_words: Vec<&str> = first_line_words.iter()
                .filter(|w| {
                    let w_lower = w.to_lowercase();
                    !matches!(w_lower.as_str(), "a" | "an" | "the" | "and" | "or" | "but" | "of" | "in" | "on" | "at" | "to" | "for" | "with" | "from")
                })
                .take(4)
                .copied()
                .collect();
            
            if !key_words.is_empty() {
                // Create a summary that describes what the content is about
                let summary = if lines.len() > 1 {
                    // Use title and extract topic from second line
                    let topic = lines[1].split_whitespace()
                        .filter(|w| w.len() > 3)
                        .take(3)
                        .collect::<Vec<_>>()
                        .join(" ");
                    if !topic.is_empty() {
                        format!("{} - {}", key_words.join(" "), topic)
                    } else {
                        key_words.join(" ")
                    }
                } else {
                    key_words.join(" ")
                };
                return Some(truncate_string(&summary, 50));
            }
        }
    }

    // Pattern: Product names followed by descriptions (improved)
    // Look for capitalized product names at the start, but create descriptive summary
    if words.len() >= 2 {
        let first_word = words[0];
        let second_word = words[1];
        
        if first_word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) &&
           (second_word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) || 
            second_word.to_lowercase() == "ai" || second_word.to_lowercase() == "ai,") {
            // Likely a product/company name - look for what it does
            // Find the main verb or action word in the content
            let action_words: Vec<&str> = words.iter()
                .enumerate()
                .filter(|(_, w)| {
                    let w_lower = w.to_lowercase();
                    matches!(w_lower.as_str(), "harnesses" | "uses" | "transforms" | "enables" | "provides" | "offers" | "helps" | "allows" | "improves" | "enhances" | "discovers")
                })
                .take(1)
                .map(|(_, w)| *w)
                .collect();
            
            if let Some(action) = action_words.first() {
                // Extract what it does (next few words after action)
                let action_pos = words.iter().position(|w| w == action).unwrap_or(0);
                let topic_words: Vec<&str> = words.iter()
                    .skip(action_pos + 1)
                    .take(4)
                    .copied()
                    .collect();
                
                if !topic_words.is_empty() {
                    let summary = format!("{} {} {}", first_word, action, topic_words.join(" "));
                    return Some(truncate_string(&summary, 50));
                }
            }
            
            // Fallback: just use product name with first meaningful phrase
            let meaningful_words: Vec<&str> = words.iter()
                .skip(2)
                .filter(|w| w.len() > 2)
                .take(4)
                .copied()
                .collect();
            
            if !meaningful_words.is_empty() {
                let summary = format!("{} - {}", format!("{} {}", first_word, second_word), meaningful_words.join(" "));
                return Some(truncate_string(&summary, 50));
            }
        }
    }

    None
}

/// Save snippet to database (instant save)
pub async fn save_snippet(
    content: String,
    source_app: Option<String>,
    metadata: Option<serde_json::Value>
) -> Result<i64> {
    let pool = get_pool().await?;
    let now = Utc::now();

    // Calculate content hash
    let content_hash = crate::dedup::calculate_content_hash(&content);

    // Check for duplicate
    if let Some(duplicate) = crate::dedup::find_duplicate(&pool, &content).await? {
        log::info!("⏭️  Snippet already exists (ID: {}), skipping duplicate", duplicate.id);
        log::info!("   Existing snippet created at: {}", duplicate.created_at);
        return Ok(duplicate.id);
    }

    // Generate summary - try LLM first for text content, fall back to rule-based
    let summary = generate_summary_with_llm_fallback(&content).await;

    // Extract website_url and website_title from metadata if available
    let website_url = metadata
        .as_ref()
        .and_then(|m| m.get("url"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let website_title = metadata
        .as_ref()
        .and_then(|m| m.get("title"))
        .and_then(|v| v.as_str())
        .map(String::from);

    // Convert metadata to JSON string
    let metadata_str = metadata.as_ref().map(|m| m.to_string());

    // Log what we're saving
    log::info!("💾 Saving snippet:");
    log::info!("   Length: {} characters", content.len());
    log::info!("   Source app: {:?}", source_app);
    log::info!("   Website URL: {:?}", website_url);
    log::info!("   Website Title: {:?}", website_title);
    log::info!("   Metadata: {:?}", metadata_str);
    log::info!(
        "   Preview (first 200 chars): {}",
        if content.len() > 200 {
            format!("{}...", &content[..200])
        } else {
            content.clone()
        }
    );

    // Use INSERT OR IGNORE to handle race conditions
    let result = sqlx::query(
        r#"
        INSERT OR IGNORE INTO snippets (content, summary, created_at, updated_at, source_app, metadata, content_hash, website_url, website_title)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&content)
    .bind(&summary)
    .bind(now.to_rfc3339())
    .bind(now.to_rfc3339())
    .bind(&source_app)
    .bind(&metadata_str)
    .bind(&content_hash)
    .bind(&website_url)
    .bind(&website_title)
    .execute(&pool)
    .await
    .context("Failed to insert snippet")?;

    // Check if insert was ignored (duplicate detected)
    let id = if result.rows_affected() == 0 {
        // Duplicate detected - return existing ID (already fetched in dedup check)
        if let Some(duplicate) = crate::dedup::find_duplicate(&pool, &content).await? {
            log::info!("⏭️  Snippet duplicate detected (race condition), using existing ID: {}", duplicate.id);
            duplicate.id
        } else {
            // This shouldn't happen, but handle gracefully
            anyhow::bail!("Snippet duplicate detected but existing ID not found");
        }
    } else {
        // Insert succeeded - get the ID
        result.last_insert_rowid()
    };

    // Verify it was saved correctly
    let saved_content: Option<String> =
        sqlx::query_scalar("SELECT content FROM snippets WHERE id = ?")
            .bind(id)
            .fetch_optional(&pool)
            .await
            .context("Failed to verify saved snippet")?;

    if let Some(saved) = saved_content {
        log::info!("✅ Snippet saved successfully with ID: {}", id);
        log::info!("   Saved length: {} characters", saved.len());
        if saved != content {
            log::error!("❌ CONTENT MISMATCH! Saved content differs from original");
            log::error!(
                "   Original length: {}, Saved length: {}",
                content.len(),
                saved.len()
            );
        } else {
            log::info!("✅ Content verified: matches original");
        }
    } else {
        log::error!("❌ Failed to verify saved snippet - not found in database!");
    }

    Ok(id)
}

/// Get snippet by ID
pub async fn get_snippet(id: i64) -> Result<Option<Snippet>> {
    let pool = get_pool().await?;

    let row = sqlx::query(
        r#"
        SELECT id, content, summary, created_at, updated_at, source_app, metadata,
               type, file_path, working_directory, exit_code, website_url, website_title
        FROM snippets
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .context("Failed to fetch snippet")?;

    if let Some(row) = row {
        let created_at_str: String = row.get(3);
        let created_at = if created_at_str.contains('T') {
            DateTime::parse_from_rfc3339(&created_at_str)
                .context(format!(
                    "Failed to parse RFC3339 datetime: {}",
                    created_at_str
                ))?
                .with_timezone(&Utc)
        } else {
            chrono::NaiveDateTime::parse_from_str(&created_at_str, "%Y-%m-%d %H:%M:%S")
                .or_else(|_| {
                    chrono::NaiveDateTime::parse_from_str(&created_at_str, "%Y-%m-%d %H:%M:%S%.f")
                })
                .context(format!(
                    "Failed to parse SQLite datetime: {}",
                    created_at_str
                ))?
                .and_utc()
        };

        let updated_at: Option<DateTime<Utc>> = row.try_get::<Option<String>, _>(4).ok().flatten().and_then(|s| {
            parse_datetime_flexible(s).ok()
        });

        Ok(Some(Snippet {
            id: row.get(0),
            content: row.get(1),
            summary: row.get(2),
            created_at,
            updated_at,
            source_app: row.get(5),
            metadata: row.get(6),
            content_type: row.get(7),
            file_path: row.get(8),
            working_directory: row.get(9),
            exit_code: row.get(10),
            website_url: row.get(11),
            website_title: row.get(12),
        }))
    } else {
        Ok(None)
    }
}

/// Delete snippet by ID (FTS5 trigger will automatically remove from FTS5 table)
/// Get all snippet IDs
pub async fn get_all_snippet_ids() -> Result<Vec<i64>> {
    let pool = get_pool().await?;
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM snippets")
        .fetch_all(&pool)
        .await
        .context("Failed to fetch snippet IDs")?;
    Ok(ids)
}

pub async fn delete_snippet(id: i64) -> Result<bool> {
    let pool = match get_pool().await {
        Ok(pool) => pool,
        Err(e) => {
            log::error!("Failed to get database pool: {}", e);
            return Err(e).context("Database not available");
        }
    };

    log::debug!("Deleting snippet with id: {}", id);

    // First, verify the snippet exists
    let snippet_exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM snippets WHERE id = ?")
        .bind(id)
        .fetch_one(&pool)
        .await
        .context("Failed to check if snippet exists")?;

    if snippet_exists == 0 {
        log::warn!(
            "Snippet {} does not exist, cleaning up orphaned FTS5 entries",
            id
        );
        // Clean up any orphaned FTS5 entries
        let _ = sqlx::query("DELETE FROM snippets_fts WHERE rowid = ?")
            .bind(id)
            .execute(&pool)
            .await;
        return Ok(false);
    }

    // Delete the snippet (trigger will handle FTS5 cleanup)
    let result = sqlx::query(
        r#"
        DELETE FROM snippets
        WHERE id = ?
        "#,
    )
    .bind(id)
    .execute(&pool)
    .await;

    match result {
        Ok(exec_result) => {
            let rows_affected = exec_result.rows_affected();
            log::debug!(
                "Delete query affected {} rows for snippet {}",
                rows_affected,
                id
            );

            // Double-check FTS5 cleanup (in case trigger didn't fire)
            if rows_affected > 0 {
                let _ = sqlx::query("DELETE FROM snippets_fts WHERE rowid = ?")
                    .bind(id)
                    .execute(&pool)
                    .await;
                log::debug!("Cleaned up FTS5 entry for snippet {}", id);
            }

            Ok(rows_affected > 0)
        }
        Err(e) => {
            log::error!("SQL error deleting snippet {}: {}", id, e);
            Err(anyhow::anyhow!(
                "Failed to execute delete query for snippet {}: {}",
                id,
                e
            ))
            .context(format!("Database error while deleting snippet {}", id))
        }
    }
}

/// Get snippets by IDs
pub async fn get_snippets_by_ids(ids: &[i64]) -> Result<Vec<Snippet>> {
    if ids.is_empty() {
        return Ok(vec![]);
    }

    let pool = get_pool().await?;
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

    let query = format!(
        r#"
        SELECT id, content, summary, created_at, updated_at, source_app, metadata,
               type, file_path, working_directory, exit_code, website_url, website_title
        FROM snippets
        WHERE id IN ({})
        ORDER BY created_at DESC
        "#,
        placeholders
    );

    let mut query_builder = sqlx::query(&query);
    for id in ids {
        query_builder = query_builder.bind(id);
    }

    let rows = query_builder
        .fetch_all(&pool)
        .await
        .context("Failed to fetch snippets")?;

    let mut snippets = Vec::new();
    for row in rows {
        let created_at_str: String = row.get(3);
        let created_at = if created_at_str.contains('T') {
            DateTime::parse_from_rfc3339(&created_at_str)
                .context(format!(
                    "Failed to parse RFC3339 datetime: {}",
                    created_at_str
                ))?
                .with_timezone(&Utc)
        } else {
            chrono::NaiveDateTime::parse_from_str(&created_at_str, "%Y-%m-%d %H:%M:%S")
                .or_else(|_| {
                    chrono::NaiveDateTime::parse_from_str(&created_at_str, "%Y-%m-%d %H:%M:%S%.f")
                })
                .context(format!(
                    "Failed to parse SQLite datetime: {}",
                    created_at_str
                ))?
                .and_utc()
        };

        let updated_at: Option<DateTime<Utc>> = row.try_get::<Option<String>, _>(4).ok().flatten().and_then(|s| {
            parse_datetime_flexible(s).ok()
        });

        snippets.push(Snippet {
            id: row.get(0),
            content: row.get(1),
            summary: row.get(2),
            created_at,
            updated_at,
            source_app: row.get(5),
            metadata: row.get(6),
            content_type: row.get(7),
            file_path: row.get(8),
            working_directory: row.get(9),
            exit_code: row.get(10),
            website_url: row.get(11),
            website_title: row.get(12),
        });
    }

    Ok(snippets)
}

// Search functions will be called via helper functions to avoid module path issues
// We'll create wrapper functions that handle the search parsing

/// Returns a query that matches words (not exact phrases - allows flexible matching)
/// For advanced parsing (phrases, boolean, proximity), use the search module via commands.rs
pub fn sanitize_fts5_query(query: &str) -> String {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Check for quoted phrases (simple case)
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() > 2 {
        let phrase = trimmed.trim_matches('"');
        return format!("\"{}\"", phrase.replace("\"", "\"\""));
    }

    // Check if this is already a formatted FTS5 query (has operators)
    // If so, don't process it further
    if trimmed.contains(" AND ")
        || trimmed.contains(" OR ")
        || trimmed.contains(" NOT ")
        || trimmed.contains(" NEAR/")
    {
        return trimmed.to_string();
    }

    // Basic word-based query
    let words: Vec<&str> = trimmed
        .split_whitespace()
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        return String::new();
    }

    words
        .iter()
        .enumerate()
        .map(|(idx, w)| {
            if idx == 0 && w.chars().count() >= 3 {
                format!("{}*", w)
            } else {
                w.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

/// Search snippets using FTS5 (keyword search) - legacy function
pub async fn search_fts5(query: &str, limit: i64) -> Result<Vec<SearchResult>> {
    search_fts5_with_original(query, query, limit).await
}

/// Search snippets using FTS5 with separate formatted query and original query for validation
pub async fn search_fts5_with_original(
    fts5_query: &str,
    original_query: &str,
    limit: i64,
) -> Result<Vec<SearchResult>> {
    let pool = get_pool().await?;

    // Use the FTS5 formatted query directly
    let sanitized_query = fts5_query.to_string();

    // If query is empty after sanitization, return empty results
    if sanitized_query.is_empty() {
        log::debug!("Search query empty after sanitization, returning empty results");
        return Ok(vec![]);
    }

    log::debug!("FTS5 search query: '{}'", sanitized_query);

    // Check if query uses prefix matching or phrase matching
    let uses_prefix = sanitized_query.contains('*');
    let is_phrase = sanitized_query.starts_with('"') || original_query.trim().starts_with('"');

    // Use appropriate threshold based on query type
    // BM25 scores are negative, with better matches being MORE negative
    // Prefix queries have scores very close to zero (e.g., -9e-07), so we need a very lenient threshold
    // Word queries can also have weak scores (very close to zero), so use lenient threshold
    let rank_threshold = if uses_prefix {
        0.001 // Very lenient for prefixes - accept all scores (positive or negative)
    } else if is_phrase {
        0.001 // Very lenient for phrases - accept all negative BM25 scores
    } else {
        // For word queries, be lenient - scores like -9e-07 need positive threshold
        // This accepts all matches that FTS5 finds (no BM25 filtering for word queries)
        0.001 // Lenient for exact word queries too
    };

    log::debug!(
        "Using rank threshold: {} (prefix: {}, phrase: {})",
        rank_threshold,
        uses_prefix,
        is_phrase
    );

    // FTS5 search with BM25 ranking
    // Note: BM25 scores are negative, more negative = better match
    // For prefix queries, scores are typically closer to zero, so we use a lenient threshold
    let rows = sqlx::query(
        r#"
        SELECT
            s.id,
            s.content,
            s.summary,
            s.created_at,
            s.updated_at,
            s.source_app,
            s.metadata,
            bm25(snippets_fts) as rank
        FROM snippets_fts
        JOIN snippets s ON s.id = snippets_fts.rowid
        WHERE snippets_fts MATCH ?
            AND bm25(snippets_fts) <= ?
        ORDER BY rank ASC
        LIMIT ?
        "#,
    )
    .bind(&sanitized_query)
    .bind(rank_threshold)
    .bind(limit)
    .fetch_all(&pool)
    .await
    .context("Failed to execute FTS5 search")?;

    let mut results = Vec::new();
    for row in rows {
        let snippet_id: i64 = row.get(0);
        let content: String = row.get(1);
        let summary: Option<String> = row.get(2);
        let created_at_str: String = row.get(3);
        let rank: f64 = row.get(7);

        log::debug!(
            "Search result: id={}, rank={}, content_len={}",
            snippet_id,
            rank,
            content.len()
        );

        // Check if original query is a phrase (quoted)
        let is_phrase_query =
            original_query.trim().starts_with('"') && original_query.trim().ends_with('"');

        if is_phrase_query {
            // For phrase queries, FTS5 already did the exact phrase matching
            // Trust FTS5's phrase matching - if it returned a result, the phrase exists
            // Just do a simple verification that the phrase words appear in content
            let phrase = original_query.trim().trim_matches('"').to_lowercase();

            let content_lower = content.to_lowercase();

            // Simple check: phrase must appear as substring (FTS5 ensures it's a proper phrase match)
            if !content_lower.contains(&phrase) {
                log::debug!(
                    "❌ FILTERING OUT result id={} - phrase '{}' not found in content (unexpected - FTS5 should have filtered this)",
                    snippet_id,
                    phrase
                );
                continue;
            }

            log::debug!(
                "✅ Result id={} passed phrase validation for '{}'",
                snippet_id,
                phrase
            );
        } else {
            // For word queries, trust FTS5 completely
            // FTS5 already did the matching - if it returned a result, it's relevant
            // No additional validation needed - this prevents false negatives
            log::debug!(
                "✅ Result id={} accepted (FTS5 matched) for query '{}'",
                snippet_id,
                original_query
            );
        }

        // Parse datetime - handle both RFC3339 and SQLite formats
        let created_at = if created_at_str.contains('T') {
            // RFC3339 format: 2025-11-01T16:40:35.268044+00:00
            DateTime::parse_from_rfc3339(&created_at_str)
                .context(format!(
                    "Failed to parse RFC3339 datetime: {}",
                    created_at_str
                ))?
                .with_timezone(&Utc)
        } else {
            // SQLite format: 2025-11-01 17:57:29
            // Parse as naive datetime and assume UTC
            chrono::NaiveDateTime::parse_from_str(&created_at_str, "%Y-%m-%d %H:%M:%S")
                .or_else(|_| {
                    chrono::NaiveDateTime::parse_from_str(&created_at_str, "%Y-%m-%d %H:%M:%S%.f")
                })
                .context(format!(
                    "Failed to parse SQLite datetime: {}",
                    created_at_str
                ))?
                .and_utc()
        };

        // Convert rank to positive (higher is better for UI)
        // BM25 scores are negative, so we negate them
        let bm25_score = (-rank).max(0.0);

        // Boost recent results (recency factor)
        // More recent snippets get a slight boost
        let days_old = (Utc::now() - created_at).num_days();
        let recency_boost = (-days_old as f64 / 30.0).exp(); // Exponential decay over 30 days
        let final_rank = bm25_score * 0.8 + recency_boost * 0.2;

        let updated_at: Option<DateTime<Utc>> = row.try_get::<Option<String>, _>(4).ok().flatten().and_then(|s| {
            parse_datetime_flexible(s).ok()
        });

        results.push(SearchResult {
            snippet: Snippet {
                id: snippet_id,
                content,
                summary,
                created_at,
                updated_at,
                source_app: row.get(5),
                metadata: row.get(6),
                content_type: Some("text".to_string()),
                file_path: None,
                working_directory: None,
                exit_code: None,
                website_url: None,
                website_title: None,
            },
            rank: final_rank,
            match_type: MatchType::Keyword,
        });
    }

    log::debug!(
        "Search '{}' returned {} results (after filtering)",
        original_query,
        results.len()
    );
    Ok(results)
}

/// Get all unique source apps from the database (for filter dropdown)
pub async fn get_unique_source_apps() -> Result<Vec<String>> {
    let pool = get_pool().await?;

    let rows = sqlx::query(
        r#"
        SELECT DISTINCT source_app
        FROM snippets
        WHERE source_app IS NOT NULL AND source_app != ''
        ORDER BY source_app ASC
        "#
    )
    .fetch_all(&pool)
    .await?;

    let source_apps: Vec<String> = rows
        .into_iter()
        .filter_map(|row| row.try_get::<Option<String>, _>("source_app").ok().flatten())
        .collect();

    log::debug!("Found {} unique source apps", source_apps.len());
    Ok(source_apps)
}

// ============================================================================
// Category CRUD operations
// ============================================================================

/// Create a new category
pub async fn create_category(
    name: String,
    parent_id: Option<i64>,
    emoji: Option<String>,
) -> Result<i64> {
    let pool = get_pool().await?;
    let now = Utc::now();

    // Try to insert the category
    let result = sqlx::query(
        r#"
        INSERT INTO categories (name, parent_id, emoji, created_at, is_app_folder, app_name)
        VALUES (?, ?, ?, ?, 0, NULL)
        "#,
    )
    .bind(&name)
    .bind(parent_id)
    .bind(emoji.unwrap_or_else(|| "📁".to_string()))
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await;

    match result {
        Ok(result) => {
            let id = result.last_insert_rowid();
            log::info!("✅ Created category '{}' with ID: {}", name, id);
            Ok(id)
        }
        Err(sqlx::Error::Database(db_err)) if db_err.message().contains("UNIQUE constraint") => {
            // Category already exists - return the existing category ID
            log::info!("Category '{}' already exists (unique constraint), fetching existing ID", name);
            let existing_id: Option<i64> = if let Some(pid) = parent_id {
                sqlx::query_scalar(
                    r#"
                    SELECT id FROM categories WHERE name = ? AND parent_id = ?
                    "#,
                )
                .bind(&name)
                .bind(pid)
                .fetch_optional(&pool)
                .await
            } else {
                sqlx::query_scalar(
                    r#"
                    SELECT id FROM categories WHERE name = ? AND parent_id IS NULL
                    "#,
                )
                .bind(&name)
                .fetch_optional(&pool)
                .await
            }
            .context("Failed to query existing category")?;

            match existing_id {
                Some(id) => {
                    log::info!("✅ Using existing category '{}' with ID: {}", name, id);
                    Ok(id)
                }
                None => {
                    // This shouldn't happen, but handle it gracefully
                    anyhow::bail!("Category '{}' violates unique constraint but not found in database", name)
                }
            }
        }
        Err(e) => {
            Err(e).context("Failed to insert category")
        }
    }
}

/// Get or create an app folder for the given source app
/// Returns the folder ID
pub async fn get_or_create_app_folder(source_app: &str) -> Result<i64> {
    let pool = get_pool().await?;

    // Validate source_app is not empty
    let trimmed_app = source_app.trim();
    if trimmed_app.is_empty() {
        anyhow::bail!("Cannot create app folder with empty source_app name");
    }

    // Try to find existing app folder
    let existing: Option<(i64,)> = sqlx::query_as(
        r#"
        SELECT id
        FROM categories
        WHERE is_app_folder = 1 AND app_name = ?
        "#,
    )
    .bind(trimmed_app)
    .fetch_optional(&pool)
    .await
    .context("Failed to query app folder")?;

    if let Some((id,)) = existing {
        return Ok(id);
    }

    // Create new app folder
    let emoji = match trimmed_app {
        "Cursor" => "⌨️",
        "LocalMind" => "🧠",
        "Chrome" | "Safari" | "Firefox" => "🌐",
        "VS Code" | "VSCode" => "💻",
        "Terminal" => "🖥️",
        "Notes" => "📝",
        _ => "📱",
    };

    let now = Utc::now();
    let id = sqlx::query(
        r#"
        INSERT INTO categories (name, parent_id, emoji, created_at, is_app_folder, app_name)
        VALUES (?, NULL, ?, ?, 1, ?)
        "#,
    )
    .bind(trimmed_app)
    .bind(emoji)
    .bind(now.to_rfc3339())
    .bind(trimmed_app)
    .execute(&pool)
    .await
    .context("Failed to create app folder")?
    .last_insert_rowid();

    log::info!("✅ Created app folder '{}' with ID: {}", trimmed_app, id);
    Ok(id)
}

/// Get category by ID
pub async fn get_category(id: i64) -> Result<Option<Category>> {
    let pool = get_pool().await?;

    let row = sqlx::query(
        r#"
        SELECT id, name, parent_id, emoji, created_at
        FROM categories
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .context("Failed to fetch category")?;

    if let Some(row) = row {
        let created_at_str: String = row.get(4);
        let created_at = if created_at_str.contains('T') {
            DateTime::parse_from_rfc3339(&created_at_str)
                .context("Failed to parse RFC3339 datetime")?
                .with_timezone(&Utc)
        } else {
            chrono::NaiveDateTime::parse_from_str(&created_at_str, "%Y-%m-%d %H:%M:%S")
                .or_else(|_| {
                    chrono::NaiveDateTime::parse_from_str(&created_at_str, "%Y-%m-%d %H:%M:%S%.f")
                })
                .context("Failed to parse SQLite datetime")?
                .and_utc()
        };

        Ok(Some(Category {
            id: row.get(0),
            name: row.get(1),
            parent_id: row.get(2),
            emoji: row.get(3),
            created_at,
        }))
    } else {
        Ok(None)
    }
}

/// Get all categories, optionally filtered by parent_id
/// If parent_id is None, returns root categories (parent_id IS NULL)
/// If parent_id is Some(id), returns children of that category
pub async fn get_categories(parent_id: Option<Option<i64>>, content_type: Option<String>) -> Result<Vec<Category>> {
    let pool = get_pool().await?;

    let rows = if let Some(parent) = parent_id {
        // Filter by specific parent (including NULL for root categories)
        if let Some(parent_val) = parent {
            if let Some(ref filter_type) = content_type {
                // Filter by parent AND content type
                sqlx::query(
                    r#"
                    SELECT DISTINCT c.id, c.name, c.parent_id, c.emoji, c.created_at
                    FROM categories c
                    INNER JOIN snippet_categories sc ON c.id = sc.category_id
                    INNER JOIN snippets s ON sc.snippet_id = s.id
                    WHERE c.parent_id = ? AND s.type = ?
                    ORDER BY c.name ASC
                    "#,
                )
                .bind(parent_val)
                .bind(filter_type)
                .fetch_all(&pool)
                .await?
            } else {
                // Filter by parent only
                sqlx::query(
                    r#"
                    SELECT id, name, parent_id, emoji, created_at
                    FROM categories
                    WHERE parent_id = ?
                    ORDER BY name ASC
                    "#,
                )
                .bind(parent_val)
                .fetch_all(&pool)
                .await?
            }
        } else {
            // Get root categories (parent_id IS NULL)
            if let Some(ref filter_type) = content_type {
                // Filter root categories by content type
                sqlx::query(
                    r#"
                    SELECT DISTINCT c.id, c.name, c.parent_id, c.emoji, c.created_at
                    FROM categories c
                    INNER JOIN snippet_categories sc ON c.id = sc.category_id
                    INNER JOIN snippets s ON sc.snippet_id = s.id
                    WHERE c.parent_id IS NULL AND s.type = ?
                    ORDER BY c.name ASC
                    "#,
                )
                .bind(filter_type)
                .fetch_all(&pool)
                .await?
            } else {
                // Get all root categories
                sqlx::query(
                    r#"
                    SELECT id, name, parent_id, emoji, created_at
                    FROM categories
                    WHERE parent_id IS NULL
                    ORDER BY name ASC
                    "#,
                )
                .fetch_all(&pool)
                .await?
            }
        }
    } else {
        // Get all categories
        if let Some(ref filter_type) = content_type {
            // Filter all categories by content type
            sqlx::query(
                r#"
                SELECT DISTINCT c.id, c.name, c.parent_id, c.emoji, c.created_at
                FROM categories c
                INNER JOIN snippet_categories sc ON c.id = sc.category_id
                INNER JOIN snippets s ON sc.snippet_id = s.id
                WHERE s.type = ?
                ORDER BY c.name ASC
                "#,
            )
            .bind(filter_type)
            .fetch_all(&pool)
            .await?
        } else {
            // Get all categories without filtering
            sqlx::query(
                r#"
                SELECT id, name, parent_id, emoji, created_at
                FROM categories
                ORDER BY name ASC
                "#,
            )
            .fetch_all(&pool)
            .await?
        }
    };

    let mut categories = Vec::new();
    for row in rows {
        let created_at_str: String = row.get(4);
        let created_at = if created_at_str.contains('T') {
            DateTime::parse_from_rfc3339(&created_at_str)
                .context("Failed to parse RFC3339 datetime")?
                .with_timezone(&Utc)
        } else {
            chrono::NaiveDateTime::parse_from_str(&created_at_str, "%Y-%m-%d %H:%M:%S")
                .or_else(|_| {
                    chrono::NaiveDateTime::parse_from_str(&created_at_str, "%Y-%m-%d %H:%M:%S%.f")
                })
                .context("Failed to parse SQLite datetime")?
                .and_utc()
        };

        categories.push(Category {
            id: row.get(0),
            name: row.get(1),
            parent_id: row.get(2),
            emoji: row.get(3),
            created_at,
        });
    }

    log::debug!("Found {} categories", categories.len());
    Ok(categories)
}

/// Update category name and/or emoji
pub async fn update_category(id: i64, name: Option<String>, emoji: Option<String>) -> Result<()> {
    let pool = get_pool().await?;

    // Build dynamic update query based on what's provided
    if let Some(new_name) = name {
        sqlx::query("UPDATE categories SET name = ? WHERE id = ?")
            .bind(&new_name)
            .bind(id)
            .execute(&pool)
            .await
            .context("Failed to update category name")?;
    }

    if let Some(new_emoji) = emoji {
        sqlx::query("UPDATE categories SET emoji = ? WHERE id = ?")
            .bind(&new_emoji)
            .bind(id)
            .execute(&pool)
            .await
            .context("Failed to update category emoji")?;
    }

    log::info!("✅ Updated category ID: {}", id);
    Ok(())
}

/// Delete category and all its content (cascades to children, snippets, embeddings, and files)
pub async fn delete_category(id: i64) -> Result<()> {
    let pool = get_pool().await?;

    // Collect all category IDs to delete (parent + all descendants)
    let mut to_delete = vec![id];
    let mut to_check = vec![id];

    // Find all descendant categories (iterative BFS)
    while !to_check.is_empty() {
        let current_batch = to_check.clone();
        to_check.clear();

        for parent_id in current_batch {
            let children: Vec<i64> = sqlx::query_scalar(
                "SELECT id FROM categories WHERE parent_id = ?"
            )
            .bind(parent_id)
            .fetch_all(&pool)
            .await
            .context("Failed to fetch child categories")?;

            to_delete.extend(children.iter());
            to_check.extend(children);
        }
    }

    log::info!("Deleting category {} and {} descendants", id, to_delete.len() - 1);

    // Get all snippet IDs in these categories
    let mut snippet_ids: Vec<i64> = Vec::new();
    for cat_id in &to_delete {
        let ids: Vec<i64> = sqlx::query_scalar(
            "SELECT snippet_id FROM snippet_categories WHERE category_id = ?"
        )
        .bind(cat_id)
        .fetch_all(&pool)
        .await
        .context("Failed to fetch snippet IDs")?;
        snippet_ids.extend(ids);
    }

    // Deduplicate snippet IDs
    snippet_ids.sort_unstable();
    snippet_ids.dedup();

    log::info!("Found {} snippets to delete", snippet_ids.len());

    // Delete screenshot files from filesystem
    for snippet_id in &snippet_ids {
        if let Ok(Some(snippet)) = get_snippet(*snippet_id).await {
            if snippet.content_type.as_deref() == Some("screenshot") {
                if let Some(file_path) = snippet.file_path {
                    if let Err(e) = tokio::fs::remove_file(&file_path).await {
                        log::warn!("Failed to delete screenshot file {}: {}", file_path, e);
                    } else {
                        log::debug!("Deleted screenshot file: {}", file_path);
                    }
                }
            }
        }
    }

    // Delete embeddings from vector store
    for snippet_id in &snippet_ids {
        if let Err(e) = crate::db::lancedb::delete_embedding(*snippet_id).await {
            log::warn!("Failed to delete embedding for snippet {}: {}", snippet_id, e);
        }
    }

    // Delete snippets (FTS will be auto-cleaned by trigger)
    for snippet_id in &snippet_ids {
        sqlx::query("DELETE FROM snippets WHERE id = ?")
            .bind(snippet_id)
            .execute(&pool)
            .await
            .context("Failed to delete snippet")?;
    }

    // Delete snippet category assignments
    for cat_id in &to_delete {
        sqlx::query("DELETE FROM snippet_categories WHERE category_id = ?")
            .bind(cat_id)
            .execute(&pool)
            .await
            .context("Failed to unassign snippets from category")?;
    }

    // Delete all categories (children first, then parent)
    to_delete.reverse(); // Delete from leaves to root
    for cat_id in &to_delete {
        sqlx::query("DELETE FROM categories WHERE id = ?")
            .bind(cat_id)
            .execute(&pool)
            .await
            .context("Failed to delete category")?;
    }

    log::info!("✅ Deleted category ID: {}, {} descendants, and {} snippets", id, to_delete.len() - 1, snippet_ids.len());
    Ok(())
}

/// Delete all data (snippets, commands, screenshots, categories, embeddings)
pub async fn delete_all_data() -> Result<()> {
    let pool = get_pool().await?;

    log::info!("🗑️  Starting complete data deletion...");

    // Get all snippet IDs first
    let snippet_ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM snippets")
        .fetch_all(&pool)
        .await
        .context("Failed to fetch snippet IDs")?;

    log::info!("Found {} total snippets to delete", snippet_ids.len());

    // Delete all screenshot files from filesystem
    let screenshots: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, file_path FROM snippets WHERE type = 'screenshot' AND file_path IS NOT NULL"
    )
    .fetch_all(&pool)
    .await
    .context("Failed to fetch screenshot paths")?;

    for (snippet_id, file_path) in screenshots {
        if let Err(e) = tokio::fs::remove_file(&file_path).await {
            log::warn!("Failed to delete screenshot file {} (snippet {}): {}", file_path, snippet_id, e);
        } else {
            log::debug!("Deleted screenshot file: {}", file_path);
        }
    }

    // Delete all embeddings from vector store
    for snippet_id in &snippet_ids {
        if let Err(e) = crate::db::lancedb::delete_embedding(*snippet_id).await {
            log::warn!("Failed to delete embedding for snippet {}: {}", snippet_id, e);
        }
    }

    // Delete all snippets (this will cascade to snippet_categories and trigger FTS cleanup)
    sqlx::query("DELETE FROM snippets")
        .execute(&pool)
        .await
        .context("Failed to delete snippets")?;

    // Delete all categories
    sqlx::query("DELETE FROM categories")
        .execute(&pool)
        .await
        .context("Failed to delete categories")?;

    // Delete snippet_categories (should already be empty due to cascade, but just in case)
    sqlx::query("DELETE FROM snippet_categories")
        .execute(&pool)
        .await
        .context("Failed to delete snippet categories")?;

    // Delete export history
    sqlx::query("DELETE FROM export_history")
        .execute(&pool)
        .await
        .context("Failed to delete export history")?;

    // Delete snippet versions
    sqlx::query("DELETE FROM snippet_versions")
        .execute(&pool)
        .await
        .context("Failed to delete snippet versions")?;

    log::info!("✅ Successfully deleted all data: {} snippets, all categories, and all files", snippet_ids.len());
    Ok(())
}

/// Assign a snippet to a category
pub async fn assign_snippet_to_category(
    snippet_id: i64,
    category_id: i64,
    confidence: f64,
    is_manual: bool,
) -> Result<()> {
    let pool = get_pool().await?;
    let now = Utc::now();

    sqlx::query(
        r#"
        INSERT OR REPLACE INTO snippet_categories (snippet_id, category_id, confidence, is_manual, assigned_at)
        VALUES (?, ?, ?, ?, ?)
        "#,
    )
    .bind(snippet_id)
    .bind(category_id)
    .bind(confidence)
    .bind(is_manual)
    .bind(now.to_rfc3339())
    .execute(&pool)
    .await
    .context("Failed to assign snippet to category")?;

    log::debug!(
        "Assigned snippet {} to category {} (confidence: {}, manual: {})",
        snippet_id,
        category_id,
        confidence,
        is_manual
    );
    Ok(())
}

/// Assign snippet to category with categorization method tracking
pub async fn assign_snippet_to_category_with_method(
    snippet_id: i64,
    category_id: i64,
    confidence: f64,
    is_manual: bool,
    method: &str,
    reasoning: Option<String>,
) -> Result<()> {
    let pool = get_pool().await?;
    let now = Utc::now();

    sqlx::query(
        r#"
        INSERT OR REPLACE INTO snippet_categories
        (snippet_id, category_id, confidence, is_manual, assigned_at, categorization_method, llm_reasoning)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(snippet_id)
    .bind(category_id)
    .bind(confidence)
    .bind(is_manual)
    .bind(now.to_rfc3339())
    .bind(method)
    .bind(reasoning)
    .execute(&pool)
    .await
    .context("Failed to assign snippet to category with method")?;

    log::debug!(
        "Assigned snippet {} to category {} (confidence: {}, manual: {}, method: {})",
        snippet_id,
        category_id,
        confidence,
        is_manual,
        method
    );
    Ok(())
}

/// Get all snippets in a category with pagination
pub async fn get_snippets_by_category(
    category_id: i64,
    limit: i64,
    offset: i64,
    content_type: Option<String>,
) -> Result<Vec<Snippet>> {
    let pool = get_pool().await?;

    let rows = if let Some(ref filter_type) = content_type {
        // Filter by content type if provided
        sqlx::query(
            r#"
            SELECT s.id, s.content, s.summary, s.created_at, s.updated_at, s.source_app, s.metadata,
                   s.type, s.file_path, s.working_directory, s.exit_code, s.website_url, s.website_title
            FROM snippets s
            INNER JOIN snippet_categories sc ON s.id = sc.snippet_id
            WHERE sc.category_id = ? AND s.type = ?
            ORDER BY sc.assigned_at DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(category_id)
        .bind(filter_type)
        .bind(limit)
        .bind(offset)
        .fetch_all(&pool)
        .await?
    } else {
        // No filter - get all types
        sqlx::query(
            r#"
            SELECT s.id, s.content, s.summary, s.created_at, s.updated_at, s.source_app, s.metadata,
                   s.type, s.file_path, s.working_directory, s.exit_code, s.website_url, s.website_title
            FROM snippets s
            INNER JOIN snippet_categories sc ON s.id = sc.snippet_id
            WHERE sc.category_id = ?
            ORDER BY sc.assigned_at DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(category_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&pool)
        .await?
    };

    // Pre-allocate with exact capacity for better performance
    let mut snippets = Vec::with_capacity(rows.len());

    for row in rows {
        let created_at = parse_datetime_flexible(row.get(3))?;
        let updated_at: Option<DateTime<Utc>> = row.try_get::<Option<String>, _>(4).ok().flatten().and_then(|s| {
            parse_datetime_flexible(s).ok()
        });

        snippets.push(Snippet {
            id: row.get(0),
            content: row.get(1),
            summary: row.get(2),
            created_at,
            updated_at,
            source_app: row.get(5),
            metadata: row.get(6),
            content_type: row.get(7),
            file_path: row.get(8),
            working_directory: row.get(9),
            exit_code: row.get(10),
            website_url: row.get(11),
            website_title: row.get(12),
        });
    }

    Ok(snippets)
}

/// Get the category assignment for a snippet
pub async fn get_category_for_snippet(snippet_id: i64) -> Result<Option<SnippetCategory>> {
    let pool = get_pool().await?;

    let row = sqlx::query(
        r#"
        SELECT snippet_id, category_id, confidence, is_manual, assigned_at
        FROM snippet_categories
        WHERE snippet_id = ?
        "#,
    )
    .bind(snippet_id)
    .fetch_optional(&pool)
    .await?;

    if let Some(row) = row {
        let assigned_at_str: String = row.get(4);
        let assigned_at = if assigned_at_str.contains('T') {
            DateTime::parse_from_rfc3339(&assigned_at_str)
                .context("Failed to parse RFC3339 datetime")?
                .with_timezone(&Utc)
        } else {
            chrono::NaiveDateTime::parse_from_str(&assigned_at_str, "%Y-%m-%d %H:%M:%S")
                .or_else(|_| {
                    chrono::NaiveDateTime::parse_from_str(&assigned_at_str, "%Y-%m-%d %H:%M:%S%.f")
                })
                .context("Failed to parse SQLite datetime")?
                .and_utc()
        };

        Ok(Some(SnippetCategory {
            snippet_id: row.get(0),
            category_id: row.get(1),
            confidence: row.get(2),
            is_manual: row.get(3),
            assigned_at,
        }))
    } else {
        Ok(None)
    }
}

/// Get categorization reasoning for a snippet
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CategorizationReasoning {
    pub snippet_id: i64,
    pub category_id: i64,
    pub category_name: String,
    pub emoji: String,
    pub categorization_method: String,
    pub reasoning: Option<String>,
    pub confidence: f64,
    pub is_manual: bool,
    pub assigned_at: String,
}

/// Get categorization reasoning for a snippet
pub async fn get_categorization_reasoning(snippet_id: i64) -> Result<Option<CategorizationReasoning>> {
    let pool = get_pool().await?;

    let row = sqlx::query(
        r#"
        SELECT 
            sc.snippet_id,
            sc.category_id,
            c.name as category_name,
            c.emoji,
            sc.categorization_method,
            sc.llm_reasoning,
            sc.confidence,
            sc.is_manual,
            sc.assigned_at
        FROM snippet_categories sc
        INNER JOIN categories c ON sc.category_id = c.id
        WHERE sc.snippet_id = ?
        "#,
    )
    .bind(snippet_id)
    .fetch_optional(&pool)
    .await?;

    if let Some(row) = row {
        let assigned_at_str: String = row.get(8);
        
        Ok(Some(CategorizationReasoning {
            snippet_id: row.get(0),
            category_id: row.get(1),
            category_name: row.get(2),
            emoji: row.get(3),
            categorization_method: row.get(4),
            reasoning: row.get(5),
            confidence: row.get(6),
            is_manual: row.get(7),
            assigned_at: assigned_at_str,
        }))
    } else {
        Ok(None)
    }
}

/// Get count of snippets in a category
pub async fn get_category_snippet_count(category_id: i64, content_type: Option<String>) -> Result<i64> {
    let pool = get_pool().await?;

    let count: i64 = if let Some(filter_type) = content_type {
        // Count only snippets of the specified type
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM snippet_categories sc
            INNER JOIN snippets s ON sc.snippet_id = s.id
            WHERE sc.category_id = ? AND s.type = ?
            "#,
        )
        .bind(category_id)
        .bind(filter_type)
        .fetch_one(&pool)
        .await?
    } else {
        // Count all snippets
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM snippet_categories
            WHERE category_id = ?
            "#,
        )
        .bind(category_id)
        .fetch_one(&pool)
        .await?
    };

    Ok(count)
}

/// Edit an existing snippet (updates content, summary, and updated_at)
/// Optionally saves old version to snippet_versions table
pub async fn edit_snippet(
    id: i64,
    new_content: String,
    save_version: bool,
) -> Result<()> {
    let pool = get_pool().await?;
    let now = Utc::now();

    // Get current snippet to save as version
    if save_version {
        let current = get_snippet(id).await?.ok_or_else(|| {
            anyhow::anyhow!("Snippet {} not found", id)
        })?;

        // Get current version count
        let version_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM snippet_versions WHERE snippet_id = ?"
        )
        .bind(id)
        .fetch_one(&pool)
        .await
        .context("Failed to count versions")?;

        // Save old version
        sqlx::query(
            r#"
            INSERT INTO snippet_versions (snippet_id, version_number, content, summary, created_at)
            VALUES (?, ?, ?, ?, ?)
            "#
        )
        .bind(id)
        .bind(version_count + 1)
        .bind(&current.content)
        .bind(&current.summary)
        .bind(now.to_rfc3339())
        .execute(&pool)
        .await
        .context("Failed to save version")?;

        log::info!("Saved version {} for snippet {}", version_count + 1, id);
    }

    // Generate new summary
    let new_summary = generate_summary(&new_content);

    // Update snippet
    sqlx::query(
        r#"
        UPDATE snippets
        SET content = ?, summary = ?, updated_at = ?
        WHERE id = ?
        "#
    )
    .bind(&new_content)
    .bind(&new_summary)
    .bind(now.to_rfc3339())
    .bind(id)
    .execute(&pool)
    .await
    .context("Failed to update snippet")?;

    log::info!("✅ Updated snippet {}", id);
    Ok(())
}

/// Get version history for a snippet
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnippetVersion {
    pub id: i64,
    pub snippet_id: i64,
    pub version_number: i64,
    pub content: String,
    pub summary: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub async fn get_snippet_versions(snippet_id: i64) -> Result<Vec<SnippetVersion>> {
    let pool = get_pool().await?;

    let rows = sqlx::query(
        r#"
        SELECT id, snippet_id, version_number, content, summary, created_at
        FROM snippet_versions
        WHERE snippet_id = ?
        ORDER BY version_number DESC
        "#
    )
    .bind(snippet_id)
    .fetch_all(&pool)
    .await
    .context("Failed to fetch snippet versions")?;

    let mut versions = Vec::new();
    for row in rows {
        let created_at_str: String = row.get(5);
        let created_at = parse_datetime_flexible(created_at_str)?;

        versions.push(SnippetVersion {
            id: row.get(0),
            snippet_id: row.get(1),
            version_number: row.get(2),
            content: row.get(3),
            summary: row.get(4),
            created_at,
        });
    }

    Ok(versions)
}

/// Regenerate summaries for all snippets in the database
pub async fn regenerate_all_summaries() -> Result<usize> {
    let pool = get_pool().await?;

    log::info!("Starting to regenerate all snippet summaries");

    // Get all snippets
    let snippets: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, content FROM snippets ORDER BY id"
    )
    .fetch_all(&pool)
    .await
    .context("Failed to fetch snippets")?;

    let total = snippets.len();
    log::info!("Found {} snippets to process", total);

    let mut updated = 0;

    for (id, content) in snippets {
        let new_summary = generate_summary(&content);

        sqlx::query(
            "UPDATE snippets SET summary = ? WHERE id = ?"
        )
        .bind(&new_summary)
        .bind(id)
        .execute(&pool)
        .await
        .context(format!("Failed to update summary for snippet {}", id))?;

        updated += 1;

        if updated % 100 == 0 {
            log::info!("Progress: {}/{} summaries regenerated", updated, total);
        }
    }

    log::info!("✅ Successfully regenerated {} summaries", updated);
    Ok(updated)
}
