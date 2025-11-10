// RAG prompt builder - Phase 2

/// Build RAG context from snippets
pub async fn build_rag_context(
    query: &str,
    snippet_ids: Vec<i64>,
) -> Result<String, String> {
    use crate::db::sqlite::get_snippets_by_ids;

    // Fetch snippets
    let snippets = get_snippets_by_ids(&snippet_ids)
        .await
        .map_err(|e| format!("Failed to fetch snippets: {}", e))?;

    // Smart context window management
    // Reserve tokens for query + answer (rough estimate: 4 chars per token)
    let query_tokens = estimate_tokens(query);
    let available_tokens = 512 - query_tokens - 150; // Reserve for answer

    let mut context = String::new();
    let mut used_tokens = 0;

    for snippet in snippets {
        let snippet_tokens = estimate_tokens(&snippet.content);

        if used_tokens + snippet_tokens > available_tokens {
            // Try to fit a truncated version
            let remaining = available_tokens - used_tokens;
            if remaining > 50 {
                // Only if meaningful space left
                context.push_str(&truncate_smart(&snippet.content, remaining));
                context.push_str("\n---\n");
            }
            break;
        }

        context.push_str(&format!(
            "[{}] {}\n---\n",
            snippet.source_app.as_deref().unwrap_or("Unknown"),
            snippet.content
        ));
        used_tokens += snippet_tokens;
    }

    Ok(context)
}

/// Build RAG prompt
pub fn build_rag_prompt(query: &str, context: &str) -> String {
    format!(
        r#"You are a helpful assistant. Use *only* the context provided below to answer the user's question. Do not make information up.

Context:
{}

Question: {}

Answer:
"#,
        context, query
    )
}

/// Estimate token count (rough: 4 characters per token)
fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

/// Smart truncation - keep first and last sentences
fn truncate_smart(text: &str, max_tokens: usize) -> String {
    let max_chars = max_tokens * 4; // ~4 chars per token
    
    if text.len() <= max_chars {
        return text.to_string();
    }

    let sentences: Vec<&str> = text.split(". ").collect();
    
    if sentences.len() <= 2 {
        return text.chars().take(max_chars).collect();
    }

    let first = sentences[0];
    let last = sentences.last().unwrap();
    
    format!("{} ... {}", first, last)
}
