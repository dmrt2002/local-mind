// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use log::{error, info, warn};
use std::sync::Arc;
use tauri::{Manager, SystemTray, SystemTrayEvent, SystemTrayMenu};

mod analytics;
mod app_state;
mod clipboard;
mod commands;
mod db;
mod dedup;
mod embedding;
mod export;
mod inference;
mod job_queue;
mod monitors;
mod processing;
mod rag;
mod search;
mod settings;
mod shortcuts;
mod suggestions;

use analytics::search_analytics::SearchAnalytics;
use app_state::AppState;
use job_queue::PersistentJobQueue;

/// Copy model files from source to cache directory (one-time operation)
fn copy_model_to_cache(source: &std::path::Path, target: &std::path::Path) -> std::io::Result<()> {
    // Create target directory
    std::fs::create_dir_all(target)?;

    // List of required files to copy
    let files_to_copy = vec![
        "config.json",
        "model.onnx",
        "tokenizer.json",
        "tokenizer_config.json",
        "vocab.txt",
    ];

    for file_name in files_to_copy {
        let source_file = source.join(file_name);
        let target_file = target.join(file_name);

        if source_file.exists() {
            info!("Copying {}...", file_name);
            std::fs::copy(&source_file, &target_file)?;
        } else {
            warn!("File not found in source: {}", file_name);
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    // Initialize logger FIRST before any logging
    env_logger::init();
    info!("Starting LocalMind...");

    // CRITICAL: Set XDG_CACHE_HOME and setup symlink in USER'S CACHE (not source directory!)
    // This tells fastembed where to find local models (no downloads!)
    // IMPORTANT: We use ~/.cache/fastembed/ NOT src-tauri/models/ to avoid triggering rebuilds
    if let Ok(local_model_path) = embedding::model_setup::get_local_model_path() {
        // Get the REAL cache directory (~/.cache on Unix)
        let cache_home = std::env::var("XDG_CACHE_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::var("HOME")
                    .ok()
                    .map(std::path::PathBuf::from)
                    .map(|h| h.join(".cache"))
                    .unwrap_or_else(|| std::path::PathBuf::from(".cache"))
            });

        let fastembed_cache = cache_home
            .join("fastembed")
            .join("sentence-transformers_all-MiniLM-L6-v2");

        info!("Setting up fastembed cache: {:?}", fastembed_cache);
        info!("Local model path: {:?}", local_model_path);

        // Create parent directory
        if let Some(parent) = fastembed_cache.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                warn!("Failed to create cache parent: {}", e);
            }
        }

        // Remove old cache if exists (must be completely removed before copying)
        if fastembed_cache.exists() {
            info!("Removing old cache at: {:?}", fastembed_cache);
            // Try as file first (symlink), then as directory
            if std::fs::remove_file(&fastembed_cache).is_err() {
                if let Err(e) = std::fs::remove_dir_all(&fastembed_cache) {
                    warn!("Failed to remove old cache: {}", e);
                }
            }
        }

        // CRITICAL: fastembed may not follow symlinks properly, so we COPY files
        // This is a one-time operation - files are only copied if cache doesn't exist
        info!("📦 Copying model files to cache (one-time operation)...");
        if let Err(e) = copy_model_to_cache(&local_model_path, &fastembed_cache) {
            error!("❌ Failed to copy model to cache: {}", e);
            error!("   Source: {:?}", local_model_path);
            error!("   Target: {:?}", fastembed_cache);
        } else {
            info!("✅ Model files copied to cache successfully");

            // Verify files are accessible
            let test_file = fastembed_cache.join("model.onnx");
            if test_file.exists() {
                if let Ok(metadata) = std::fs::metadata(&test_file) {
                    info!("✅ Verified model.onnx in cache: {} bytes", metadata.len());
                }
            } else {
                warn!("⚠️  model.onnx not found after copy");
            }
        }

        // Set XDG_CACHE_HOME to point to the actual cache location
        // fastembed will use this
        std::env::set_var("XDG_CACHE_HOME", cache_home.to_string_lossy().to_string());
        info!("Set XDG_CACHE_HOME to: {:?}", cache_home);
    } else {
        warn!("Local model not found - fastembed will try to download (may fail)");
    }

    // Initialize database (migrations will handle hash recalculation)
    if let Err(e) = db::sqlite::init_database().await {
        error!("Failed to initialize database: {}", e);
        std::process::exit(1);
    }

    // Initialize settings (loads into cache)
    match db::sqlite::get_pool().await {
        Ok(pool) => {
            if let Err(e) = settings::load_settings(&pool).await {
                error!("Failed to load settings: {}", e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            error!("Failed to get database pool for settings: {}", e);
            std::process::exit(1);
        }
    }

    // Initialize search analytics
    let search_analytics = match db::sqlite::get_pool().await {
        Ok(pool) => match SearchAnalytics::new(pool).await {
            Ok(analytics) => {
                // Clean up old analytics data (older than 30 days)
                if let Err(e) = analytics.cleanup_old_data(30).await {
                    warn!("Failed to cleanup old analytics: {}", e);
                }
                analytics
            }
            Err(e) => {
                error!("Failed to initialize search analytics: {}", e);
                std::process::exit(1);
            }
        },
        Err(e) => {
            error!("Failed to get database pool for analytics: {}", e);
            std::process::exit(1);
        }
    };

    // Initialize LanceDB
    if let Err(e) = db::lancedb::init_lancedb().await {
        error!("Failed to initialize LanceDB: {}", e);
        // Continue anyway - semantic search will be unavailable
    } else {
        // Clean up orphaned embeddings on startup
        if let Err(e) = db::lancedb::cleanup_orphaned_embeddings().await {
            warn!("Failed to cleanup orphaned embeddings: {}", e);
        }
    }

    // Initialize job queue
    let mut job_queue = match PersistentJobQueue::new().await {
        Ok(queue) => queue,
        Err(e) => {
            error!("Failed to initialize job queue: {}", e);
            error!("App cannot continue without job queue. Exiting.");
            std::process::exit(1);
        }
    };

    // Recover incomplete jobs before starting worker
    // This is non-critical - if it fails, we just continue without recovering old jobs
    if let Err(e) = job_queue.recover_on_startup().await {
        warn!("⚠️  Failed to recover jobs on startup (non-critical): {}", e);
        warn!("⚠️  App will continue normally, but pending jobs from previous session may not be recovered");
    }

    // Clean up old completed jobs (older than 7 days)
    if let Err(e) = job_queue.cleanup_old_jobs(7).await {
        warn!("Failed to cleanup old jobs: {}", e);
    }

    // Initialize LLM manager BEFORE starting worker (so worker can use it)
    let llm_manager = match inference::llm_manager::get_default_model_path() {
        Ok(model_path) => {
            info!("LLM model path: {:?}", model_path);
            if model_path.exists() {
                info!("✅ LLM model found, initializing manager");
                inference::llm_manager::LlmManager::new(model_path)
            } else {
                warn!("⚠️  LLM model not found at {:?}", model_path);
                warn!("   LLM categorization will not be available");
                inference::llm_manager::LlmManager::new(model_path)
            }
        }
        Err(e) => {
            warn!("Failed to get LLM model path: {}", e);
            warn!("   LLM categorization will not be available");
            // Create with dummy path - will fail when trying to load
            inference::llm_manager::LlmManager::new(std::path::PathBuf::from("model_not_found.gguf"))
        }
    };

    // Initialize global LLM for use in background workers (BEFORE starting worker!)
    inference::global_llm::init_global_llm(llm_manager.clone());

    // Start worker (NOW that LLM is initialized and available)
    if let Err(e) = job_queue.start_worker() {
        error!("Failed to start job queue worker: {}", e);
        error!("App cannot continue without worker. Exiting.");
        std::process::exit(1);
    }

    // Initialize app state
    let app_state = AppState::detect_initial_state().await;

    // Create Arc for monitoring systems (job_queue is moved below)
    let job_queue_arc = Arc::new(job_queue);
    {
        let job_queue_clone = job_queue_arc.clone();

        // Load settings to check if monitoring is enabled
        match db::sqlite::get_pool().await {
            Ok(pool) => {
                match settings::load_settings(&pool).await {
                    Ok(settings) => {
                        // Start terminal monitoring if enabled
                        if settings.terminal_monitoring_enabled {
                            info!("🔄 Terminal monitoring enabled, starting monitor...");

                            let blocklist: Vec<String> = settings.terminal_blocklist
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .collect();
                            let allowlist: Vec<String> = settings.terminal_allowlist
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .collect();

                            let filter = monitors::terminal::CommandFilter::new(
                                blocklist,
                                allowlist,
                                settings.terminal_min_length as usize,
                            );

                            match monitors::terminal::TerminalMonitor::new(
                                job_queue_clone.clone(),
                                filter,
                            ) {
                                Ok(monitor) => {
                                    tokio::spawn(async move {
                                        if let Err(e) = monitor.start_monitoring().await {
                                            error!("Terminal monitoring failed: {}", e);
                                        }
                                    });

                                    info!("✅ Terminal monitoring started");
                                }
                                Err(e) => {
                                    warn!("⚠️  Failed to initialize terminal monitor: {}", e);
                                    warn!("   Shell hooks may not be installed. Run 'install_shell_hooks' command.");
                                }
                            }
                        } else {
                            info!("ℹ️  Terminal monitoring disabled in settings");
                        }

                        // Start screenshot monitoring if enabled
                        if settings.screenshot_monitoring_enabled {
                            info!("🔄 Screenshot monitoring enabled, starting monitor...");

                            // Setup Florence-2 if caption generation is enabled
                            if settings.screenshot_caption_enabled {
                                if !processing::vision::is_florence2_downloaded() {
                                    info!("📥 Setting up Florence-2 vision model for screenshot captions...");
                                    if let Err(e) = processing::vision::download_florence2_model().await {
                                        warn!("Failed to setup Florence-2: {}", e);
                                        warn!("   Screenshot captions will use placeholder text.");
                                        warn!("   Install Python dependencies: pip install transformers torch pillow");
                                    } else {
                                        info!("✅ Florence-2 setup complete");
                                    }
                                }
                            }

                            let screenshot_dir = if settings.screenshot_directory.is_empty() {
                                monitors::screenshots::get_default_screenshot_dir()
                            } else {
                                Ok(std::path::PathBuf::from(&settings.screenshot_directory))
                            };

                            match screenshot_dir {
                                Ok(dir) => {
                                    if dir.exists() {
                                        let monitor = monitors::screenshots::ScreenshotMonitor::new(
                                            dir.clone(),
                                            job_queue_clone.clone(),
                                        );

                                        tokio::spawn(async move {
                                            if let Err(e) = monitor.start_monitoring().await {
                                                error!("Screenshot monitoring failed: {}", e);
                                            }
                                        });

                                        info!("✅ Screenshot monitoring started for: {}", dir.display());
                                    } else {
                                        warn!("⚠️  Screenshot directory not found: {}", dir.display());
                                        warn!("   Please configure a valid screenshot directory in settings.");
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to determine screenshot directory: {}", e);
                                }
                            }
                        } else {
                            info!("ℹ️  Screenshot monitoring disabled in settings");
                        }
                    }
                    Err(e) => {
                        warn!("Failed to load settings for monitoring: {}", e);
                        warn!("Monitoring systems will not start automatically.");
                    }
                }
            }
            Err(e) => {
                warn!("Failed to get database pool for monitoring: {}", e);
            }
        }
    }

    // Start background task to auto-unload idle models (memory optimization)
    {
        let llm_manager_bg = llm_manager.clone();
        let embedding_engine = embedding::engine::EmbeddingEngine::new();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60)); // Check every minute
            loop {
                interval.tick().await;

                // Check and unload LLM if idle
                if llm_manager_bg.unload_if_idle() {
                    info!("💾 Memory optimization: LLM model auto-unloaded (940 MB freed)");
                }

                // Check and unload embedding model if idle
                if embedding_engine.unload_if_idle() {
                    info!("💾 Memory optimization: Embedding model auto-unloaded (86 MB freed)");
                }
            }
        });
        info!("🔄 Started background task for auto-unloading idle models");
    }

    // Create system tray
    let tray_menu = SystemTrayMenu::new();
    let system_tray = SystemTray::new().with_menu(tray_menu);

    // Build Tauri app
    tauri::Builder::default()
        // TODO: Re-enable single instance plugin once compatible version is found
        // .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        //     if let Some(window) = app.get_window("main") {
        //         let _ = window.show();
        //         let _ = window.set_focus();
        //     }
        // }))
        .system_tray(system_tray)
        .on_system_tray_event(|app, event| match event {
            SystemTrayEvent::LeftClick { .. } => {
                if let Some(window) = app.get_window("main") {
                    if let Ok(visible) = window.is_visible() {
                        if visible {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                }
            }
            SystemTrayEvent::RightClick { .. } => {
                // Show context menu if needed
            }
            _ => {}
        })
        .manage(job_queue_arc)
        .manage(app_state)
        .manage(search_analytics)
        .manage(llm_manager)
        .invoke_handler(tauri::generate_handler![
            commands::save_snippet,
            commands::get_snippet,
            commands::edit_snippet,
            commands::get_snippet_versions,
            commands::delete_snippet,
            commands::search,
            commands::get_search_stats,
            commands::get_source_apps,
            // Category management
            commands::create_category,
            commands::get_categories,
            commands::get_root_categories,
            commands::get_child_categories,
            commands::update_category,
            commands::delete_category,
            commands::delete_all_data,
            commands::assign_snippet_to_category,
            commands::get_snippets_by_category,
            commands::get_snippet_category,
            commands::get_categorization_reasoning,
            // Auto-categorization
            commands::auto_categorize_snippet,
            commands::auto_categorize_batch,
            commands::calculate_category_centroid,
            commands::clear_centroid_cache,
            // Utility commands
            commands::regenerate_all_summaries,
            // Settings commands
            commands::get_settings,
            commands::update_settings,
            commands::get_storage_stats,
            commands::get_app_memory_usage,
            // Export/Import commands
            commands::export_to_json_file,
            commands::export_to_markdown_file,
            commands::import_from_json_file,
            commands::get_export_history,
            // Duplicate detection commands
            commands::check_duplicate,
            commands::find_all_duplicates,
            commands::get_duplicate_stats,
            commands::merge_duplicate_group,
            commands::update_all_content_hashes,
            commands::recalculate_command_hashes,
            // Smart suggestions commands
            commands::get_suggestions,
            commands::find_related_snippets,
            commands::dismiss_suggestion,
            // Terminal monitoring commands
            commands::detect_shell,
            commands::install_shell_hooks,
            commands::uninstall_shell_hooks,
            commands::are_shell_hooks_installed,
            commands::get_terminal_log_path,
            // Screenshot monitoring commands
            commands::get_default_screenshot_dir,
            commands::is_tesseract_installed,
            commands::is_apple_vision_available,
            commands::get_apple_architecture,
            commands::is_florence2_downloaded,
            commands::download_florence2_model,
            commands::search_text_in_database,
            commands::rescan_screenshots,
            commands::check_screenshot_status,
            commands::get_uncategorized_screenshots,
        ])
        .setup(|app| {
            // Register global shortcuts
            let app_handle = app.handle();
            if let Err(e) = shortcuts::register_shortcuts(&app_handle) {
                error!("Failed to register shortcuts: {}", e);
            }

            #[cfg(debug_assertions)]
            {
                if let Some(window) = app.get_window("main") {
                    window.show().unwrap();
                    window.set_focus().unwrap();
                }
            }

            Ok(())
        })
        .on_window_event(|event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event.event() {
                // Get cached settings (fast, no async needed)
                use crate::settings::SETTINGS_CACHE;

                let run_in_background = SETTINGS_CACHE
                    .try_lock()
                    .ok()
                    .and_then(|guard| guard.as_ref().map(|s| s.run_in_background))
                    .unwrap_or(true); // Default to background mode

                if run_in_background {
                    // Don't quit, just hide the window
                    event.window().hide().unwrap();
                    api.prevent_close();
                    info!("Window hidden (running in background)");
                }
                // If run_in_background is false, allow normal close behavior
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
