# Search Test Examples - FTS5 vs Semantic Search

This guide provides test examples to verify both **FTS5 Keyword Search** and **Semantic Search** are working correctly.

## 📋 Prerequisites

Before testing semantic search:
1. **Save some test snippets** (see examples below)
2. **Wait 10-15 seconds** for embeddings to be generated in the background
3. **Check console logs** to confirm embeddings were created

---

## 🔍 **FTS5 Keyword Search** (Instant - No Embeddings Required)

FTS5 search matches **exact words and phrases** in snippet content. It's fast and works immediately after saving a snippet.

### Test Cases

#### 1. **Exact Word Match**
```
Query: greensage
Expected: Returns snippets containing "greensage"
Speed: Instant (<50ms)
```

#### 2. **Prefix Match** (First word only, 3+ chars)
```
Query: gre
Expected: Returns snippets starting with "gre" (e.g., "greensage", "green")
Note: Only first word gets prefix matching
Speed: Instant
```

#### 3. **Phrase Search** (Quoted)
```
Query: "greensage ai"
Expected: Returns snippets with exact phrase "greensage ai" in order
Speed: Instant
```

#### 4. **Boolean AND** (All terms must appear)
```
Query: greensage AND algorithm
Expected: Snippets containing both "greensage" and "algorithm"
Speed: Instant
```

#### 5. **Boolean OR** (Any term can appear)
```
Query: machine OR learning
Expected: Snippets containing either "machine" or "learning" (or both)
Speed: Instant
```

#### 6. **Boolean NOT** (Exclude terms)
```
Query: greensage NOT bad
Expected: Snippets with "greensage" but NOT containing "bad"
Speed: Instant
```

---

## 🧠 **Semantic Search** (Meaning-Based - Requires Embeddings)

Semantic search finds snippets based on **meaning and concepts**, not just exact word matches. It requires embeddings to be generated (takes 10-15 seconds after saving).

### How It Works
1. Your query is converted to a vector (embedding)
2. The system compares your query vector with all snippet vectors
3. Returns snippets with **similarity score ≥ 0.3** (cosine similarity)
4. Results are ranked by semantic relevance

### Test Cases

#### 1. **Synonym Matching**
**Save snippet:**
```
"Machine learning algorithm for classification tasks"
```

**Wait 10-15 seconds for embedding**

**Test queries (should all find the snippet):**
```
Query: AI
Expected: Finds "machine learning" snippet (AI is related to ML)

Query: neural networks
Expected: Finds "machine learning" snippet (related concepts)

Query: deep learning
Expected: Finds "machine learning" snippet (related field)
```

#### 2. **Concept Matching**
**Save snippet:**
```
"The neural network model processes images using convolutional layers"
```

**Test queries:**
```
Query: computer vision
Expected: Finds snippet about image processing (related concept)

Query: image recognition
Expected: Finds snippet about image processing (similar meaning)

Query: AI model
Expected: Finds snippet about neural networks (AI = neural networks)
```

#### 3. **Context Matching**
**Save snippet:**
```
"I need to optimize this Python function for better performance"
```

**Test queries:**
```
Query: code improvement
Expected: Finds snippet about optimization (similar intent)

Query: speed up program
Expected: Finds snippet about performance optimization (related goal)

Query: make faster
Expected: Finds snippet about optimization (same meaning)
```

#### 4. **Abstract Concept Matching**
**Save snippet:**
```
"Implementing a REST API endpoint for user authentication"
```

**Test queries:**
```
Query: backend service
Expected: Finds snippet about API (related concept)

Query: login system
Expected: Finds snippet about authentication (same concept)

Query: HTTP interface
Expected: Finds snippet about REST API (same thing)
```

---

## 🧪 **Complete Test Workflow**

### Step 1: Prepare Test Data

Save these snippets one by one:

1. **AI/ML Snippet:**
   ```
   "Machine learning and artificial intelligence are transforming industries. Neural networks enable deep learning models."
   ```

2. **Programming Snippet:**
   ```
   "I wrote a Python function to sort data. The algorithm uses quicksort for efficiency."
   ```

3. **Optimization Snippet:**
   ```
   "Need to optimize database queries. The current implementation is too slow for production."
   ```

### Step 2: Wait for Embeddings
Wait **10-15 seconds** after saving each snippet. Check console logs for:
```
✅ Embedding generated for snippet X
```

### Step 3: Test FTS5 Search (Immediate)

**Exact matches:**
```
✅ Query: "machine learning"
   Expected: Finds snippet #1 immediately

✅ Query: "Python"
   Expected: Finds snippet #2 immediately

✅ Query: "database"
   Expected: Finds snippet #3 immediately
```

**Prefix matches:**
```
✅ Query: "mach"
   Expected: Finds snippet #1 (matches "machine")

✅ Query: "Pyth"
   Expected: Finds snippet #2 (matches "Python")

✅ Query: "data"
   Expected: Finds snippet #3 (matches "database")
```

**Boolean operators:**
```
✅ Query: "machine AND learning"
   Expected: Finds snippet #1

✅ Query: "Python OR JavaScript"
   Expected: Finds snippet #2 (contains Python)

✅ Query: "optimize NOT slow"
   Expected: Finds snippet #3 (but wait - it contains "slow"!)
```

### Step 4: Test Semantic Search (After Embeddings Ready)

**Concept matching:**
```
✅ Query: "AI"
   Expected: Finds snippet #1 (AI = artificial intelligence = machine learning)

✅ Query: "deep learning"
   Expected: Finds snippet #1 (related to neural networks)

✅ Query: "algorithm"
   Expected: Finds snippet #1 or #2 (both discuss algorithms)

✅ Query: "efficiency"
   Expected: Finds snippet #2 or #3 (both about optimization/performance)

✅ Query: "speed improvement"
   Expected: Finds snippet #3 (optimization = speed improvement)
```

**Cross-domain matching:**
```
✅ Query: "artificial intelligence"
   Expected: Finds snippet #1 (exact match via FTS5, also semantic)

✅ Query: "neural networks"
   Expected: Finds snippet #1 (semantic match)

✅ Query: "code optimization"
   Expected: Finds snippet #2 or #3 (both are about optimization)
```

---

## 🔍 **How to Verify Which Search Type Found Results**

### In Console Logs:
- **FTS5/Keyword:** Look for `Keyword search 'X' returned N results`
- **Semantic:** Look for `Semantic search 'X' returned N results`

### In UI:
- Results from keyword search appear first (faster)
- Semantic results are merged and ranked by relevance
- Combined results show both types

---

## 🐛 **Troubleshooting**

### Semantic Search Returns No Results

**Problem:** Semantic search always returns empty results.

**Check:**
1. **Embeddings generated?** 
   ```bash
   # Check if vectors file exists
   ls -lh src-tauri/data/local-mind/vectors.lance
   ```

2. **Model downloaded?**
   - First semantic search triggers model download (~90MB)
   - Check console for: `Downloading fast-all-MiniLM-L6-v2 model`
   - Wait for: `Embedding model loaded successfully`

3. **Embeddings saved?**
   - Wait 10-15 seconds after saving snippets
   - Check console logs for embedding job completion

4. **Similarity threshold too high?**
   - Current threshold: **0.3** (cosine similarity)
   - Lower = more results, Higher = more precise

### FTS5 Search Returns No Results

**Problem:** Keyword search returns empty.

**Check:**
1. **Snippet actually saved?**
   ```bash
   sqlite3 src-tauri/data/local-mind/snippets.db "SELECT id, content FROM snippets;"
   ```

2. **FTS5 table exists?**
   ```bash
   sqlite3 src-tauri/data/local-mind/snippets.db "SELECT COUNT(*) FROM snippets_fts;"
   ```

3. **Query syntax correct?**
   - Prefix: Only first word (3+ chars) gets `*`
   - Phrases: Use quotes `"exact phrase"`
   - Boolean: `term1 AND term2`, `term1 OR term2`

---

## 📊 **Expected Performance**

| Search Type | Speed | Works After | Requires Embeddings |
|------------|-------|-------------|---------------------|
| **FTS5 Keyword** | <50ms | Immediately | ❌ No |
| **Semantic** | 100-500ms | 10-15 sec delay | ✅ Yes |

---

## 💡 **Pro Tips**

1. **Use FTS5 for:** Exact names, code snippets, specific terms
2. **Use Semantic for:** Concepts, synonyms, related topics
3. **Combine both:** The system automatically merges results for best coverage
4. **Prefix matching:** Only first word (3+ chars) gets automatic prefix (`gre*`)
5. **Phrase search:** Use quotes for exact phrase: `"greensage ai"`

---

## 🎯 **Quick Test Checklist**

```
□ Save test snippet with content about "machine learning"
□ Wait 10-15 seconds
□ Test FTS5: Search "machine" → Should find immediately
□ Test Semantic: Search "AI" → Should find after embedding ready
□ Test Prefix: Search "mach" → Should find "machine"
□ Test Phrase: Search "machine learning" → Should find exact phrase
□ Test Boolean: Search "machine AND learning" → Should find
□ Check console logs to see which search type found results
```

---

## 📚 **Reference**

- **FTS5 Documentation:** [SQLite FTS5](https://www.sqlite.org/fts5.html)
- **Embeddings:** [Understanding Embeddings](https://platform.openai.com/docs/guides/embeddings)
- **Cosine Similarity:** [Cosine Similarity Explained](https://en.wikipedia.org/wiki/Cosine_similarity)
- **Model Used:** [all-MiniLM-L6-v2](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2)

