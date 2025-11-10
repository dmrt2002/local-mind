# LocalMind - Complete Testing Guide

This guide provides step-by-step instructions to test all features of LocalMind.

## Prerequisites

### Required Software

1. **Node.js** (v18 or higher)
   - Check: `node --version`
   - Download: https://nodejs.org/

2. **Rust** (latest stable)
   - Check: `rustc --version`
   - Install: https://rustup.rs/
   - Run: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`

3. **System Dependencies**

   **macOS:**
   ```bash
   xcode-select --install
   ```

   **Linux (Ubuntu/Debian):**
   ```bash
   sudo apt-get update
   sudo apt-get install -y build-essential libssl-dev libgtk-3-dev libwebkit2gtk-4.0-dev
   ```

   **Windows:**
   - Install Visual Studio Build Tools
   - Download: https://visualstudio.microsoft.com/downloads/

---

## Step 1: Initial Setup

### 1.1 Navigate to Project Directory
```bash
cd /Users/apple/Projects/local-mind
```

### 1.2 Install Frontend Dependencies
```bash
npm install
```
Expected output: Should complete without errors. This installs React, TypeScript, Vite, and Tauri dependencies.

**If you see errors:**
- Try `npm cache clean --force` and run again
- Check Node.js version is 18+

### 1.3 Verify Rust Installation
```bash
rustc --version
cargo --version
```
Both commands should show version numbers.

---

## Step 2: First Build

### 2.1 Build Rust Backend (First Time)
```bash
cd src-tauri
cargo build
```

**First build takes 5-10 minutes** - Rust compiles all dependencies.

**Expected output:**
```
Compiling local-mind v1.0.0
Finished dev [unoptimized + debuginfo] target(s)
```

**If you see errors:**
- **SQLite errors**: You may need `libsqlite3-dev` (Linux) or ensure Xcode tools are installed (macOS)
- **Linker errors**: Install system dependencies listed above
- **Permission errors**: Use `sudo` (Linux) or check macOS security settings

### 2.2 Return to Root Directory
```bash
cd ..
```

---

## Step 3: Run Development Server

### 3.1 Start Tauri Dev Mode
```bash
npm run tauri dev
```

**Expected behavior:**
1. Vite dev server starts (port 1420)
2. Rust backend compiles (if changes detected)
3. Application window opens automatically
4. Console shows: `Starting LocalMind...`

**If window doesn't open:**
- Check console for errors
- Verify port 1420 is not in use: `lsof -i :1420`
- Try closing other applications using the port

**First run notes:**
- The app window might be hidden initially (check system tray)
- Database will be created in `data/local-mind/` directory
- Embedding model will download on first use (~90MB)

---

## Step 4: Testing Features

### Test 1: Verify Application Launches

**Steps:**
1. Application window should open
2. Window should appear transparent/minimal (search window is hidden initially)
3. System tray icon should be visible (bottom-right on Windows, top-right on macOS/Linux)

**Expected result:** ✅ Application runs without crashes

**If issues:**
- Check console logs for error messages
- Verify all dependencies installed correctly
- Check system tray area for icon

---

### Test 2: Global Keyboard Shortcuts

#### Test 2.1: Register Shortcuts

**Steps:**
1. Open application (should run from `npm run tauri dev`)
2. Check console logs - should see: `Global shortcuts registered successfully`

**macOS Note:** You may need to grant accessibility permissions:
1. Go to System Preferences → Security & Privacy → Privacy → Accessibility
2. Add Terminal/IDE to allowed apps
3. Restart the app

**Expected result:** ✅ No errors in console

---

#### Test 2.2: Test Search Window Shortcut (Alt+Shift+F)

**Steps:**
1. Press `Alt+Shift+F` (on macOS: `Option+Shift+F`)
2. Search window should appear
3. Window should have search input field focused

**Expected result:** ✅ Search window appears and can type immediately

**If not working:**
- Check console for shortcut registration errors
- Verify no other app is using the shortcut
- Try restarting the application
- Check system permissions (macOS accessibility)

---

#### Test 2.3: Test Save Snippet Shortcut (Alt+Shift+C)

**Steps:**
1. Copy some text to clipboard (select text and Ctrl+C / Cmd+C)
   - Example: "This is a test snippet"
2. Press `Alt+Shift+C` (macOS: `Option+Shift+C`)
3. Check console logs - should see: `Saving snippet from clipboard`
4. Wait 1-2 seconds
5. Check for toast notification: "Snippet saved (ID: X)"

**Expected result:** ✅ 
- Console shows save message
- Toast notification appears
- No error messages

**If not working:**
- Verify clipboard has text (try pasting manually first)
- Check console for errors
- Verify database was created: `ls data/local-mind/` should show `snippets.db`

---

### Test 3: Database and Storage

#### Test 3.1: Verify Database Creation

**Steps:**
1. After first save, check if database exists:
   ```bash
   ls -la data/local-mind/
   ```

**Expected files:**
- `snippets.db` (SQLite database)
- `job_queue.db` (job queue database)
- `vectors.lance` (vector store file, JSON format)

**If files missing:**
- Check console for database errors
- Verify write permissions in project directory
- Try saving a snippet again

---

#### Test 3.2: Verify Snippet Storage

**Steps:**
1. Save a snippet using Alt+Shift+C
2. Wait for toast notification
3. Open search window (Alt+Shift+F)
4. Type part of the snippet text in search
5. Press Enter or wait for search

**Expected result:** ✅ Snippet appears in search results

**Alternative test (using SQLite CLI):**
```bash
sqlite3 data/local-mind/snippets.db "SELECT id, content FROM snippets LIMIT 5;"
```
Should show saved snippets.

---

### Test 4: Search Functionality

#### Test 4.1: Keyword Search (FTS5)

**Steps:**
1. Save 3-4 different snippets:
   - "Python function for sorting"
   - "JavaScript async await pattern"
   - "Rust memory safety explanation"
2. Open search (Alt+Shift+F)
3. Type "Python" in search box
4. Wait for results (should be instant)

**Expected result:** ✅
- Results appear immediately (< 100ms)
- "Python function for sorting" appears
- Results show match type as "keyword"

**Verify in console:**
- Should see: `Keyword search (placeholder)` or similar
- No errors

---

#### Test 4.2: Search Loading State

**Steps:**
1. Type a search query
2. Observe loading indicator appears
3. Wait for results

**Expected result:** ✅
- Spinner appears during search
- "Searching..." message shown
- Smooth transition to results

---

#### Test 4.3: Empty Search Results

**Steps:**
1. Search for something that doesn't exist: "xyzabc123nonexistent"
2. Wait for search to complete

**Expected result:** ✅
- "No results found" message appears
- No error messages
- UI remains responsive

---

#### Test 4.4: Semantic Search (Once Embeddings Ready)

**Note:** Semantic search requires embeddings to be generated (background process)

**Steps:**
1. Save a snippet: "Machine learning algorithm for classification"
2. Wait 10-15 seconds (embedding generation happens in background)
3. Search for: "AI classification method"
4. Check results

**Expected result:** ✅
- Results include semantic matches (not just keyword)
- Match type shows "semantic" for some results
- Combined results show both keyword and semantic matches

**To verify embedding was created:**
- Check console: Should see "Embedding model loaded" after first semantic search
- Check `data/local-mind/vectors.lance` file size (should increase)

---

### Test 5: Job Queue System

#### Test 5.1: Background Embedding

**Steps:**
1. Save a new snippet
2. Immediately check console logs
3. Should see job queued: `Job queued for snippet X`
4. Wait 10-15 seconds
5. Check console - should see: `Successfully embedded snippet X`

**Expected result:** ✅
- Job queued immediately (non-blocking)
- Embedding completes in background
- No UI freezing

**Verify job status:**
```bash
sqlite3 data/local-mind/job_queue.db "SELECT snippet_id, status FROM job_queue ORDER BY created_at DESC LIMIT 5;"
```
Should show jobs with status "completed" or "pending"

---

#### Test 5.2: Crash Recovery

**Steps:**
1. Save a snippet
2. Immediately close application (before embedding completes)
3. Restart application
4. Check console logs on startup

**Expected result:** ✅
- Should see: "Recovering incomplete jobs"
- Pending embedding jobs resume automatically
- No data loss

---

### Test 6: Error Handling

#### Test 6.1: Empty Clipboard

**Steps:**
1. Clear clipboard (select nothing)
2. Press Alt+Shift+C
3. Check console

**Expected result:** ✅
- Console shows: "Clipboard is empty, skipping save"
- No crash
- No error toast

---

#### Test 6.2: Invalid Search

**Steps:**
1. Open search window
2. Type special characters: `"; DROP TABLE snippets; --`
3. Search

**Expected result:** ✅
- No SQL injection
- Query is sanitized
- Search completes safely

**Verify:** Check console for sanitized query log

---

### Test 7: UI/UX Features

#### Test 7.1: Toast Notifications

**Steps:**
1. Save a snippet (Alt+Shift+C)
2. Observe top-right corner

**Expected result:** ✅
- Green toast appears: "Snippet saved (ID: X)"
- Toast auto-dismisses after 3 seconds
- Smooth fade animation

---

#### Test 7.2: Keyboard Navigation

**Steps:**
1. Open search window
2. Type in search box
3. Press `Escape` key

**Expected result:** ✅
- Window closes immediately
- No errors

---

#### Test 7.3: Window Focus

**Steps:**
1. Open search window
2. Click outside window
3. Press Alt+Shift+F again

**Expected result:** ✅
- Window shows/hides correctly
- Focus returns to search input

---

## Step 5: Advanced Testing

### Test Embedding Model Download

**Steps:**
1. Delete embedding cache (if exists):
   ```bash
   rm -rf ~/.cache/fastembed  # Linux/macOS
   # Or locate fastembed cache directory
   ```
2. Trigger first semantic search
3. Observe console logs

**Expected result:** ✅
- Model downloads (~90MB)
- Progress shown in console
- First embedding takes 10-15 seconds
- Subsequent embeddings are faster

---

### Test Performance

#### Large Dataset Test

**Steps:**
1. Save 50+ snippets (you can automate with a script)
2. Perform searches
3. Measure response times

**Expected results:**
- Keyword search: < 100ms even with 1000 snippets
- Semantic search: < 2 seconds with model loaded
- UI remains responsive

---

### Test Cross-Platform

**If testing on different OS:**

**macOS specific:**
- Verify accessibility permissions
- Test Command+Shift shortcuts (if configured)
- Check system tray icon

**Windows specific:**
- Verify Visual Studio Build Tools installed
- Test Alt+Shift shortcuts work
- Check taskbar icon

**Linux specific:**
- Verify GTK dependencies
- Test window manager compatibility
- Check system tray (may vary by desktop environment)

---

## Troubleshooting

### Common Issues

#### Issue: "Failed to register shortcuts"

**Solution:**
- **macOS**: Grant accessibility permissions in System Preferences
- **Linux**: Ensure window manager supports global shortcuts
- **Windows**: Check antivirus isn't blocking
- Try running as administrator (Windows)

---

#### Issue: "Database not initialized"

**Solution:**
```bash
# Check if data directory exists
ls -la data/local-mind/

# If missing, create it
mkdir -p data/local-mind

# Check permissions
chmod 755 data/local-mind
```

---

#### Issue: "Embedding model failed to load"

**Solution:**
- Check internet connection (first download)
- Verify disk space available (~100MB needed)
- Check console for specific error
- Try deleting cache and re-downloading:
  ```bash
  rm -rf ~/.cache/fastembed
  ```

---

#### Issue: "Window doesn't appear"

**Solution:**
- Check system tray for icon
- Verify window isn't off-screen (reset window position)
- Check console for errors
- Try: `npm run tauri dev` with `--debug` flag

---

#### Issue: "Port 1420 already in use"

**Solution:**
```bash
# Find process using port
lsof -i :1420  # macOS/Linux
netstat -ano | findstr :1420  # Windows

# Kill process or change port in vite.config.ts
```

---

## Verification Checklist

After testing, verify:

- [ ] Application launches without errors
- [ ] Global shortcuts register successfully
- [ ] Search window opens/closes correctly
- [ ] Snippets save successfully
- [ ] Toast notifications appear
- [ ] Keyword search works instantly
- [ ] Semantic search works (after embeddings generated)
- [ ] Loading indicators show during operations
- [ ] Error handling works (empty clipboard, invalid queries)
- [ ] Database files created correctly
- [ ] Job queue processes embeddings in background
- [ ] Crash recovery works (pending jobs resume)
- [ ] Window keyboard navigation works (Escape closes)
- [ ] Multiple snippets can be saved
- [ ] Search results display correctly
- [ ] No memory leaks after extended use

---

## Performance Benchmarks

Expected performance targets:

| Operation | Target | Notes |
|-----------|--------|-------|
| Save snippet | < 15ms | Instant save to SQLite |
| Keyword search | < 100ms | FTS5 search even with 10k snippets |
| Semantic search | < 2s | After model loaded |
| Embedding generation | 5-15s | Background process, doesn't block UI |
| First token (Phase 2) | < 5s | When LLM integrated |
| Window show/hide | < 50ms | Instant response |

---

## Next Steps After Testing

1. **Report Issues**: Note any bugs or unexpected behavior
2. **Performance Notes**: Document any performance issues
3. **Platform Notes**: Document platform-specific behavior
4. **Feature Requests**: Note any missing features

---

## Quick Test Script

For rapid testing, save this as `test.sh`:

```bash
#!/bin/bash
echo "Testing LocalMind..."
echo "1. Starting dev server..."
npm run tauri dev &
TAURI_PID=$!

sleep 10
echo "2. Application should be running"
echo "3. Test shortcuts: Alt+Shift+F (search), Alt+Shift+C (save)"
echo "4. Press Ctrl+C to stop"

wait $TAURI_PID
```

---

## Support

If you encounter issues not covered here:
1. Check console logs for error messages
2. Verify all prerequisites are installed
3. Check `CURRENT_STATUS.md` for known limitations
4. Review `TECHNICAL_DOCUMENTATION.md` for architecture details

Happy Testing! 🚀
