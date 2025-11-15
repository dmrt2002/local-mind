# LocalMind Monitoring Guide

Complete guide to LocalMind's terminal command and screenshot monitoring features.

## Overview

LocalMind can automatically monitor and index:
- **Terminal Commands** - Shell command history with context
- **Screenshots** - Images with OCR text extraction and AI captions

All monitoring is **100% local** with no external API calls or data transmission.

---

## Terminal Command Monitoring

### Features

- ✅ Automatic capture of shell commands
- ✅ Records working directory and exit codes
- ✅ Intelligent filtering (blocklist/allowlist/heuristics)
- ✅ Multi-shell support (Zsh, Bash, Fish)
- ✅ Time-window deduplication
- ✅ Searchable via keyword and semantic search
- ✅ Auto-categorization

### Supported Shells

| Shell | Status | Hook Method |
|-------|--------|-------------|
| **Zsh** | ✅ Full Support | `preexec` + `precmd` |
| **Bash** | ✅ Full Support | `PROMPT_COMMAND` + `trap DEBUG` |
| **Fish** | ✅ Full Support | `fish_preexec` + `fish_postexec` |

### Setup Instructions

#### 1. Enable in Settings

1. Open LocalMind Settings (Settings tab)
2. Scroll to "Monitoring" section
3. Toggle "Enable Terminal Monitoring"
4. Save settings

#### 2. Install Shell Hooks

Terminal monitoring requires shell hook installation. Choose your shell:

**For Zsh (default on macOS):**
```bash
# Hooks will be added to ~/.zshrc
```

**For Bash:**
```bash
# Hooks will be added to ~/.bashrc
```

**For Fish:**
```bash
# Hooks will be added to ~/.config/fish/config.fish
```

**Installation Command:**
- Open LocalMind
- Navigate to Settings → Monitoring
- Click "Install Shell Hooks" button
- Restart your terminal

#### 3. Verify Installation

Check if hooks are installed:
```bash
# Zsh
cat ~/.zshrc | grep "localmind"

# Bash
cat ~/.bashrc | grep "localmind"

# Fish
cat ~/.config/fish/config.fish | grep "localmind"
```

You should see hook functions that log to `~/.localmind/terminal.log`

### Command Filtering

LocalMind uses a 3-tier filtering system to avoid capturing trivial commands:

#### 1. Blocklist (Skip Always)
Commands in the blocklist are never saved:
```
ls, cd, pwd, clear, exit, history, echo, cat, which, type
```

#### 2. Allowlist (Save Always)
Important commands are always saved regardless of length:
```
docker, git, kubectl, npm, cargo, python, ffmpeg, curl,
aws, gcloud, az, terraform, ansible, ssh, scp, rsync
```

#### 3. Heuristics (Intelligent Filtering)
Commands not in blocklist/allowlist are evaluated:
- ✅ **Length** - Commands >60 characters
- ✅ **Pipes** - Commands with `|` operators
- ✅ **Redirects** - Commands with `>`, `>>`, `<`
- ✅ **Sudo** - Commands starting with `sudo`

### Configuration Options

Located in **Settings → Monitoring → Terminal Commands**:

| Option | Default | Description |
|--------|---------|-------------|
| **Enabled** | Off | Master toggle for terminal monitoring |
| **Blocklist** | (see above) | Comma-separated command names to skip |
| **Allowlist** | (see above) | Comma-separated important commands |
| **Min Length** | 60 | Minimum characters for heuristic save |
| **Shell Type** | zsh | Target shell for hook installation |

### What Gets Saved

For each captured command, LocalMind stores:
```rust
{
    content: "docker-compose up -d",        // The command
    type: "command",                         // Content type
    working_directory: "/Users/me/project",  // Where it was run
    exit_code: 0,                            // Success/failure (0 = success)
    created_at: "2025-01-11T10:30:45Z",     // Timestamp
    source_app: "iTerm2",                    // Terminal application
}
```

### Deduplication

- Commands are deduplicated within a **5-minute window**
- Prevents saving repeated command execution
- Hash-based matching for exact duplicates

### Viewing Commands

1. **Home Tab** → **Commands** filter
2. Commands display shows:
   - Command text with syntax highlighting
   - Working directory
   - Exit code badge (✓ green for success, ✗ red for errors)
   - Timestamp (relative: "2h ago", "Yesterday")
   - Copy to clipboard button

---

## Screenshot Monitoring

### Features

- ✅ Automatic screenshot detection
- ✅ OCR text extraction (Tesseract)
- ✅ AI image captioning (Florence-2)
- ✅ Browser metadata capture (URL + title)
- ✅ Grid layout with thumbnails
- ✅ Full-screen lightbox viewer
- ✅ Searchable by caption and OCR text
- ✅ Optional visual similarity search (SigLIP)

### Setup Instructions

#### 1. Enable in Settings

1. Open LocalMind Settings (Settings tab)
2. Scroll to "Monitoring" section
3. Toggle "Enable Screenshot Monitoring"
4. Configure sub-options:
   - **OCR** - Extract text using Tesseract (requires installation)
   - **Captioning** - Generate descriptions using Florence-2 (model required)
5. Save settings

#### 2. Install Dependencies

**OCR (Tesseract) - Required for text extraction:**

```bash
# macOS
brew install tesseract

# Ubuntu/Debian
sudo apt-get install tesseract-ocr

# Windows
# Download from: https://github.com/UB-Mannheim/tesseract/wiki
```

**Verify installation:**
```bash
tesseract --version
```

**Vision Model (Florence-2) - Optional for captioning:**

Currently placeholder implementation. For production:

**Option 1: Python + Transformers (Simpler)**
```bash
pip install transformers torch pillow
# Model auto-downloads on first use (~465MB)
```

**Option 2: ONNX Runtime (More integrated)**
```bash
# Download from: https://huggingface.co/onnx-community/Florence-2-base
# Place in: ~/.localmind/models/florence2/
```

#### 3. Configure Screenshot Directory

**Default directories:**
- **macOS**: `~/Desktop`
- **Linux**: `~/Pictures/Screenshots` (or `~/Desktop`)
- **Windows**: `%USERPROFILE%\Pictures\Screenshots`

**Custom directory:**
1. Settings → Monitoring → Screenshots
2. Enter custom path
3. Save

### Screenshot Processing Pipeline

When a new screenshot is detected:

```
Screenshot saved → ScreenshotMonitor detects
  ↓
Extract metadata (dimensions, browser URL/title)
  ↓
Save to database (type='screenshot', file_path, etc.)
  ↓
Screenshot Processor:
  1. OCR (Tesseract) → extract text
  2. Caption (Florence-2) → generate description
  3. Combine → "[Caption: ...] [Text: ...]"
  4. Update database
  5. Queue for embedding (all-MiniLM-L6-v2)
  ↓
Searchable in LocalMind!
```

### What Gets Saved

For each screenshot, LocalMind stores:
```rust
{
    content: "[Caption: Code editor] [Text: function main() {...}]",
    summary: "Screenshot of code editor",     // Generated caption
    type: "screenshot",                        // Content type
    file_path: "/Users/me/Desktop/Screen Shot.png",
    website_url: "https://github.com/...",    // If from browser
    website_title: "GitHub Repository",       // Browser title
    created_at: "2025-01-11T10:30:45Z",
    source_app: "Google Chrome",               // Source application
}
```

### Viewing Screenshots

1. **Home Tab** → **Screenshots** filter
2. Screenshots display in grid layout showing:
   - Thumbnail preview (16:9 aspect ratio)
   - Caption/title
   - Website URL (if browser screenshot)
   - Timestamp and source app

3. **Click any screenshot** to open lightbox with:
   - Full-size image
   - Extracted OCR text (monospace)
   - Generated caption
   - Website link (opens in browser)
   - File path
   - Metadata sidebar

### Browser Metadata Capture

When taking screenshots from web browsers, LocalMind automatically captures:
- **URL** - The webpage address
- **Title** - The page title
- **Browser** - Chrome, Firefox, Safari, etc.

Supported browsers:
- Google Chrome
- Firefox
- Safari
- Microsoft Edge
- Brave
- Opera

### Configuration Options

Located in **Settings → Monitoring → Screenshots**:

| Option | Default | Description |
|--------|---------|-------------|
| **Enabled** | Off | Master toggle for screenshot monitoring |
| **Directory** | (OS default) | Path to monitor for new screenshots |
| **OCR** | On | Extract text using Tesseract |
| **Captioning** | On | Generate descriptions using Florence-2 |
| **Visual Search** | Off | Enable SigLIP for visual similarity (future) |

### Supported Image Formats

- PNG (.png)
- JPEG (.jpg, .jpeg)
- WebP (.webp)

---

## Search Integration

Both commands and screenshots are indexed for search:

### Keyword Search (FTS5)
- **Commands**: Search command text, working directory
- **Screenshots**: Search caption text, OCR text, URLs

### Semantic Search (all-MiniLM-L6-v2)
- **Commands**: Semantic meaning of commands
- **Screenshots**: Combined caption + OCR text embeddings
- Finds similar content even with different wording

### Content Type Filters

Use tabs to filter by type:
- **All** - Show everything
- **Snippets** - Text-only snippets
- **Commands** - Terminal commands
- **Screenshots** - Images

---

## Performance & Storage

### Terminal Monitoring

- **CPU**: Minimal (file polling every 2s)
- **Memory**: <1MB RAM
- **Disk**: ~200 bytes per command
- **Processing**: Instant capture, async embedding

### Screenshot Monitoring

- **CPU**: Light (file polling every 2s)
- **Memory**: ~500MB peak during processing
- **Disk**: Original file + 384-byte embedding
- **Processing**:
  - OCR: 1-3 seconds (Tesseract)
  - Caption: 2-5 seconds (Florence-2, when available)
  - Embedding: <1 second (all-MiniLM-L6-v2)

### Model Sizes

| Component | Size | Location |
|-----------|------|----------|
| Tesseract | ~10MB | System install |
| Florence-2 (INT8) | ~465MB | `~/.localmind/models/florence2/` |
| SigLIP (INT8) | ~150MB | `~/.localmind/models/siglip/` |
| Embeddings (shared) | ~86MB | Already in use |

---

## Troubleshooting

### Terminal Monitoring

**Commands not being captured:**
1. Check if shell hooks are installed: `cat ~/.zshrc | grep localmind`
2. Verify terminal log exists: `ls -la ~/.localmind/terminal.log`
3. Restart terminal after hook installation
4. Check Settings → Monitoring → Terminal is enabled

**Too many/few commands saved:**
1. Adjust Min Length setting (default: 60 characters)
2. Customize blocklist/allowlist in Settings
3. Check command filter heuristics

**Hooks conflict with existing setup:**
- Hooks are designed to coexist with other prompt customizations
- If conflicts occur, manually review `~/.zshrc` backup
- Uninstall hooks: Settings → Monitoring → "Uninstall Hooks"

### Screenshot Monitoring

**Screenshots not being detected:**
1. Check screenshot directory path in Settings
2. Verify directory exists and is readable
3. Ensure Screenshots monitoring is enabled
4. Restart LocalMind after enabling

**OCR not working:**
1. Verify Tesseract is installed: `tesseract --version`
2. Check OCR toggle in Settings → Monitoring → Screenshots
3. Review logs for Tesseract errors

**Captions not generated:**
- Florence-2 model currently in placeholder mode
- Caption will show timestamp until model is integrated
- OCR text extraction still works independently

**Browser metadata missing:**
- Only captured when screenshot is taken from browser windows
- Requires browser to be active/focused during screenshot
- Supported browsers: Chrome, Firefox, Safari, Edge, Brave, Opera

---

## Privacy & Security

### Data Collection
- ✅ **100% Local** - No data leaves your machine
- ✅ **No Analytics** - No usage tracking or telemetry
- ✅ **No Cloud** - All processing happens on-device

### Sensitive Information
Terminal monitoring **intentionally skips**:
- Environment variable exports with secrets
- Commands containing tokens/keys (based on patterns)
- Standard blocklist commands

**Best practices:**
- Review blocklist regularly
- Avoid inline secrets in commands
- Use environment files for sensitive data

### File Storage
- Terminal log: `~/.localmind/terminal.log`
- Screenshots: Original files remain unchanged
- Database: `~/.localmind/data/snippets.db`
- Embeddings: `~/.localmind/data/vectors/`

---

## API / Command Line

### Tauri Commands (Frontend ↔ Backend)

**Terminal Monitoring:**
```rust
detect_shell() -> Result<String>
install_shell_hooks() -> Result<String>
uninstall_shell_hooks() -> Result<String>
are_shell_hooks_installed() -> Result<bool>
get_terminal_log_path() -> Result<String>
```

**Screenshot Monitoring:**
```rust
get_default_screenshot_dir() -> Result<String>
is_tesseract_installed() -> bool
is_florence2_downloaded() -> bool
download_florence2_model() -> Result<String>
```

---

## Roadmap

### In Progress
- ✅ Terminal command capture (Completed)
- ✅ Screenshot OCR (Completed)
- 🚧 Florence-2 integration (Placeholder)

### Planned
- ⏳ Visual similarity search (SigLIP)
- ⏳ Command parameter extraction
- ⏳ Screenshot duplicate detection
- ⏳ Automatic command categorization
- ⏳ Terminal session tracking

---

## Related Documentation

- [Technical Documentation](../TECHNICAL_DOCUMENTATION.md) - Architecture details
- [Search Features](SEARCH_FEATURES.md) - Search system overview
- [Quick Start](../setup/QUICK_START.md) - Getting started guide
