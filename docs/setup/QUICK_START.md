# 🚀 LocalMind Quick Start Guide

## 📋 Keyboard Shortcuts

### macOS
- **Save Snippet from Clipboard**: `Option + Shift + C`
  - Copy any text first, then press this shortcut
  - The text will be saved automatically
  
- **Open Search Window**: `Option + Shift + F`
  - Opens the search interface to find saved snippets

- **Close Search Window**: Press `Escape` when search window is open

- **Terminal Command Picker**: `Ctrl + R` (in terminal)
  - Shows dropdown of saved commands
  - Type to filter, select with arrow keys, press Enter to execute
  - Requires terminal monitoring to be enabled

### Windows/Linux
- **Save Snippet**: `Alt + Shift + C`
- **Open Search**: `Alt + Shift + F`
- **Close**: `Escape`
- **Terminal Command Picker**: `Ctrl + R` (in terminal)

## 🔍 Where to See Console Logs

### Step 1: Open DevTools
Press **`Cmd + Option + I`** (macOS) or **`F12`** (Windows/Linux)

**Alternative methods:**
- Right-click in the app window → **"Inspect"** or **"Inspect Element"**
- Menu: **View → Developer → Toggle Developer Tools** (if available)

### Step 2: Open Console Tab
In the DevTools window that opens, click the **"Console"** tab at the top.

### Step 3: See Your Logs
You'll see structured logs like:
```
[INFO] main.tsx: Application starting
[DEBUG] App: Tauri window API loaded
[App] MOUNT - Component mounted
[SearchWindow] RENDER - Render #1
```

## 📝 How to Use LocalMind

### Save a Snippet
1. **Copy any text** to your clipboard (Cmd+C)
2. Press **`Option + Shift + C`** 
3. You'll see a toast notification: "Snippet saved (ID: X)"
4. The text is now saved in your local database

### Search Snippets
1. Press **`Option + Shift + F`** to open search
2. Type your search query
3. Results appear instantly (keyword search)
4. Semantic search results appear below (if embeddings are ready)

### Use Terminal Command Picker
1. **Enable Terminal Monitoring** in Settings → Monitoring
2. **Install Shell Hooks** (one-time setup)
3. **Run some commands** in your terminal (they'll be saved automatically)
4. **Press `Ctrl+R`** in your terminal
5. **Type to filter** saved commands
6. **Select a command** with arrow keys
7. **Press Enter** to execute the selected command

### Example Workflow
```
1. Copy code snippet from Stack Overflow
   → Press Option+Shift+C
   → ✅ Snippet saved!

2. Later, want to find it
   → Press Option+Shift+F
   → Type "stack overflow code"
   → See results instantly

3. In terminal, run: docker build api-v2
   → Command automatically saved

4. Later, want to run it again
   → Press Ctrl+R in terminal
   → Type "docker build"
   → Select command and press Enter
   → ✅ Command executed!
```

## 🐛 Debugging

If something doesn't work:
1. **Open DevTools** (`Cmd + Option + I`)
2. **Check Console tab** for errors
3. **Check Terminal** (where you ran `npm run tauri dev`) for Rust backend errors

## 💡 Tips

- The app runs in the background - check the system tray icon
- Snippets are saved instantly, but embeddings process in the background
- Press `Escape` to close the search window
- DevTools logs show what's happening in real-time

