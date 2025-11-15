use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub theme: Theme,
    pub enable_semantic_search: bool,
    pub enable_search_analytics: bool,
    pub shortcut_save: String,
    pub shortcut_search: String,
    pub run_in_background: bool,
    // Terminal monitoring settings
    pub terminal_monitoring_enabled: bool,
    pub terminal_blocklist: String,
    pub terminal_allowlist: String,
    pub terminal_min_length: i32,
    pub shell_type: String,
    // Command picker settings
    pub command_picker_enabled: bool,
    pub command_picker_shortcut: String, // e.g., "Alt+C" or "Ctrl+R"
    // Spotlight search settings
    pub spotlight_enabled: bool,
    pub spotlight_shortcut: String, // e.g., "Cmd+Space" or "Alt+Space"
    // Screenshot monitoring settings
    pub screenshot_monitoring_enabled: bool,
    pub screenshot_directory: String,
    pub screenshot_ocr_enabled: bool,
    pub screenshot_caption_enabled: bool,
    pub visual_search_enabled: bool,
    // OCR engine settings
    pub ocr_engine: String, // "auto", "apple_vision", "tesseract"
    pub ocr_recognition_level: String, // "fast", "accurate" (for Apple Vision)
    pub ocr_cleaning_level: String, // "minimal", "balanced", "aggressive"
    pub tesseract_psm_mode: i32, // Page segmentation mode (3 = automatic)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::Light,
            enable_semantic_search: true,
            enable_search_analytics: true,
            shortcut_save: "Alt+Shift+C".to_string(),
            shortcut_search: "Alt+Shift+F".to_string(),
            run_in_background: true,
            terminal_monitoring_enabled: false,
            terminal_blocklist: "ls,cd,pwd,clear,exit,history,echo,cat,which,type".to_string(),
            terminal_allowlist: "docker,git,kubectl,npm,cargo,python,ffmpeg,curl,aws,gcloud,az,terraform,ansible,ssh,scp,rsync".to_string(),
            terminal_min_length: 60,
            shell_type: "zsh".to_string(),
            command_picker_enabled: true,
            command_picker_shortcut: "Ctrl+R".to_string(),
            spotlight_enabled: true,
            spotlight_shortcut: {
                #[cfg(target_os = "macos")]
                {
                    "Ctrl+Space".to_string()
                }
                #[cfg(not(target_os = "macos"))]
                {
                    "Alt+Space".to_string()
                }
            },
            screenshot_monitoring_enabled: true,
            screenshot_directory: String::from(
                std::env::var("HOME")
                    .map(|h| format!("{}/Desktop", h))
                    .unwrap_or_else(|_| String::from("~/Desktop"))
                    .as_str()
            ),  // Default to Desktop for screenshots
            screenshot_ocr_enabled: true,
            screenshot_caption_enabled: true,
            visual_search_enabled: false,
            ocr_engine: "auto".to_string(),
            ocr_recognition_level: "accurate".to_string(),
            ocr_cleaning_level: "balanced".to_string(),
            tesseract_psm_mode: 3, // Automatic segmentation
        }
    }
}

pub static SETTINGS_CACHE: Lazy<Arc<Mutex<Option<Settings>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

/// Load settings from database
pub async fn load_settings(pool: &SqlitePool) -> Result<Settings> {
    let row = sqlx::query(
        r#"
        SELECT theme, enable_semantic_search, enable_search_analytics,
               shortcut_save, shortcut_search, run_in_background,
               terminal_monitoring_enabled, terminal_blocklist, terminal_allowlist,
               terminal_min_length, shell_type,
               COALESCE(command_picker_enabled, 1) as command_picker_enabled,
               COALESCE(command_picker_shortcut, 'Ctrl+R') as command_picker_shortcut,
               COALESCE(spotlight_enabled, 1) as spotlight_enabled,
               COALESCE(spotlight_shortcut, 'Ctrl+Space') as spotlight_shortcut,
               screenshot_monitoring_enabled, screenshot_directory,
               screenshot_ocr_enabled, screenshot_caption_enabled, visual_search_enabled,
               COALESCE(ocr_engine, 'auto') as ocr_engine,
               COALESCE(ocr_recognition_level, 'accurate') as ocr_recognition_level,
               COALESCE(ocr_cleaning_level, 'balanced') as ocr_cleaning_level,
               COALESCE(tesseract_psm_mode, 3) as tesseract_psm_mode
        FROM settings
        WHERE id = 1
        "#,
    )
    .fetch_optional(pool)
    .await
    .context("Failed to query settings")?;

    let settings = if let Some(row) = row {
        let theme_str: String = row.get(0);
        let theme = match theme_str.as_str() {
            "dark" => Theme::Dark,
            _ => Theme::Light,
        };

        Settings {
            theme,
            enable_semantic_search: row.get(1),
            enable_search_analytics: row.get(2),
            shortcut_save: row.get(3),
            shortcut_search: row.get(4),
            run_in_background: row.get(5),
            terminal_monitoring_enabled: row.get(6),
            terminal_blocklist: row.get(7),
            terminal_allowlist: row.get(8),
            terminal_min_length: row.get(9),
            shell_type: row.get(10),
            command_picker_enabled: row.get(11),
            command_picker_shortcut: row.get(12),
            spotlight_enabled: row.get(13),
            spotlight_shortcut: row.get(14),
            screenshot_monitoring_enabled: row.get(15),
            screenshot_directory: row.get(16),
            screenshot_ocr_enabled: row.get(17),
            screenshot_caption_enabled: row.get(18),
            visual_search_enabled: row.get(19),
            ocr_engine: row.get(20),
            ocr_recognition_level: row.get(21),
            ocr_cleaning_level: row.get(22),
            tesseract_psm_mode: row.get(23),
        }
    } else {
        // No settings exist, create default
        let default_settings = Settings::default();
        save_settings(pool, &default_settings).await?;
        default_settings
    };

    // Update cache
    if let Ok(mut cache) = SETTINGS_CACHE.lock() {
        *cache = Some(settings.clone());
    }

    Ok(settings)
}

/// Save settings to database
pub async fn save_settings(pool: &SqlitePool, settings: &Settings) -> Result<()> {
    let theme_str = match settings.theme {
        Theme::Light => "light",
        Theme::Dark => "dark",
    };

    sqlx::query(
        r#"
        INSERT INTO settings (
            id, theme, enable_semantic_search, enable_search_analytics,
            shortcut_save, shortcut_search, run_in_background,
            terminal_monitoring_enabled, terminal_blocklist, terminal_allowlist,
            terminal_min_length, shell_type,
            command_picker_enabled, command_picker_shortcut,
            spotlight_enabled, spotlight_shortcut,
            screenshot_monitoring_enabled, screenshot_directory,
            screenshot_ocr_enabled, screenshot_caption_enabled, visual_search_enabled,
            ocr_engine, ocr_recognition_level, ocr_cleaning_level, tesseract_psm_mode
        )
        VALUES (1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            theme = excluded.theme,
            enable_semantic_search = excluded.enable_semantic_search,
            enable_search_analytics = excluded.enable_search_analytics,
            shortcut_save = excluded.shortcut_save,
            shortcut_search = excluded.shortcut_search,
            run_in_background = excluded.run_in_background,
            terminal_monitoring_enabled = excluded.terminal_monitoring_enabled,
            terminal_blocklist = excluded.terminal_blocklist,
            terminal_allowlist = excluded.terminal_allowlist,
            terminal_min_length = excluded.terminal_min_length,
            shell_type = excluded.shell_type,
            command_picker_enabled = excluded.command_picker_enabled,
            command_picker_shortcut = excluded.command_picker_shortcut,
            spotlight_enabled = excluded.spotlight_enabled,
            spotlight_shortcut = excluded.spotlight_shortcut,
            screenshot_monitoring_enabled = excluded.screenshot_monitoring_enabled,
            screenshot_directory = excluded.screenshot_directory,
            screenshot_ocr_enabled = excluded.screenshot_ocr_enabled,
            screenshot_caption_enabled = excluded.screenshot_caption_enabled,
            visual_search_enabled = excluded.visual_search_enabled,
            ocr_engine = excluded.ocr_engine,
            ocr_recognition_level = excluded.ocr_recognition_level,
            ocr_cleaning_level = excluded.ocr_cleaning_level,
            tesseract_psm_mode = excluded.tesseract_psm_mode
        "#,
    )
    .bind(theme_str)
    .bind(settings.enable_semantic_search)
    .bind(settings.enable_search_analytics)
    .bind(&settings.shortcut_save)
    .bind(&settings.shortcut_search)
    .bind(settings.run_in_background)
    .bind(settings.terminal_monitoring_enabled)
    .bind(&settings.terminal_blocklist)
    .bind(&settings.terminal_allowlist)
    .bind(settings.terminal_min_length)
    .bind(&settings.shell_type)
    .bind(settings.command_picker_enabled)
    .bind(&settings.command_picker_shortcut)
    .bind(settings.spotlight_enabled)
    .bind(&settings.spotlight_shortcut)
    .bind(settings.screenshot_monitoring_enabled)
    .bind(&settings.screenshot_directory)
    .bind(settings.screenshot_ocr_enabled)
    .bind(settings.screenshot_caption_enabled)
    .bind(settings.visual_search_enabled)
    .bind(&settings.ocr_engine)
    .bind(&settings.ocr_recognition_level)
    .bind(&settings.ocr_cleaning_level)
    .bind(settings.tesseract_psm_mode)
    .execute(pool)
    .await
    .context("Failed to save settings")?;

    // Update cache
    if let Ok(mut cache) = SETTINGS_CACHE.lock() {
        *cache = Some(settings.clone());
    }

    Ok(())
}

/// Get cached settings (fast, no database query)
pub fn get_cached_settings() -> Option<Settings> {
    SETTINGS_CACHE.lock().ok().and_then(|cache| cache.clone())
}

/// Initialize settings on startup
pub async fn init_settings(pool: &SqlitePool) -> Result<()> {
    log::info!("Initializing settings...");
    load_settings(pool).await?;
    log::info!("Settings loaded successfully");
    Ok(())
}
