use std::collections::HashMap;

/// Extract a category name from content using weighted keyword analysis
pub fn suggest_category_from_content(content: &str) -> (String, String) {
    let content_lower = content.to_lowercase();
    let first_50_words = content_lower.split_whitespace().take(50).collect::<Vec<_>>().join(" ");

    // Technical keywords with their categories, emojis, and weights (higher = more important)
    let category_keywords: Vec<(&str, &str, &str, i32)> = vec![
        // Documentation (HIGH PRIORITY - should win over code keywords)
        ("how to activate", "Documentation", "📚", 10),
        ("how to use", "Documentation", "📚", 10),
        ("how to", "Documentation", "📚", 8),
        ("guide", "Documentation", "📚", 7),
        ("tutorial", "Documentation", "📚", 7),
        ("documentation", "Documentation", "📚", 7),
        ("instructions", "Documentation", "📚", 7),
        ("readme", "Documentation", "📚", 6),
        ("manual", "Documentation", "📚", 6),
        ("steps to", "Documentation", "📚", 6),
        ("you can", "Documentation", "📚", 4),
        ("to enable", "Documentation", "📚", 5),
        ("to activate", "Documentation", "📚", 5),

        // Programming & Code (only if it looks like actual code)
        ("function(", "Code Snippets", "💻", 8),
        ("class ", "Code Snippets", "💻", 7),
        ("import {", "Code Snippets", "💻", 8),
        ("const ", "Code Snippets", "💻", 6),
        ("let ", "Code Snippets", "💻", 6),
        ("var ", "Code Snippets", "💻", 6),
        ("def ", "Code Snippets", "💻", 7),
        ("async function", "Code Snippets", "💻", 8),
        ("=> {", "Code Snippets", "💻", 7),
        ("struct {", "Code Snippets", "💻", 7),
        ("interface {", "Code Snippets", "💻", 7),
        ("public class", "Code Snippets", "💻", 8),
        ("private ", "Code Snippets", "💻", 5),

        // Commands & Terminal - USE CANONICAL CATEGORY NAMES
        // Docker commands
        ("docker-compose ", "Docker Commands", "🐳", 8),
        ("docker compose ", "Docker Commands", "🐳", 8),
        ("docker run", "Docker Commands", "🐳", 7),
        ("docker build", "Docker Commands", "🐳", 7),
        ("docker ", "Docker Commands", "🐳", 5),

        // Node.js / NPM commands
        ("npm install", "Node.js Development", "📦", 7),
        ("npm run", "Node.js Development", "📦", 7),
        ("yarn ", "Node.js Development", "📦", 7),
        ("pnpm ", "Node.js Development", "📦", 7),
        ("npx ", "Node.js Development", "📦", 7),
        ("npm ", "Node.js Development", "📦", 5),

        // Git commands
        ("git commit", "Git Commands", "🔀", 7),
        ("git push", "Git Commands", "🔀", 7),
        ("git pull", "Git Commands", "🔀", 7),
        ("git ", "Git Commands", "🔀", 5),

        // Rust development
        ("cargo build", "Rust Development", "🦀", 7),
        ("cargo run", "Rust Development", "🦀", 7),
        ("cargo test", "Rust Development", "🦀", 7),
        ("cargo ", "Rust Development", "🦀", 5),
        ("rustc ", "Rust Development", "🦀", 5),

        // Python development
        ("python ", "Python Development", "🐍", 6),
        ("pip install", "Python Development", "🐍", 7),
        ("pip ", "Python Development", "🐍", 5),
        ("poetry ", "Python Development", "🐍", 6),

        // Kubernetes
        ("kubectl ", "Kubernetes", "☸️", 7),
        ("helm ", "Kubernetes", "☸️", 7),
        ("k9s ", "Kubernetes", "☸️", 7),

        // Cloud CLI
        ("aws ", "Cloud CLI", "☁️", 6),
        ("gcloud ", "Cloud CLI", "☁️", 6),
        ("az ", "Cloud CLI", "☁️", 6),

        // System Administration
        ("ssh ", "System Administration", "🔧", 6),
        ("scp ", "System Administration", "🔧", 6),
        ("rsync ", "System Administration", "🔧", 6),
        ("sudo ", "System Administration", "🔧", 5),

        // Build Tools
        ("make ", "Build Tools", "🔨", 6),
        ("cmake ", "Build Tools", "🔨", 6),
        ("gradle ", "Build Tools", "🔨", 6),
        ("mvn ", "Build Tools", "🔨", 6),

        // Generic shell (only if nothing else matches)
        ("brew ", "Package Management", "📦", 4),
        ("apt ", "Package Management", "📦", 4),
        ("$ ", "Shell Commands", "⌨️", 3),

        // Web Development
        ("<html", "Web Development", "🌐", 8),
        ("</div>", "Web Development", "🌐", 7),
        ("css:", "Web Development", "🌐", 6),
        ("react component", "Web Development", "🌐", 8),
        ("api endpoint", "Web Development", "🌐", 7),
        ("fetch(", "Web Development", "🌐", 7),
        ("axios", "Web Development", "🌐", 6),

        // Database & SQL
        ("select * from", "Database", "🗄️", 8),
        ("insert into", "Database", "🗄️", 8),
        ("update ", "Database", "🗄️", 4),
        ("delete from", "Database", "🗄️", 7),
        ("create table", "Database", "🗄️", 8),
        ("database", "Database", "🗄️", 5),
        ("query", "Database", "🗄️", 4),
        ("sql", "Database", "🗄️", 5),

        // Ideas & Notes
        ("idea:", "Ideas", "💡", 7),
        ("thinking about", "Ideas", "💡", 6),
        ("note:", "Notes", "📝", 6),
        ("remember to", "Notes", "📝", 5),
        ("todo:", "Tasks", "✅", 7),
        ("task:", "Tasks", "✅", 7),
        ("[ ]", "Tasks", "✅", 6),

        // Links & URLs
        ("http://", "Links", "🔗", 8),
        ("https://", "Links", "🔗", 8),
        ("www.", "Links", "🔗", 6),

        // Configuration
        ("config:", "Configuration", "⚙️", 6),
        ("settings:", "Configuration", "⚙️", 6),
        (".env", "Configuration", "⚙️", 7),
        (".json", "Configuration", "⚙️", 4),
        (".yaml", "Configuration", "⚙️", 5),
        (".toml", "Configuration", "⚙️", 5),

        // Error messages
        ("error:", "Errors & Fixes", "🐛", 7),
        ("exception:", "Errors & Fixes", "🐛", 7),
        ("bug:", "Errors & Fixes", "🐛", 6),
        ("fix:", "Errors & Fixes", "🐛", 6),
        ("failed to", "Errors & Fixes", "🐛", 5),
        ("traceback", "Errors & Fixes", "🐛", 7),

        // Research & Learning
        ("research:", "Research", "🔬", 7),
        ("study:", "Learning", "📖", 6),
        ("learning about", "Learning", "📖", 6),
        ("article:", "Articles", "📰", 6),
        ("blog post", "Articles", "📰", 6),
    ];

    // Count keyword matches for each category using WEIGHTED scores
    let mut category_scores: HashMap<String, (i32, String)> = HashMap::new();

    for (keyword, category, emoji, weight) in &category_keywords {
        // Check in first 50 words for better accuracy (intro text is more indicative)
        let check_in = &first_50_words;

        if check_in.contains(keyword) {
            let entry = category_scores
                .entry(category.to_string())
                .or_insert((0, emoji.to_string()));
            entry.0 += weight; // Use weighted score instead of just counting
        }
    }

    // Find the category with the highest score
    if let Some((category_name, (_, emoji))) = category_scores
        .iter()
        .max_by_key(|(_, (score, _))| score)
    {
        return (category_name.clone(), emoji.clone());
    }

    // Fallback: analyze content quality, not just line count
    // Don't blindly categorize as "Long Notes" based on lines -
    // screenshots often have many lines of garbage

    let line_count = content.lines().count();
    let char_count = content.len();
    let word_count = content.split_whitespace().count();

    // Calculate content quality metrics
    let avg_line_length = if line_count > 0 {
        char_count as f32 / line_count as f32
    } else {
        0.0
    };

    let avg_word_length = if word_count > 0 {
        char_count as f32 / word_count as f32
    } else {
        0.0
    };

    // High quality multi-line content (actual notes/documentation)
    // - Many lines with good average length
    // - Reasonable word length (not gibberish)
    if line_count > 8 && avg_line_length > 30.0 && avg_word_length > 4.0 && avg_word_length < 12.0 {
        return ("Notes".to_string(), "📄".to_string());
    }

    // Short content
    if char_count < 100 || word_count < 10 {
        return ("Quick Notes".to_string(), "✏️".to_string());
    }

    // Default category for everything else
    ("Uncategorized".to_string(), "📁".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_detection() {
        let code = "function test() {\n  const x = 10;\n  return x;\n}";
        let (category, emoji) = suggest_category_from_content(code);
        assert_eq!(category, "Code Snippets");
        assert_eq!(emoji, "💻");
    }

    #[test]
    fn test_command_detection() {
        let cmd = "npm install react\ncargo build";
        let (category, emoji) = suggest_category_from_content(cmd);
        assert_eq!(category, "Commands");
        assert_eq!(emoji, "⚙️");
    }

    #[test]
    fn test_url_detection() {
        let url = "Check out https://example.com for more info";
        let (category, emoji) = suggest_category_from_content(url);
        assert_eq!(category, "Links");
        assert_eq!(emoji, "🔗");
    }

    #[test]
    fn test_fallback() {
        let text = "Random text without specific keywords";
        let (category, _) = suggest_category_from_content(text);
        assert_eq!(category, "Quick Notes");
    }

    #[test]
    fn test_documentation_over_code() {
        // This should be categorized as Documentation, not Code Snippets
        // even though it contains words like "code" and "Claude Code"
        let doc_text = "How to Activate Plan Mode in Claude Code:\n\
                       1. Open Claude Code in your IDE\n\
                       2. Enable Plan Mode from the settings\n\
                       3. You can now approve plans before execution";
        let (category, emoji) = suggest_category_from_content(doc_text);
        assert_eq!(category, "Documentation");
        assert_eq!(emoji, "📚");
    }
}
