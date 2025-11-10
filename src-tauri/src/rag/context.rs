// Smart context window management - Phase 2

use crate::db::sqlite::Snippet;

/// Select best snippets for context based on relevance
pub fn select_context_snippets(
    snippets: Vec<Snippet>,
    max_tokens: usize,
) -> Vec<Snippet> {
    // Simple implementation: take first N snippets that fit
    // In production, could use more sophisticated ranking
    
    let mut selected = Vec::new();
    let mut used_tokens = 0;

    for snippet in snippets {
        let snippet_tokens = estimate_tokens(&snippet.content);
        
        if used_tokens + snippet_tokens <= max_tokens {
            selected.push(snippet);
            used_tokens += snippet_tokens;
        } else {
            break;
        }
    }

    selected
}

fn estimate_tokens(text: &str) -> usize {
    text.len() / 4 // Rough estimate
}
