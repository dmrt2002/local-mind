import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { save, open } from "@tauri-apps/api/dialog";
import { logger } from "../utils/logger";
import { useToast } from "../hooks/useToast";
import DuplicateManager from "./DuplicateManager";
import DeleteAllDataModal from "./DeleteAllDataModal";
import "./SettingsWindow.css";

interface Settings {
  theme: "light" | "dark";
  enable_semantic_search: boolean;
  enable_search_analytics: boolean;
  shortcut_save: string;
  shortcut_search: string;
  run_in_background: boolean;
  // Terminal monitoring
  terminal_monitoring_enabled: boolean;
  terminal_blocklist: string;
  terminal_allowlist: string;
  terminal_min_length: number;
  shell_type: string;
  // Command picker
  command_picker_enabled: boolean;
  command_picker_shortcut: string;
  // Spotlight search
  spotlight_enabled: boolean;
  spotlight_shortcut: string;
  // Screenshot monitoring
  screenshot_monitoring_enabled: boolean;
  screenshot_directory: string;
  screenshot_ocr_enabled: boolean;
  screenshot_caption_enabled: boolean;
  visual_search_enabled: boolean;
  // OCR settings
  ocr_engine: string;
  ocr_recognition_level: string;
  ocr_cleaning_level: string;
  tesseract_psm_mode: number;
}

interface StorageStats {
  snippets_db_size: number;
  job_queue_db_size: number;
  vectors_db_size: number;
  total_size: number;
  snippets_count: number;
  categories_count: number;
  embeddings_count: number;
}

interface MemoryStats {
  rss_mb: number;
  virtual_mb: number;
  cpu_percent: number;
}

interface ExportRecord {
  id: number;
  format: string;
  file_path: string;
  item_count: number;
  file_size_bytes: number | null;
  exported_at: string;
}

export default function SettingsWindow() {
  const { showToast } = useToast();
  const [settings, setSettings] = useState<Settings>({
    theme: "light",
    enable_semantic_search: true,
    enable_search_analytics: true,
    shortcut_save: "Alt+Shift+C",
    shortcut_search: "Alt+Shift+F",
    run_in_background: true,
    terminal_monitoring_enabled: false,
    terminal_blocklist: "ls,cd,pwd,clear,exit,history,echo,cat,which,type",
    terminal_allowlist:
      "docker,git,kubectl,npm,cargo,python,ffmpeg,curl,aws,gcloud,az,terraform,ansible,ssh,scp,rsync",
    terminal_min_length: 60,
    shell_type: "zsh",
    command_picker_enabled: true,
    command_picker_shortcut: "Alt+C",
    spotlight_enabled: true,
    spotlight_shortcut: "Ctrl+Space",
    screenshot_monitoring_enabled: false,
    screenshot_directory: "",
    screenshot_ocr_enabled: true,
    screenshot_caption_enabled: true,
    visual_search_enabled: false,
    ocr_engine: "auto",
    ocr_recognition_level: "accurate",
    ocr_cleaning_level: "balanced",
    tesseract_psm_mode: 3,
  });

  const [appleVisionAvailable, setAppleVisionAvailable] = useState(false);
  const [appleArchitecture, setAppleArchitecture] = useState<string>("");
  const [rescanning, setRescanning] = useState(false);

  // Format shortcuts for display (Mac uses Option instead of Alt)
  const formatShortcutDisplay = (shortcut: string): string => {
    return shortcut.replace(/Alt/g, "Option");
  };
  const [storageStats, setStorageStats] = useState<StorageStats | null>(null);
  const [memoryStats, setMemoryStats] = useState<MemoryStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [isDeleteModalOpen, setIsDeleteModalOpen] = useState(false);

  // Load settings on mount
  useEffect(() => {
    loadSettings();
    loadStorageStats();
    loadMemoryStats();
    checkAppleVisionAvailability();

    // Update memory stats every 2 seconds
    const interval = setInterval(loadMemoryStats, 2000);
    return () => clearInterval(interval);
  }, []);

  const checkAppleVisionAvailability = async () => {
    try {
      const available = await invoke<boolean>("is_apple_vision_available");
      setAppleVisionAvailable(available);

      // Get architecture info for display
      const arch = await invoke<string>("get_apple_architecture");
      setAppleArchitecture(arch);
    } catch (error) {
      logger.error("Failed to check Apple Vision availability:", error);
      setAppleVisionAvailable(false);
      setAppleArchitecture("");
    }
  };

  const loadSettings = async () => {
    try {
      const data = await invoke<Settings>("get_settings");
      setSettings(data);
      setLoading(false);
    } catch (err) {
      logger.error("Failed to load settings", err);
      setLoading(false);
    }
  };

  const loadStorageStats = async () => {
    try {
      const data = await invoke<StorageStats>("get_storage_stats");
      setStorageStats(data);
    } catch (err) {
      logger.error("Failed to load storage stats", err);
    }
  };

  const loadMemoryStats = async () => {
    try {
      const data = await invoke<MemoryStats>("get_app_memory_usage");
      setMemoryStats(data);
    } catch (err) {
      logger.error("Failed to load memory stats", err);
    }
  };

  const saveSettings = async (newSettings: Settings) => {
    setSaving(true);
    try {
      await invoke("update_settings", { settings: newSettings });
      setSettings(newSettings);

      // Apply theme immediately
      document.documentElement.setAttribute("data-theme", newSettings.theme);

      // Also save to localStorage for instant loading on next startup
      localStorage.setItem("theme", newSettings.theme);

      logger.info("Settings saved successfully");
    } catch (err) {
      logger.error("Failed to save settings", err);
    } finally {
      setSaving(false);
    }
  };

  const handleThemeChange = (theme: "light" | "dark") => {
    saveSettings({ ...settings, theme });
  };

  const handleToggle = (key: keyof Settings, value: boolean) => {
    saveSettings({ ...settings, [key]: value });
  };

  const handleTextChange = (key: keyof Settings, value: string) => {
    saveSettings({ ...settings, [key]: value });
  };

  const handleNumberChange = (key: keyof Settings, value: number) => {
    saveSettings({ ...settings, [key]: value });
  };

  const handleRescanScreenshots = async () => {
    setRescanning(true);
    try {
      const result = await invoke<string>("rescan_screenshots");
      showToast(result, "success");
      logger.info("Screenshot rescan completed:", result);
      // Reload storage stats to reflect changes
      await loadStorageStats();
    } catch (error) {
      logger.error("Failed to rescan screenshots:", error);
      showToast("Failed to rescan screenshots", "error");
    } finally {
      setRescanning(false);
    }
  };

  const formatBytes = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(2)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
  };

  const handleDeleteAllData = async () => {
    try {
      await invoke("delete_all_data");
      showToast("All data has been permanently deleted", "success");
      // Reload storage stats
      await loadStorageStats();
      // Close the modal
      setIsDeleteModalOpen(false);
    } catch (err) {
      logger.error("Failed to delete all data", err);
      showToast("Failed to delete data", "error");
    }
  };

  if (loading) {
    return (
      <div className="settings-window">
        <div className="settings-loading">Loading settings...</div>
      </div>
    );
  }

  return (
    <div className="settings-window">
      <div className="settings-header">
        <h1>Settings</h1>
      </div>

      <div className="settings-content">
        {/* Appearance Section */}
        <section className="settings-section">
          <h2>Appearance</h2>
          <div className="setting-item">
            <div className="setting-info">
              <label>Dark Mode</label>
              <p>Use dark color scheme</p>
            </div>
            <div className="setting-control">
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={settings.theme === "dark"}
                  onChange={(e) =>
                    handleThemeChange(e.target.checked ? "dark" : "light")
                  }
                  disabled={saving}
                />
                <span className="toggle-slider"></span>
              </label>
            </div>
          </div>
        </section>

        {/* Keyboard Shortcuts Section */}
        <section className="settings-section">
          <h2>Keyboard Shortcuts</h2>
          <p className="section-description">
            Customize global keyboard shortcuts. Press keys in the input to
            record.
          </p>
          <div className="setting-item">
            <div className="setting-info">
              <label>Save Snippet</label>
              <p>Shortcut to save clipboard content as snippet</p>
            </div>
            <div className="setting-control">
              <input
                type="text"
                className="shortcut-input"
                value={formatShortcutDisplay(settings.shortcut_save)}
                onKeyDown={(e) => {
                  e.preventDefault();
                  const keys = [];
                  if (e.ctrlKey) keys.push("Ctrl");
                  if (e.altKey) keys.push("Alt");
                  if (e.shiftKey) keys.push("Shift");
                  if (e.metaKey) keys.push("Cmd");
                  if (
                    e.key &&
                    !["Control", "Alt", "Shift", "Meta"].includes(e.key)
                  ) {
                    keys.push(e.key.toUpperCase());
                  }
                  if (keys.length > 1) {
                    const shortcut = keys.join("+");
                    saveSettings({ ...settings, shortcut_save: shortcut });
                  }
                }}
                readOnly
                placeholder="Press keys..."
              />
            </div>
          </div>
          <div className="setting-item">
            <div className="setting-info">
              <label>Open Search</label>
              <p>Shortcut to open/hide search window</p>
            </div>
            <div className="setting-control">
              <input
                type="text"
                className="shortcut-input"
                value={formatShortcutDisplay(settings.shortcut_search)}
                onKeyDown={(e) => {
                  e.preventDefault();
                  const keys = [];
                  if (e.ctrlKey) keys.push("Ctrl");
                  if (e.altKey) keys.push("Alt");
                  if (e.shiftKey) keys.push("Shift");
                  if (e.metaKey) keys.push("Cmd");
                  if (
                    e.key &&
                    !["Control", "Alt", "Shift", "Meta"].includes(e.key)
                  ) {
                    keys.push(e.key.toUpperCase());
                  }
                  if (keys.length > 1) {
                    const shortcut = keys.join("+");
                    saveSettings({ ...settings, shortcut_search: shortcut });
                  }
                }}
                readOnly
                placeholder="Press keys..."
              />
            </div>
          </div>
        </section>

        {/* Background Mode Section */}
        <section className="settings-section">
          <h2>Application Behavior</h2>
          <div className="setting-item">
            <div className="setting-info">
              <label>Run in Background</label>
              <p>Keep app running in system tray when window is closed</p>
            </div>
            <div className="setting-control">
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={settings.run_in_background}
                  onChange={(e) =>
                    handleToggle("run_in_background", e.target.checked)
                  }
                  disabled={saving}
                />
                <span className="toggle-slider"></span>
              </label>
            </div>
          </div>
        </section>

        {/* Search Section */}
        <section className="settings-section">
          <h2>Search</h2>
          <div className="setting-item">
            <div className="setting-info">
              <label>Semantic Search</label>
              <p>Use AI embeddings for meaning-based search</p>
            </div>
            <div className="setting-control">
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={settings.enable_semantic_search}
                  onChange={(e) =>
                    handleToggle("enable_semantic_search", e.target.checked)
                  }
                  disabled={saving}
                />
                <span className="toggle-slider"></span>
              </label>
            </div>
          </div>
          <div className="setting-item">
            <div className="setting-info">
              <label>Search Analytics</label>
              <p>Collect search metrics for insights</p>
            </div>
            <div className="setting-control">
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={settings.enable_search_analytics}
                  onChange={(e) =>
                    handleToggle("enable_search_analytics", e.target.checked)
                  }
                  disabled={saving}
                />
                <span className="toggle-slider"></span>
              </label>
            </div>
          </div>
        </section>

        {/* Storage Section */}
        <section className="settings-section">
          <h2>Storage</h2>
          {storageStats ? (
            <>
              <div className="stats-grid">
                <div className="stat-card">
                  <div className="stat-label">Total Size</div>
                  <div className="stat-value">
                    {formatBytes(storageStats.total_size)}
                  </div>
                </div>
                <div className="stat-card">
                  <div className="stat-label">Snippets</div>
                  <div className="stat-value">
                    {storageStats.snippets_count}
                  </div>
                </div>
                <div className="stat-card">
                  <div className="stat-label">Categories</div>
                  <div className="stat-value">
                    {storageStats.categories_count}
                  </div>
                </div>
                <div className="stat-card">
                  <div className="stat-label">Embeddings</div>
                  <div className="stat-value">
                    {storageStats.embeddings_count}
                  </div>
                </div>
              </div>
              <div className="storage-breakdown">
                <h3>Database Files</h3>
                <div className="storage-item">
                  <span>Snippets Database</span>
                  <span>{formatBytes(storageStats.snippets_db_size)}</span>
                </div>
                <div className="storage-item">
                  <span>Job Queue Database</span>
                  <span>{formatBytes(storageStats.job_queue_db_size)}</span>
                </div>
                <div className="storage-item">
                  <span>Vector Database</span>
                  <span>{formatBytes(storageStats.vectors_db_size)}</span>
                </div>
              </div>
            </>
          ) : (
            <div className="loading-stats">Loading storage stats...</div>
          )}
        </section>

        {/* Danger Zone Section */}
        <section className="settings-section danger-zone-section">
          <h2>Danger Zone</h2>
          <p className="section-description">
            Irreversible actions that permanently delete your data.
          </p>
          <div className="danger-zone">
            <div className="danger-action">
              <div className="danger-info">
                <h3>Delete All Data</h3>
                <p>
                  Permanently delete all snippets, commands, screenshots,
                  categories, and embeddings. This action cannot be undone.
                </p>
              </div>
              <button
                className="danger-button"
                onClick={() => setIsDeleteModalOpen(true)}
              >
                Delete All Data
              </button>
            </div>
          </div>
        </section>

        {/* Performance Section */}
        <section className="settings-section">
          <h2>Performance</h2>
          {memoryStats ? (
            <>
              <div className="stats-grid">
                <div className="stat-card">
                  <div className="stat-label">Memory Usage (RAM)</div>
                  <div className="stat-value">
                    {memoryStats.rss_mb.toFixed(2)} MB
                  </div>
                  <div className="stat-note">Actual physical memory in use</div>
                </div>
                <div className="stat-card">
                  <div className="stat-label">CPU Usage</div>
                  <div className="stat-value">
                    {memoryStats.cpu_percent.toFixed(1)}%
                  </div>
                  <div className="stat-note">Current processor usage</div>
                </div>
              </div>
              <div className="info-note">
                <strong>Note:</strong> Virtual memory (
                {(memoryStats.virtual_mb / 1024).toFixed(1)} GB) includes
                memory-mapped files and reserved address space. The RAM usage
                above is the actual memory being used.
              </div>
            </>
          ) : (
            <div className="loading-stats">Loading memory stats...</div>
          )}
        </section>

        {/* Export/Backup Section */}
        <ExportBackupSection />

        {/* Monitoring Section */}
        <section className="settings-section">
          <h2>Monitoring</h2>
          <p className="section-description">
            Monitor and index terminal commands and screenshots
          </p>

          {/* Terminal Monitoring */}
          <div className="setting-group">
            <h3>Terminal Commands</h3>
            <div className="setting-item">
              <div className="setting-info">
                <label>Enable Terminal Monitoring</label>
                <p>Automatically capture and index shell commands</p>
              </div>
              <div className="setting-control">
                <label className="toggle">
                  <input
                    type="checkbox"
                    checked={settings.terminal_monitoring_enabled}
                    onChange={(e) =>
                      handleToggle(
                        "terminal_monitoring_enabled",
                        e.target.checked
                      )
                    }
                    disabled={saving}
                  />
                  <span className="toggle-slider"></span>
                </label>
              </div>
            </div>
          </div>

          {/* Command Picker */}
          <div className="setting-group">
            <h3>Command Picker (Terminal Autocomplete)</h3>
            <div className="setting-item">
              <div className="setting-info">
                <label>Enable Command Picker</label>
                <p>Show saved commands in terminal with keyboard shortcut (default: {formatShortcutDisplay("Alt+C")})</p>
              </div>
              <div className="setting-control">
                <label className="toggle">
                  <input
                    type="checkbox"
                    checked={settings.command_picker_enabled}
                    onChange={(e) =>
                      handleToggle(
                        "command_picker_enabled",
                        e.target.checked
                      )
                    }
                    disabled={saving}
                  />
                  <span className="toggle-slider"></span>
                </label>
              </div>
            </div>
            {settings.command_picker_enabled && (
              <div className="setting-item">
                <div className="setting-info">
                  <label>Keyboard Shortcut</label>
                  <p>Shortcut to open command picker (e.g., {formatShortcutDisplay("Alt+C")}, Ctrl+R). Note: On macOS, Alt is the Option key.</p>
                </div>
                <div className="setting-control">
                  <input
                    type="text"
                    value={settings.command_picker_shortcut}
                    onChange={(e) =>
                      handleTextChange("command_picker_shortcut", e.target.value)
                    }
                    disabled={saving}
                    placeholder="Alt+C"
                    className="shortcut-input"
                  />
                </div>
              </div>
            )}
          </div>

          {/* Spotlight Search */}
          <div className="setting-group">
            <h3>Spotlight Search (System-Wide)</h3>
            <div className="setting-item">
              <div className="setting-info">
                <label>Enable Spotlight Search</label>
                <p>Quick search overlay accessible from anywhere (default: {formatShortcutDisplay("Ctrl+Space")} on macOS, Alt+Space on others)</p>
              </div>
              <div className="setting-control">
                <label className="toggle">
                  <input
                    type="checkbox"
                    checked={settings.spotlight_enabled}
                    onChange={(e) =>
                      handleToggle(
                        "spotlight_enabled",
                        e.target.checked
                      )
                    }
                    disabled={saving}
                  />
                  <span className="toggle-slider"></span>
                </label>
              </div>
            </div>
            {settings.spotlight_enabled && (
              <div className="setting-item">
                <div className="setting-info">
                  <label>Keyboard Shortcut</label>
                  <p>Shortcut to open Spotlight search (e.g., {formatShortcutDisplay("Ctrl+Space")}, Alt+Space). Note: On macOS, Ctrl is the Control key.</p>
                </div>
                <div className="setting-control">
                  <input
                    type="text"
                    value={settings.spotlight_shortcut}
                    onChange={(e) =>
                      handleTextChange("spotlight_shortcut", e.target.value)
                    }
                    disabled={saving}
                    placeholder="Ctrl+Space"
                    className="shortcut-input"
                  />
                </div>
              </div>
            )}
          </div>

          {/* Screenshot Monitoring */}
          <div className="setting-group">
            <h3>Screenshots</h3>
            <div className="setting-item">
              <div className="setting-info">
                <label>Enable Screenshot Monitoring</label>
                <p>Automatically index screenshots with OCR and captioning</p>
              </div>
              <div className="setting-control">
                <label className="toggle">
                  <input
                    type="checkbox"
                    checked={settings.screenshot_monitoring_enabled}
                    onChange={(e) =>
                      handleToggle(
                        "screenshot_monitoring_enabled",
                        e.target.checked
                      )
                    }
                    disabled={saving}
                  />
                  <span className="toggle-slider"></span>
                </label>
              </div>
            </div>

            {settings.screenshot_monitoring_enabled && (
              <>
                <div className="setting-item">
                  <div className="setting-info">
                    <label>OCR (Text Extraction)</label>
                    <p>Extract text from screenshots using OCR engines</p>
                  </div>
                  <div className="setting-control">
                    <label className="toggle">
                      <input
                        type="checkbox"
                        checked={settings.screenshot_ocr_enabled}
                        onChange={(e) =>
                          handleToggle(
                            "screenshot_ocr_enabled",
                            e.target.checked
                          )
                        }
                        disabled={saving}
                      />
                      <span className="toggle-slider"></span>
                    </label>
                  </div>
                </div>

                {/* OCR Settings (shown when OCR is enabled) */}
                {settings.screenshot_ocr_enabled && (
                  <div className="ocr-settings-section">
                    <div className="setting-item">
                      <div className="setting-info">
                        <label>OCR Engine</label>
                        <p>Choose OCR engine for text extraction</p>
                      </div>
                      <div className="setting-control">
                        <select
                          value={settings.ocr_engine}
                          onChange={(e) =>
                            handleTextChange("ocr_engine", e.target.value)
                          }
                          disabled={saving}
                          className="ocr-engine-select"
                        >
                          <option value="auto">
                            Auto{" "}
                            {appleVisionAvailable
                              ? `(Apple Vision${
                                  appleArchitecture
                                    ? ` - ${appleArchitecture}`
                                    : ""
                                })`
                              : "(Tesseract)"}
                          </option>
                          <option
                            value="apple_vision"
                            disabled={!appleVisionAvailable}
                          >
                            Apple Vision
                            {appleArchitecture
                              ? ` (${appleArchitecture})`
                              : ""}{" "}
                            {!appleVisionAvailable ? "(Not Available)" : ""}
                          </option>
                          <option value="tesseract">Tesseract</option>
                        </select>
                      </div>
                    </div>

                    {settings.ocr_engine !== "tesseract" &&
                      appleVisionAvailable && (
                        <div className="setting-item">
                          <div className="setting-info">
                            <label>Recognition Level</label>
                            <p>Fast (~130ms) or Accurate (~200ms)</p>
                          </div>
                          <div className="setting-control">
                            <select
                              value={settings.ocr_recognition_level}
                              onChange={(e) =>
                                handleTextChange(
                                  "ocr_recognition_level",
                                  e.target.value
                                )
                              }
                              disabled={saving}
                            >
                              <option value="fast">Fast</option>
                              <option value="accurate">Accurate</option>
                            </select>
                          </div>
                        </div>
                      )}

                    <div className="setting-item">
                      <div className="setting-info">
                        <label>Text Cleaning Level</label>
                        <p>How aggressively to filter OCR noise</p>
                      </div>
                      <div className="setting-control">
                        <div className="cleaning-level-slider">
                          <input
                            type="range"
                            min="0"
                            max="2"
                            value={
                              settings.ocr_cleaning_level === "minimal"
                                ? 0
                                : settings.ocr_cleaning_level === "balanced"
                                ? 1
                                : 2
                            }
                            onChange={(e) => {
                              const level = [
                                "minimal",
                                "balanced",
                                "aggressive",
                              ][parseInt(e.target.value)];
                              handleTextChange("ocr_cleaning_level", level);
                            }}
                            disabled={saving}
                          />
                          <div className="slider-labels">
                            <span>Minimal</span>
                            <span>Balanced</span>
                            <span>Aggressive</span>
                          </div>
                        </div>
                      </div>
                    </div>

                    {/* Advanced Tesseract Settings */}
                    <details className="advanced-ocr-settings">
                      <summary>Advanced Settings</summary>
                      <div className="setting-item">
                        <div className="setting-info">
                          <label>Tesseract PSM Mode</label>
                          <p>Page Segmentation Mode for Tesseract</p>
                        </div>
                        <div className="setting-control">
                          <select
                            value={settings.tesseract_psm_mode}
                            onChange={(e) =>
                              handleNumberChange(
                                "tesseract_psm_mode",
                                parseInt(e.target.value)
                              )
                            }
                            disabled={saving}
                          >
                            <option value="3">
                              3 - Automatic (Recommended)
                            </option>
                            <option value="6">6 - Single uniform block</option>
                            <option value="11">
                              11 - Sparse text (Old default)
                            </option>
                            <option value="4">4 - Single column</option>
                          </select>
                        </div>
                      </div>
                    </details>
                  </div>
                )}

                <div className="setting-item">
                  <div className="setting-info">
                    <label>Image Captioning</label>
                    <p>Generate descriptions using Florence-2 vision model</p>
                  </div>
                  <div className="setting-control">
                    <label className="toggle">
                      <input
                        type="checkbox"
                        checked={settings.screenshot_caption_enabled}
                        onChange={(e) =>
                          handleToggle(
                            "screenshot_caption_enabled",
                            e.target.checked
                          )
                        }
                        disabled={saving}
                      />
                      <span className="toggle-slider"></span>
                    </label>
                  </div>
                </div>

                {/* Rescan Button */}
                <div className="setting-item">
                  <div className="setting-info">
                    <label>Rescan Existing Screenshots</label>
                    <p>
                      Process screenshots that exist on disk but aren't in the
                      database
                    </p>
                  </div>
                  <div className="setting-control">
                    <button
                      onClick={handleRescanScreenshots}
                      disabled={rescanning || saving}
                      className="rescan-button"
                    >
                      {rescanning ? "Rescanning..." : "Rescan Now"}
                    </button>
                  </div>
                </div>
              </>
            )}
          </div>
        </section>

        {/* Duplicate Detection Section */}
        <section className="settings-section">
          <DuplicateManager />
        </section>

        {/* About Section */}
        <section className="settings-section">
          <h2>About</h2>
          <div className="about-info">
            <p>
              <strong>LocalMind</strong>
            </p>
            <p>Version 1.0.0</p>
            <p>100% Local Cognitive Context Assistant</p>
          </div>
        </section>
      </div>

      {/* Delete All Data Modal */}
      {storageStats && (
        <DeleteAllDataModal
          isOpen={isDeleteModalOpen}
          onClose={() => setIsDeleteModalOpen(false)}
          onConfirm={handleDeleteAllData}
          storageStats={{
            snippets_count: storageStats.snippets_count,
            categories_count: storageStats.categories_count,
            embeddings_count: storageStats.embeddings_count,
          }}
        />
      )}
    </div>
  );
}

function ExportBackupSection() {
  const [exporting, setExporting] = useState(false);
  const [importing, setImporting] = useState(false);
  const [exportHistory, setExportHistory] = useState<ExportRecord[]>([]);
  const [showHistory, setShowHistory] = useState(false);
  const { showToast } = useToast();

  useEffect(() => {
    loadExportHistory();
  }, []);

  const loadExportHistory = async () => {
    try {
      const history = await invoke<ExportRecord[]>("get_export_history", {
        limit: 10,
      });
      setExportHistory(history);
    } catch (err) {
      logger.error("Failed to load export history", err);
    }
  };

  const handleExportJSON = async () => {
    try {
      const filePath = await save({
        defaultPath: `localmind-export-${
          new Date().toISOString().split("T")[0]
        }.json`,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });

      if (!filePath) return;

      setExporting(true);
      const count = await invoke<number>("export_to_json_file", {
        filePath,
        includeVersions: true,
      });

      showToast(`Successfully exported ${count} snippets to JSON`, "success");

      await loadExportHistory();
    } catch (err) {
      logger.error("Failed to export to JSON", err);
      showToast(`Export failed: ${err}`, "error");
    } finally {
      setExporting(false);
    }
  };

  const handleExportMarkdown = async () => {
    try {
      const filePath = await save({
        defaultPath: `localmind-export-${
          new Date().toISOString().split("T")[0]
        }.md`,
        filters: [{ name: "Markdown", extensions: ["md"] }],
      });

      if (!filePath) return;

      setExporting(true);
      const count = await invoke<number>("export_to_markdown_file", {
        filePath,
      });

      showToast(
        `Successfully exported ${count} snippets to Markdown`,
        "success"
      );

      await loadExportHistory();
    } catch (err) {
      logger.error("Failed to export to Markdown", err);
      showToast(`Export failed: ${err}`, "error");
    } finally {
      setExporting(false);
    }
  };

  const handleImportJSON = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{ name: "JSON", extensions: ["json"] }],
      });

      if (!selected || Array.isArray(selected)) return;

      setImporting(true);
      const count = await invoke<number>("import_from_json_file", {
        filePath: selected,
      });

      showToast(`Successfully imported ${count} snippets from JSON`, "success");
    } catch (err) {
      logger.error("Failed to import from JSON", err);
      showToast(`Import failed: ${err}`, "error");
    } finally {
      setImporting(false);
    }
  };

  const formatBytes = (bytes: number | null): string => {
    if (!bytes) return "N/A";
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(2)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
  };

  const formatDate = (dateStr: string): string => {
    try {
      return new Date(dateStr).toLocaleString();
    } catch {
      return dateStr;
    }
  };

  return (
    <section className="settings-section">
      <h2>Export & Backup</h2>
      <p className="section-description">
        Export your data for backup or migration. Exports include all snippets,
        categories, and metadata.
      </p>

      <div className="export-actions">
        <button
          className="export-button"
          onClick={handleExportJSON}
          disabled={exporting || importing}
        >
          {exporting ? "Exporting..." : "Export to JSON"}
        </button>
        <button
          className="export-button"
          onClick={handleExportMarkdown}
          disabled={exporting || importing}
        >
          {exporting ? "Exporting..." : "Export to Markdown"}
        </button>
        <button
          className="export-button import-button"
          onClick={handleImportJSON}
          disabled={exporting || importing}
        >
          {importing ? "Importing..." : "Import from JSON"}
        </button>
      </div>

      <div className="export-history-section">
        <button
          className="history-toggle"
          onClick={() => setShowHistory(!showHistory)}
        >
          {showHistory ? "Hide" : "Show"} Export History ({exportHistory.length}
          )
        </button>

        {showHistory && exportHistory.length > 0 && (
          <div className="export-history">
            <table className="history-table">
              <thead>
                <tr>
                  <th>Date</th>
                  <th>Format</th>
                  <th>Items</th>
                  <th>Size</th>
                  <th>Path</th>
                </tr>
              </thead>
              <tbody>
                {exportHistory.map((record) => (
                  <tr key={record.id}>
                    <td>{formatDate(record.exported_at)}</td>
                    <td>{record.format.toUpperCase()}</td>
                    <td>{record.item_count}</td>
                    <td>{formatBytes(record.file_size_bytes)}</td>
                    <td className="file-path" title={record.file_path}>
                      {record.file_path.split("/").pop() || record.file_path}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      <div className="export-info">
        <h3>Export Formats</h3>
        <div className="format-info">
          <div className="format-item">
            <strong>JSON:</strong> Machine-readable format with complete data
            structure. Includes all metadata, categories, tags, and version
            history. Best for backup and data migration.
          </div>
          <div className="format-item">
            <strong>Markdown:</strong> Human-readable format optimized for
            reading. Organized by categories with formatted content. Best for
            documentation and sharing.
          </div>
        </div>
      </div>
    </section>
  );
}
