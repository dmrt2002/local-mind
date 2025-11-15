use anyhow::Result;
use log::{error, info, warn};
use tauri::{AppHandle, GlobalShortcutManager, Manager};
use once_cell::sync::Lazy;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Register global keyboard shortcuts from settings
pub fn register_shortcuts(app: &AppHandle) -> Result<()> {
    // Get shortcuts from cached settings (already loaded at startup)
    let settings = crate::settings::get_cached_settings()
        .ok_or_else(|| anyhow::anyhow!("Settings not initialized"))?;

    let mut manager = app.global_shortcut_manager();

    // Unregister all existing shortcuts first
    manager.unregister_all()?;

    info!("Re-registering global shortcuts...");

    // Register save shortcut
    let save_shortcut = settings.shortcut_save.clone();
    info!("🔑 Attempting to register save shortcut: '{}'", save_shortcut);

    match manager.register(&save_shortcut, {
        let app = app.clone();
        move || {
            info!("🎯 SAVE SHORTCUT TRIGGERED!");
            if let Err(e) = handle_save_shortcut(app.clone()) {
                error!("Failed to handle save shortcut: {}", e);
            }
        }
    }) {
        Ok(_) => info!("✅ Successfully registered save shortcut: {}", save_shortcut),
        Err(e) => {
            error!("❌ Failed to register save shortcut '{}': {}", save_shortcut, e);
            return Err(e.into());
        }
    }

    // Register search shortcut
    let search_shortcut = settings.shortcut_search.clone();
    manager.register(&search_shortcut, {
        let app = app.clone();
        move || {
            if let Err(e) = handle_search_shortcut(app.clone()) {
                error!("Failed to handle search shortcut: {}", e);
            }
        }
    })?;
    info!("Registered search shortcut: {}", search_shortcut);

    // Register spotlight shortcut (if enabled)
    if settings.spotlight_enabled {
        let spotlight_shortcut = settings.spotlight_shortcut.clone();
        match manager.register(&spotlight_shortcut, {
            let app = app.clone();
            move || {
                if let Err(e) = handle_spotlight_shortcut(app.clone()) {
                    error!("Failed to handle spotlight shortcut: {}", e);
                }
            }
        }) {
            Ok(_) => info!("✅ Registered spotlight shortcut: {}", spotlight_shortcut),
            Err(e) => {
                error!("❌ Failed to register spotlight shortcut '{}': {}", spotlight_shortcut, e);
                // Don't fail registration if spotlight shortcut fails
            }
        }
    }

    info!("✅ Global shortcuts registered successfully");
    Ok(())
}

/// Handle Alt+Shift+C (save snippet)
static LAST_SAVE_GUARD: Lazy<Mutex<(Option<String>, Option<Instant>)>> =
    Lazy::new(|| Mutex::new((None, None)));

fn handle_save_shortcut(app: AppHandle) -> Result<()> {
    use crate::clipboard;
    use crate::commands;
    use crate::job_queue::PersistentJobQueue;

    // Read clipboard
    let content = clipboard::read_clipboard(&app)
        .map_err(|e| anyhow::anyhow!("Failed to read clipboard: {}", e))?;

    if content.trim().is_empty() {
        info!("Clipboard is empty, skipping save");
        return Ok(());
    }

    // Debounce/guard: skip duplicate rapid triggers with identical content
    {
        let mut guard = LAST_SAVE_GUARD.lock().unwrap();
        let now = Instant::now();
        let debounce_window = Duration::from_millis(800);

        if let (Some(last_content), Some(last_time)) = (&guard.0, guard.1) {
            if *last_content == content && now.duration_since(last_time) < debounce_window {
                warn!(
                    "⚠️  Save shortcut ignored by debounce (same content within {:?})",
                    debounce_window
                );
                return Ok(());
            }
        }
        guard.0 = Some(content.clone());
        guard.1 = Some(now);
    }

    info!("=== SAVE SNIPPET TRIGGERED ===");
    info!("Clipboard content length: {} chars", content.len());
    info!("Clipboard content preview: {}", 
          if content.len() > 200 { 
              format!("{}...", &content[..200])
          } else { 
              content.clone()
          });
    info!("Clipboard content (first 500 chars): {}", 
          if content.len() > 500 { 
              format!("{}...", &content[..500])
          } else { 
              content.clone()
          });

    // Get source app (platform-specific)
    let source_app = clipboard::get_source_app();
    info!("Source app: {:?}", source_app);

    // Get browser metadata (URL, title) if source is a browser
    let metadata = if let Some(ref app) = source_app {
        clipboard::get_browser_metadata(app)
    } else {
        None
    };
    info!("Browser metadata: {:?}", metadata);

    // Save snippet (async call)
    let app_clone = app.clone();
    tokio::spawn(async move {
        use crate::db::sqlite;
        use std::sync::Arc;

        // Get job queue from app state (managed as Arc<PersistentJobQueue>)
        let job_queue_arc: tauri::State<'_, Arc<PersistentJobQueue>> = app_clone.state();

        // Save to SQLite immediately (instant keyword search)
        let id = match sqlite::save_snippet(content.clone(), source_app, metadata).await {
            Ok(id) => id,
            Err(e) => {
                error!("✗✗✗ Failed to save snippet: {}", e);
                if let Some(window) = app_clone.get_window("main") {
                    let _ = window.emit("snippet-save-error", e.to_string());
                }
                return;
            }
        };

        // Get the snippet to retrieve its summary
        let snippet = match sqlite::get_snippet(id).await {
            Ok(Some(s)) => s,
            Ok(None) => {
                error!("✗✗✗ Snippet {} not found after saving", id);
                return;
            }
            Err(e) => {
                error!("✗✗✗ Failed to retrieve snippet: {}", e);
                return;
            }
        };

        // Queue embedding job (background processing) with summary
        if let Err(e) = job_queue_arc.push(id, content.clone(), snippet.summary, crate::job_queue::Priority::Normal).await {
            error!("✗✗✗ Failed to queue embedding job: {}", e);
            return;
        }

        info!("✓✓✓ Snippet saved successfully with ID: {}", id);
        info!("Saved content verification - Original: {} chars, Snippet ID: {}",
              content.len(), id);

        // Emit event to frontend via window
        if let Some(window) = app_clone.get_window("main") {
            let _ = window.emit("snippet-saved", commands::SnippetId { id });
        }
    });

    Ok(())
}

/// Handle Alt+Shift+F (search window)
fn handle_search_shortcut(app: AppHandle) -> Result<()> {
    let window = app
        .get_window("main")
        .ok_or_else(|| anyhow::anyhow!("Main window not found"))?;

    if window.is_visible()? {
        window.hide()?;
        info!("Search window hidden");
    } else {
        window.show()?;
        window.set_focus()?;
        info!("Search window shown");
    }

    Ok(())
}

/// Handle Spotlight shortcut (Ctrl+Space on macOS, Alt+Space on others)
fn handle_spotlight_shortcut(app: AppHandle) -> Result<()> {
    info!("🔍 Spotlight shortcut pressed");
    
    // Get or create spotlight window
    let spotlight_window = match app.get_window("spotlight") {
        Some(window) => {
            info!("✅ Spotlight window found (label: spotlight)");
            window
        }
        None => {
            error!("❌ Spotlight window not found - this should not happen!");
            return Err(anyhow::anyhow!("Spotlight window not found"));
        }
    };

    // CRITICAL: Explicitly hide main window if it's visible when spotlight opens
    // This prevents the app window from appearing when spotlight shortcut is pressed
    if let Some(main_window) = app.get_window("main") {
        if let Ok(is_visible) = main_window.is_visible() {
            if is_visible {
                warn!("⚠️  Main window is visible when spotlight opened - hiding it");
                if let Err(e) = main_window.hide() {
                    error!("Failed to hide main window: {}", e);
                } else {
                    info!("✅ Main window hidden to prevent interference with spotlight");
                }
            } else {
                info!("✅ Main window is already hidden");
            }
        }
    }

    if spotlight_window.is_visible()? {
        // Hide spotlight window
        spotlight_window.hide()?;
        info!("✅ Spotlight window hidden");
    } else {
        // Show and focus spotlight window only
        info!("📂 Showing spotlight window (should load spotlight.html)");

        // DEBUG: Check window state before showing
        info!("🔧 DEBUG - Window state BEFORE show:");
        info!("   - Visible: {:?}", spotlight_window.is_visible());
        info!("   - Outer size: {:?}", spotlight_window.outer_size());
        info!("   - Position: {:?}", spotlight_window.outer_position());
        info!("   - Is minimized: {:?}", spotlight_window.is_minimized());
        info!("   - Is maximized: {:?}", spotlight_window.is_maximized());

        spotlight_window.show()?;

        // DEBUG: Check window state after showing
        info!("🔧 DEBUG - Window state AFTER show:");
        info!("   - Visible: {:?}", spotlight_window.is_visible());
        info!("   - Focused: {:?}", spotlight_window.is_focused());

        spotlight_window.set_focus()?;

        // DEBUG: Check window state after focus
        info!("🔧 DEBUG - Window state AFTER set_focus:");
        info!("   - Focused: {:?}", spotlight_window.is_focused());

        // Emit event to focus search input
        spotlight_window.emit("spotlight-focus", ())?;
        info!("✅ Spotlight window shown and focused");
        info!("   Expected content: SpotlightSearch component (from spotlight.html)");
    }

    Ok(())
}
