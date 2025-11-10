use once_cell::sync::Lazy;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

static LAST_CLIPBOARD_CONTENT: Lazy<Arc<Mutex<Option<String>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

/// Start monitoring clipboard for changes
pub fn start_monitoring(app: tauri::AppHandle) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(500));

        loop {
            interval.tick().await;

            if let Err(e) = check_clipboard(app.clone()).await {
                log::error!("Clipboard check error: {}", e);
            }
        }
    });
}

async fn check_clipboard(_app: tauri::AppHandle) -> anyhow::Result<()> {
    // Read clipboard - using a simple approach for now
    // In production, use platform-specific clipboard APIs
    let current = read_clipboard_platform().unwrap_or_default();

    let mut last_content = LAST_CLIPBOARD_CONTENT.lock().await;

    // Check if content changed
    if let Some(ref last) = *last_content {
        if last != &current && !current.is_empty() {
            // Content changed - could trigger save if shortcut pressed
            // The actual save is triggered by the global shortcut handler
        }
    }

    *last_content = Some(current);
    Ok(())
}

/// Read current clipboard content (platform-specific)
fn read_clipboard_platform() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("pbpaste")
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        Command::new("xclip")
            .arg("-selection")
            .arg("clipboard")
            .arg("-o")
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
    }

    #[cfg(target_os = "windows")]
    {
        // For Windows, we'd need a different approach
        // This is a placeholder
        None
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

/// Read current clipboard content
pub fn read_clipboard(_app: &tauri::AppHandle) -> Result<String, String> {
    read_clipboard_platform().ok_or_else(|| "Failed to read clipboard".to_string())
}

/// Get browser URL and title from the active tab (macOS only for now)
pub fn get_browser_metadata(app_name: &str) -> Option<serde_json::Value> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        use serde_json::json;

        // Check if it's a supported browser
        let (browser_name, script) = match app_name {
            "Google Chrome" | "Chromium" => ("Chrome", r#"
                tell application "Google Chrome"
                    if (count of windows) > 0 then
                        set activeTab to active tab of front window
                        set tabURL to URL of activeTab
                        set tabTitle to title of activeTab
                        return tabURL & "|||||" & tabTitle
                    end if
                end tell
            "#),
            "Safari" => ("Safari", r#"
                tell application "Safari"
                    if (count of windows) > 0 then
                        set currentURL to URL of current tab of front window
                        set currentTitle to name of current tab of front window
                        return currentURL & "|||||" & currentTitle
                    end if
                end tell
            "#),
            "Firefox" => {
                // Firefox doesn't support AppleScript well, skip for now
                log::debug!("Firefox metadata extraction not supported yet");
                return None;
            }
            _ => return None,
        };

        log::debug!("Attempting to get {} metadata", browser_name);

        let output = Command::new("osascript")
            .arg("-e")
            .arg(script)
            .output()
            .ok()?;

        if output.status.success() {
            let result = String::from_utf8(output.stdout).ok()?;
            let parts: Vec<&str> = result.trim().split("|||||").collect();

            if parts.len() == 2 {
                let url = parts[0].trim();
                let title = parts[1].trim();

                // Check for about:blank - common with Gmail iframes/chat windows
                if url == "about:blank" {
                    log::warn!("⚠️  Detected about:blank URL (likely iframe/popup)");
                    log::info!("   Title: {}", title);

                    // Try to infer actual URL from title
                    let inferred_url = if title.contains("Gmail") || title.contains("Google Mail") || title.contains("mail.google.com") {
                        Some("https://mail.google.com")
                    } else if title.contains("Google Docs") {
                        Some("https://docs.google.com")
                    } else if title.contains("Google Drive") {
                        Some("https://drive.google.com")
                    } else if title.contains("YouTube") {
                        Some("https://youtube.com")
                    } else {
                        None
                    };

                    if let Some(inferred) = inferred_url {
                        log::info!("✅ Inferred URL from title: {}", inferred);
                        return Some(json!({
                            "url": inferred,
                            "title": title,
                            "browser": browser_name
                        }));
                    } else {
                        log::warn!("⚠️  Could not infer URL from title, skipping metadata");
                        return None;
                    }
                }

                if !url.is_empty() {
                    log::info!("📋 Captured browser metadata:");
                    log::info!("   URL: {}", url);
                    log::info!("   Title: {}", title);

                    return Some(json!({
                        "url": url,
                        "title": title,
                        "browser": browser_name
                    }));
                }
            }
        }
        None
    }

    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

/// Get the application name that owns the clipboard (platform-specific)
pub fn get_source_app() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        // Use AppleScript to get the frontmost application
        let output = Command::new("osascript")
            .arg("-e")
            .arg("tell application \"System Events\" to get name of first application process whose frontmost is true")
            .output()
            .ok()?;

        if output.status.success() {
            let app_name = String::from_utf8(output.stdout)
                .ok()?
                .trim()
                .to_string();

            if !app_name.is_empty() {
                log::debug!("Detected source app: {}", app_name);
                return Some(app_name);
            }
        }
        None
    }

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;

        // Try xdotool first
        if let Ok(output) = Command::new("xdotool")
            .arg("getactivewindow")
            .arg("getwindowname")
            .output()
        {
            if output.status.success() {
                if let Ok(window_name) = String::from_utf8(output.stdout) {
                    let trimmed = window_name.trim();
                    if !trimmed.is_empty() {
                        log::debug!("Detected source app (Linux): {}", trimmed);
                        return Some(trimmed.to_string());
                    }
                }
            }
        }

        // Try wmctrl as fallback
        if let Ok(output) = Command::new("wmctrl")
            .arg("-a")
            .arg(":ACTIVE:")
            .output()
        {
            if output.status.success() {
                if let Ok(window_info) = String::from_utf8(output.stdout) {
                    // Parse wmctrl output to get app name
                    if let Some(app) = window_info.split_whitespace().last() {
                        log::debug!("Detected source app (Linux): {}", app);
                        return Some(app.to_string());
                    }
                }
            }
        }

        None
    }

    #[cfg(target_os = "windows")]
    {
        use std::process::Command;

        // Use PowerShell to get active window
        let output = Command::new("powershell")
            .arg("-Command")
            .arg("(Get-Process | Where-Object {$_.MainWindowHandle -eq (Get-Process | Where-Object {$_.MainWindowTitle}).MainWindowHandle} | Select-Object -First 1).ProcessName")
            .output()
            .ok()?;

        if output.status.success() {
            if let Ok(process_name) = String::from_utf8(output.stdout) {
                let trimmed = process_name.trim();
                if !trimmed.is_empty() {
                    log::debug!("Detected source app (Windows): {}", trimmed);
                    return Some(trimmed.to_string());
                }
            }
        }

        None
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}
