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
               shortcut_save, shortcut_search, run_in_background
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
        INSERT INTO settings (id, theme, enable_semantic_search, enable_search_analytics,
                             shortcut_save, shortcut_search, run_in_background)
        VALUES (1, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            theme = excluded.theme,
            enable_semantic_search = excluded.enable_semantic_search,
            enable_search_analytics = excluded.enable_search_analytics,
            shortcut_save = excluded.shortcut_save,
            shortcut_search = excluded.shortcut_search,
            run_in_background = excluded.run_in_background
        "#,
    )
    .bind(theme_str)
    .bind(settings.enable_semantic_search)
    .bind(settings.enable_search_analytics)
    .bind(&settings.shortcut_save)
    .bind(&settings.shortcut_search)
    .bind(settings.run_in_background)
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
