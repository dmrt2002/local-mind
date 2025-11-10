# LLM Categorization System - Implementation Guide

## Overview

Your local-mind application now uses a local Qwen2.5-1.5B model for intelligent snippet categorization. This document explains how it works and how to troubleshoot issues.

## How It Works

### Categorization Flow

```
User saves snippet
    ↓
1. Generate embedding (for semantic search)
    ↓
2. 🤖 TRY LLM CATEGORIZATION
   ├─ Load Qwen2.5-1.5B model (lazy, cached)
   ├─ Fetch all existing categories
   ├─ Build prompt with snippet + categories
   ├─ Run inference (2-5 seconds)
   ├─ Parse JSON response
   └─ Extract decision
    ↓
3. ✅ If LLM succeeds
   └─ Assign to existing OR create new category
    ↓
4. ❌ If LLM fails (timeout/error)
   └─ Fall back to semantic similarity (embedding-based)
    ↓
5. ⚠️  If semantic fails
   └─ Fall back to keyword-based rules
```

### LLM Prompt Format

The system uses Qwen's chat template:

```
<|im_start|>system
You are a categorization assistant. Respond ONLY with valid JSON.<|im_end|>
<|im_start|>user
Existing Categories:
- 💻 Code
  - 🐍 Python
  - ⚛️ React

Snippet:
import pandas as pd...

Respond with JSON:
{"action":"use_existing","category_name":"Python","emoji":"🐍","reasoning":"...","confidence":0.9}<|im_end|>
<|im_start|>assistant
{{
```

The prompt is pre-seeded with `{{` to encourage JSON output.

## Viewing Debug Logs

The system logs detailed information. Run with debug logging:

```bash
RUST_LOG=debug cargo run
```

Look for these log messages:

### Successful LLM Categorization
```
[INFO] Loading LLM model from: src-tauri/models/llm/qwen2.5-1.5b-instruct-q4_k_m.gguf
[INFO] ✅ LLM model loaded successfully
[DEBUG] Generating LLM response for prompt (length: 523)
[DEBUG] Tokenized prompt: 156 tokens
[DEBUG] Generated 87 tokens
[DEBUG] Raw LLM response: {"action":"create_new","category_name":"Python Scripts","emoji":"🐍"...}
[DEBUG] Parsed decision: action=CreateNew, category=Python Scripts
[INFO] ✅ Created new category (LLM): 🐍 Python Scripts (ID: 5)
```

### Failed LLM Categorization
```
[WARN] LLM categorization failed: Failed to parse LLM response as JSON
[ERROR] JSON parse error: expected value at line 1 column 1
[ERROR] Attempted to parse: The snippet appears to be Python code...
[WARN] ⚠️  LLM categorization failed: ..., falling back to semantic search
```

## Common Issues & Solutions

### Issue 1: "Failed to parse LLM response as JSON"

**Cause:** Model is generating explanatory text instead of pure JSON

**Solution:** The prompt has been updated to use Qwen's chat template and prime the response with `{{`. If this persists:

1. Check the logs for "Raw LLM response"
2. The model might need more explicit instructions
3. Try reducing `max_tokens` in `LlamaParams::default()` (currently 200)

**Alternative Fix:** Edit `src-tauri/src/inference/llama.rs` line 51:
```rust
max_tokens: 150, // Shorter responses = less likely to add extra text
```

### Issue 2: "Model file not found"

**Cause:** Model not at expected path

**Solution:** Verify model exists:
```bash
ls -lh src-tauri/models/llm/qwen2.5-1.5b-instruct-q4_k_m.gguf
# Should show: 940M file
```

If missing, download again:
```bash
curl -L -o src-tauri/models/llm/qwen2.5-1.5b-instruct-q4_k_m.gguf \
  "https://huggingface.co/bartowski/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf"
```

### Issue 3: LLM inference is too slow

**Cause:** Model running on CPU with default thread count

**Solutions:**

1. **Reduce context window** (faster processing):
   ```rust
   // src-tauri/src/inference/llama.rs
   n_ctx: 1024, // Down from 2048
   ```

2. **Increase thread count** (if you have spare cores):
   ```rust
   // src-tauri/src/inference/llama.rs
   n_threads: 8, // Or your CPU core count
   ```

3. **Use greedy sampling** (deterministic, faster):
   ```rust
   // src-tauri/src/inference/llama.rs
   temperature: 0.0, // Greedy mode
   ```

### Issue 4: Model suggestions are inaccurate

**Cause:** Prompt needs tuning for your use case

**Solution:** Edit the prompt in `src-tauri/src/inference/categorization.rs:67-84`:

```rust
format!(
    r#"<|im_start|>system
You are an expert at categorizing code snippets and documentation.
Focus on programming languages, frameworks, and content types.
Always respond with valid JSON only.<|im_end|>
<|im_start|>user
...your custom instructions...<|im_end|>
<|im_start|>assistant
{{"#
)
```

### Issue 5: Always falls back to semantic search

**Causes:**
1. Model file doesn't exist → Check logs for "Model file not found"
2. JSON parsing fails → Check logs for "Failed to parse LLM response"
3. Timeout (>10 seconds) → Check logs for timing

**Debug Steps:**
```bash
# Run with full logging
RUST_LOG=debug cargo run

# Save a test snippet
# Watch for:
# - "Loading LLM model" (should appear once)
# - "Generating LLM response" (should appear for each snippet)
# - "Raw LLM response" (see what model generates)
```

## Performance Tuning

### Memory Usage

- **Model size:** 940MB (Q4_K_M quantization)
- **Runtime memory:** ~1.5GB total
- **Cached after first load:** Yes

### Speed Optimization

Current default settings (balanced):
```rust
LlamaParams {
    n_threads: CPU_CORES,
    n_ctx: 2048,
    temperature: 0.3,
    max_tokens: 200,
}
```

Fast mode (2-3 seconds):
```rust
LlamaParams {
    n_threads: CPU_CORES,
    n_ctx: 1024,        // ← Smaller context
    temperature: 0.0,   // ← Greedy sampling
    max_tokens: 150,    // ← Shorter responses
}
```

### Fallback Behavior

The system has three layers:

1. **LLM (primary):** 2-5 seconds, most intelligent
2. **Semantic similarity (fallback #1):** <100ms, very accurate
3. **Keyword rules (fallback #2):** <1ms, simple patterns

If LLM is consistently failing, the semantic similarity fallback is actually very good! You can monitor which method was used:

```sql
-- Query the database
SELECT
    categorization_method,
    COUNT(*) as count
FROM snippet_categories
GROUP BY categorization_method;

-- Results:
-- llm: 45
-- embedding: 23
-- keyword: 12
-- manual: 5
```

## Testing the LLM

### Test with different snippet types:

1. **Code snippet:**
   ```python
   def hello():
       print("Hello World")
   ```
   Expected: Creates "Python Code" or uses existing Python category

2. **Documentation:**
   ```
   How to install Django:
   1. pip install django
   2. django-admin startproject mysite
   ```
   Expected: Creates "Documentation" or "Tutorials"

3. **Command:**
   ```bash
   git commit -m "Initial commit"
   git push origin main
   ```
   Expected: Creates "Git Commands" or similar

### Check the logs:

```bash
# Should see:
🤖 Using LLM for smart categorization...
🤖 LLM decision: CreateNew category 'Python Code' (confidence: 0.92)
✅ Created new category (LLM): 🐍 Python Code (ID: 7)
```

## Advanced: Switching Models

To use a different model (e.g., Phi-3, TinyLlama):

1. Download GGUF model to `src-tauri/models/llm/`
2. Update path in `src-tauri/src/inference/llm_manager.rs:103`:
   ```rust
   "models/llm/your-model-name.gguf"
   ```
3. Adjust prompt format if needed (different models use different templates)

## Database Schema

New columns in `snippet_categories` table:

- `categorization_method`: `'llm'`, `'embedding'`, `'keyword'`, or `'manual'`
- `llm_reasoning`: Text explanation from LLM (for debugging)

## Monitoring & Analytics

Query categorization effectiveness:

```sql
-- LLM success rate
SELECT
    CASE
        WHEN categorization_method = 'llm' THEN 'LLM Success'
        ELSE 'LLM Failed (fallback)'
    END as status,
    COUNT(*) as count
FROM snippet_categories
WHERE is_manual = 0
GROUP BY status;

-- View LLM reasoning
SELECT
    s.content as snippet,
    c.name as category,
    sc.llm_reasoning,
    sc.confidence
FROM snippets s
JOIN snippet_categories sc ON s.id = sc.snippet_id
JOIN categories c ON sc.category_id = c.id
WHERE sc.categorization_method = 'llm'
ORDER BY sc.assigned_at DESC
LIMIT 10;
```

## Files Modified

- `src/inference/llama.rs` - LLM model loading & inference
- `src/inference/categorization.rs` - Prompt engineering & JSON parsing
- `src/inference/llm_manager.rs` - Lazy loading & caching
- `src/job_queue.rs` - Integration with snippet processing
- `src/db/migrations.rs` - Database schema updates
- `src/db/sqlite.rs` - New database functions

## Support

If you encounter issues:

1. Check logs with `RUST_LOG=debug`
2. Look for "Raw LLM response" to see what model generates
3. Verify model file exists and is 940MB
4. Try fallback to semantic search (should work perfectly)
5. Adjust prompt or parameters as needed

The fallback systems ensure categorization always works, even if LLM fails!
