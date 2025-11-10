import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { logger } from "../utils/logger";
import { useToast } from "../hooks/useToast";
import "./CommandPalette.css";

export interface Command {
  id: string;
  label: string;
  category: "Navigation" | "Actions" | "Settings" | "Export";
  description?: string;
  action: () => void | Promise<void>;
  keywords?: string[];
}

interface CommandPaletteProps {
  isOpen: boolean;
  onClose: () => void;
  onNavigate: (tab: string) => void;
}

export default function CommandPalette({ isOpen, onClose, onNavigate }: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [recentCommands, setRecentCommands] = useState<string[]>([]);
  const inputRef = useRef<HTMLInputElement>(null);
  const { showToast } = useToast();

  // Load recent commands from localStorage
  useEffect(() => {
    const recent = localStorage.getItem("recentCommands");
    if (recent) {
      setRecentCommands(JSON.parse(recent));
    }
  }, []);

  // Focus input when opened
  useEffect(() => {
    if (isOpen && inputRef.current) {
      inputRef.current.focus();
      setQuery("");
      setSelectedIndex(0);
    }
  }, [isOpen]);

  const handleCommandExecute = (commandId: string) => {
    // Save to recent commands
    const updated = [commandId, ...recentCommands.filter((id) => id !== commandId)].slice(0, 10);
    setRecentCommands(updated);
    localStorage.setItem("recentCommands", JSON.stringify(updated));
  };

  const commands: Command[] = [
    // Navigation
    {
      id: "nav-home",
      label: "Go to Home",
      category: "Navigation",
      description: "View all snippets and categories",
      action: () => {
        onNavigate("home");
        onClose();
      },
      keywords: ["home", "snippets", "browse"],
    },
    {
      id: "nav-search",
      label: "Go to Search",
      category: "Navigation",
      description: "Search your snippets",
      action: () => {
        onNavigate("search");
        onClose();
      },
      keywords: ["search", "find", "query"],
    },
    {
      id: "nav-analytics",
      label: "Go to Analytics",
      category: "Navigation",
      description: "View search analytics and insights",
      action: () => {
        onNavigate("analytics");
        onClose();
      },
      keywords: ["analytics", "stats", "insights"],
    },
    {
      id: "nav-settings",
      label: "Go to Settings",
      category: "Navigation",
      description: "Configure application settings",
      action: () => {
        onNavigate("settings");
        onClose();
      },
      keywords: ["settings", "preferences", "config"],
    },

    // Actions
    {
      id: "action-copy-last",
      label: "Copy Last Saved Snippet",
      category: "Actions",
      description: "Copy the most recently saved snippet to clipboard",
      action: async () => {
        try {
          // Get the last snippet (implement this command in Rust)
          showToast("Feature coming soon!", "info");
          onClose();
        } catch (err) {
          logger.error("Failed to copy last snippet", err);
          showToast("Failed to copy snippet", "error");
        }
      },
      keywords: ["copy", "last", "recent", "clipboard"],
    },
    {
      id: "action-new-snippet",
      label: "Add New Snippet",
      category: "Actions",
      description: "Create a new snippet manually",
      action: () => {
        showToast("Use Alt+Shift+C to save from clipboard", "info");
        onClose();
      },
      keywords: ["new", "add", "create", "snippet"],
    },

    // Export
    {
      id: "export-json",
      label: "Export to JSON",
      category: "Export",
      description: "Export all snippets to JSON format",
      action: () => {
        onNavigate("settings");
        showToast("Navigate to Settings > Export/Backup", "info");
        onClose();
      },
      keywords: ["export", "json", "backup", "save"],
    },
    {
      id: "export-markdown",
      label: "Export to Markdown",
      category: "Export",
      description: "Export all snippets to Markdown format",
      action: () => {
        onNavigate("settings");
        showToast("Navigate to Settings > Export/Backup", "info");
        onClose();
      },
      keywords: ["export", "markdown", "backup", "save"],
    },
    {
      id: "import-json",
      label: "Import from JSON",
      category: "Export",
      description: "Import snippets from JSON file",
      action: () => {
        onNavigate("settings");
        showToast("Navigate to Settings > Export/Backup", "info");
        onClose();
      },
      keywords: ["import", "json", "restore", "load"],
    },

    // Settings
    {
      id: "settings-theme",
      label: "Toggle Dark Mode",
      category: "Settings",
      description: "Switch between light and dark themes",
      action: async () => {
        try {
          const settings = await invoke<any>("get_settings");
          const newTheme = settings.theme === "dark" ? "light" : "dark";
          await invoke("update_settings", {
            settings: { ...settings, theme: newTheme },
          });
          document.documentElement.setAttribute("data-theme", newTheme);
          localStorage.setItem("theme", newTheme);
          showToast(`Switched to ${newTheme} mode`, "success");
          onClose();
        } catch (err) {
          logger.error("Failed to toggle theme", err);
          showToast("Failed to toggle theme", "error");
        }
      },
      keywords: ["theme", "dark", "light", "mode"],
    },
    {
      id: "settings-semantic",
      label: "Toggle Semantic Search",
      category: "Settings",
      description: "Enable or disable AI-powered semantic search",
      action: async () => {
        try {
          const settings = await invoke<any>("get_settings");
          await invoke("update_settings", {
            settings: {
              ...settings,
              enable_semantic_search: !settings.enable_semantic_search,
            },
          });
          showToast(
            `Semantic search ${settings.enable_semantic_search ? "disabled" : "enabled"}`,
            "success"
          );
          onClose();
        } catch (err) {
          logger.error("Failed to toggle semantic search", err);
          showToast("Failed to toggle semantic search", "error");
        }
      },
      keywords: ["semantic", "search", "ai", "embeddings"],
    },
  ];

  // Filter commands based on query
  const filteredCommands = commands.filter((cmd) => {
    const searchText = query.toLowerCase();
    return (
      cmd.label.toLowerCase().includes(searchText) ||
      cmd.description?.toLowerCase().includes(searchText) ||
      cmd.keywords?.some((kw) => kw.toLowerCase().includes(searchText)) ||
      cmd.category.toLowerCase().includes(searchText)
    );
  });

  // Sort by recent commands first, then by relevance
  const sortedCommands = [...filteredCommands].sort((a, b) => {
    const aRecent = recentCommands.indexOf(a.id);
    const bRecent = recentCommands.indexOf(b.id);

    if (aRecent !== -1 && bRecent !== -1) {
      return aRecent - bRecent;
    }
    if (aRecent !== -1) return -1;
    if (bRecent !== -1) return 1;

    // Sort by category
    const categories = ["Navigation", "Actions", "Export", "Settings"];
    return categories.indexOf(a.category) - categories.indexOf(b.category);
  });

  // Keyboard navigation
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (!isOpen) return;

      if (e.key === "Escape") {
        onClose();
      } else if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((prev) => (prev + 1) % sortedCommands.length);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((prev) => (prev - 1 + sortedCommands.length) % sortedCommands.length);
      } else if (e.key === "Enter") {
        e.preventDefault();
        if (sortedCommands[selectedIndex]) {
          handleCommandExecute(sortedCommands[selectedIndex].id);
          sortedCommands[selectedIndex].action();
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, selectedIndex, sortedCommands]);

  // Reset selected index when query changes
  useEffect(() => {
    setSelectedIndex(0);
  }, [query]);

  if (!isOpen) return null;

  return (
    <div className="command-palette-overlay" onClick={onClose}>
      <div className="command-palette" onClick={(e) => e.stopPropagation()}>
        <div className="command-palette-header">
          <svg
            className="search-icon"
            width="20"
            height="20"
            viewBox="0 0 20 20"
            fill="none"
          >
            <path
              d="M9 17A8 8 0 1 0 9 1a8 8 0 0 0 0 16zM19 19l-4.35-4.35"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
          <input
            ref={inputRef}
            type="text"
            className="command-palette-input"
            placeholder="Type a command or search..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <kbd className="command-palette-kbd">ESC</kbd>
        </div>

        <div className="command-palette-results">
          {sortedCommands.length === 0 ? (
            <div className="no-results">No commands found</div>
          ) : (
            <>
              {/* Group by category */}
              {["Navigation", "Actions", "Export", "Settings"].map((category) => {
                const categoryCommands = sortedCommands.filter(
                  (cmd) => cmd.category === category
                );
                if (categoryCommands.length === 0) return null;

                return (
                  <div key={category} className="command-group">
                    <div className="command-group-title">{category}</div>
                    {categoryCommands.map((cmd, index) => {
                      const globalIndex = sortedCommands.indexOf(cmd);
                      const isRecent = recentCommands.includes(cmd.id);

                      return (
                        <div
                          key={cmd.id}
                          className={`command-item ${
                            globalIndex === selectedIndex ? "selected" : ""
                          }`}
                          onClick={() => {
                            handleCommandExecute(cmd.id);
                            cmd.action();
                          }}
                        >
                          <div className="command-content">
                            <div className="command-label">
                              {isRecent && <span className="recent-badge">★</span>}
                              {cmd.label}
                            </div>
                            {cmd.description && (
                              <div className="command-description">{cmd.description}</div>
                            )}
                          </div>
                          {globalIndex === selectedIndex && (
                            <kbd className="command-kbd">↵</kbd>
                          )}
                        </div>
                      );
                    })}
                  </div>
                );
              })}
            </>
          )}
        </div>

        <div className="command-palette-footer">
          <div className="command-palette-hints">
            <span><kbd>↑</kbd><kbd>↓</kbd> Navigate</span>
            <span><kbd>↵</kbd> Execute</span>
            <span><kbd>ESC</kbd> Close</span>
          </div>
        </div>
      </div>
    </div>
  );
}
