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
    manager.register(&save_shortcut, {
        let app = app.clone();
        move || {
            if let Err(e) = handle_save_shortcut(app.clone()) {
                error!("Failed to handle save shortcut: {}", e);
            }
        }
    })?;
    info!("Registered save shortcut: {}", save_shortcut);

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
        let job_queue: tauri::State<'_, PersistentJobQueue> = app_clone.state();

        match commands::save_snippet(content.clone(), source_app, metadata, job_queue).await {
            Ok(snippet_id) => {
                info!("✓✓✓ Snippet saved successfully with ID: {}", snippet_id.id);
                info!("Saved content verification - Original: {} chars, Snippet ID: {}", 
                      content.len(), snippet_id.id);
                // Emit event to frontend via window
                if let Some(window) = app_clone.get_window("main") {
                    let _ = window.emit("snippet-saved", snippet_id);
                }
            }
            Err(e) => {
                error!("✗✗✗ Failed to save snippet: {}", e);
                if let Some(window) = app_clone.get_window("main") {
                    let _ = window.emit("snippet-save-error", e.to_string());
                }
            }
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
