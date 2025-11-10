# Advanced Search Techniques for LocalMind

## Current System (What You Have)

✅ **Hybrid Search Architecture**
- **FTS5 Keyword Search**: Fast exact/prefix matching with BM25 ranking
- **Semantic Search**: Vector embeddings for meaning-based search
- **Combined Results**: Merges both with deduplication

✅ **Current Features**
- Prefix matching (3+ characters)
- Whole-word validation
- Majority-word matching (50%+ for multi-word queries)
- BM25 relevance ranking

---

## Advanced Search Techniques (Ranked by Value)

### 🔥 **Tier 1: High Impact, Medium Effort**

#### 1. **Fuzzy Matching / Typo Tolerance**
**What it does**: Finds results even with spelling mistakes
- "greensage" → matches "greensage", "greensage", "greensage"
- Handles 1-2 character typos, transpositions

**Implementation**:
```rust
// Use a library like strsim or implement Levenshtein distance
// Check if query words match content words within edit distance threshold
fn fuzzy_match(query: &str, content: &str, max_distance: usize) -> bool {
    // Levenshtein distance algorithm
    // Allow 1-2 character differences
}
```

**Value**: ⭐⭐⭐⭐⭐ Very high - Users make typos constantly
**Effort**: Medium (add validation layer)

---

#### 2. **Query Expansion (Synonyms & Related Terms)**
**What it does**: Expands query to include synonyms/related terms
- "AI" → also searches for "artificial intelligence", "machine learning"
- "ML" → also searches for "machine learning", "ML algorithm"

**Implementation Options**:
- **Local Dictionary**: Small synonym map for common terms
- **WordNet Integration**: More comprehensive but larger
- **Context-Based**: Learn from user's own snippets

**Example**:
```rust
fn expand_query(query: &str) -> Vec<String> {
    let mut expanded = vec![query.to_string()];
    
    // Add synonyms
    if query.contains("AI") {
        expanded.push(query.replace("AI", "artificial intelligence"));
        expanded.push(query.replace("AI", "machine learning"));
    }
    
    expanded
}
```

**Value**: ⭐⭐⭐⭐ High - Finds semantically related content
**Effort**: Medium (build synonym dictionary)

---

#### 3. **Phrase Matching (Quoted Strings)**
**What it does**: Exact phrase search with quotes
- `"greensage ai"` → matches exact phrase (order matters)
- `greensage ai` → matches both words anywhere (current behavior)

**Implementation**:
```rust
fn parse_query(query: &str) -> QueryType {
    if query.starts_with('"') && query.ends_with('"') {
        QueryType::Phrase(query.trim_matches('"').to_string())
    } else {
        QueryType::Words(query.split_whitespace().collect())
    }
}
```

**Value**: ⭐⭐⭐⭐ High - Users expect this behavior
**Effort**: Low (modify query parser)

---

#### 4. **Proximity Search (NEAR operator)**
**What it does**: Finds words within N words of each other
- `greensage NEAR/5 algorithm` → finds "greensage" within 5 words of "algorithm"
- Better than AND (words can be anywhere)

**Implementation**:
```rust
// FTS5 supports NEAR operator
"greensage NEAR/5 algorithm"
```

**Value**: ⭐⭐⭐⭐ High - More precise than AND
**Effort**: Low (FTS5 native feature)

---

#### 5. **Boost Recent Results**
**What it does**: Slightly favor recently saved snippets
- Combine BM25 score with recency factor
- `final_score = bm25_score * 0.8 + recency_boost * 0.2`

**Implementation**:
```rust
fn calculate_boosted_rank(bm25: f64, created_at: DateTime<Utc>) -> f64 {
    let days_old = (Utc::now() - created_at).num_days();
    let recency_boost = (-days_old as f64 / 30.0).exp(); // Decay over 30 days
    bm25 * 0.8 + recency_boost * 0.2
}
```

**Value**: ⭐⭐⭐ Medium - Recent content is often more relevant
**Effort**: Low (modify ranking)

---

### 🔶 **Tier 2: Medium Impact, Variable Effort**

#### 6. **Stemming & Lemmatization**
**What it does**: Matches word variations
- "running" → matches "run", "runs", "ran"
- "algorithms" → matches "algorithm", "algorithmic"

**Implementation**:
- **Lightweight**: Porter stemmer (simple, fast)
- **Better**: Full lemmatization (needs dictionary)

**Value**: ⭐⭐⭐ Medium - Improves recall
**Effort**: Medium (add stemming library)

---

#### 7. **Field-Based Search (Metadata)**
**What it does**: Search specific fields (content, source_app, tags)
- `content:greensage` → only searches content field
- `app:TablePlus` → only searches source_app
- `tag:work` → searches metadata tags

**Implementation**:
```rust
// Parse query for field:value patterns
fn parse_field_query(query: &str) -> (Option<String>, String) {
    if let Some(colon_pos) = query.find(':') {
        let field = &query[..colon_pos];
        let value = &query[colon_pos + 1..];
        return (Some(field.to_string()), value.to_string());
    }
    (None, query.to_string())
}
```

**Value**: ⭐⭐⭐ Medium - Power users love this
**Effort**: Medium (extend query parser + UI)

---

#### 8. **Boolean Operators (AND, OR, NOT)**
**What it does**: Explicit control over search logic
- `greensage AND algorithm` → both must appear (default now)
- `greensage OR AI` → either can appear
- `greensage NOT bad` → exclude "bad"

**Implementation**:
```rust
// FTS5 supports these operators
"greensage OR AI"
"greensage NOT bad"
```

**Value**: ⭐⭐⭐ Medium - Power users only
**Effort**: Low (FTS5 native)

---

#### 9. **Autocomplete / Search Suggestions**
**What it does**: Show suggestions as user types
- Type "gre" → suggests "greensage", "green machine", etc.
- Based on popular queries or existing content

**Value**: ⭐⭐⭐ Medium - Better UX
**Effort**: Medium (build suggestion index)

---

#### 10. **Result Highlighting**
**What it does**: Highlight matching terms in results
- Shows which words matched
- Helps user quickly understand relevance

**Value**: ⭐⭐⭐ Medium - Better UX
**Effort**: Low (frontend only)

---

### 🔷 **Tier 3: Lower Impact or Higher Effort**

#### 11. **Phonetic Matching (Soundex/Metaphone)**
**What it does**: Matches similar-sounding words
- "smith" → matches "smyth"
- Useful for names/proper nouns

**Value**: ⭐⭐ Low - Limited use case
**Effort**: Medium

---

#### 12. **Multi-Language Support**
**What it does**: Search across multiple languages
- Detect language of query/content
- Apply language-specific stemming

**Value**: ⭐⭐ Low - Depends on user base
**Effort**: High

---

#### 13. **Learning from User Behavior**
**What it does**: Boost results user actually clicks/uses
- Track which results user opens
- Boost those in future searches

**Value**: ⭐⭐⭐ Medium - Improves over time
**Effort**: High (needs tracking system)

---

## Recommended Implementation Priority

### Phase 1: Quick Wins (1-2 days)
1. ✅ **Phrase Matching** - Low effort, high value
2. ✅ **Proximity Search (NEAR)** - FTS5 native
3. ✅ **Result Highlighting** - Frontend only
4. ✅ **Boost Recent Results** - Simple ranking change

### Phase 2: Core Improvements (3-5 days)
5. ✅ **Fuzzy Matching** - High value for typos
6. ✅ **Query Expansion** - Build synonym dictionary
7. ✅ **Boolean Operators** - FTS5 native

### Phase 3: Advanced Features (1-2 weeks)
8. ✅ **Stemming** - Improve recall
9. ✅ **Field-Based Search** - Power users
10. ✅ **Autocomplete** - Better UX

---

## Implementation Example: Fuzzy Matching

Here's how you could add fuzzy matching to your current system:

```rust
// Add to src-tauri/src/db/sqlite.rs

use strsim::levenshtein;

fn fuzzy_match_word(word: &str, content: &str, max_edit_distance: usize) -> bool {
    // Split content into words
    let content_words: Vec<&str> = content
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| !w.is_empty())
        .collect();
    
    // Check if any content word is within edit distance
    for content_word in content_words {
        let distance = levenshtein(word, content_word);
        if distance <= max_edit_distance {
            return true;
        }
    }
    
    false
}

// Modify validation in search_fts5:
if is_first_word && word_chars.len() >= 3 {
    // Try prefix match first
    // ... existing prefix logic ...
    
    // If prefix match fails, try fuzzy match
    if !prefix_found {
        let fuzzy_match = fuzzy_match_word(word, &content_lower, 2); // Allow 2 char edits
        if fuzzy_match {
            return true;
        }
    }
}
```

---

## Key Metrics to Track

Once you implement improvements, track:
- **Precision**: % of results that are actually relevant
- **Recall**: % of relevant results found
- **Search Time**: Average query time
- **User Satisfaction**: Click-through rate on results

---

## References

- [FTS5 Query Syntax](https://www.sqlite.org/fts5.html#fts5_query_syntax)
- [BM25 Algorithm](https://en.wikipedia.org/wiki/Okapi_BM25)
- [Levenshtein Distance](https://en.wikipedia.org/wiki/Levenshtein_distance)
- [Query Expansion Techniques](https://en.wikipedia.org/wiki/Query_expansion)

