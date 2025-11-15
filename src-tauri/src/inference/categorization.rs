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
    #[serde(default, deserialize_with = "deserialize_confidence")]
    pub confidence: f32,
}

/// Custom deserializer that accepts both string and number for confidence
fn deserialize_confidence<'de, D>(deserializer: D) -> Result<f32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct ConfidenceVisitor;

    impl<'de> Visitor<'de> for ConfidenceVisitor {
        type Value = f32;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a number or string representing a confidence value")
        }

        fn visit_f64<E>(self, value: f64) -> Result<f32, E>
        where
            E: de::Error,
        {
            Ok(value as f32)
        }

        fn visit_i64<E>(self, value: i64) -> Result<f32, E>
        where
            E: de::Error,
        {
            Ok(value as f32)
        }

        fn visit_u64<E>(self, value: u64) -> Result<f32, E>
        where
            E: de::Error,
        {
            Ok(value as f32)
        }

        fn visit_str<E>(self, value: &str) -> Result<f32, E>
        where
            E: de::Error,
        {
            value.parse::<f32>().map_err(de::Error::custom)
        }
    }

    deserializer.deserialize_any(ConfidenceVisitor)
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
    content_type: Option<&str>,
    website_url: Option<&str>,
    website_title: Option<&str>,
) -> Result<CategoryDecision> {
    // Build prompt with content-type awareness and source information
    let prompt = build_categorization_prompt(
        snippet_content,
        existing_categories,
        content_type,
        website_url,
        website_title,
    );

    // Log prompt details for debugging
    log::info!("📝 LLM Categorization Prompt Details:");
    log::info!("   Content Type: {:?}", content_type);
    log::info!("   Website URL: {:?}", website_url);
    log::info!("   Website Title: {:?}", website_title);
    log::info!("   Prompt Length: {} characters", prompt.len());

    // Log prompt structure (first 500 chars and last 200 chars)
    if prompt.len() > 700 {
        log::info!(
            "   Prompt Preview (first 500 chars):\n{}",
            &prompt[..500.min(prompt.len())]
        );
        log::info!(
            "   Prompt Preview (last 200 chars):\n{}",
            &prompt[prompt.len().saturating_sub(200)..]
        );
    } else {
        log::info!("   Full Prompt:\n{}", prompt);
    }

    // Check if critical warnings are in the prompt (for text content)
    if content_type == Some("text") {
        let has_critical_warning =
            prompt.contains("CRITICAL WARNING") || prompt.contains("CRITICAL RULE");
        let has_doc_warning = prompt.contains("Documentation") && prompt.contains("NOT for");
        log::info!(
            "   Prompt includes CRITICAL WARNING: {}",
            has_critical_warning
        );
        log::info!(
            "   Prompt includes Documentation exclusion rules: {}",
            has_doc_warning
        );
    }

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
    let mut decision = parse_llm_response(&response)?;

    // Validate and potentially adjust decision
    decision = validate_category_decision(
        decision,
        existing_categories,
        snippet_content,
        content_type,
        website_url,
    )?;

    Ok(decision)
}

/// Build categorization prompt for LLM with comprehensive guidance
fn build_categorization_prompt(
    snippet_content: &str,
    categories: &[Category],
    content_type: Option<&str>,
    website_url: Option<&str>,
    website_title: Option<&str>,
) -> String {
    // Route to appropriate prompt builder based on content type
    match content_type {
        Some("command") => {
            build_command_prompt(snippet_content, categories, website_url, website_title)
        }
        Some("text") => build_text_prompt(snippet_content, categories, website_url, website_title),
        Some("screenshot") => {
            build_screenshot_prompt(snippet_content, categories, website_url, website_title)
        }
        _ => build_general_prompt(snippet_content, categories, website_url, website_title),
    }
}

/// Build prompt specifically for terminal command content
fn build_command_prompt(
    snippet_content: &str,
    categories: &[Category],
    _website_url: Option<&str>,
    _website_title: Option<&str>,
) -> String {
    let categories_list = if categories.is_empty() {
        "No existing categories yet - you will need to create the first one.".to_string()
    } else {
        format_categories_enhanced(categories)
    };

    let snippet_preview = truncate_intelligently(snippet_content, Some("command"), 400);

    format!(
        r#"<|im_start|>system
You are an expert categorization assistant for terminal commands. Your goal is to maintain a clean, well-organized taxonomy by ALWAYS reusing existing categories.

🚨 CRITICAL RULES - READ FIRST:
1. ALWAYS check EXISTING CATEGORIES list FIRST before creating anything new
2. NEVER create variations like "docker build", "dockercmd", "fileList_with_docker" if "Docker Commands" exists
3. NEVER create category names with underscores, special characters, or file paths
4. Use ONLY the EXACT category names from the existing list
5. If a canonical category exists (e.g., "Docker Commands"), use it - DO NOT create new ones

CRITICAL: THINK LIKE A SENIOR DEVELOPER
- A senior developer ALWAYS checks existing categories first
- They NEVER create duplicate categories for the same tool
- They recognize tool families: "docker", "docker-compose", "docker compose" = ALL "Docker Commands"
- They use BROAD, canonical categories, NOT specific command names
- Generic categories like "Software Development" are LAST RESORT only

CORE PRINCIPLES:
1. CHECK EXISTING FIRST: ALWAYS look at EXISTING CATEGORIES list - if a suitable category exists, use it
2. REUSE OVER CREATE: Only create if NO existing category matches (confidence < 0.50)
3. BROAD OVER SPECIFIC: Use broad tool categories, NOT specific command names
4. CANONICAL NAMES ONLY: Use standard category names, never create weird names

CANONICAL CATEGORY NAMES (use these if they exist, or create if first time):
- docker*, docker-compose*, docker compose* → "Docker Commands" 🐳
- node*, npm*, npx*, yarn*, pnpm* → "Node.js Development" 📦
- git* → "Git Commands" 🔀
- kubectl*, helm* → "Kubernetes" ☸️
- cargo*, rustc* → "Rust Development" 🦀
- python*, pip* → "Python Development" 🐍
- aws*, gcloud*, az* → "Cloud CLI" ☁️
- ssh*, scp*, rsync* → "System Administration" 🔧
- make*, cmake*, gradle* → "Build Tools" 🔨

FORBIDDEN: NEVER create categories like:
- "docker build" (use "Docker Commands")
- "dockercmd" (use "Docker Commands")
- "fileList_with_docker" (use "Docker Commands")
- Any name with underscores, special chars, or file paths
- Any variation of an existing category name

{}

EXAMPLES:
Example 1: Command "docker build -t myapp:latest ." with existing "Docker Commands" → {{"action":"use_existing","category_name":"Docker Commands","emoji":"🐳","reasoning":"Docker command - use existing Docker Commands category","confidence":0.95}}
Example 2: Command "docker-compose up -d" with existing "Docker Commands" → {{"action":"use_existing","category_name":"Docker Commands","emoji":"🐳","reasoning":"docker-compose is Docker - use existing Docker Commands","confidence":0.95}}
Example 3: Command "docker compose up -d --build" with existing "Docker Commands" → {{"action":"use_existing","category_name":"Docker Commands","emoji":"🐳","reasoning":"docker compose is Docker - use existing Docker Commands","confidence":0.95}}
Example 4: Command "npm run test" with existing "Node.js Development" → {{"action":"use_existing","category_name":"Node.js Development","emoji":"📦","reasoning":"npm is Node.js - use existing Node.js Development","confidence":0.95}}

REQUIRED JSON FORMAT (you MUST include ALL these fields):
{{
  "action": "use_existing" or "create_new",
  "category_name": "Exact category name (TEXT ONLY - NO EMOJIS)",
  "emoji": "🐳",
  "reasoning": "Brief explanation of your decision",
  "confidence": 0.95
}}

🚨 CRITICAL: category_name MUST BE TEXT ONLY - NO EMOJIS ALLOWED
- WRONG: {{"category_name": "🐳 Docker Commands", "emoji": "🐳"}}
- RIGHT: {{"category_name": "Docker Commands", "emoji": "🐳"}}
- Emojis belong ONLY in the "emoji" field, NEVER in "category_name"

STEP-BY-STEP CHECKING PROCESS:
1. Normalize the command: "docker-compose" = "docker compose" = "Docker Compose" (all same)
2. Check existing categories for case-insensitive match (e.g., "Docker Commands" matches "docker commands")
3. Check for space/underscore variations: "Docker_compose" = "Docker Compose" = "docker compose"
4. Only create_new if NO existing category matches after normalization
5. When using use_existing, use the EXACT category name from the list (case-sensitive)

CRITICAL: You MUST return JSON with ALL 5 fields: action, category_name, emoji, reasoning, confidence.

Respond ONLY with valid JSON (no markdown, no explanation).<|im_end|>
<|im_start|>user
EXISTING CATEGORIES ({} total):
{}

CRITICAL RULE: When using action "use_existing", you MUST use the EXACT category name from the EXISTING CATEGORIES list above.

COMMAND TO CATEGORIZE:
{}

Now categorize the command above. Respond with ONLY the JSON object:<|im_end|>
<|im_start|>assistant
{{"#,
        COMMAND_GUIDANCE,
        categories.len(),
        categories_list,
        snippet_preview
    )
}

/// Build prompt specifically for text/snippet content (non-command)
fn build_text_prompt(
    snippet_content: &str,
    categories: &[Category],
    website_url: Option<&str>,
    website_title: Option<&str>,
) -> String {
    let categories_list = if categories.is_empty() {
        "No existing categories yet - you will need to create the first one.".to_string()
    } else {
        format_categories_enhanced(categories)
    };

    let snippet_preview = truncate_intelligently(snippet_content, Some("text"), 400);

    // Build source information section if available
    let source_info_section = if website_url.is_some() || website_title.is_some() {
        let mut source_parts = Vec::new();
        if let Some(url) = website_url {
            source_parts.push(format!("- Website: {}", url));
        }
        if let Some(title) = website_title {
            source_parts.push(format!("- Page Title: {}", title));
        }
        format!(
            "\n\nSOURCE INFORMATION:\n{}\n\nUse this source information to understand the context. Business/product websites (e.g., joist.ai, company.com, product pages) indicate business/marketing content, NOT technical documentation. Use the website domain and page title to infer the content type and purpose.",
            source_parts.join("\n")
        )
    } else {
        String::new()
    };

    format!(
        r#"<|im_start|>system
You are an expert categorization assistant for text content. Your goal is to categorize content by its TOPIC and MEANING, not by technical tools.

🚨 CRITICAL: "Documentation" is ONLY for technical docs (API refs, guides, manuals). 
If content has: company names, "case study", "partnership", "harnesses", "transformed", "discover", business websites → NOT Documentation. Create specific business category instead.

CORE PRINCIPLES:
1. REUSE OVER CREATE: Only create a new category if NO existing category is remotely suitable (confidence < 0.60)
2. TOPIC-BASED CATEGORIZATION: Focus on what the content is ABOUT, not what tools it mentions
3. SEMANTIC MATCHING: Group content by meaning and purpose
4. SPECIFIC OVER BROAD: Create specific, descriptive categories rather than overly broad ones

CATEGORIZATION GUIDELINES:
- Business content → Create specific categories like "AEC Proposal Management", "Construction Marketing", "Engineering Case Studies" rather than generic "Business & Technology"
- Technical documentation → "Documentation", "Technical Guides", or specific topic (ONLY for actual technical guides, API docs, tutorials - NOT for business case studies or marketing content)
- Product descriptions → Specific product categories like "Joist AI", "AEC Software", or "Marketing Tools" rather than generic "Marketing"
- News/articles → "News & Articles", "Research", or specific topic
- Personal notes → "Notes & Ideas", "Personal"
- Code snippets → "Code Snippets" (only if actual code syntax present)
- URLs/links → "Links & References"

CRITICAL RULE - "Documentation" Category:
"Documentation" is ONLY for:
- Technical guides and tutorials
- API documentation
- Software manuals
- How-to guides with technical instructions

"Documentation" is NOT for:
- Business case studies (e.g., "AEC Firm, McKim & Creed, Harnesses the Power of Joist AI")
- Marketing content or product descriptions
- Company success stories or testimonials
- Business proposals or sales content
- News articles or blog posts about products
- Content mentioning company names, product names, partnerships, or business outcomes

If content is from a business/product website (joist.ai, company.com, product pages), it's likely business/marketing content, NOT documentation.

IMPORTANT RULES:
1. Do NOT categorize business/marketing text as technical categories like "Docker Commands" just because it mentions technology
2. Do NOT categorize business case studies, marketing content, or product descriptions as "Documentation" - THIS IS CRITICAL
3. Do NOT create overly broad categories like "Business & Technology" - be SPECIFIC
4. Create descriptive categories that capture the actual topic: "AEC Proposal Management" is better than "Business & Technology"
5. If content is about a specific industry/tool combination, create a specific category: "AEC AI Tools" > "Business & Technology"
6. Focus on the TOPIC and PURPOSE of the content, and make the category name reflect that specifically
7. Use source information (website URL and title) to understand context - business websites = business content
8. If you see patterns like "case study", "partnership", company names, product names → Create a specific business category, NOT "Documentation"

🚨 CRITICAL: Category Name Requirements
- Category names MUST be descriptive phrases with at least 2 words (e.g., "AEC Business Content", "Business Case Studies")
- NEVER create category names that are acronyms, abbreviations, or codes (e.g., "PD2", "ABC", "XYZ123")
- Category names MUST contain meaningful words that describe the content topic
- Single-word names are ONLY allowed for well-known categories like "Documentation", "Commands", "Code"
- Examples:
  * WRONG: {{"category_name": "PD2", "emoji": "📄"}}
  * WRONG: {{"category_name": "ABC", "emoji": "📄"}}
  * WRONG: {{"category_name": "Tech", "emoji": "💻"}}
  * RIGHT: {{"category_name": "AEC Business Content", "emoji": "📋"}}
  * RIGHT: {{"category_name": "Business Case Studies", "emoji": "📊"}}
  * RIGHT: {{"category_name": "AEC Proposal Management", "emoji": "📋"}}

{}{}

EXAMPLES:
Example 1: "AEC Firm, McKim & Creed, Harnesses the Power of Joist AI" (from joist.ai) → {{"action":"create_new","category_name":"AEC Proposal Management","emoji":"📋","reasoning":"Case study with company names from business website - NOT Documentation","confidence":0.90}}
Example 1b (WRONG): Same → {{"action":"create_new","category_name":"Documentation","emoji":"📄","reasoning":"WRONG - business case study, not technical docs"}}
Example 2: "Win More Work with Less Effort - AEC marketing platform" → {{"action":"create_new","category_name":"AEC Marketing Tools","emoji":"📢","reasoning":"Marketing content - specific category","confidence":0.88}}
Example 3: "React API Reference - useState Hook" (from react.dev/docs) → {{"action":"use_existing","category_name":"Documentation","emoji":"📄","reasoning":"Technical API docs - correct use of Documentation","confidence":0.95}}

KEY PRINCIPLE: Create SPECIFIC, DESCRIPTIVE categories that capture the actual topic. Avoid generic combinations like "X & Y" unless truly necessary. Use source information to understand context.

REQUIRED JSON FORMAT (you MUST include ALL these fields):
{{
  "action": "use_existing" or "create_new",
  "category_name": "Exact category name (without emoji)",
  "emoji": "📋",
  "reasoning": "Brief explanation of your decision",
  "confidence": 0.90
}}

CRITICAL: You MUST return JSON with ALL 5 fields: action, category_name, emoji, reasoning, confidence.
Do NOT return simplified formats like {{"category": "Name"}} - use the full format above.

Respond ONLY with valid JSON (no markdown, no explanation).<|im_end|>
<|im_start|>user
EXISTING CATEGORIES ({} total):
{}

CRITICAL RULE: When using action "use_existing", you MUST use the EXACT category name from the EXISTING CATEGORIES list above.
{}

TEXT TO CATEGORIZE:
{}

Now categorize the text above. Respond with ONLY the JSON object:<|im_end|>
<|im_start|>assistant
{{"#,
        TEXT_GUIDANCE,
        source_info_section,
        categories.len(),
        categories_list,
        if source_info_section.is_empty() {
            String::new()
        } else {
            format!("\n\nUse the source information above to understand the context and categorize appropriately.")
        },
        snippet_preview
    )
}

/// Build prompt specifically for screenshot content
fn build_screenshot_prompt(
    snippet_content: &str,
    categories: &[Category],
    _website_url: Option<&str>,
    _website_title: Option<&str>,
) -> String {
    let categories_list = if categories.is_empty() {
        "No existing categories yet - you will need to create the first one.".to_string()
    } else {
        format_categories_enhanced(categories)
    };

    // Parse caption and OCR text from the combined content
    // Format is: [Caption: ...] [Text: ...]
    let (caption_text, ocr_text) = parse_screenshot_content(snippet_content);

    // Build the content section with clear labels
    // Truncate caption and OCR if too long to avoid token limits
    let truncate_to = 300; // characters
    let content_section = if let Some(caption) = &caption_text {
        let caption_truncated: String = caption.chars().take(truncate_to).collect();
        let caption_display = if caption.len() > truncate_to {
            format!("{}...", caption_truncated)
        } else {
            caption_truncated
        };

        if let Some(ocr) = &ocr_text {
            let ocr_truncated: String = ocr.chars().take(truncate_to).collect();
            let ocr_display = if ocr.len() > truncate_to {
                format!("{}...", ocr_truncated)
            } else {
                ocr_truncated
            };

            format!(
                "IMAGE CAPTION (from vision model):\n{}\n\nOCR TEXT (extracted text from image):\n{}\n\nUse BOTH the caption (which describes what the image shows) and the OCR text (which contains actual text content) to determine the category. The caption gives you the visual context, and the OCR text gives you the actual content.",
                caption_display,
                ocr_display
            )
        } else {
            format!(
                "IMAGE CAPTION (from vision model):\n{}\n\nUse the caption to determine the category based on what the image shows.",
                caption_display
            )
        }
    } else if let Some(ocr) = &ocr_text {
        let ocr_truncated: String = ocr.chars().take(truncate_to).collect();
        let ocr_display = if ocr.len() > truncate_to {
            format!("{}...", ocr_truncated)
        } else {
            ocr_truncated
        };

        format!(
            "OCR TEXT (extracted text from image):\n{}\n\nUse the OCR text to determine the category based on the content.",
            ocr_display
        )
    } else {
        let truncated: String = snippet_content.chars().take(truncate_to).collect();
        format!("SCREENSHOT CONTENT:\n{}", truncated)
    };

    format!(
        r#"<|im_start|>system
You are an expert categorization assistant for screenshot content. You will receive BOTH a caption (from a vision model describing the image) and OCR text (extracted text from the image). Use BOTH to determine the proper category.

CORE PRINCIPLES:
1. REUSE OVER CREATE: Only create a new category if NO existing category is remotely suitable (confidence < 0.60)
2. USE BOTH SOURCES: The caption describes what the image shows visually, the OCR text contains actual readable content
3. CONTENT-FOCUSED: Categorize by what the screenshot SHOWS and CONTAINS, not UI elements
4. TOPIC-BASED: Focus on the main topic/subject visible in the screenshot

HOW TO USE THE INFORMATION:
- CAPTION: Tells you what the image shows (e.g., "code editor", "meeting interface", "web page")
- OCR TEXT: Contains actual text content from the image (e.g., code, URLs, commands, article text)
- COMBINE BOTH: Use caption for visual context, OCR for specific content details

{}

EXAMPLES:
Example 1: Caption: "code editor with function", OCR: "function calculate() {{ return x + y; }}" → {{"action":"use_existing","category_name":"Code Snippets","emoji":"💻","reasoning":"Caption shows code editor, OCR contains code syntax - code category","confidence":0.90}}
Example 2: Caption: "meeting interface", OCR: "Google Meet - abc-xyz-123" → {{"action":"create_new","category_name":"Meetings & Collaboration","emoji":"📹","reasoning":"Caption shows meeting, OCR confirms meeting ID - collaboration topic","confidence":0.85}}
Example 3: Caption: "web article", OCR: "Breaking News: Tech Update..." → {{"action":"create_new","category_name":"News & Articles","emoji":"📰","reasoning":"Caption shows article, OCR contains news content - news category","confidence":0.88}}

REQUIRED JSON FORMAT (you MUST include ALL these fields):
{{
  "action": "use_existing" or "create_new",
  "category_name": "Exact category name (without emoji)",
  "emoji": "📰",
  "reasoning": "Brief explanation of your decision",
  "confidence": 0.88
}}

CRITICAL: You MUST return JSON with ALL 5 fields: action, category_name, emoji, reasoning, confidence.

Respond ONLY with valid JSON (no markdown, no explanation).<|im_end|>
<|im_start|>user
EXISTING CATEGORIES ({} total):
{}

CRITICAL RULE: When using action "use_existing", you MUST use the EXACT category name from the EXISTING CATEGORIES list above.

{}

Now categorize the screenshot above. Respond with ONLY the JSON object:<|im_end|>
<|im_start|>assistant
{{"#,
        SCREENSHOT_GUIDANCE,
        categories.len(),
        categories_list,
        content_section
    )
}

/// Parse screenshot content to extract caption and OCR text
/// Format: [Caption: ...] [Text: ...]
fn parse_screenshot_content(content: &str) -> (Option<String>, Option<String>) {
    let mut caption = None;
    let mut ocr = None;

    // Try to extract [Caption: ...]
    if let Some(caption_start) = content.find("[Caption:") {
        // Find the matching closing bracket, handling potential nested brackets
        let mut bracket_count = 0;
        let mut caption_end = None;
        for (i, ch) in content[caption_start..].char_indices() {
            if ch == '[' {
                bracket_count += 1;
            } else if ch == ']' {
                bracket_count -= 1;
                if bracket_count == 0 {
                    caption_end = Some(caption_start + i);
                    break;
                }
            }
        }

        if let Some(end) = caption_end {
            let caption_content = &content[caption_start + 9..end];
            if !caption_content.trim().is_empty() {
                caption = Some(caption_content.trim().to_string());
            }
        }
    }

    // Try to extract [Text: ...]
    if let Some(text_start) = content.find("[Text:") {
        // Find the matching closing bracket, handling potential nested brackets
        let mut bracket_count = 0;
        let mut text_end = None;
        for (i, ch) in content[text_start..].char_indices() {
            if ch == '[' {
                bracket_count += 1;
            } else if ch == ']' {
                bracket_count -= 1;
                if bracket_count == 0 {
                    text_end = Some(text_start + i);
                    break;
                }
            }
        }

        if let Some(end) = text_end {
            let text_content = &content[text_start + 6..end];
            if !text_content.trim().is_empty() {
                ocr = Some(text_content.trim().to_string());
            }
        }
    }

    (caption, ocr)
}

/// Build general prompt for unknown content types
fn build_general_prompt(
    snippet_content: &str,
    categories: &[Category],
    website_url: Option<&str>,
    website_title: Option<&str>,
) -> String {
    let categories_list = if categories.is_empty() {
        "No existing categories yet - you will need to create the first one.".to_string()
    } else {
        format_categories_enhanced(categories)
    };

    let snippet_preview = truncate_intelligently(snippet_content, None, 400);

    // Build source information section if available
    let source_info_section = if website_url.is_some() || website_title.is_some() {
        let mut source_parts = Vec::new();
        if let Some(url) = website_url {
            source_parts.push(format!("- Website: {}", url));
        }
        if let Some(title) = website_title {
            source_parts.push(format!("- Page Title: {}", title));
        }
        format!(
            "\n\nSOURCE INFORMATION:\n{}\n\nUse this source information to understand the context of the content.",
            source_parts.join("\n")
        )
    } else {
        String::new()
    };

    format!(
        r#"<|im_start|>system
You are an expert categorization assistant. Your goal is to maintain a clean, well-organized taxonomy by STRONGLY PREFERRING to reuse existing categories whenever possible.

CORE PRINCIPLES:
1. REUSE OVER CREATE: Only create a new category if NO existing category is remotely suitable (confidence < 0.60)
2. TOPIC-BASED: Categorize by content topic and purpose
3. BROAD OVER SPECIFIC: Prefer broader categories that can contain related items

{}{}

REQUIRED JSON FORMAT (you MUST include ALL these fields):
{{
  "action": "use_existing" or "create_new",
  "category_name": "Exact category name (without emoji)",
  "emoji": "📁",
  "reasoning": "Brief explanation of your decision",
  "confidence": 0.85
}}

CRITICAL: You MUST return JSON with ALL 5 fields: action, category_name, emoji, reasoning, confidence.

Respond ONLY with valid JSON (no markdown, no explanation).<|im_end|>
<|im_start|>user
EXISTING CATEGORIES ({} total):
{}

CRITICAL RULE: When using action "use_existing", you MUST use the EXACT category name from the EXISTING CATEGORIES list above.
{}

CONTENT TO CATEGORIZE:
{}

Now categorize the content above. Respond with ONLY the JSON object:<|im_end|>
<|im_start|>assistant
{{"#,
        GENERAL_GUIDANCE,
        source_info_section,
        categories.len(),
        categories_list,
        if source_info_section.is_empty() {
            String::new()
        } else {
            format!("\n\nUse the source information above to understand the context and categorize appropriately.")
        },
        snippet_preview
    )
}

// Content-specific guidance constants
const COMMAND_GUIDANCE: &str = r#"
TERMINAL COMMAND SPECIFIC GUIDANCE:
- STEP 1: Check EXISTING CATEGORIES list - if a matching category exists, use it
- STEP 2: Identify PRIMARY TOOL from first word (before space/dash)
- STEP 3: Map to canonical category (see list above)
- CRITICAL: "docker compose", "docker-compose", "docker" = ALL "Docker Commands"
- CRITICAL: "node", "npm", "npx", "yarn" = ALL "Node.js Development"
- NEVER create: "docker build", "dockercmd", "fileList_with_docker" - use "Docker Commands"
- NEVER create category names with underscores, special chars, or file paths
- Ignore subcommands, flags, arguments - focus ONLY on the tool name
- Multi-command chains: use the PRIMARY tool's category
- Tool prefixes: docker* → "Docker Commands", node*/npm* → "Node.js Development", git* → "Git Commands"
- If existing category matches tool family → use_existing with confidence 0.90+
- Only create_new if NO existing category matches the tool family"#;

const SCREENSHOT_GUIDANCE: &str = r#"
SCREENSHOT SPECIFIC GUIDANCE:
- Analyze OCR text for content type and visible context
- Meeting screenshots:
  * Google Meet, Zoom, Teams, Slack calls → "Meetings & Collaboration"
  * Look for: meeting IDs, participant names, video tiles
- Browser/Web screenshots:
  * Google Search queries → categorize by search topic
    Example: "Google Search uttarakhand" → "Travel & Geography" or "Research"
  * Shopping sites (Amazon, eBay) → "Shopping & E-commerce"
  * Social media (Twitter, Facebook) → "Social Media"
  * News sites → "News & Articles"
- Code/Development:
  * Code syntax (curly braces, functions, imports) → "Code Snippets"
  * Terminal output → "Terminal Output" or specific tool category
  * Error messages with stack traces → "Error Messages" or "Debugging"
- Documentation:
  * API docs, tutorials, guides → "Documentation"
  * Wikipedia, educational content → "Research" or specific topic
- Design/Media:
  * Figma, design tools → "Design & UI"
  * Image galleries → "Images & Media"
- Extract TOPIC from visible content (URLs, titles, main text)
- IGNORE UI noise (browser chrome, buttons, meeting IDs)
- Focus on MAIN CONTENT and PURPOSE of screenshot"#;

const TEXT_GUIDANCE: &str = r#"
TEXT CONTENT SPECIFIC GUIDANCE:
- Focus on TOPIC and PURPOSE, not technical tools mentioned
- Business/marketing content → Business, Marketing, Sales categories
- Product descriptions → Products & Services, Marketing
- News/articles → News & Articles, Research
- Personal notes → Notes & Ideas, Personal
- Code snippets → Only if actual code syntax present (curly braces, functions, etc.)
- URLs/links → Links & References
- Configuration files → Configuration Files
- Error messages → Error Messages, Debugging
- CRITICAL: Do NOT categorize business text as "Docker Commands" just because it mentions Docker/AI/technology. Categorize by what the content is ABOUT."#;

const GENERAL_GUIDANCE: &str = r#"
GENERAL GUIDANCE:
- When in doubt, prefer broader existing categories
- Consider content's PURPOSE not just its form
- Err on side of reusing categories"#;

/// Truncate content intelligently based on content type
fn truncate_intelligently(content: &str, content_type: Option<&str>, max_len: usize) -> String {
    if content.len() <= max_len {
        return content.to_string();
    }

    match content_type {
        Some("command") => {
            // For commands, keep beginning (most important part)
            let lines: Vec<&str> = content.lines().collect();
            if lines.len() > 1 {
                // Multi-line: keep first 3 lines
                lines.iter().take(3).cloned().collect::<Vec<_>>().join("\n")
            } else {
                // Single line: truncate at max_len using char-based truncation
                let truncated: String = content.chars().take(max_len).collect();
                format!("{}...", truncated)
            }
        }
        Some("screenshot") => {
            // For screenshots, keep beginning (usually title/context)
            // Use char-based truncation to avoid splitting multi-byte characters
            let truncated: String = content.chars().take(max_len).collect();
            format!("{}...\n[OCR Text Truncated]", truncated)
        }
        _ => {
            // Default: simple truncation
            // Use char-based truncation to avoid splitting multi-byte characters
            let truncated: String = content.chars().take(max_len).collect();
            format!("{}...", truncated)
        }
    }
}

/// Remove emojis from category name (emojis should only be in the emoji field, not the name)
fn remove_emojis_from_name(name: &str) -> String {
    name.chars()
        .filter(|c| {
            // Filter out emoji characters based on Unicode ranges
            let code = *c as u32;
            !(
                (0x1F300..=0x1F5FF).contains(&code) || // Miscellaneous Symbols and Pictographs
                (0x1F600..=0x1F64F).contains(&code) || // Emoticons
                (0x1F680..=0x1F6FF).contains(&code) || // Transport and Map Symbols
                (0x1F900..=0x1F9FF).contains(&code) || // Supplemental Symbols and Pictographs
                (0x2600..=0x26FF).contains(&code)   || // Miscellaneous Symbols
                (0x2700..=0x27BF).contains(&code)   || // Dingbats
                (0xFE00..=0xFE0F).contains(&code)    || // Variation Selectors
                (0x1F1E0..=0x1F1FF).contains(&code)
                // Regional Indicator Symbols (flags)
            )
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Normalize category name for comparison (lowercase, normalize spaces/underscores)
fn normalize_category_name(name: &str) -> String {
    name.to_lowercase()
        .replace("_", " ")
        .replace("-", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

/// Generate a fallback category name from content when LLM generates invalid names
fn generate_fallback_category_name(content: &str, content_type: Option<&str>) -> String {
    // Extract first 200 chars for analysis
    let preview: String = content.chars().take(200).collect();
    let preview_lower = preview.to_lowercase();

    // Check for business/marketing content patterns
    let business_keywords = [
        ("case study", "Business Case Studies"),
        ("partnership", "Business Partnerships"),
        ("success story", "Business Success Stories"),
        ("testimonial", "Business Testimonials"),
        ("harnesses", "Business Content"),
        ("transformed", "Business Transformation"),
        ("company", "Business Content"),
        ("firm", "Business Content"),
        ("corporation", "Business Content"),
    ];

    for (keyword, category) in business_keywords.iter() {
        if preview_lower.contains(keyword) {
            // Check for industry-specific terms
            if preview_lower.contains("aec")
                || preview_lower.contains("architecture")
                || preview_lower.contains("engineering")
            {
                return "AEC Business Content".to_string();
            }
            if preview_lower.contains("construction") {
                return "Construction Business Content".to_string();
            }
            if preview_lower.contains("marketing") {
                return "Marketing Content".to_string();
            }
            return category.to_string();
        }
    }

    // Check for product/software mentions
    if preview_lower.contains("joist") || preview_lower.contains("ai") {
        if preview_lower.contains("aec") || preview_lower.contains("proposal") {
            return "AEC Proposal Management".to_string();
        }
        return "AI Business Tools".to_string();
    }

    // Check for technical content
    if content_type == Some("command") {
        return "Commands".to_string();
    }

    if preview_lower.contains("api")
        || preview_lower.contains("documentation")
        || preview_lower.contains("reference")
    {
        return "Documentation".to_string();
    }

    if preview_lower.contains("code")
        || preview_lower.contains("function")
        || preview_lower.contains("class")
    {
        return "Code Snippets".to_string();
    }

    // Check for news/article patterns
    if preview_lower.contains("news")
        || preview_lower.contains("article")
        || preview_lower.contains("breaking")
    {
        return "News & Articles".to_string();
    }

    // Extract key words from first sentence
    let first_sentence: String = preview
        .chars()
        .take_while(|c| *c != '.' && *c != '!' && *c != '?')
        .collect();
    let words: Vec<&str> = first_sentence
        .split_whitespace()
        .filter(|w| w.len() > 3 && w.chars().all(|c| c.is_alphabetic()))
        .take(3)
        .collect();

    if words.len() >= 2 {
        // Capitalize first letter of each word
        let category: String = words
            .iter()
            .map(|w| {
                let mut chars = w.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        return format!("{} Content", category);
    }

    // Default fallback based on content type
    match content_type {
        Some("command") => "Commands".to_string(),
        Some("screenshot") => "Screenshots".to_string(),
        Some("text") => "Text Content".to_string(),
        _ => "General Content".to_string(),
    }
}

/// Validate and potentially adjust categorization decision
fn validate_category_decision(
    mut decision: CategoryDecision,
    existing_categories: &[Category],
    snippet_content: &str,
    content_type: Option<&str>,
    website_url: Option<&str>,
) -> Result<CategoryDecision> {
    // CRITICAL: Remove emojis from category_name (emojis belong in emoji field only)
    let original_name = decision.category_name.clone();
    decision.category_name = remove_emojis_from_name(&decision.category_name);
    if original_name != decision.category_name {
        log::warn!(
            "⚠️  Removed emojis from category name: '{}' → '{}'",
            original_name,
            decision.category_name
        );
    }

    // CRITICAL: Check for duplicate categories (case-insensitive, normalize spaces/underscores)
    let normalized_new = normalize_category_name(&decision.category_name);
    for existing in existing_categories {
        let normalized_existing = normalize_category_name(&existing.name);

        // Exact match after normalization
        if normalized_new == normalized_existing {
            log::warn!(
                "⚠️  DUPLICATE DETECTED: '{}' matches existing '{}' (normalized: '{}') - converting to UseExisting",
                decision.category_name,
                existing.name,
                normalized_new
            );
            decision = CategoryDecision {
                action: CategoryAction::UseExisting,
                category_name: existing.name.clone(),
                emoji: Some(existing.emoji.clone()),
                reasoning: format!(
                    "Duplicate detected: '{}' matches existing '{}' - using existing category",
                    decision.category_name, existing.name
                ),
                confidence: 0.95,
            };
            break;
        }

        // Fuzzy match for very similar names (similarity > 0.85)
        let similarity = string_similarity(&normalized_new, &normalized_existing);
        if similarity > 0.85 && similarity < 1.0 {
            log::warn!(
                "⚠️  SIMILAR CATEGORY DETECTED: '{}' is {:.2}% similar to existing '{}' - converting to UseExisting",
                decision.category_name,
                similarity * 100.0,
                existing.name
            );
            decision = CategoryDecision {
                action: CategoryAction::UseExisting,
                category_name: existing.name.clone(),
                emoji: Some(existing.emoji.clone()),
                reasoning: format!(
                    "Similar category detected: '{}' is {:.0}% similar to existing '{}' - using existing category",
                    decision.category_name,
                    similarity * 100.0,
                    existing.name
                ),
                confidence: 0.90,
            };
            break;
        }
    }

    // CRITICAL: General validation for ALL content types - reject nonsensical category names
    let category_trimmed = decision.category_name.trim();
    let category_lower = category_trimmed.to_lowercase();
    let word_count = category_trimmed.split_whitespace().count();
    let has_letter = category_trimmed.chars().any(|c| c.is_alphabetic());
    let is_all_uppercase = category_trimmed.chars().all(|c| !c.is_lowercase());
    let is_alphanumeric_code = category_trimmed
        .chars()
        .all(|c| c.is_alphanumeric() || c == ' ' || c == '-')
        && category_trimmed
            .chars()
            .filter(|c| c.is_alphabetic())
            .count()
            <= 3
        && category_trimmed.chars().any(|c| c.is_numeric());

    // List of valid single-word category names (exceptions to the 2-word rule)
    let valid_single_words = [
        "documentation",
        "commands",
        "code",
        "notes",
        "links",
        "research",
        "news",
        "articles",
        "personal",
        "ideas",
        "shopping",
        "social",
    ];

    let is_valid_single_word =
        word_count == 1 && valid_single_words.contains(&category_lower.as_str());

    // Validation checks
    let is_too_short = category_trimmed.len() < 3;
    let is_acronym = is_all_uppercase && category_trimmed.len() < 5 && category_trimmed.len() >= 2;
    let is_too_few_words = word_count < 2 && !is_valid_single_word;
    let is_code_like = is_alphanumeric_code;
    let has_no_letters = !has_letter;

    if is_too_short || is_acronym || is_too_few_words || is_code_like || has_no_letters {
        let reason = if is_too_short {
            format!("too short ({} chars, minimum 3)", category_trimmed.len())
        } else if is_acronym {
            format!(
                "appears to be an acronym (all uppercase, {} chars)",
                category_trimmed.len()
            )
        } else if is_too_few_words {
            format!("too few words ({} word(s), minimum 2)", word_count)
        } else if is_code_like {
            "appears to be an alphanumeric code (e.g., PD2, ABC123)".to_string()
        } else {
            "contains no letters".to_string()
        };

        log::error!(
            "❌ REJECTED: Invalid category name '{}' - {}",
            decision.category_name,
            reason
        );

        // Generate fallback category name from content
        let fallback_name = generate_fallback_category_name(snippet_content, content_type);
        log::warn!(
            "   ✅ Generated fallback category name: '{}' (from content analysis)",
            fallback_name
        );

        decision = CategoryDecision {
            action: CategoryAction::CreateNew,
            category_name: fallback_name.clone(),
            emoji: decision.emoji.clone(),
            reasoning: format!(
                "REJECTED invalid category name '{}' ({}) - generated fallback: '{}'",
                decision.category_name, reason, fallback_name
            ),
            confidence: 0.75, // Lower confidence since it's a fallback
        };
    }

    // CRITICAL: For commands, reject invalid category names (underscores, file paths, specific commands)
    if content_type == Some("command") {
        let category_lower = decision.category_name.to_lowercase();

        // Check for forbidden patterns in command category names
        let is_valid_canonical = category_lower == "docker commands"
            || category_lower == "git commands"
            || category_lower == "node.js development"
            || category_lower.contains("kubernetes")
            || category_lower.contains("rust development")
            || category_lower.contains("python development")
            || category_lower.contains("cloud cli")
            || category_lower.contains("system administration")
            || category_lower.contains("build tools");

        let has_invalid_pattern = !is_valid_canonical
            && (category_lower.contains("_")
            || category_lower.contains("/")
            || category_lower.contains("\\")
            || category_lower.contains("filelist")
            || category_lower.contains("file_list")
            || (category_lower.starts_with("docker") && category_lower != "docker commands")
            || (category_lower.contains("docker") && category_lower.len() < 15) // Too short, likely a variation
            || category_lower == "docker build"
            || category_lower == "dockercmd");

        if has_invalid_pattern {
            log::error!(
                "❌ REJECTED: Invalid command category name '{}' - contains forbidden patterns",
                decision.category_name
            );

            // Try to find a matching canonical category
            let command_lower = snippet_content.to_lowercase();
            let canonical_category = if command_lower.starts_with("docker")
                || command_lower.contains("docker-compose")
                || command_lower.contains("docker compose")
            {
                // Look for "Docker Commands" in existing categories
                existing_categories
                    .iter()
                    .find(|c| c.name.to_lowercase() == "docker commands")
            } else if command_lower.starts_with("git") {
                existing_categories
                    .iter()
                    .find(|c| c.name.to_lowercase() == "git commands")
            } else if command_lower.starts_with("npm")
                || command_lower.starts_with("node")
                || command_lower.starts_with("npx")
            {
                existing_categories
                    .iter()
                    .find(|c| c.name.to_lowercase().contains("node"))
            } else {
                None
            };

            if let Some(canonical) = canonical_category {
                log::error!(
                    "   ✅ Converting to canonical category: '{}'",
                    canonical.name
                );
                decision = CategoryDecision {
                    action: CategoryAction::UseExisting,
                    category_name: canonical.name.clone(),
                    emoji: Some(canonical.emoji.clone()),
                    reasoning: format!(
                        "REJECTED invalid category name '{}' - converted to canonical '{}'",
                        decision.category_name, canonical.name
                    ),
                    confidence: 0.90,
                };
            } else {
                // Create a proper canonical category based on the command
                let (new_category, emoji) = if command_lower.starts_with("docker") {
                    ("Docker Commands", "🐳")
                } else if command_lower.starts_with("git") {
                    ("Git Commands", "🔀")
                } else if command_lower.starts_with("npm") || command_lower.starts_with("node") {
                    ("Node.js Development", "📦")
                } else {
                    ("Commands", "⚡")
                };

                log::error!(
                    "   ✅ Creating proper canonical category: '{}'",
                    new_category
                );
                decision = CategoryDecision {
                    action: CategoryAction::CreateNew,
                    category_name: new_category.to_string(),
                    emoji: Some(emoji.to_string()),
                    reasoning: format!(
                        "REJECTED invalid category name '{}' - creating proper canonical category '{}'",
                        decision.category_name,
                        new_category
                    ),
                    confidence: 0.90,
                };
            }
        }
    }

    // CRITICAL: Check if "Documentation" was chosen for business/marketing content
    let category_lower = decision.category_name.to_lowercase();
    if category_lower.contains("documentation") {
        // Check website URL for business indicators
        let is_business_website = if let Some(url) = website_url {
            let url_lower = url.to_lowercase();
            // Common business website patterns (exclude technical docs sites)
            (url_lower.contains(".com") && !url_lower.contains("docs."))
                || url_lower.contains("joist.ai")
                || url_lower.contains("company")
                || url_lower.contains("product")
                || (url_lower.contains(".ai") && !url_lower.contains("docs."))
                || url_lower.contains("business")
        } else {
            false
        };

        // Check for business/marketing keywords
        let content_lower = snippet_content.to_lowercase();
        let business_keywords = [
            "case study",
            "success story",
            "partnership",
            "harnesses",
            "transformed",
            "discover",
            "company",
            "firm",
            "corporation",
            "business",
            "marketing",
            "product",
            "testimonial",
            "outcome",
            "value proposition",
            "sales",
        ];

        let has_business_keywords = business_keywords
            .iter()
            .any(|keyword| content_lower.contains(keyword));

        // Check for company/product name patterns (capitalized words that might be names)
        let has_company_pattern = content_lower.split_whitespace().any(|word| {
            word.len() > 2
                && word
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false)
                && word
                    .chars()
                    .skip(1)
                    .all(|c| c.is_lowercase() || c == '&' || c == '.')
        });

        if has_business_keywords || has_company_pattern || is_business_website {
            log::error!(
                "❌ REJECTED: LLM incorrectly suggested 'Documentation' for business/marketing content. Content preview: '{}'",
                snippet_content.chars().take(100).collect::<String>()
            );
            log::error!(
                "   Detected business keywords: {}, Company pattern: {}, Business website: {}",
                has_business_keywords,
                has_company_pattern,
                is_business_website
            );
            log::error!(
                "   REJECTING 'Documentation' category - converting to appropriate business category"
            );

            // For text content, automatically convert to a business category
            if content_type == Some("text") {
                // Determine the best business category based on content
                let (new_category, emoji) = if content_lower.contains("case study")
                    || content_lower.contains("success story")
                    || content_lower.contains("partnership")
                {
                    ("Business Case Studies", "📊")
                } else if content_lower.contains("marketing") || content_lower.contains("product") {
                    ("Business Marketing", "📢")
                } else if content_lower.contains("partnership")
                    || content_lower.contains("harnesses")
                {
                    ("Business Partnerships", "🤝")
                } else if content_lower.contains("aec")
                    || content_lower.contains("construction")
                    || content_lower.contains("engineering")
                {
                    ("AEC Business Content", "🏗️")
                } else {
                    ("Business Content", "💼")
                };

                log::error!("   ✅ Converting to: '{}' {}", new_category, emoji);

                // Override the decision with the correct business category
                decision = CategoryDecision {
                    action: CategoryAction::CreateNew,
                    category_name: new_category.to_string(),
                    emoji: Some(emoji.to_string()),
                    reasoning: format!(
                        "REJECTED 'Documentation' - content contains business indicators (keywords: {}, company pattern: {}, business website: {}). Converted to appropriate business category.",
                        has_business_keywords,
                        has_company_pattern,
                        is_business_website
                    ),
                    confidence: 0.85, // High confidence in rejection
                };
            } else {
                // For non-text content, reject it entirely so it falls back to semantic similarity
                anyhow::bail!(
                    "LLM incorrectly categorized business/marketing content as 'Documentation'. Rejecting decision."
                );
            }
        }
    }

    // CRITICAL: Validate UseExisting suggestions - ensure category actually exists
    if decision.action == CategoryAction::UseExisting {
        let category_name_lower = decision.category_name.to_lowercase();
        let category_exists = existing_categories
            .iter()
            .any(|c| c.name.to_lowercase() == category_name_lower);

        if !category_exists {
            // Try fuzzy matching to see if LLM meant a similar category
            let mut best_match: Option<(&Category, f32)> = None;
            for cat in existing_categories.iter() {
                let similarity = string_similarity(&decision.category_name, &cat.name);
                if similarity > 0.80 {
                    if let Some((_, best_sim)) = best_match {
                        if similarity > best_sim {
                            best_match = Some((cat, similarity));
                        }
                    } else {
                        best_match = Some((cat, similarity));
                    }
                }
            }

            if let Some((matched_category, similarity)) = best_match {
                log::warn!(
                    "LLM suggested UseExisting for '{}' which doesn't exist, but found similar category '{}' (similarity: {:.2}) - using matched category",
                    decision.category_name,
                    matched_category.name,
                    similarity
                );
                decision = CategoryDecision {
                    action: CategoryAction::UseExisting,
                    category_name: matched_category.name.clone(),
                    emoji: Some(matched_category.emoji.clone()),
                    reasoning: format!(
                        "LLM suggested '{}' but matched to existing '{}' (similarity: {:.0}%)",
                        decision.category_name,
                        matched_category.name,
                        similarity * 100.0
                    ),
                    confidence: (decision.confidence * similarity).min(0.95),
                };
            } else {
                // Category doesn't exist and no similar match found - change to CreateNew
                log::warn!(
                    "LLM suggested UseExisting for category '{}' which doesn't exist in database ({} categories available). Changing to CreateNew action.",
                    decision.category_name,
                    existing_categories.len()
                );
                log::warn!(
                    "Available categories: {}",
                    existing_categories
                        .iter()
                        .map(|c| c.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                decision = CategoryDecision {
                    action: CategoryAction::CreateNew,
                    category_name: decision.category_name.clone(),
                    emoji: decision.emoji.clone(),
                    reasoning: format!(
                        "LLM suggested '{}' with UseExisting but category doesn't exist. Creating new category instead.",
                        decision.category_name
                    ),
                    confidence: decision.confidence * 0.9, // Slightly lower confidence since LLM was wrong
                };
            }
        }
    }

    // If creating new category, check for similar existing ones
    if decision.action == CategoryAction::CreateNew && !existing_categories.is_empty() {
        for cat in existing_categories {
            let similarity = string_similarity(&decision.category_name, &cat.name);

            // High similarity (> 0.85) - suggest using existing instead
            if similarity > 0.85 {
                log::warn!(
                    "New category '{}' is very similar to existing '{}' (similarity: {:.2}), suggesting reuse",
                    decision.category_name,
                    cat.name,
                    similarity
                );

                decision = CategoryDecision {
                    action: CategoryAction::UseExisting,
                    category_name: cat.name.clone(),
                    emoji: Some(cat.emoji.clone()),
                    reasoning: format!(
                        "Similar to proposed '{}' (similarity: {:.0}%)",
                        decision.category_name,
                        similarity * 100.0
                    ),
                    confidence: (decision.confidence * 0.9).min(0.95),
                };
                break;
            }
        }
    }

    // For command-type content, ALWAYS apply pattern matching (even with high confidence)
    // Pattern matching is rule-based and more reliable for well-known command tools
    // CRITICAL: For commands, pattern matching ALWAYS overrides LLM to ensure accuracy
    if content_type == Some("command") {
        if let Some(canonical_info) = get_canonical_category_info(snippet_content) {
            // Check if canonical category exists (with fuzzy matching)
            if let Some(cat) = existing_categories
                .iter()
                .find(|c| string_similarity(&c.name, canonical_info.name) > 0.80)
            {
                // Canonical category EXISTS - ALWAYS use it for commands (override LLM)
                let llm_differs = string_similarity(&decision.category_name, &cat.name) < 0.80;

                if llm_differs {
                    log::warn!(
                        "LLM chose '{}' but pattern matching detected '{}' for command: {} - ALWAYS overriding with pattern match (commands require canonical categories)",
                        decision.category_name,
                        cat.name,
                        snippet_content.lines().next().unwrap_or(snippet_content)
                    );
                } else {
                    log::info!(
                        "Pattern match confirmed LLM choice '{}' for command: {}",
                        cat.name,
                        snippet_content.lines().next().unwrap_or(snippet_content)
                    );
                }

                decision = CategoryDecision {
                    action: CategoryAction::UseExisting,
                    category_name: cat.name.clone(),
                    emoji: Some(cat.emoji.clone()),
                    reasoning: format!("Pattern-matched canonical category '{}' for command (overriding LLM suggestion '{}' with confidence {:.0}%)", cat.name, decision.category_name, decision.confidence * 100.0),
                    confidence: 0.95, // High confidence for pattern-matched canonical categories
                };
            } else {
                // Canonical category DOES NOT EXIST - ALWAYS suggest creating it for commands
                log::info!(
                    "Pattern match detected canonical category '{}' for command '{}' but it doesn't exist yet - ALWAYS creating canonical category (overriding LLM suggestion '{}')",
                    canonical_info.name,
                    snippet_content.lines().next().unwrap_or(snippet_content),
                    decision.category_name
                );

                decision = CategoryDecision {
                    action: CategoryAction::CreateNew,
                    category_name: canonical_info.name.to_string(),
                    emoji: Some(canonical_info.emoji.to_string()),
                    reasoning: format!(
                        "Pattern-matched canonical category '{}' for command tool (ALWAYS overriding LLM suggestion '{}' to ensure accuracy)",
                        canonical_info.name,
                        decision.category_name
                    ),
                    confidence: 0.95, // Very high confidence for canonical categories
                };
            }
        }
    } else if decision.confidence < 0.70 {
        // For non-command content, only apply pattern matching if confidence is low
        if let Some(canonical_info) = get_canonical_category_info(snippet_content) {
            // Check if canonical category exists (with fuzzy matching)
            if let Some(cat) = existing_categories
                .iter()
                .find(|c| string_similarity(&c.name, canonical_info.name) > 0.80)
            {
                log::info!(
                    "Low confidence ({:.2}), applying pattern match: {} → {}",
                    decision.confidence,
                    snippet_content.lines().next().unwrap_or(snippet_content),
                    cat.name
                );

                decision = CategoryDecision {
                    action: CategoryAction::UseExisting,
                    category_name: cat.name.clone(),
                    emoji: Some(cat.emoji.clone()),
                    reasoning: format!(
                        "Pattern-matched tool prefix (LLM confidence was {:.0}%)",
                        decision.confidence * 100.0
                    ),
                    confidence: 0.88, // Boost confidence with pattern match
                };
            } else {
                // Canonical category DOES NOT EXIST - suggest creating it
                log::info!(
                    "Pattern match detected canonical category '{}' for '{}' but it doesn't exist yet - suggesting creation",
                    canonical_info.name,
                    snippet_content.lines().next().unwrap_or(snippet_content)
                );

                decision = CategoryDecision {
                    action: CategoryAction::CreateNew,
                    category_name: canonical_info.name.to_string(),
                    emoji: Some(canonical_info.emoji.to_string()),
                    reasoning: format!(
                        "Pattern-matched canonical category for {} tool (overriding LLM suggestion '{}')",
                        canonical_info.name,
                        decision.category_name
                    ),
                    confidence: 0.92, // High confidence for canonical categories
                };
            }
        }
    }

    Ok(decision)
}

/// Canonical category information (name + emoji)
#[derive(Debug, Clone)]
pub struct CanonicalCategory {
    pub name: &'static str,
    pub emoji: &'static str,
}

/// Get canonical category info based on tool prefix patterns
/// This is publicly accessible so it can be used by job_queue.rs and other modules
pub fn get_canonical_category_info(command: &str) -> Option<CanonicalCategory> {
    let first_line = command.lines().next().unwrap_or(command);
    let first_word = first_line.split_whitespace().next()?;
    let tool = first_word.split('-').next()?.to_lowercase();

    match tool.as_str() {
        "docker" => Some(CanonicalCategory {
            name: "Docker Commands",
            emoji: "🐳",
        }),
        "node" | "npm" | "npx" | "yarn" | "pnpm" | "ts-node" | "nodemon" => {
            Some(CanonicalCategory {
                name: "Node.js Development",
                emoji: "📦",
            })
        }
        "git" => Some(CanonicalCategory {
            name: "Git Commands",
            emoji: "🔀",
        }),
        "kubectl" | "helm" | "k9s" => Some(CanonicalCategory {
            name: "Kubernetes",
            emoji: "☸️",
        }),
        "cargo" | "rustc" | "rustup" => Some(CanonicalCategory {
            name: "Rust Development",
            emoji: "🦀",
        }),
        "python" | "pip" | "poetry" => Some(CanonicalCategory {
            name: "Python Development",
            emoji: "🐍",
        }),
        "aws" | "gcloud" | "az" => Some(CanonicalCategory {
            name: "Cloud CLI",
            emoji: "☁️",
        }),
        "ssh" | "scp" | "rsync" => Some(CanonicalCategory {
            name: "System Administration",
            emoji: "🔧",
        }),
        "make" | "cmake" | "gradle" | "mvn" => Some(CanonicalCategory {
            name: "Build Tools",
            emoji: "🔨",
        }),
        _ => None,
    }
}

/// Detect command category name based on tool prefix patterns (for backward compatibility)
fn detect_command_category(command: &str) -> Option<&'static str> {
    get_canonical_category_info(command).map(|cat| cat.name)
}

/// Calculate string similarity (Jaro-Winkler-like simple version)
pub fn string_similarity(s1: &str, s2: &str) -> f32 {
    let s1_lower = s1.to_lowercase();
    let s2_lower = s2.to_lowercase();

    // Exact match
    if s1_lower == s2_lower {
        return 1.0;
    }

    // One contains the other
    if s1_lower.contains(&s2_lower) || s2_lower.contains(&s1_lower) {
        let shorter = s1_lower.len().min(s2_lower.len()) as f32;
        let longer = s1_lower.len().max(s2_lower.len()) as f32;
        return shorter / longer;
    }

    // Calculate character overlap
    let s1_chars: std::collections::HashSet<char> = s1_lower.chars().collect();
    let s2_chars: std::collections::HashSet<char> = s2_lower.chars().collect();

    let intersection = s1_chars.intersection(&s2_chars).count() as f32;
    let union = s1_chars.union(&s2_chars).count() as f32;

    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

/// Enhanced category formatting with numbering and structure
fn format_categories_enhanced(categories: &[Category]) -> String {
    let mut formatted = String::new();

    // Add header note to clarify format
    formatted.push_str(
        "IMPORTANT: Category names are TEXT ONLY (no emojis). Emojis are stored separately.\n",
    );
    formatted.push_str("Format: Category name (emoji: <emoji>)\n\n");

    // Group by parent/child hierarchy
    let mut root_categories: Vec<&Category> = categories
        .iter()
        .filter(|c| c.parent_id.is_none())
        .collect();
    root_categories.sort_by(|a, b| a.name.cmp(&b.name));

    for (idx, category) in root_categories.iter().enumerate() {
        formatted.push_str(&format!(
            "{}. {} (emoji: {})\n",
            idx + 1,
            category.name,
            category.emoji
        ));

        // Add children
        let mut children: Vec<&Category> = categories
            .iter()
            .filter(|c| c.parent_id == Some(category.id))
            .collect();
        children.sort_by(|a, b| a.name.cmp(&b.name));

        for child in children {
            formatted.push_str(&format!(
                "   - {} (emoji: {}, child of {})\n",
                child.name, child.emoji, category.name
            ));
        }
    }

    formatted
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
            match serde_json::from_str(&cleaned) {
                Ok(d) => d,
                Err(_) => {
                    // Try to recover from simple format like {"category": "Name"} or {"category": "🚀 Name"}
                    if let Some(category_value) = extract_category_from_simple_json(&json_str) {
                        log::warn!(
                            "LLM returned simple format, converting to full format: {}",
                            category_value
                        );
                        // Extract emoji if present (check if first char is emoji - typically > 0x1F000)
                        let (emoji, name) = {
                            let first_char = category_value.chars().next();
                            if let Some(ch) = first_char {
                                // Check if it's likely an emoji (Unicode emoji ranges)
                                let is_emoji = ch as u32 > 0x1F000
                                    || (ch as u32 >= 0x1F300 && ch as u32 <= 0x1F9FF)
                                    || (ch as u32 >= 0x2600 && ch as u32 <= 0x26FF)
                                    || (ch as u32 >= 0x2700 && ch as u32 <= 0x27BF);

                                if is_emoji {
                                    let name = category_value
                                        .chars()
                                        .skip(1)
                                        .collect::<String>()
                                        .trim()
                                        .to_string();
                                    (Some(ch.to_string()), name)
                                } else {
                                    (None, category_value)
                                }
                            } else {
                                (None, category_value)
                            }
                        };

                        // Determine action - assume create_new if we can't find in existing categories
                        // This will be validated later
                        CategoryDecision {
                            action: CategoryAction::CreateNew,
                            category_name: name,
                            emoji,
                            reasoning: "LLM returned simplified format, converted automatically"
                                .to_string(),
                            confidence: 0.75,
                        }
                    } else {
                        return Err(anyhow::anyhow!(
                            "Failed to parse LLM response. Original: {}",
                            json_str
                        ));
                    }
                }
            }
        }
    };

    log::debug!(
        "Parsed decision: action={:?}, category={}",
        decision.action,
        decision.category_name
    );

    // Validate and sanitize
    if decision.category_name.is_empty() {
        anyhow::bail!("Category name is empty");
    }

    // Default emoji if missing
    if decision.emoji.is_none()
        || decision
            .emoji
            .as_ref()
            .map(|s| s.is_empty())
            .unwrap_or(true)
    {
        decision.emoji = Some(default_emoji_for_category(&decision.category_name));
    }

    // Clamp confidence
    decision.confidence = decision.confidence.clamp(0.0, 1.0);

    Ok(decision)
}

/// Try to extract category from simple JSON format like {"category": "Name"} or {"category": "🚀 Name"}
fn extract_category_from_simple_json(json_str: &str) -> Option<String> {
    // Try to parse as simple object with "category" field
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) {
        if let Some(category) = value.get("category").and_then(|v| v.as_str()) {
            return Some(category.to_string());
        }
        // Also try "category_name"
        if let Some(category) = value.get("category_name").and_then(|v| v.as_str()) {
            return Some(category.to_string());
        }
    }
    None
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
