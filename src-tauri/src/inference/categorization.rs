// LLM-based smart categorization
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::db::sqlite::Category;
use crate::inference::llama::{LlamaModel, LlamaParams};

/// Decision from LLM about categorization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryDecision {
    pub action: CategoryAction,
    pub category_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
    pub reasoning: String,
    #[serde(default)]
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CategoryAction {
    UseExisting,
    CreateNew,
}

/// Categorize snippet using LLM
pub async fn categorize_with_llm(
    model: &LlamaModel,
    snippet_content: &str,
    existing_categories: &[Category],
) -> Result<CategoryDecision> {
    // Build prompt
    let prompt = build_categorization_prompt(snippet_content, existing_categories);

    // Generate response with timeout
    let params = LlamaParams::default();

    let response = tokio::task::spawn_blocking({
        let model = model.clone();
        let prompt = prompt.clone();
        move || model.generate(&prompt, &params)
    })
    .await
    .context("LLM task panicked")??;

    // Parse JSON response
    parse_llm_response(&response)
}

/// Build categorization prompt for LLM
fn build_categorization_prompt(snippet_content: &str, categories: &[Category]) -> String {
    // Format existing categories
    let categories_list = if categories.is_empty() {
        "No existing categories yet.".to_string()
    } else {
        format_categories(categories)
    };

    // Truncate snippet if too long (keep first 300 chars for faster processing)
    let snippet_preview = if snippet_content.len() > 300 {
        format!("{}...", &snippet_content[..300])
    } else {
        snippet_content.to_string()
    };

    format!(
        r#"<|im_start|>system
You are a categorization assistant. Analyze snippets and suggest categories. Respond ONLY with valid JSON, no other text.<|im_end|>
<|im_start|>user
Existing Categories:
{categories_list}

Snippet:
{snippet_preview}

Analyze this snippet and respond with ONLY this JSON structure (no markdown, no explanation):
{{"action":"use_existing","category_name":"exact name","emoji":"📁","reasoning":"brief explanation","confidence":0.85}}

Or if creating new:
{{"action":"create_new","category_name":"new name","emoji":"📁","reasoning":"why new","confidence":0.90}}<|im_end|>
<|im_start|>assistant
{{"#
    )
}

/// Format categories for prompt
fn format_categories(categories: &[Category]) -> String {
    let mut formatted = String::new();

    // Group by parent/child hierarchy
    let mut root_categories: Vec<&Category> = categories
        .iter()
        .filter(|c| c.parent_id.is_none())
        .collect();
    root_categories.sort_by(|a, b| a.name.cmp(&b.name));

    for category in root_categories {
        formatted.push_str(&format!("- {} {}\n", category.emoji, category.name));

        // Add children
        let mut children: Vec<&Category> = categories
            .iter()
            .filter(|c| c.parent_id == Some(category.id))
            .collect();
        children.sort_by(|a, b| a.name.cmp(&b.name));

        for child in children {
            formatted.push_str(&format!("  - {} {}\n", child.emoji, child.name));
        }
    }

    formatted
}

/// Parse LLM JSON response
fn parse_llm_response(response: &str) -> Result<CategoryDecision> {
    log::debug!("Raw LLM response: {}", response);

    // Handle case where model continues from our priming "{{"
    // Response might be: "action": "...", ... instead of {"action": ...
    let json_str = if response.trim_start().starts_with('"') && response.contains('}') {
        // Missing opening brace - add it
        let end = response.rfind('}').unwrap() + 1;
        format!("{{{}", &response[..end])
    } else {
        // Normal case - extract JSON from response
        let json_start = response.find('{');
        let json_end = response.rfind('}');

        if json_start.is_none() || json_end.is_none() {
            log::error!("No JSON found in LLM response: {}", response);
            anyhow::bail!("No JSON object found in response");
        }

        response[json_start.unwrap()..json_end.unwrap() + 1].to_string()
    };

    log::debug!("Extracted JSON: {}", json_str);

    // Try to parse JSON
    let mut decision: CategoryDecision = match serde_json::from_str(&json_str) {
        Ok(d) => d,
        Err(e) => {
            log::error!("JSON parse error: {}", e);
            log::error!("Attempted to parse: {}", json_str);

            // Try to fix common issues
            // Sometimes the model adds trailing commas or formatting issues
            let cleaned = json_str
                .replace(",}", "}")
                .replace(",]", "]")
                .replace("\n", " ")
                .replace("  ", " ");

            log::debug!("Trying cleaned JSON: {}", cleaned);
            serde_json::from_str(&cleaned)
                .with_context(|| format!("Failed to parse LLM response. Original: {}", json_str))?
        }
    };

    log::debug!("Parsed decision: action={:?}, category={}", decision.action, decision.category_name);

    // Validate and sanitize
    if decision.category_name.is_empty() {
        anyhow::bail!("Category name is empty");
    }

    // Default emoji if missing
    if decision.emoji.is_none() || decision.emoji.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
        decision.emoji = Some(default_emoji_for_category(&decision.category_name));
    }

    // Clamp confidence
    decision.confidence = decision.confidence.clamp(0.0, 1.0);

    Ok(decision)
}

/// Get default emoji based on category name
fn default_emoji_for_category(name: &str) -> String {
    let name_lower = name.to_lowercase();

    if name_lower.contains("code") || name_lower.contains("snippet") {
        "💻".to_string()
    } else if name_lower.contains("doc") || name_lower.contains("guide") {
        "📚".to_string()
    } else if name_lower.contains("command") || name_lower.contains("terminal") {
        "⌨️".to_string()
    } else if name_lower.contains("link") || name_lower.contains("url") {
        "🔗".to_string()
    } else if name_lower.contains("error") || name_lower.contains("bug") {
        "🐛".to_string()
    } else if name_lower.contains("task") || name_lower.contains("todo") {
        "✅".to_string()
    } else if name_lower.contains("note") {
        "📝".to_string()
    } else if name_lower.contains("config") || name_lower.contains("setting") {
        "⚙️".to_string()
    } else {
        "📁".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_parse_llm_response() {
        let response = r#"{"action": "create_new", "category_name": "Python Scripts", "emoji": "🐍", "reasoning": "This is Python code", "confidence": 0.9}"#;

        let decision = parse_llm_response(response).unwrap();
        assert_eq!(decision.action, CategoryAction::CreateNew);
        assert_eq!(decision.category_name, "Python Scripts");
        assert_eq!(decision.emoji, Some("🐍".to_string()));
        assert_eq!(decision.confidence, 0.9);
    }

    #[test]
    fn test_parse_llm_response_with_extra_text() {
        let response = r#"Here is the categorization:
{"action": "use_existing", "category_name": "Documentation", "emoji": "📚", "reasoning": "Tutorial content", "confidence": 0.85}
Hope this helps!"#;

        let decision = parse_llm_response(response).unwrap();
        assert_eq!(decision.action, CategoryAction::UseExisting);
        assert_eq!(decision.category_name, "Documentation");
    }

    #[test]
    fn test_format_categories() {
        let categories = vec![
            Category {
                id: 1,
                name: "Code".to_string(),
                parent_id: None,
                emoji: "💻".to_string(),
                created_at: Utc::now(),
            },
            Category {
                id: 2,
                name: "Python".to_string(),
                parent_id: Some(1),
                emoji: "🐍".to_string(),
                created_at: Utc::now(),
            },
        ];

        let formatted = format_categories(&categories);
        assert!(formatted.contains("💻 Code"));
        assert!(formatted.contains("🐍 Python"));
    }

    #[test]
    fn test_default_emoji() {
        assert_eq!(default_emoji_for_category("Python Code"), "💻");
        assert_eq!(default_emoji_for_category("Documentation"), "📚");
        assert_eq!(default_emoji_for_category("Terminal Commands"), "⌨️");
        assert_eq!(default_emoji_for_category("Random Category"), "📁");
    }
}
