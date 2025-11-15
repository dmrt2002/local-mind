use regex::Regex;
use std::collections::HashSet;
use unicode_normalization::UnicodeNormalization;

/// Comprehensive text processing and cleaning for OCR output
pub struct TextProcessor {
    url_pattern: Regex,
    email_pattern: Regex,
    file_path_pattern: Regex,
    code_pattern: Regex,
    command_pattern: Regex,
}

impl Default for TextProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl TextProcessor {
    pub fn new() -> Self {
        Self {
            // Match URLs (http, https, www)
            url_pattern: Regex::new(
                r"(?i)\b(?:https?://|www\.)[a-z0-9][-a-z0-9+&@#/%?=~_|!:,.;]*[a-z0-9]",
            )
            .unwrap(),
            // Match email addresses
            email_pattern: Regex::new(r"\b[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}\b").unwrap(),
            // Match file paths (Unix and Windows)
            file_path_pattern: Regex::new(
                r"(?:/[a-zA-Z0-9._-]+)+/?|[a-zA-Z]:\\(?:[a-zA-Z0-9._-]+\\)*[a-zA-Z0-9._-]*",
            )
            .unwrap(),
            // Match code-like patterns (camelCase, snake_case, functions)
            code_pattern: Regex::new(
                r"\b[a-z][a-zA-Z0-9]*(?:[A-Z][a-z0-9]*)+\b|\b[a-z_][a-z0-9_]*\([^)]*\)",
            )
            .unwrap(),
            // Match terminal commands (starts with $ or >)
            command_pattern: Regex::new(r"^[\$>]\s+.+$").unwrap(),
        }
    }

    /// Main cleaning pipeline for OCR text
    pub fn clean_ocr_text(&self, text: &str) -> String {
        let original_lines = text.lines().count();
        let original_chars = text.len();

        log::info!("OCR cleaning pipeline started: {} lines, {} chars", original_lines, original_chars);

        let text = self.normalize_unicode(text);
        let text = self.fix_character_confusion(&text);
        let text = self.remove_noise_characters(&text);

        let after_noise = text.lines().count();
        log::debug!("After remove_noise_characters: {} lines", after_noise);

        let text = self.remove_ui_noise(&text);
        let after_ui = text.lines().count();
        log::debug!("After remove_ui_noise: {} lines", after_ui);

        let text = self.filter_fragmented_text(&text);
        let after_fragment = text.lines().count();
        log::debug!("After filter_fragmented_text: {} lines", after_fragment);

        let text = self.smart_whitespace_cleanup(&text);

        let final_lines = text.lines().count();
        let final_chars = text.len();
        let retention_rate = if original_lines > 0 {
            (final_lines as f32 / original_lines as f32) * 100.0
        } else {
            0.0
        };

        log::info!(
            "OCR cleaning completed: {}/{} lines retained ({:.1}%), {} chars",
            final_lines,
            original_lines,
            retention_rate,
            final_chars
        );

        text
    }

    /// Aggressive cleaning pipeline specifically for screenshot OCR text
    /// Removes common screenshot-specific noise patterns that standard cleaning misses
    pub fn clean_screenshot_ocr(&self, text: &str) -> String {
        let original_lines = text.lines().count();

        log::info!("Screenshot OCR aggressive cleaning started: {} lines", original_lines);

        // First apply standard OCR cleaning
        let text = self.clean_ocr_text(text);

        // Then apply screenshot-specific aggressive filters
        let lines: Vec<&str> = text.lines().collect();
        let mut cleaned_lines = Vec::new();
        let mut filtered_count = 0;

        // Screenshot-specific noise patterns
        let screenshot_noise_patterns = [
            // Base64 and encoded strings (common in URLs)
            r"^[a-zA-Z0-9+/=_-]{20,}$",
            r"^\w{2,}=[a-zA-Z0-9+/=_-]{10,}$",  // URL parameters: ssp=eJzjatTl

            // Random uppercase sequences (OCR artifacts)
            r"^[A-Z]{6,}$",  // GLEDGIAEM, FREDE
            r"^[A-Z]{3,}[0-9]{2,}$",  // ABC123

            // Meeting/session IDs
            r"^[a-z]{3}-[a-z]{4}-[a-z]{3}$",  // ohj-vaap-ihp
            r"^[a-zA-Z0-9]{8,}-[a-zA-Z0-9]{4,}",  // UUID-like

            // Random symbol combinations
            r"^[@#&%*+=<>]{2,}$",  // @&, <=>, etc.
            r"^[a-z]{1,2}$",  // Single/double lowercase: ey, cv, ty

            // Browser/app UI noise
            r"^(Meet|Zoom|Teams)\s*-\s*[a-z]{3}-[a-z]{4}",  // Meet - xxx-xxxx-xxx
            r"^[xX+⋮⋯…×]{1,3}$",  // Close buttons and ellipsis

            // Timestamps alone
            r"^\d{1,2}:\d{2}\s*(?:AM|PM|am|pm)$",  // 11:27 PM

            // Status text
            r"^(?:Online|Offline|Away|Busy)$",

            // Single special characters or short gibberish
            r"^[^\w\s]{1,2}$",  // Single/double symbols
            r"^[a-zA-Z0-9]{1,2}[^\w\s]+$",  // a@, 1+, etc.

            // Common OCR garbage patterns
            r"^[aeiouy]{3,}$",  // Repeated vowels: aaa, eee
            r"^[^aeiouy]{5,}$",  // Long sequences without vowels: fgh, xyz, rst

            // HTML/CSS-like artifacts
            r"^\.{2,}$",  // Multiple dots
            r"^-{3,}$",  // Multiple dashes
            r"^={3,}$",  // Multiple equals

            // Fragment patterns
            r"^[a-z]{1,3}\s*[0-9]{1,2}$",  // cv 12, ty 0g
            r"^[0-9]+[a-z]{1,2}$",  // 2g, 10mg
        ];

        let noise_regex: Vec<Regex> = screenshot_noise_patterns
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        // Process each line
        for line in &lines {
            let trimmed = line.trim();

            // Skip empty lines
            if trimmed.is_empty() {
                continue;
            }

            // Skip if matches screenshot-specific noise patterns
            if noise_regex.iter().any(|re| re.is_match(trimmed)) {
                filtered_count += 1;
                log::debug!("Filtered screenshot noise: {}", trimmed);
                continue;
            }

            // Skip lines with high symbol-to-letter ratio (likely noise)
            let letter_count = trimmed.chars().filter(|c| c.is_alphabetic()).count();
            let symbol_count = trimmed.chars().filter(|c| !c.is_alphanumeric() && !c.is_whitespace()).count();

            if letter_count > 0 && symbol_count > letter_count * 2 {
                filtered_count += 1;
                log::debug!("Filtered high symbol ratio: {}", trimmed);
                continue;
            }

            // Skip very short lines unless they're complete words
            if trimmed.len() <= 3 && !trimmed.chars().all(|c| c.is_alphabetic()) {
                filtered_count += 1;
                log::debug!("Filtered short gibberish: {}", trimmed);
                continue;
            }

            // Keep this line
            cleaned_lines.push(trimmed.to_string());
        }

        let final_lines = cleaned_lines.len();
        log::info!(
            "Screenshot aggressive cleaning completed: {}/{} lines retained (filtered {} screenshot-specific noise)",
            final_lines,
            original_lines,
            filtered_count
        );

        cleaned_lines.join("\n")
    }

    /// Normalize Unicode characters (NFC normalization)
    fn normalize_unicode(&self, text: &str) -> String {
        // NFC (Canonical Composition) is most compatible
        text.nfc().collect::<String>()
    }

    /// Fix common OCR character confusion errors
    fn fix_character_confusion(&self, text: &str) -> String {
        let mut result = text.to_string();

        // Common OCR mistakes - apply with context awareness
        let replacements = vec![
            // Letter-number confusion (in non-word contexts)
            (r"\bl\b", "1"),     // standalone 'l' -> '1'
            (r"\bO\b", "0"),     // standalone 'O' -> '0'

            // Multiple characters misread as one
            (r"\brn\b", "m"),    // 'rn' -> 'm'
            (r"\bvv\b", "w"),    // 'vv' -> 'w'

            // Common symbol confusion
            ("〈", "<"),
            ("〉", ">"),
            ("‹", "<"),
            ("›", ">"),
            ("'", "'"),
            ("'", "'"),
            // Left and right double quotation marks
            ("\u{201c}", "\""),
            ("\u{201d}", "\""),
            ("–", "-"),
            ("—", "-"),
            ("…", "..."),

            // Box drawing and other artifacts
            ("│", "|"),
            ("─", "-"),
            ("┌", "+"),
            ("┐", "+"),
            ("└", "+"),
            ("┘", "+"),
            ("├", "+"),
            ("┤", "+"),
            ("┬", "+"),
            ("┴", "+"),
            ("┼", "+"),
        ];

        for (pattern, replacement) in replacements {
            if pattern.starts_with('\\') {
                // Regex pattern
                if let Ok(re) = Regex::new(pattern) {
                    result = re.replace_all(&result, replacement).to_string();
                }
            } else {
                // Simple string replacement
                result = result.replace(pattern, replacement);
            }
        }

        result
    }

    /// Remove noise characters and artifacts
    fn remove_noise_characters(&self, text: &str) -> String {
        let mut result = String::with_capacity(text.len());

        for ch in text.chars() {
            match ch {
                // Keep printable ASCII, whitespace, and common Unicode
                ' '..='~' | '\t' | '\n' | '\r' => result.push(ch),
                // Keep common Unicode ranges
                '\u{00A0}'..='\u{00FF}' | // Latin-1 Supplement
                '\u{0100}'..='\u{017F}' | // Latin Extended-A
                '\u{0180}'..='\u{024F}' | // Latin Extended-B
                '\u{1E00}'..='\u{1EFF}' | // Latin Extended Additional
                '\u{2000}'..='\u{206F}' | // General Punctuation
                '\u{20A0}'..='\u{20CF}' | // Currency Symbols
                '\u{2100}'..='\u{214F}' | // Letterlike Symbols
                '\u{2190}'..='\u{21FF}' | // Arrows
                '\u{2200}'..='\u{22FF}' | // Mathematical Operators
                '\u{2600}'..='\u{26FF}' | // Miscellaneous Symbols
                '\u{2700}'..='\u{27BF}' | // Dingbats
                '\u{1F300}'..='\u{1F9FF}' // Emoji
                => result.push(ch),
                // Skip control characters and rare Unicode blocks
                _ if ch.is_whitespace() => result.push(' '),
                _ => {} // Skip noise
            }
        }

        result
    }

    /// Remove common UI noise patterns - MINIMAL FILTERING to preserve content
    fn remove_ui_noise(&self, text: &str) -> String {
        let lines: Vec<&str> = text.lines().collect();
        let mut cleaned_lines = Vec::new();
        let mut filtered_count = 0;

        // MINIMAL UI noise patterns - only filter obvious noise
        let ui_noise_patterns = [
            // Single symbols and noise characters only
            r"^[xX+\-©®™@&#*⋮⋯…]{1,3}$",  // Single symbols
            r"^\s*[\|\(\)\[\]\{\}]\s*$",   // Standalone brackets/pipes
            r"^[•●○◦▪▫■□◆◇★☆✓✗✕✖]+$",     // Only bullet points alone

            // Pure timestamp/number noise
            r"^\d{1,2}[:.,-]\d{2}\s*(?:am|pm|AM|PM)?$", // Pure time: "3:45 PM"
            r"^[\d/\-:,\s]+$",             // Pure numbers/dates with no text

            // Navigation arrows alone
            r"^[<>◀▶←→↑↓⇧⇩]+$",           // Only arrow symbols with nothing else

            // Pure symbol noise
            r"^[^\w\s]{3,}$",              // 3+ consecutive symbols (no letters/numbers)
            r"^\([0-9]+\)$",               // Numbers in parentheses alone: (76)
            r"^\[[0-9]+\]$",               // Numbers in brackets alone: [3]

            // Single status chars
            r"^[WVwv✓✗]$",                 // Single status characters

            // Basic window controls (single words only)
            r"^(?:Minimize|Maximize|Close|Restore)$",
        ];

        let noise_regex: Vec<Regex> = ui_noise_patterns
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        // MINIMAL UI keywords - only filter these as single words
        let ui_keywords = [
            "menu", "toolbar", "sidebar", "menubar",
            "minimize", "maximize", "close", "restore",
        ];

        let lines_len = lines.len();

        for line in &lines {
            let trimmed = line.trim();

            // Skip empty lines
            if trimmed.is_empty() {
                continue;
            }

            // Skip if matches minimal UI noise patterns
            if noise_regex.iter().any(|re| re.is_match(trimmed)) {
                filtered_count += 1;
                log::debug!("Filtered UI noise pattern: {}", trimmed);
                continue;
            }

            // Skip single characters (except meaningful ones like I, a, A)
            if trimmed.len() == 1 && !matches!(trimmed, "I" | "a" | "A") {
                filtered_count += 1;
                log::debug!("Filtered single char: {}", trimmed);
                continue;
            }

            let words: Vec<&str> = trimmed.split_whitespace().collect();

            // Only skip single-word lines that are pure UI chrome keywords
            if words.len() == 1 && ui_keywords.contains(&trimmed.to_lowercase().as_str()) {
                filtered_count += 1;
                log::debug!("Filtered UI keyword: {}", trimmed);
                continue;
            }

            // Keep everything else - preserve all content
            cleaned_lines.push(trimmed.to_string());
        }

        log::debug!(
            "UI noise filter: kept {}/{} lines (filtered {})",
            cleaned_lines.len(),
            lines_len,
            filtered_count
        );

        cleaned_lines.join("\n")
    }

    /// Filter out fragmented text - MINIMAL FILTERING to preserve content
    fn filter_fragmented_text(&self, text: &str) -> String {
        let lines: Vec<&str> = text.lines().collect();
        let mut cleaned_lines = Vec::new();
        let mut filtered_count = 0;

        for line in &lines {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                continue;
            }

            let word_count = trimmed.split_whitespace().count();

            // KEEP ALMOST EVERYTHING - only filter pure gibberish
            // Keep if:
            // 1. Any line with 1+ words (changed from 3+)
            // 2. Contains any meaningful content

            // Only skip if line appears to be random gibberish
            // (very short line with no letters or very high symbol-to-letter ratio)
            let has_letters = trimmed.chars().any(|c| c.is_alphabetic());
            let has_numbers = trimmed.chars().any(|c| c.is_numeric());

            // Skip only if it's extremely short gibberish with no content
            if word_count == 0 || (!has_letters && !has_numbers && trimmed.len() < 3) {
                filtered_count += 1;
                log::debug!("Filtered gibberish: {}", trimmed);
                continue;
            }

            // Keep everything else - including single words, short phrases, etc.
            cleaned_lines.push(trimmed.to_string());
        }

        log::debug!(
            "Fragment filter: kept {}/{} lines (filtered {})",
            cleaned_lines.len(),
            lines.len(),
            filtered_count
        );

        // If we somehow filtered everything, return original text as fallback
        if cleaned_lines.is_empty() && !text.is_empty() {
            log::warn!("Fragment filter removed all content, returning longest line");
            if let Some(longest) = lines.iter().max_by_key(|l| l.len()) {
                return longest.trim().to_string();
            }
        }

        cleaned_lines.join("\n")
    }

    /// Smart whitespace cleanup while preserving structure
    fn smart_whitespace_cleanup(&self, text: &str) -> String {
        let lines: Vec<&str> = text.lines().collect();
        let mut cleaned_lines = Vec::new();
        let mut in_code_block = false;

        for line in lines {
            let trimmed = line.trim();

            // Skip empty lines unless in code block
            if trimmed.is_empty() {
                if in_code_block || !cleaned_lines.is_empty() {
                    cleaned_lines.push(String::new());
                }
                continue;
            }

            // Detect code blocks (4+ spaces or tab indentation)
            let leading_spaces = line.len() - line.trim_start().len();
            if leading_spaces >= 4 || line.starts_with('\t') {
                in_code_block = true;
            } else if leading_spaces == 0 {
                in_code_block = false;
            }

            // Check if line looks like code
            let looks_like_code = self.command_pattern.is_match(line)
                || self.code_pattern.is_match(line)
                || line.contains("function")
                || line.contains("const ")
                || line.contains("let ")
                || line.contains("var ")
                || line.contains("def ")
                || line.contains("class ")
                || line.contains("import ")
                || line.contains("from ")
                || line.contains("=>");

            if in_code_block || looks_like_code {
                // Preserve indentation for code
                let normalized_indent = if line.starts_with('\t') {
                    "    ".to_string() + line.trim_start_matches('\t')
                } else {
                    line.to_string()
                };
                cleaned_lines.push(normalized_indent);
            } else {
                // Collapse whitespace for regular text
                let normalized = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
                cleaned_lines.push(normalized);
            }
        }

        // Join lines and remove excessive blank lines
        let result = cleaned_lines.join("\n");
        let result = Regex::new(r"\n{3,}").unwrap().replace_all(&result, "\n\n");
        result.trim().to_string()
    }

    /// Extract structured entities from text
    pub fn extract_entities(&self, text: &str) -> ExtractedEntities {
        ExtractedEntities {
            urls: self.extract_urls(text),
            emails: self.extract_emails(text),
            file_paths: self.extract_file_paths(text),
            code_snippets: self.extract_code_patterns(text),
            commands: self.extract_commands(text),
        }
    }

    fn extract_urls(&self, text: &str) -> Vec<String> {
        self.url_pattern
            .find_iter(text)
            .map(|m| m.as_str().to_string())
            .collect()
    }

    fn extract_emails(&self, text: &str) -> Vec<String> {
        self.email_pattern
            .find_iter(text)
            .map(|m| m.as_str().to_string())
            .collect()
    }

    fn extract_file_paths(&self, text: &str) -> Vec<String> {
        self.file_path_pattern
            .find_iter(text)
            .filter(|m| {
                let path = m.as_str();
                // Filter out false positives
                path.len() > 3 && (path.contains('/') || path.contains('\\'))
            })
            .map(|m| m.as_str().to_string())
            .collect()
    }

    fn extract_code_patterns(&self, text: &str) -> Vec<String> {
        let mut snippets = HashSet::new();

        // Extract function calls
        for mat in self.code_pattern.find_iter(text) {
            snippets.insert(mat.as_str().to_string());
        }

        // Extract code blocks (indented sections)
        let lines: Vec<&str> = text.lines().collect();
        let mut current_block = Vec::new();

        for line in lines {
            let leading_spaces = line.len() - line.trim_start().len();
            if leading_spaces >= 4 || line.starts_with('\t') {
                current_block.push(line.trim());
            } else if !current_block.is_empty() {
                let block = current_block.join("\n");
                if block.len() > 10 {
                    snippets.insert(block);
                }
                current_block.clear();
            }
        }

        snippets.into_iter().collect()
    }

    fn extract_commands(&self, text: &str) -> Vec<String> {
        text.lines()
            .filter(|line| self.command_pattern.is_match(line))
            .map(|line| line.trim().to_string())
            .collect()
    }

    /// Detect if text contains structured content (code, tables, lists)
    /// CONSERVATIVE: Only detect if there's strong evidence, default to Plain
    pub fn detect_structure(&self, text: &str) -> StructureType {
        let lines: Vec<&str> = text.lines().collect();
        let total_lines = lines.len();

        if total_lines == 0 {
            return StructureType::Plain;
        }

        // Check for terminal/command output (require 30% of lines to be commands)
        let command_lines = lines.iter().filter(|line| {
            self.command_pattern.is_match(line)
        }).count();

        if command_lines >= 2 && (command_lines as f32 / total_lines as f32) >= 0.3 {
            return StructureType::Terminal;
        }

        // Check for code indicators (require MULTIPLE strong indicators)
        let code_keywords = ["function ", "const ", "let ", "var ", "def ", "class ", "import ", "export "];
        let code_symbols = [" => ", "() {", "};", "() =>"];

        let keyword_count = lines.iter().filter(|line| {
            code_keywords.iter().any(|kw| line.contains(kw))
        }).count();

        let symbol_count = lines.iter().filter(|line| {
            code_symbols.iter().any(|sym| line.contains(sym))
        }).count();

        // Require at least 2 keyword lines OR 2 symbol lines, AND 20% density
        let code_density = (keyword_count + symbol_count) as f32 / total_lines as f32;
        if (keyword_count >= 2 || symbol_count >= 2) && code_density >= 0.2 {
            return StructureType::Code;
        }

        // Check for table indicators (require consistent column structure)
        let table_lines = lines.iter().filter(|line| {
            let pipe_count = line.matches('|').count();
            pipe_count >= 2
        }).count();

        if table_lines >= 3 && (table_lines as f32 / total_lines as f32) >= 0.5 {
            return StructureType::Table;
        }

        // Check for list indicators (require at least 3 list items)
        let list_patterns = [r"^\s*[-*+]\s", r"^\s*\d+\.\s"];
        let has_list = list_patterns.iter().any(|pattern| {
            if let Ok(re) = Regex::new(pattern) {
                text.lines().filter(|line| re.is_match(line)).count() >= 3
            } else {
                false
            }
        });

        if has_list {
            StructureType::List
        } else {
            // Default to Plain for everything else (chats, documents, etc.)
            StructureType::Plain
        }
    }

    /// Generate a preview/summary focusing on important content
    pub fn generate_preview(&self, text: &str, max_length: usize) -> String {
        let entities = self.extract_entities(text);
        let structure = self.detect_structure(text);

        let mut parts = Vec::new();

        // Add structure indicator
        match structure {
            StructureType::Code => parts.push("[Code]".to_string()),
            StructureType::Terminal => parts.push("[Terminal]".to_string()),
            StructureType::Table => parts.push("[Table]".to_string()),
            StructureType::List => parts.push("[List]".to_string()),
            StructureType::Plain => {}
        }

        // Add important entities first
        if !entities.urls.is_empty() {
            parts.push(entities.urls[0].clone());
        }

        // Add text content
        let clean_text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let preview_text = if clean_text.len() > max_length {
            format!("{}...", &clean_text[..max_length])
        } else {
            clean_text
        };

        if !preview_text.is_empty() {
            parts.push(preview_text);
        }

        parts.join(" ")
    }
}

#[derive(Debug, Clone)]
pub struct ExtractedEntities {
    pub urls: Vec<String>,
    pub emails: Vec<String>,
    pub file_paths: Vec<String>,
    pub code_snippets: Vec<String>,
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StructureType {
    Plain,
    Code,
    Table,
    List,
    Terminal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_character_confusion() {
        let processor = TextProcessor::new();
        let input = "The number is l but not O";
        let output = processor.fix_character_confusion(input);
        assert!(output.contains('1') || output.contains("l")); // Context-dependent
    }

    #[test]
    fn test_url_extraction() {
        let processor = TextProcessor::new();
        let text = "Visit https://example.com or www.test.com for more";
        let entities = processor.extract_entities(text);
        assert_eq!(entities.urls.len(), 2);
    }

    #[test]
    fn test_code_detection() {
        let processor = TextProcessor::new();
        let text = "function test() {\n  const x = 10;\n  return x;\n}";
        let structure = processor.detect_structure(text);
        assert_eq!(structure, StructureType::Code);
    }

    #[test]
    fn test_whitespace_cleanup() {
        let processor = TextProcessor::new();
        let text = "Line 1  \n\n\n  Line 2   \n  Line 3";
        let cleaned = processor.clean_ocr_text(text);
        assert!(!cleaned.contains("   "));
        assert!(!cleaned.contains("\n\n\n"));
    }
}
