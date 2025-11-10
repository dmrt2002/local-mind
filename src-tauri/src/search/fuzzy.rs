use strsim::levenshtein;

/// Check if a word matches content using fuzzy matching (typo tolerance)
pub fn fuzzy_match_word(word: &str, content: &str, max_edit_distance: usize) -> bool {
    let word_lower = word.to_lowercase();
    let content_lower = content.to_lowercase();

    // Split content into words (simple whitespace split)
    let content_words: Vec<&str> = content_lower
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| !w.is_empty())
        .collect();

    // Check if any content word is within edit distance
    for content_word in content_words {
        let distance = levenshtein(&word_lower, content_word);
        if distance <= max_edit_distance && distance < word_lower.len() {
            // Only accept if edit distance is less than word length
            return true;
        }

        // Also check if word is a prefix/suffix of content word (handles truncation)
        if content_word.starts_with(&word_lower) || word_lower.starts_with(content_word) {
            let len_diff = (content_word.len() as i32 - word_lower.len() as i32).abs() as usize;
            if len_diff <= max_edit_distance {
                return true;
            }
        }
    }

    false
}

/// Calculate maximum allowed edit distance based on word length
pub fn calculate_max_edit_distance(word: &str) -> usize {
    let len = word.chars().count();
    match len {
        0..=2 => 0, // Too short for fuzzy matching
        3..=4 => 1, // Allow 1 typo
        5..=8 => 2, // Allow 2 typos
        _ => 2,     // Cap at 2 for very long words
    }
}

/// Check if query words match content with fuzzy tolerance
pub fn fuzzy_match_all_words(query_words: &[&str], content: &str, require_majority: bool) -> bool {
    if query_words.is_empty() {
        return true;
    }

    let mut matches = 0;
    for word in query_words {
        let max_dist = calculate_max_edit_distance(word);
        if fuzzy_match_word(word, content, max_dist) {
            matches += 1;
        }
    }

    if require_majority {
        let min_required = if query_words.len() <= 2 {
            1
        } else {
            (query_words.len() + 1) / 2
        };
        matches >= min_required
    } else {
        matches > 0
    }
}
