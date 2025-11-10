# LocalMind: Technical Documentation

## Table of Contents
1. [Overview](#overview)
2. [Architecture](#architecture)
3. [User Flows & Technical Implementation](#user-flows--technical-implementation)
4. [Core Components](#core-components)
5. [Data Flow Diagrams](#data-flow-diagrams)
6. [Key Technologies](#key-technologies)
7. [Development Setup](#development-setup)
8. [Testing Strategy](#testing-strategy)

---

## Overview

LocalMind is a 100% local-first cognitive context assistant that helps users save, search, and query their clipboard snippets using semantic search and local AI inference. All processing happens on your machine - no data ever leaves your computer.

**Key Principles:**
- **Privacy First:** Zero data transmission to external servers
- **Low-Spec Friendly:** Designed to run on machines with as little as 4GB RAM
- **Performance Conscious:** Optimizations for CPU-bound tasks on commodity hardware

---

## Architecture

### High-Level System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    User's Computer                          │
│                                                             │
│  ┌──────────────┐         ┌──────────────────────────┐   │
│  │  React UI    │◄───────►│  Tauri Backend (Rust)    │   │
│  │  (Frontend)  │  Events │                          │   │
│  └──────────────┘         └──────────────────────────┘   │
│                                   │                        │
│                    ┌──────────────┼──────────────┐        │
│                    │              │              │         │
│            ┌───────▼──────┐ ┌─────▼──────┐ ┌─────▼──────┐│
│            │   SQLite +   │ │  LanceDB   │ │  Models    ││
│            │   FTS5       │ │  (Vectors) │ │ (GGUF)     ││
│            └──────────────┘ └────────────┘ └────────────┘│
│                                                           │
└───────────────────────────────────────────────────────────┘
```

### UI Architecture (Current State)

LocalMind uses a **tab-based navigation** interface with three main sections:

```
┌─────────────────────────────────────────────────────────────────────┐
│  LocalMind          [ Home | Search | Analytics | Settings ]        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  HOME TAB (Ctrl+1):                                                │
│  ┌──────────────┬──────────────────────────────────────┐          │
│  │ Categories   │ Content Panel                         │          │
│  │ Tree         │                                       │          │
│  │              │ - Suggestions Panel (AI insights)     │          │
│  │ 📁 Work      │ - Selected category snippets          │          │
│  │   📁 Code    │ - Snippet detail view                │          │
│  │     📄 snip  │ - Edit, Copy, Delete actions         │          │
│  │ 📁 Personal  │                                       │          │
│  │              │                                       │          │
│  │ [+ Category] │                                       │          │
│  └──────────────┴──────────────────────────────────────┘          │
│                                                                     │
│  SEARCH TAB (Ctrl+2):                                              │
│  - Search input with advanced filters                              │
│  - Keyword + Semantic search results (RRF ranking)                 │
│  - Click result → Navigate to Home tab with snippet selected       │
│  - Edit and delete actions inline                                  │
│                                                                     │
│  ANALYTICS TAB (Ctrl+3):                                           │
│  - Search statistics (total, avg latency, hit rates)               │
│  - Performance charts (keyword vs semantic)                        │
│  - Popular queries ranking                                         │
│  - Time range selector (24h, 7d, 30d, 90d)                        │
│  - Performance insights and recommendations                        │
│                                                                     │
│  SETTINGS TAB (Ctrl+4):                                            │
│  - Dark Mode toggle                                                │
│  - Semantic Search toggle                                          │
│  - Search Analytics toggle                                         │
│  - Export/Backup (JSON, Markdown)                                  │
│  - Duplicate Management (scan, merge, delete)                      │
│  - Storage statistics                                              │
│  - Performance metrics (RAM, CPU)                                  │
└─────────────────────────────────────────────────────────────────────┘
```

**Key UI Features:**
- **Category Tree**: Hierarchical organization with expand/collapse, auto-expand on navigation
- **Content Panel**: Displays snippets from selected category or search result
- **Search Navigation**: Click search results to navigate to snippet location in Home tab
- **Command Palette (Ctrl+K)**: Fuzzy search for all actions and commands
- **Smart Suggestions**: AI-powered recommendations panel in Home tab
- **Search Filters**: Advanced filtering by date, source app, embedding status
- **Theme System**: CSS variables with localStorage caching for instant dark mode
- **Real-time Stats**: Live memory and CPU usage monitoring
- **Analytics Dashboard**: Visualization of search patterns and performance
- **Export/Backup**: JSON and Markdown export with import functionality
- **Duplicate Detection**: SHA-256 hash-based deduplication with merge tools

### Theme System Architecture

The theme system uses a **three-tier loading strategy** to eliminate white flash on startup:

```
App Startup
    │
    ├─► Tier 1: Inline Script (index.html)
    │   - Reads theme from localStorage synchronously
    │   - Applies theme BEFORE React loads (instant, <5ms)
    │   - Fallback to system preference if not found
    │
    ├─► Tier 2: React Mount (App.tsx)
    │   - Loads theme from database (source of truth)
    │   - Syncs to localStorage for next startup
    │   - Updates DOM if theme changed
    │
    └─► Tier 3: Settings Change (SettingsWindow.tsx)
        - User toggles Dark Mode
        - Saves to database (persistent)
        - Updates localStorage (instant next load)
        - Applies to DOM immediately
```

**Implementation Details:**

1. **CSS Variables** (src/index.css)
   - All colors defined as CSS custom properties
   - Light theme in `:root`
   - Dark theme in `[data-theme="dark"]`
   - Instant switching via attribute change

2. **Instant Loading** (index.html lines 9-27)
   ```javascript
   // Synchronous localStorage read before React
   const savedTheme = localStorage.getItem('theme');
   document.documentElement.setAttribute('data-theme', savedTheme);
   ```

3. **Database Sync** (App.tsx lines 24-41)
   ```typescript
   // Load from DB and sync to localStorage
   const settings = await invoke<Settings>("get_settings");
   localStorage.setItem("theme", settings.theme);
   ```

4. **User Control** (SettingsWindow.tsx)
   - Toggle switch (not dropdown)
   - Saves to both DB and localStorage
   - Immediate visual feedback

**Why This Approach:**
- **localStorage**: Fast synchronous access (instant theme on startup)
- **Database**: Source of truth (survives localStorage clear)
- **CSS Variables**: No component re-renders needed for theme switch

### Category System Architecture

LocalMind uses a **hierarchical category system** for organizing snippets:

```
Database Schema:
┌─────────────────────────┐
│ categories              │
├─────────────────────────┤
│ id (PRIMARY KEY)        │
│ name                    │
│ parent_id (FOREIGN KEY) │ ──┐ Self-referencing
│ emoji                   │   │ for hierarchy
│ created_at              │   │
└─────────────────────────┘ ◄─┘

UI Tree Structure:
📁 Work (parent_id = NULL)
  ├─ 📁 Projects (parent_id = Work.id)
  │   ├─ 📄 Snippet 1
  │   └─ 📄 Snippet 2
  └─ 📁 Meetings (parent_id = Work.id)
      └─ 📄 Snippet 3
```

**Key Features:**

1. **Lazy Loading** (CategoryTree.tsx)
   - Children loaded only when category expanded
   - Reduces initial load time
   - Parallel fetch of categories + snippets

2. **Snippet Counts**
   - Shows number of snippets in each category
   - Updated dynamically
   - Helps users locate content

3. **Category Manager**
   - Create new categories with emoji picker
   - Assign parent category
   - Validation and error handling

4. **Tree Navigation**
   - Expand/collapse animation
   - Visual hierarchy with indentation
   - Selected state highlighting

**Database Queries:**
- `get_root_categories`: Fetch top-level categories (parent_id = NULL)
- `get_child_categories`: Fetch children of specific category
- `get_snippets_by_category`: Fetch snippets in category
- All queries optimized with proper indexing

### Component Layers

1. **Presentation Layer (React/TypeScript)**
   - User interface components
   - Event handling and state management
   - Communication with backend via Tauri commands

2. **Business Logic Layer (Rust)**
   - Clipboard monitoring
   - Search algorithms (keyword + semantic)
   - Job queue management
   - Model inference coordination

3. **Data Layer**
   - **SQLite:** Raw text storage with full-text search (FTS5)
   - **LanceDB:** Vector embeddings for semantic search
   - **File System:** Model files (GGUF format)

4. **AI Models Layer**
   - **Embedding Model:** all-MiniLM-L6-v2 (384 dimensions)
   - **LLM Model:** TinyLlama-1.1B-Chat-v1.0 (Q4_K_M quantization)

---

## User Flows & Technical Implementation

### Flow 1: Saving a Snippet (Alt+Shift+C)

#### User Action
1. User copies text to clipboard
2. User presses `Alt+Shift+C`
3. Text is saved to LocalMind

#### Technical Flow

```
┌─────────┐     ┌──────────────┐     ┌──────────┐     ┌──────────┐
│ User    │     │  Shortcut    │     │ SQLite   │     │ Job      │
│ Action  │────►│  Handler     │────►│ Save     │────►│ Queue    │
└─────────┘     └──────────────┘     └──────────┘     └──────────┘
                                                    │
                                                    ▼
                                            ┌──────────────┐
                                            │  Background  │
                                            │  Worker      │
                                            └──────────────┘
```

**Step-by-Step Implementation:**

1. **Global Shortcut Detection** (`src-tauri/src/shortcuts.rs`)
   ```rust
   // Platform-specific shortcut registration
   // Windows: Uses RegisterHotKey API
   // macOS: Uses Carbon/Cocoa event handlers  
   // Linux: Uses X11 keybindings
   ```

   **Technical Details:**
   - On macOS: Requires accessibility permissions
   - Uses `rdev` crate or Tauri's global shortcut API
   - Shortcut listener runs in separate thread to avoid blocking UI

2. **Clipboard Reading** (`src-tauri/src/clipboard.rs`)
   ```rust
   fn read_clipboard() -> Result<String> {
       // Platform-specific clipboard access
       // Windows: GetClipboardData
       // macOS: NSPasteboard
       // Linux: X11 clipboard
   }
   ```

3. **Instant SQLite Save** (`src-tauri/src/db/sqlite.rs`)
   ```rust
   async fn save_snippet(text: String) -> Result<u64> {
       // Insert into snippets table (instant - <10ms)
       let id = sqlx::query!(
           "INSERT INTO snippets (content, created_at) VALUES (?, ?)",
           text,
           Utc::now()
       ).execute(&db).await?.last_insert_rowid();
       
       // Insert into FTS5 table for instant keyword search
       sqlx::query!(
           "INSERT INTO snippets_fts (rowid, content) VALUES (?, ?)",
           id, text
       ).execute(&db).await?;
       
       Ok(id)
   }
   ```

   **What is FTS5?**
   - FTS5 is SQLite's Full-Text Search extension
   - Creates a virtual table optimized for text searching
   - Provides fast keyword matching and ranking
   - **Learn more:** [SQLite FTS5 Documentation](https://www.sqlite.org/fts5.html)
   - **Why FTS5?** Instant search results without waiting for embeddings

4. **Background Embedding Job** (`src-tauri/src/job_queue.rs`)
   ```rust
   // Job is queued immediately (non-blocking)
   job_queue.push(EmbedJob {
       snippet_id: id,
       content: text,
       priority: Priority::Normal
   }).await?;
   
   // Background worker processes asynchronously
   // Takes 5-15 seconds but doesn't block user
   ```

5. **Embedding Generation** (`src-tauri/src/embedding/engine.rs`)
   ```rust
   async fn embed_text(text: String) -> Result<Vec<f32>> {
       // Lazy load model on first use
       let model = self.model.lock().await;
       
       // Generate 384-dimensional vector
       // This converts text into a mathematical representation
       let embedding = model.embed(&text)?;
       
       Ok(embedding)
   }
   ```

   **What are Embeddings?**
   - Embeddings convert text into numerical vectors (arrays of numbers)
   - Similar texts produce similar vectors
   - Enables "semantic search" - finding meaning, not just keywords
   - **Learn more:** [Understanding Embeddings](https://platform.openai.com/docs/guides/embeddings)
   - **Model used:** [all-MiniLM-L6-v2](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2) (90MB, 384 dimensions)

6. **Vector Storage** (`src-tauri/src/db/lancedb.rs`)
   ```rust
   async fn save_embedding(snippet_id: u64, vector: Vec<f32>) -> Result<()> {
       // Store in LanceDB for fast similarity search
       lance_table.insert({
           "snippet_id": snippet_id,
           "embedding": vector,
       }).await?;
   }
   ```

   **What is LanceDB?**
   - LanceDB is a vector database designed for embeddings
   - Optimized for fast similarity search using cosine distance
   - **Learn more:** [LanceDB Documentation](https://lancedb.github.io/lancedb/)

#### Timing Breakdown
- Clipboard read: <5ms
- SQLite save: <10ms
- **User sees:** Instant feedback (<15ms total)
- Background embedding: 5-15 seconds (doesn't block UI)

---

### Flow 2: Searching Snippets (Alt+Shift+F)

#### User Action
1. User presses `Alt+Shift+F`
2. Search window appears
3. User types query
4. Results appear (keyword + semantic)

#### Technical Flow

```
┌─────────┐     ┌──────────────┐     ┌──────────────────┐
│ User    │     │  Search UI   │     │  Parallel Search │
│ Query   │────►│  (React)     │────►│                  │
└─────────┘     └──────────────┘     │  ┌────────────┐  │
                                    │  │ FTS5       │  │
                                    │  │ (Instant)  │  │
                                    │  └────────────┘  │
                                    │                  │
                                    │  ┌────────────┐  │
                                    │  │ Semantic   │  │
                                    │  │ (1-2 sec)  │  │
                                    │  └────────────┘  │
                                    └──────────────────┘
                                              │
                                              ▼
                                    ┌──────────────────┐
                                    │  Merge & Rank    │
                                    │  Results         │
                                    └──────────────────┘
```

**Step-by-Step Implementation:**

1. **Window Activation** (`src-tauri/src/shortcuts.rs`)
   ```rust
   // Show/hide search window on Alt+Shift+F
   if window.is_visible()? {
       window.hide()?;
   } else {
       window.show()?;
       window.set_focus()?;
   }
   ```

2. **Query Input** (`src/components/SearchWindow.tsx`)
   ```typescript
   const [query, setQuery] = useState("");
   const debouncedQuery = useDebounce(query, 300); // Wait 300ms after typing stops
   ```

3. **Parallel Search Execution** (`src-tauri/src/commands.rs`)
   ```rust
   async fn search(query: String) -> Result<SearchResults> {
       // Run both searches in parallel using tokio::join!
       let (keyword_results, semantic_results) = tokio::join!(
           search_fts5(&query),      // Fast keyword search
           search_semantic(&query)   // Slower but more intelligent
       );
       
       // Combine and rank results
       let combined = merge_results(keyword_results?, semantic_results?);
       Ok(combined)
   }
   ```

4. **Keyword Search (FTS5)** (`src-tauri/src/db/sqlite.rs`)
   ```rust
   async fn search_fts5(query: &str) -> Result<Vec<SearchResult>> {
       // FTS5 search is instant (<100ms even with 10k snippets)
       let results = sqlx::query_as!(
           SearchResult,
           r#"
           SELECT rowid, content, created_at,
                  bm25(snippets_fts) as rank
           FROM snippets_fts
           WHERE snippets_fts MATCH ?
           ORDER BY rank DESC
           LIMIT 10
           "#,
           sanitize_fts5_query(query) // Prevent SQL injection
       ).fetch_all(&db).await?;
       
       Ok(results)
   }
   ```

   **FTS5 Query Syntax:**
   - `MATCH` operator performs full-text search
   - `bm25()` function ranks results by relevance
   - **Security:** Always sanitize queries to prevent injection
   - **Reference:** [FTS5 Query Syntax](https://www.sqlite.org/fts5.html#fts5_query_syntax)

5. **Semantic Search** (`src-tauri/src/db/lancedb.rs`)
   ```rust
   async fn search_semantic(query: &str) -> Result<Vec<SearchResult>> {
       // Step 1: Embed the query (5-10 seconds on first use, <1s if model loaded)
       let query_embedding = embedding_cache.get_or_embed(query).await?;
       
       // Step 2: Find similar vectors using cosine similarity
       let results = lance_table
           .vector_search(&query_embedding)
           .limit(10)
           .execute()
           .await?;
       
       // Step 3: Fetch snippet details from SQLite
       let snippet_ids: Vec<u64> = results.iter()
           .map(|r| r.snippet_id)
           .collect();
       
       let snippets = fetch_snippets_by_ids(&snippet_ids).await?;
       Ok(snippets)
   }
   ```

   **Query Embedding Cache** (`src-tauri/src/embedding/cache.rs`)
   ```rust
   // Cache recent queries to skip re-embedding
   // "python code" and "python script" might have same embedding
   struct QueryCache {
       cache: LruCache<String, (Vec<f32>, SystemTime)>,
       ttl: Duration::from_secs(3600), // 1 hour
   }
   ```

6. **Result Merging** (`src-tauri/src/commands.rs`)
   ```rust
   fn merge_results(keyword: Vec<Result>, semantic: Vec<Result>) -> Vec<Result> {
       // Deduplicate by snippet_id
       // Prioritize keyword matches for exact hits
       // Add semantic results for context
       // Final ranking: exact keyword match > semantic similarity
   }
   ```

#### Timing Breakdown
- Keyword search: <100ms (always fast)
- Semantic search: 1-2 seconds (if embedding model loaded)
- **User sees:** Keyword results instantly, semantic results appear shortly after

---

### Flow 3: AI Q&A (Ask AI)

#### User Action
1. User searches for snippets
2. User checks "[ ] Ask AI" checkbox
3. User presses Enter
4. AI processes query and streams answer

#### Technical Flow

```
┌─────────┐     ┌──────────────┐     ┌──────────────────┐
│ User    │     │  RAG Builder │     │  LLM Inference   │
│ Query   │────►│              │────►│  (llama.cpp)    │
└─────────┘     └──────────────┘     └──────────────────┘
                       │                       │
                       │                       ▼
                       │              ┌──────────────────┐
                       │              │  Token Streaming │
                       │              └──────────────────┘
                       │                       │
                       ▼                       ▼
              ┌──────────────────┐    ┌──────────────────┐
              │  Top 5 Snippets │    │  UI Updates      │
              │  as Context     │    │  (React)         │
              └──────────────────┘    └──────────────────┘
```

**Step-by-Step Implementation:**

1. **Context Retrieval** (`src-tauri/src/rag/builder.rs`)
   ```rust
   async fn build_rag_context(query: &str, snippet_ids: Vec<u64>) -> String {
       // Fetch top 5 relevant snippets
       let snippets = fetch_snippets_by_ids(&snippet_ids).await?;
       
       // Smart context window management
       // Reserve tokens for query + answer
       let available_tokens = 512 - estimate_tokens(query) - 150;
       
       let mut context = String::new();
       let mut used_tokens = 0;
       
       for snippet in snippets {
           let snippet_tokens = estimate_tokens(&snippet.content);
           
           if used_tokens + snippet_tokens > available_tokens {
               // Truncate if needed
               context.push_str(&truncate_smart(&snippet.content, 
                   available_tokens - used_tokens));
               break;
           }
           
           context.push_str(&format!("[{}] {}\n---\n", 
               snippet.source_app, snippet.content));
           used_tokens += snippet_tokens;
       }
       
       Ok(context)
   }
   ```

2. **RAG Prompt Construction** (`src-tauri/src/rag/builder.rs`)
   ```rust
   fn build_rag_prompt(query: &str, context: &str) -> String {
       format!(r#"
You are a helpful assistant. Use *only* the context provided below to answer the user's question. Do not make information up.

Context:
{}

Question: {}

Answer:
"#, context, query)
   }
   ```

   **What is RAG?**
   - RAG = Retrieval Augmented Generation
   - Combines semantic search (retrieval) with LLM generation
   - Provides context to LLM so it can answer based on user's data
   - **Learn more:** [RAG Explained](https://www.pinecone.io/learn/retrieval-augmented-generation/)

3. **Progressive Model Loading** (`src-tauri/src/inference/loader.rs`)
   ```rust
   async fn load_llm_with_progress(
       model_path: &Path,
       progress_tx: mpsc::Sender<LoadProgress>
   ) -> Result<LlamaModel> {
       let file_size = std::fs::metadata(model_path)?.len();
       let mut loaded = 0u64;
       
       // Load with memory mapping (mmap) for faster loading
       let model = LlamaModel::load_with_callback(
           model_path,
           |bytes_loaded| {
               loaded += bytes_loaded as u64;
               let percent = (loaded * 100) / file_size;
               let _ = progress_tx.try_send(LoadProgress {
                   percent,
                   stage: "Loading model..."
               });
           }
       )?;
       
       progress_tx.send(LoadProgress { 
           percent: 100, 
           stage: "Ready!" 
       }).await?;
       
       Ok(model)
   }
   ```

   **Why Progressive Loading?**
   - 670MB model file can take 10-30 seconds to load on HDD
   - Shows progress bar instead of frozen UI
   - Uses memory mapping (mmap) for faster access
   - **Reference:** [Memory Mapping](https://en.wikipedia.org/wiki/Memory-mapped_file)

4. **LLM Inference** (`src-tauri/src/inference/llama.rs`)
   ```rust
   async fn generate_streaming(
       prompt: &str,
       callback: impl Fn(String) -> Result<()>
   ) -> Result<()> {
       // Initialize llama.cpp context
       let ctx = llama_context::new(&model, llama_params)?;
       
       // Tokenize prompt
       let tokens = ctx.tokenize(prompt, true)?;
       ctx.eval(tokens, 0)?;
       
       // Generate tokens one by one
       loop {
           let token = ctx.sample()?;
           
           if token == llama_context::EOS_TOKEN {
               break; // End of sequence
           }
           
           let text = ctx.token_to_str(token)?;
           callback(text)?; // Stream to UI
           
           // Continue generation
           ctx.eval(vec![token], ctx.n_ctx() - 1)?;
       }
       
       Ok(())
   }
   ```

   **What is llama.cpp?**
   - C++ library for running LLM inference on CPU
   - Supports GGUF format (quantized models)
   - Very memory efficient (uses quantization)
   - **Learn more:** [llama.cpp GitHub](https://github.com/ggerganov/llama.cpp)
   - **GGUF Format:** [GGUF Specification](https://github.com/ggerganov/ggml/blob/master/docs/gguf.md)

5. **Token Streaming to UI** (`src-tauri/src/commands.rs`)
   ```rust
   #[tauri::command]
   async fn ask_ai(
       query: String,
       snippet_ids: Vec<u64>,
       window: Window,
   ) -> Result<()> {
       let context = build_rag_context(&query, snippet_ids).await?;
       let prompt = build_rag_prompt(&query, &context);
       
       // Start inference in background task
       tokio::spawn(async move {
           generate_streaming(&prompt, |token| {
               // Emit event to frontend for each token
               window.emit("token-stream", token)?;
               Ok(())
           }).await
       });
       
       Ok(())
   }
   ```

6. **React Token Streaming** (`src/hooks/useStreamingChat.ts`)
   ```typescript
   const [message, setMessage] = useState("");
   
   useEffect(() => {
       const unlisten = listen("token-stream", (event) => {
           // Append each token as it arrives
           setMessage(prev => prev + event.payload);
       });
       
       return () => unlisten();
   }, []);
   ```

#### Timing Breakdown
- Context retrieval: <200ms
- Model loading (if not loaded): 10-30 seconds (first time only)
- First token generation: 2-5 seconds
- Token streaming: 1-2 tokens/second (CPU inference)
- Full answer: 15-30 seconds for typical response

---

## Core Components

### 1. Job Queue System

**Purpose:** Process embeddings asynchronously without blocking user

**File:** `src-tauri/src/job_queue.rs`

```rust
pub struct PersistentJobQueue {
    db: SqliteConnection,
    worker_tx: mpsc::Sender<Job>,
}

#[derive(Debug, Clone)]
pub struct Job {
    pub snippet_id: u64,
    pub content: String,
    pub priority: Priority,
    pub status: JobStatus,
}

impl PersistentJobQueue {
    // Save job to database immediately (crash recovery)
    pub async fn push(&mut self, job: Job) -> Result<()> {
        sqlx::query!(
            "INSERT INTO job_queue (snippet_id, content, priority, status) 
             VALUES (?, ?, ?, 'pending')",
            job.snippet_id,
            job.content,
            job.priority as i32
        ).execute(&mut self.db).await?;
        
        self.worker_tx.send(job).await?;
        Ok(())
    }
    
    // Recover incomplete jobs on startup
    pub async fn recover_on_startup(&mut self) -> Result<()> {
        let pending = sqlx::query_as!(
            Job,
            "SELECT * FROM job_queue WHERE status IN ('pending', 'running')"
        ).fetch_all(&mut self.db).await?;
        
        for job in pending {
            self.push(job).await?;
        }
        Ok(())
    }
}
```

**Why Persistence?**
- If app crashes during embedding, jobs aren't lost
- Jobs saved to SQLite immediately
- On restart, incomplete jobs are resumed

### 2. Streaming Embeddings

**Purpose:** Interrupt embedding work if user needs CPU

**File:** `src-tauri/src/embedding/streaming.rs`

```rust
pub struct StreamingEmbedder {
    model: Arc<Mutex<EmbeddingModel>>,
    pause_flag: Arc<AtomicBool>,
}

impl StreamingEmbedder {
    pub async fn embed_interruptible(
        &self,
        snippets: Vec<Snippet>
    ) -> Result<()> {
        for snippet in snippets {
            // Check if user activity detected
            if self.pause_flag.load(Ordering::Relaxed) {
                self.pause_and_unload().await?;
                return Ok(());
            }
            
            let embedding = self.model.lock().await.embed(&snippet.content)?;
            db.save_embedding(snippet.id, embedding).await?;
            
            // Yield every N embeddings to let other tasks run
            if snippet.id % 3 == 0 {
                tokio::task::yield_now().await;
            }
        }
        Ok(())
    }
}
```

### 3. Query Expansion

**Purpose:** Find synonyms to improve keyword search

**File:** `src-tauri/src/query/expansion.rs`

```rust
pub fn expand_query(query: &str) -> Vec<String> {
    let mut expansions = vec![query.to_string()];
    
    // Built-in synonym dictionary
    let synonyms: HashMap<&str, Vec<&str>> = hashmap! {
        "code" => vec!["script", "program", "function"],
        "bug" => vec!["error", "issue", "problem"],
        "fix" => vec!["solve", "resolve", "patch"],
        // ... 50-100 common programming synonyms
    };
    
    for word in query.split_whitespace() {
        if let Some(syns) = synonyms.get(word.to_lowercase().as_str()) {
            for syn in syns {
                expansions.push(query.replace(word, syn));
            }
        }
    }
    
    expansions.truncate(5); // Limit to 5 variations
    expansions
}
```

### 4. Graceful Degradation

**Purpose:** App continues working even if models fail to load

**File:** `src-tauri/src/app_state.rs`

```rust
pub enum AppState {
    FullyOperational {
        fts5: Ready,
        embedding: Ready,
        llm: Ready,
    },
    
    SearchOnly {
        fts5: Ready,
        embedding: Failed(Error),
        llm: Failed(Error),
    },
    
    KeywordOnly {
        fts5: Ready,
        embedding: NotInstalled,
        llm: NotInstalled,
    },
}

impl AppState {
    pub fn show_banner(&self) -> Option<String> {
        match self {
            Self::SearchOnly { embedding, llm } => {
                Some(format!(
                    "⚠️ AI features unavailable: {} | {}",
                    embedding, llm
                ))
            },
            Self::FullyOperational { .. } => None,
            _ => Some("⚠️ Running in limited mode".into()),
        }
    }
    
    pub fn can_search_semantic(&self) -> bool {
        matches!(self, Self::FullyOperational { .. } | 
                        Self::Degraded { embedding: Ready, .. })
    }
}
```

### 5. Thermal & Battery Awareness

**Purpose:** Don't drain battery or overheat CPU

**File:** `src-tauri/src/scheduler.rs`

```rust
pub struct PowerState {
    pub on_battery: bool,
    pub cpu_temp_celsius: i32,
}

#[cfg(target_os = "linux")]
pub fn get_power_state() -> PowerState {
    let on_battery = std::fs::read_to_string(
        "/sys/class/power_supply/AC/online"
    ).unwrap_or("1".into()) == "0";
    
    let temp = std::fs::read_to_string(
        "/sys/class/thermal/thermal_zone0/temp"
    ).unwrap_or("50000".into()).parse::<i32>().unwrap_or(50000) / 1000;
    
    PowerState { on_battery, cpu_temp_celsius: temp }
}

pub struct AdaptiveScheduler {
    pub fn should_run_heavy_task(&self) -> bool {
        let power = get_power_state();
        
        // Only run if:
        !power.on_battery &&           // Plugged in
        power.cpu_temp_celsius < 75 &&  // CPU not hot
        self.system_idle()              // User not active
    }
}
```

---

## Data Flow Diagrams

### Complete Save Flow

```
User Copies Text
    │
    ▼
Alt+Shift+C Pressed
    │
    ├─► Global Shortcut Handler
    │       │
    │       ├─► Read Clipboard (5ms)
    │       │
    │       └─► Save to SQLite (10ms) ──┐
    │                                    │
    │                                    ├─► User sees "Saved!" (15ms total)
    │                                    │
    └─► Job Queue Push                   │
            │                            │
            ▼                            │
    Background Worker                    │
            │                            │
            ├─► Load Embedding Model (if needed) (5-10s)
            │                            │
            ├─► Generate Embedding (5-15s)
            │                            │
            └─► Save to LanceDB ─────────┘
                    │
                    ▼
            Snippet is semantically searchable
```

### Complete Search Flow

```
User Types Query
    │
    ├─► Debounce (300ms wait)
    │
    └─► Parallel Execution
        │
        ├─────────────────────────────────────┐
        │                                     │
        ▼                                     ▼
    FTS5 Search                        Semantic Search
        │                                     │
        │                                     ├─► Check Query Cache
        │                                     │       │
        │                                     │       ├─► Cache Hit? ──► Use Cached Embedding (<1ms)
        │                                     │       │
        │                                     │       └─► Cache Miss? ──► Embed Query (1-2s)
        │                                     │
        │                                     ├─► Vector Search in LanceDB (50-200ms)
        │                                     │
        │                                     └─► Fetch Snippet Details from SQLite (<10ms)
        │
        └─────────────────────────────────────┘
                      │
                      ▼
            Merge & Deduplicate Results
                      │
                      ▼
            Rank by Relevance
                      │
                      ▼
            Return to UI (<2s total for semantic)
```

### Complete AI Q&A Flow

```
User Checks "Ask AI" + Presses Enter
    │
    ├─► Fetch Top 5 Snippets (200ms)
    │
    ├─► Build RAG Context (Smart Token Management)
    │
    ├─► Construct Prompt
    │
    └─► LLM Inference
        │
        ├─► Is Model Loaded?
        │       │
        │       ├─► No ──► Progressive Model Load (10-30s)
        │       │           │
        │       │           └─► Show Progress Bar
        │       │
        │       └─► Yes ──► Continue
        │
        ├─► Tokenize Prompt
        │
        ├─► Generate First Token (2-5s)
        │       │
        │       └─► Emit "token-stream" Event
        │
        ├─► Stream Tokens (1-2 tokens/sec)
        │       │
        │       └─► Each Token ──► UI Update
        │
        └─► End of Sequence Token
                │
                └─► Complete
```

---

## Key Technologies

### SQLite with FTS5

**What it does:** Fast full-text keyword search

**Why we use it:**
- Instant search results (<100ms)
- Works without AI models
- Built into SQLite (no extra dependencies)
- Excellent for exact keyword matches

**Key Concepts:**
- **Virtual Table:** FTS5 creates a special table optimized for search
- **BM25 Ranking:** Algorithm that scores results by relevance
- **MATCH Operator:** SQLite operator for text matching

**References:**
- [SQLite FTS5 Documentation](https://www.sqlite.org/fts5.html)
- [FTS5 Query Syntax](https://www.sqlite.org/fts5.html#fts5_query_syntax)
- [BM25 Algorithm Explanation](https://en.wikipedia.org/wiki/Okapi_BM25)

### LanceDB

**What it does:** Stores and searches vector embeddings

**Why we use it:**
- Fast similarity search (cosine distance)
- Handles high-dimensional vectors efficiently
- Embedded database (no server needed)
- Written in Rust (good performance)

**Key Concepts:**
- **Vector:** Array of numbers representing text meaning
- **Cosine Similarity:** Measure of how similar two vectors are
- **ANN Search:** Approximate Nearest Neighbor search (fast but approximate)

**References:**
- [LanceDB Documentation](https://lancedb.github.io/lancedb/)
- [Understanding Vector Similarity](https://platform.openai.com/docs/guides/embeddings)

### fastembed (Embedding Model)

**What it does:** Generates embeddings from text locally

**Model:** all-MiniLM-L6-v2
- **Size:** ~90MB
- **Dimensions:** 384 (each embedding is 384 numbers)
- **Speed:** 5-15 seconds per snippet on CPU
- **Quality:** Good for semantic search

**Why this model:**
- Small enough for low-spec machines
- Fast inference on CPU
- Good semantic understanding
- No GPU required

**References:**
- [fastembed GitHub](https://github.com/qdrant/fastembed)
- [all-MiniLM-L6-v2 Model Card](https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2)

### llama.cpp (LLM Inference)

**What it does:** Runs LLM inference on CPU

**Model:** TinyLlama-1.1B-Chat-v1.0.Q4_K_M
- **Size:** 670MB (quantized)
- **Quantization:** Q4_K_M (4-bit, medium quality)
- **Context:** 2048 tokens
- **Speed:** 1-2 tokens/second on CPU

**Why this model:**
- Small enough to fit in 2GB RAM
- Acceptable quality for simple RAG
- Can run on any CPU
- Open source

**Key Concepts:**
- **Quantization:** Reduces model size by using fewer bits per weight
- **GGUF Format:** Compressed model format used by llama.cpp
- **Token:** Word or part of word that LLM processes

**References:**
- [llama.cpp GitHub](https://github.com/ggerganov/llama.cpp)
- [GGUF Format Specification](https://github.com/ggerganov/ggml/blob/master/docs/gguf.md)
- [Understanding Quantization](https://huggingface.co/docs/transformers/main/quantization)

### Tauri

**What it does:** Builds desktop apps with Rust backend and web frontend

**Why we use it:**
- Small app size (~10MB vs 100MB+ for Electron)
- Lower memory usage
- Native performance for system APIs
- Secure by default

**Key Concepts:**
- **Commands:** Rust functions exposed to frontend
- **Events:** Frontend-to-backend and backend-to-frontend messaging
- **Window Management:** Controls app windows and system tray

**References:**
- [Tauri Documentation](https://tauri.app/)
- [Tauri Commands Guide](https://tauri.app/v1/guides/features/command)

---

## Development Setup

### Prerequisites

1. **Rust** (latest stable)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Node.js** (v18+)
   ```bash
   # macOS
   brew install node
   
   # Linux
   curl -fsSL https://deb.nodesource.com/setup_18.x | sudo -E bash -
   sudo apt-get install -y nodejs
   ```

3. **System Dependencies**
   - **macOS:** Xcode Command Line Tools
   - **Linux:** `build-essential`, `libssl-dev`, `libgtk-3-dev`
   - **Windows:** Visual Studio Build Tools

### Project Setup

```bash
# Clone repository
git clone <repository-url>
cd local-mind

# Install frontend dependencies
npm install

# Install Rust dependencies (runs automatically on first build)
cd src-tauri
cargo build

# Run development server
npm run tauri dev
```

### Project Structure Explanation

```
local-mind/
├── src/                    # React frontend code
│   ├── components/        # UI components
│   └── hooks/             # React hooks for Tauri integration
│
├── src-tauri/             # Rust backend code
│   ├── src/
│   │   ├── commands.rs   # Functions called from frontend
│   │   ├── db/           # Database operations
│   │   ├── embedding/    # Embedding model integration
│   │   └── inference/    # LLM inference
│   └── Cargo.toml        # Rust dependencies
│
├── models/                # AI model files (not in git)
│   └── TinyLlama-1.1B-Chat-v1.0.Q4_K_M.gguf
│
└── data/                  # Runtime data (not in git)
    ├── snippets.db        # SQLite database
    └── vectors.lance/     # LanceDB vector store
```

---

## Testing Strategy

### Unit Tests

**Location:** `src-tauri/src/*/tests.rs` or `#[cfg(test)]` modules

**Example:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_embedding_idempotence() {
        let text = "hello world";
        let emb1 = embedder.embed(text);
        let emb2 = embedder.embed(text);
        assert_eq!(emb1, emb2);
    }
}
```

### Integration Tests

**Location:** `src-tauri/tests/`

**Example:**
```rust
#[tokio::test]
async fn test_full_save_search_flow() {
    let app = TestApp::new().await;
    
    // Save snippet
    app.save_snippet("rust async programming").await;
    
    // Wait for embedding
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Search should return it
    let results = app.search("async rust").await;
    assert!(results.len() > 0);
}
```

### Benchmarks

**Location:** `src-tauri/benches/`

**Example:**
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_fts5_search(c: &mut Criterion) {
    let db = setup_db_with_10k_snippets();
    c.bench_function("fts5_search", |b| {
        b.iter(|| db.search_fts5(black_box("python code")))
    });
}
```

**Targets:**
- FTS5 search: <100ms with 10k snippets
- Semantic search: <2s with model loaded
- First token: <5s
- Full inference: <30s for 100 tokens

---

## Common Questions

### Q: Why two search systems (FTS5 + Semantic)?

**A:** They complement each other:
- **FTS5:** Fast exact keyword matches (instant)
- **Semantic:** Finds similar meanings even with different words (slower but smarter)
- Combined: Best of both worlds

### Q: Why is embedding slow?

**A:** Embedding models are neural networks running on CPU. 5-15 seconds per snippet is normal for CPU inference. It runs in background so it doesn't block the user.

### Q: Can I use a better LLM model?

**A:** Yes! Phase 3 includes model management. Users with more RAM (8GB+) can download Gemma-2B or connect to Ollama.

### Q: What if I run out of disk space?

**A:** Storage policy (Phase 3) automatically prunes old, unused snippets. You can configure retention policies.

### Q: How do I debug issues?

**A:** Use the diagnostic mode (Alt+Shift+D) to see system status, performance metrics, and error logs. All stored locally.

---

## Further Reading

### Essential Concepts
- [Understanding Embeddings](https://platform.openai.com/docs/guides/embeddings)
- [RAG Explained](https://www.pinecone.io/learn/retrieval-augmented-generation/)
- [Quantization in ML](https://huggingface.co/docs/transformers/main/quantization)

### Technology References
- [Tauri Documentation](https://tauri.app/)
- [SQLite FTS5](https://www.sqlite.org/fts5.html)
- [llama.cpp Guide](https://github.com/ggerganov/llama.cpp)
- [LanceDB Docs](https://lancedb.github.io/lancedb/)

### Performance Optimization
- [Memory Mapping Files](https://en.wikipedia.org/wiki/Memory-mapped_file)
- [CPU Optimization Techniques](https://www.intel.com/content/www/us/en/developer/articles/technical/optimizing-cpu-performance.html)

---

**Document Version:** 1.0  
**Last Updated:** 2024  
**Maintained by:** LocalMind Development Team
