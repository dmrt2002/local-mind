use serde::{Deserialize, Serialize};

/// Represents different types of search queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryType {
    /// Exact phrase match (quoted string)
    Phrase(String),
    /// Words that must appear near each other
    Proximity { words: Vec<String>, distance: usize },
    /// Boolean query with operators
    Boolean {
        terms: Vec<QueryTerm>,
        operator: BooleanOp,
    },
    /// Simple word-based query
    Words(Vec<String>),
}

/// Boolean operators for search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BooleanOp {
    And,
    Or,
    Not,
}

/// Individual search term
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QueryTerm {
    Word(String),
    Phrase(String),
    Not(String), // Negated term
}

/// Parsed query with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedQuery {
    pub query_type: QueryType,
    pub original: String,
    pub expanded_terms: Vec<String>, // Includes synonyms
    pub fuzzy_enabled: bool,
}

/// Parse a search query into structured format
pub fn parse_query(query: &str) -> ParsedQuery {
    let trimmed = query.trim();
    let original = trimmed.to_string();

    // Check for quoted phrases
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() > 2 {
        let phrase = trimmed.trim_matches('"').to_string();
        return ParsedQuery {
            query_type: QueryType::Phrase(phrase.clone()),
            original,
            expanded_terms: vec![phrase],
            fuzzy_enabled: false, // Phrase matching is exact
        };
    }

    // Proximity search (NEAR operator) disabled for now
    // Removed per user request

    // Check for boolean operators (simple parsing)
    let upper = trimmed.to_uppercase();
    let has_or = upper.contains(" OR ");
    let has_and = upper.contains(" AND ");
    let has_not = upper.contains(" NOT ");

    if has_or || has_and || has_not {
        let terms = parse_boolean_terms(trimmed);
        let operator = if has_or {
            BooleanOp::Or
        } else if has_not {
            BooleanOp::Not
        } else {
            BooleanOp::And
        };

        // No query expansion (synonyms disabled)
        let expanded: Vec<String> = terms
            .iter()
            .flat_map(|t| match t {
                QueryTerm::Word(w) => vec![w.clone()],
                QueryTerm::Phrase(p) => vec![p.clone()],
                QueryTerm::Not(n) => vec![n.clone()],
            })
            .collect();

        return ParsedQuery {
            query_type: QueryType::Boolean { terms, operator },
            original,
            expanded_terms: expanded,
            fuzzy_enabled: true,
        };
    }

    // Simple word-based query
    let words: Vec<String> = trimmed.split_whitespace().map(|w| w.to_string()).collect();

    // No query expansion (synonyms disabled)
    let expanded_terms: Vec<String> = words.clone();

    ParsedQuery {
        query_type: QueryType::Words(words.clone()),
        original,
        expanded_terms,
        fuzzy_enabled: true,
    }
}

/// Parse boolean query terms
fn parse_boolean_terms(query: &str) -> Vec<QueryTerm> {
    let mut terms = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let chars: Vec<char> = query.chars().collect();

    for i in 0..chars.len() {
        let c = chars[i];

        if c == '"' {
            in_quotes = !in_quotes;
            continue;
        }

        if !in_quotes {
            // Check for operators
            if i + 3 < chars.len()
                && chars[i..i + 3].iter().collect::<String>().to_uppercase() == "OR "
            {
                if !current.trim().is_empty() {
                    terms.push(QueryTerm::Word(current.trim().to_string()));
                }
                current.clear();
                continue;
            }

            if i + 4 < chars.len()
                && chars[i..i + 4].iter().collect::<String>().to_uppercase() == "AND "
            {
                if !current.trim().is_empty() {
                    terms.push(QueryTerm::Word(current.trim().to_string()));
                }
                current.clear();
                continue;
            }

            if i + 4 < chars.len()
                && chars[i..i + 4].iter().collect::<String>().to_uppercase() == "NOT "
            {
                if !current.trim().is_empty() {
                    terms.push(QueryTerm::Word(current.trim().to_string()));
                }
                current.clear();
                continue;
            }
        }

        current.push(c);
    }

    if !current.trim().is_empty() {
        if in_quotes {
            terms.push(QueryTerm::Phrase(current.trim().to_string()));
        } else {
            terms.push(QueryTerm::Word(current.trim().to_string()));
        }
    }

    if terms.is_empty() {
        // Fallback: split by whitespace
        query
            .split_whitespace()
            .map(|w| QueryTerm::Word(w.to_string()))
            .collect()
    } else {
        terms
    }
}

// Query expansion (synonyms) disabled per user request
// Function removed - synonyms not used in search

/// Build FTS5 query from parsed query
pub fn build_fts5_query(parsed: &ParsedQuery) -> String {
    match &parsed.query_type {
        QueryType::Phrase(phrase) => {
            format!("\"{}\"", phrase.replace("\"", "\"\""))
        }
        QueryType::Proximity { words, distance: _ } => {
            // Proximity search disabled - treat as regular AND query
            words
                .iter()
                .enumerate()
                .map(|(idx, w)| {
                    if idx == 0 && w.chars().count() >= 3 {
                        format!("{}*", w)
                    } else {
                        w.clone()
                    }
                })
                .collect::<Vec<_>>()
                .join(" AND ")
        }
        QueryType::Boolean { terms, operator } => {
            let term_strs: Vec<String> = terms
                .iter()
                .map(|t| match t {
                    QueryTerm::Word(w) => {
                        if w.len() >= 3 {
                            format!("{}*", w)
                        } else {
                            w.clone()
                        }
                    }
                    QueryTerm::Phrase(p) => format!("\"{}\"", p.replace("\"", "\"\"")),
                    QueryTerm::Not(n) => format!("NOT {}", n),
                })
                .collect();

            match operator {
                BooleanOp::And => term_strs.join(" AND "),
                BooleanOp::Or => term_strs.join(" OR "),
                BooleanOp::Not => format!("NOT {}", term_strs.join(" ")),
            }
        }
        QueryType::Words(words) => words
            .iter()
            .enumerate()
            .map(|(idx, w)| {
                if idx == 0 && w.chars().count() >= 3 {
                    format!("{}*", w)
                } else {
                    w.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(" AND "),
    }
}
