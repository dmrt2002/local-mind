# LocalMind

**100% Local Cognitive Context Assistant** - A privacy-first AI assistant that runs entirely on your machine.

## 🎯 Overview

LocalMind helps you save, search, and query your clipboard snippets using:
- **Instant Keyword Search** (FTS5) - Find exact matches instantly
- **Semantic Search** - Find similar content by meaning
- **Local AI Q&A** (Phase 2) - Ask questions about your snippets using a local LLM

**All processing happens on your device. No data ever leaves your computer.**

## ✨ Features

### Core Functionality
- ✅ **Instant Clipboard Capture** - Press `Alt+Shift+C` to save any text
- ✅ **Dual Search** - Fast keyword + intelligent semantic search
- ✅ **Category Organization** - Hierarchical folder structure for snippets
- ✅ **Snippet Editing** - Edit content with version history
- ✅ **Search Navigation** - Click results to jump to snippet location
- ✅ **100% Private** - Everything runs locally
- ✅ **Low-Spec Friendly** - Designed for 4GB RAM machines

### Advanced Features
- ✅ **Command Palette** (Ctrl+K) - Fuzzy search all actions
- ✅ **Analytics Dashboard** - View usage patterns and search stats
- ✅ **Export/Backup** - Export to JSON or Markdown, import backups
- ✅ **Duplicate Detection** - SHA-256 hash-based deduplication
- ✅ **Smart Suggestions** - AI-powered recommendations
- ✅ **Context Capture** - Display URLs, file paths, window titles
- ⏳ **Local AI Q&A** - Coming in Phase 2

## 🚀 Quick Start

### Prerequisites

- **Node.js** 18+ ([Download](https://nodejs.org/))
- **Rust** ([Install](https://rustup.rs/))
- **System Dependencies**:
  - macOS: Xcode Command Line Tools
  - Linux: `build-essential`, `libssl-dev`, `libgtk-3-dev`

### Installation & Build

```bash
# 1. Install dependencies
npm install

# 2. Build Rust backend (first time: 5-10 minutes)
cd src-tauri
cargo build
cd ..

# 3. Run in development mode
npm run tauri dev
```

### Quick Test

1. **Run the app**: `npm run tauri dev`
2. **Save a snippet**: Copy text, press `Alt+Shift+C` (or `Option+Shift+C` on macOS)
3. **Search**: Press `Alt+Shift+F`, type your query
4. **See results**: Instant keyword matches appear

## 📚 Documentation

All documentation is now organized in the [`docs/`](./docs/) folder:

- **[Quick Start Guide](./docs/setup/QUICK_START.md)** - Get started in 5 minutes
- **[Technical Documentation](./docs/TECHNICAL_DOCUMENTATION.md)** - Complete technical reference
- **[Testing Guide](./docs/TESTING_GUIDE.md)** - Testing procedures
- **[User Flow Documentation](./docs/USER_FLOW_DOCUMENTATION.md)** - UI and user experience guide
- **[Setup Guides](./docs/setup/)** - Model setup, icons, and configuration
- **[Development Guides](./docs/development/)** - Logging, migrations, debugging

## 🏗️ Project Structure

```
local-mind/
├── src/                    # React frontend
│   ├── components/        # UI components
│   └── hooks/             # React hooks
├── src-tauri/             # Rust backend
│   ├── src/
│   │   ├── db/           # SQLite + vector storage
│   │   ├── embedding/    # Embedding engine
│   │   ├── inference/    # LLM (Phase 2)
│   │   └── ...
│   └── Cargo.toml
├── data/                  # Runtime data (created automatically)
│   ├── snippets.db       # SQLite database
│   └── vectors.lance     # Vector store
└── models/                # LLM models (Phase 2)
```

## 🎮 Usage

### Keyboard Shortcuts

#### Global Shortcuts
- **`Alt+Shift+C`** (macOS: `Option+Shift+C`) - Save clipboard snippet
- **`Alt+Shift+F`** (macOS: `Option+Shift+F`) - Open search window

#### In-App Navigation
- **`Ctrl+K`** (macOS: `Cmd+K`) - Open Command Palette
- **`Ctrl+1`** - Switch to Home tab
- **`Ctrl+2`** - Switch to Search tab
- **`Ctrl+3`** - Switch to Analytics tab
- **`Ctrl+4`** - Switch to Settings tab
- **`Escape`** - Close modals/search window
- **`↑↓`** - Navigate search results
- **`Enter`** - Open selected snippet

### Basic Workflow

1. **Save snippets**: Copy text → Press `Alt+Shift+C`
2. **Organize**: Create categories in Home tab → Drag/assign snippets
3. **Search**: Press `Alt+Shift+F` → Type query → Click result
4. **Navigate**: Clicking search results takes you to snippet location in Home tab
5. **Edit**: Select snippet → Click edit button → Make changes → Save
6. **Export**: Go to Settings → Export to JSON or Markdown

## 📋 Current Status

### Phase 1: MVP ✅ COMPLETE

- ✅ Project setup and configuration
- ✅ Database with FTS5 keyword search
- ✅ Vector store for semantic search
- ✅ Global shortcuts and clipboard monitoring
- ✅ Search UI with combined results
- ✅ Job queue for background processing
- ✅ Toast notifications and loading states
- ✅ Category organization system
- ✅ Snippet editing with version history
- ✅ Tab-based navigation (Home, Search, Analytics, Settings)

### Phase 1.5: Features & Polish ✅ COMPLETE

- ✅ Export/Backup (JSON, Markdown)
- ✅ Analytics Dashboard (search stats, insights)
- ✅ Command Palette (Ctrl+K fuzzy search)
- ✅ Duplicate Detection (SHA-256 based)
- ✅ Smart Suggestions (AI-powered)
- ✅ Enhanced Context Capture (URLs, file paths)
- ✅ Search Result Navigation

### Known Limitations

- ⚠️ **Semantic search**: Uses placeholder embeddings (returns empty until fastembed integrated)
- ⚠️ **Windows clipboard**: Not yet implemented (macOS/Linux work)
- ⏳ **AI Q&A**: Phase 2 feature (requires model download)

### Phase 2: AI Brain ⏳ UPCOMING

- LLM integration (TinyLlama)
- RAG pipeline
- Token streaming
- Chat UI

## 🔧 Development

### Build Commands

```bash
# Development
npm run tauri dev

# Production build
npm run tauri build
```

### Testing

See [Testing Guide](./docs/TESTING_GUIDE.md) for comprehensive testing instructions.

### Adding the LLM Model

When ready for Phase 2, see [Model Setup Guide](./docs/setup/MODEL_SETUP.md) for instructions.

## 📖 Technical Details

- **Frontend**: React + TypeScript + Vite
- **Backend**: Rust + Tauri
- **Database**: SQLite with FTS5
- **Vector Store**: Custom JSON-based (LanceDB-ready)
- **Embeddings**: fastembed (placeholder currently)
- **LLM**: llama.cpp with TinyLlama (Phase 2)

## 🤝 Contributing

This is a development project. For issues or questions, refer to:
- [Testing Guide](./docs/TESTING_GUIDE.md) - How to test
- [Technical Documentation](./docs/TECHNICAL_DOCUMENTATION.md) - Architecture details
- [Documentation Index](./docs/README.md) - Complete documentation overview

## 📝 License

MIT

---

**Version**: 1.5.0 (Phase 1 Complete + All Features)
**Status**: Production Ready 🚀
**Last Updated**: November 2, 2025