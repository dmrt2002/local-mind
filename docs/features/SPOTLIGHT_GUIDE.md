# Spotlight Search Guide

**Version:** 1.0  
**Last Updated:** January 2025  
**Status:** Production Ready

---

## Overview

Spotlight Search is a macOS Spotlight-inspired quick search interface that allows users to instantly search and access their LocalMind snippets, commands, and screenshots from anywhere in the system. It provides a fast, keyboard-driven search experience without opening the main application window.

---

## User Guide

### Opening Spotlight

**Keyboard Shortcut:**
- **macOS**: `Ctrl + Space` (default)
- **Windows/Linux**: `Alt + Space` (default)

The shortcut can be customized in Settings → Keyboard Shortcuts.

### Using Spotlight

1. **Press the keyboard shortcut** to open Spotlight
2. **Type your search query** - results appear as you type
3. **Navigate results** with arrow keys (↑↓)
4. **Select a result**:
   - **Enter**: Copy content to clipboard and close Spotlight
   - **Cmd/Ctrl + Enter**: Open result in main app window
   - **Click**: Single-click to copy, double-click to open in app
5. **Close Spotlight**:
   - **ESC key**: Close without selecting
   - **Click outside**: Click on the dark overlay to close

### Keyboard Navigation

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate through results |
| `Home` | Jump to first result |
| `End` | Jump to last result |
| `Enter` | Copy selected result to clipboard |
| `Cmd/Ctrl + Enter` | Open selected result in main app |
| `Cmd/Ctrl + C` | Copy selected result (when result is selected) |
| `ESC` | Close Spotlight |

### Features

- **Fast Search**: Instant keyword and semantic search results
- **Minimal UI**: Clean, distraction-free interface
- **System-wide Access**: Works from any application
- **Smart Results**: Shows up to 8 most relevant results
- **Type Indicators**: Visual icons for snippets, commands, and screenshots
- **Match Type Display**: Shows whether result matched via keyword or semantic search

---

## Technical Implementation

### Architecture

Spotlight is implemented as a separate Tauri window that loads a dedicated React application. It uses the same search backend as the main application but provides a streamlined, overlay-style interface.

### Key Components

#### Frontend

**Files:**
- `src/components/SpotlightSearch.tsx` - Main React component
- `src/components/SpotlightSearch.css` - Styling (macOS Spotlight-inspired)
- `src/spotlight.tsx` - Entry point for spotlight window
- `spotlight.html` - HTML template for spotlight window

**Component Structure:**
```
SpotlightSearch
├── Container (backdrop + click handler)
└── Window (search box + results)
    ├── SearchBox (input + icon)
    ├── Results (list of search results)
    └── EmptyState (when no query or no results)
```

#### Backend

**Files:**
- `src-tauri/src/shortcuts.rs` - Global shortcut registration and handler
- `src-tauri/src/main.rs` - Window creation and setup

**Window Configuration:**
- **Label**: `spotlight`
- **URL**: `spotlight.html`
- **Position**: Fixed, centered on screen
- **Size**: Fullscreen (covers entire viewport)
- **Transparency**: Enabled (for backdrop blur effect)
- **Always on top**: Yes
- **Decorations**: None (borderless)
- **Resizable**: No

### Window Management

The spotlight window is created at application startup and kept hidden until the shortcut is pressed. This ensures instant opening without delay.

**Window Lifecycle:**
1. Created during app setup (hidden)
2. Shown when shortcut pressed
3. Hidden when ESC pressed or backdrop clicked
4. Reused on subsequent opens (no recreation needed)

### Search Integration

Spotlight uses the same `search` Tauri command as the main application:
- **Keyword search**: Instant results from SQLite
- **Semantic search**: Results from LanceDB embeddings
- **Combined results**: Merged and ranked by relevance
- **Limit**: 8 results maximum for minimal display

### Styling

The spotlight interface uses:
- **Fixed positioning**: `position: fixed` with `top: 50%; left: 50%; transform: translate(-50%, -50%)` for perfect centering
- **Backdrop blur**: `backdrop-filter: blur(20px)` for macOS-like effect
- **Semi-transparent overlay**: Dark background with 40-60% opacity
- **Responsive design**: Adapts to light/dark themes
- **Animation**: Smooth appear animation (0.2s cubic-bezier)

---

## Configuration

### Settings

Spotlight can be configured in Settings → Keyboard Shortcuts:

- **Enable/Disable**: Toggle spotlight feature
- **Shortcut**: Customize keyboard shortcut (default: `Ctrl+Space` on macOS, `Alt+Space` on others)

### Database Settings

Settings are stored in the `settings` table:
- `spotlight_enabled` (BOOLEAN): Whether spotlight is enabled
- `spotlight_shortcut` (TEXT): Keyboard shortcut string

---

## Implementation Details

### Dynamic Import Pattern

The spotlight component uses dynamic imports for Tauri APIs to avoid module loading errors:

```typescript
async function getCurrentWindow() {
  const module = await import("@tauri-apps/api/window");
  return module.getCurrentWindow();
}
```

This pattern ensures the component works even if Tauri APIs are not immediately available.

### Event Handling

**Backdrop Click:**
- Container div handles clicks
- Window div stops propagation to prevent closing when clicking on results
- Simple check: `target === currentTarget` to detect backdrop clicks

**Keyboard Events:**
- Global event listener on window
- ESC key closes window
- Arrow keys navigate results
- Enter key selects result

### Error Handling

- All Tauri API calls wrapped in try-catch
- Graceful fallbacks if APIs fail
- Console logging for debugging
- Error boundaries prevent crashes

---

## Troubleshooting

### Spotlight doesn't open

**Symptoms:** Pressing shortcut does nothing

**Solutions:**
1. Check if spotlight is enabled in Settings
2. Verify shortcut is registered (check console logs)
3. Check for conflicting system shortcuts
4. Restart the application

### Blank screen when opening

**Symptoms:** Dark overlay appears but no search box

**Solutions:**
1. Check browser console for JavaScript errors
2. Verify `spotlight.html` is loading correctly
3. Check if React component is mounting (look for console logs)
4. Verify CSS is loading (`SpotlightSearch.css`)

### Results not appearing

**Symptoms:** Can type but no results show

**Solutions:**
1. Check if search command is working (test in main app)
2. Verify database connection
3. Check console for search errors
4. Ensure embeddings are generated (for semantic search)

### Window not centered

**Symptoms:** Search box appears off-center

**Solutions:**
1. Check CSS for `position: fixed` and `transform: translate(-50%, -50%)`
2. Verify no conflicting styles
3. Check viewport dimensions

### Can't close with backdrop click

**Symptoms:** Clicking outside doesn't close

**Solutions:**
1. Verify click handler is attached to container
2. Check if window click handler stops propagation correctly
3. Test ESC key as alternative

---

## Development Notes

### Adding New Features

To extend spotlight functionality:

1. **Add UI elements** in `SpotlightSearch.tsx`
2. **Update styles** in `SpotlightSearch.css`
3. **Add keyboard shortcuts** in the keyboard navigation useEffect
4. **Test with different window sizes** and themes

### Testing

**Manual Testing Checklist:**
- [ ] Spotlight opens with keyboard shortcut
- [ ] Search results appear as you type
- [ ] Arrow keys navigate results
- [ ] Enter copies result to clipboard
- [ ] ESC closes spotlight
- [ ] Backdrop click closes spotlight
- [ ] Window is centered on screen
- [ ] Works in both light and dark themes
- [ ] Handles empty search gracefully
- [ ] Shows "no results" message appropriately

### Performance Considerations

- **Debounce**: 120ms debounce on search input for responsive feel
- **Result limit**: Maximum 8 results to keep UI minimal
- **Window reuse**: Window is created once and reused
- **Lazy loading**: Tauri APIs loaded dynamically

---

## Future Enhancements

Potential improvements for future versions:

- **Recent searches**: Show recently searched terms
- **Search history**: Remember past searches
- **Quick actions**: Direct actions from spotlight (edit, delete, etc.)
- **Fuzzy matching**: Better typo tolerance
- **Search suggestions**: Autocomplete suggestions
- **Result preview**: Show more content in results
- **Keyboard shortcuts display**: Show available shortcuts in UI

---

## Related Documentation

- [Search Features](SEARCH_FEATURES.md) - General search functionality
- [Search Techniques](SEARCH_TECHNIQUES.md) - Advanced search tips
- [Quick Start Guide](../setup/QUICK_START.md) - Getting started with LocalMind
- [Technical Documentation](../TECHNICAL_DOCUMENTATION.md) - Complete technical reference

---

**Last Updated:** January 2025

