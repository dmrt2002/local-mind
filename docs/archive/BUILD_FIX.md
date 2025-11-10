# Build Fix - Clipboard Feature

## Issue
Tauri v1.5 doesn't have a `clipboard-read` feature. Clipboard access is available through the `tauri::api::clipboard` API without requiring a feature flag.

## Fix Applied
Removed `clipboard-read` from the Tauri features list in `Cargo.toml`.

**Before:**
```toml
tauri = { version = "1.5", features = ["system-tray", "global-shortcut", "clipboard-read"] }
```

**After:**
```toml
tauri = { version = "1.5", features = ["system-tray", "global-shortcut"] }
```

## Why This Works
- Clipboard access is handled via `tauri::api::clipboard` API (no feature needed)
- Permissions are configured in `tauri.conf.json` under `allowlist.clipboard`
- The clipboard code in `src/clipboard.rs` already uses the correct API

## Next Steps
Now you should be able to build:

```bash
cd src-tauri
cargo build
```

Or from root:
```bash
npm run tauri dev
```
