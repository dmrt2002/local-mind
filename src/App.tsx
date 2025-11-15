import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { listen } from "@tauri-apps/api/event";
import SearchWindow from "./components/SearchWindow";
import HomeView from "./components/HomeView";
import AnalyticsView from "./components/AnalyticsView";
import SettingsWindow from "./components/SettingsWindow";
import CommandPalette from "./components/CommandPalette";
import TabNavigation, { TabType } from "./components/TabNavigation";
import { useToast } from "./hooks/useToast";
import { useKeyboardNavigation } from "./hooks/useKeyboardNavigation";
import { logComponent, logger } from "./utils/logger";
import "./App.css";

const log = logComponent("App");

interface Settings {
  theme: "light" | "dark";
  enable_semantic_search: boolean;
  enable_search_analytics: boolean;
}

function App() {
  const [visible, setVisible] = useState(true);
  const [activeTab, setActiveTab] = useState<TabType>("home");
  const [isCommandPaletteOpen, setIsCommandPaletteOpen] = useState(false);
  const [selectedSnippetId, setSelectedSnippetId] = useState<number | null>(null);
  const [selectedCategoryId, setSelectedCategoryId] = useState<number | null>(null);
  const { showToast, ToastContainer } = useToast();

  // Load and apply theme on startup
  useEffect(() => {
    const loadTheme = async () => {
      try {
        const settings = await invoke<Settings>("get_settings");
        document.documentElement.setAttribute("data-theme", settings.theme);
        // Save to localStorage for instant loading on next startup
        localStorage.setItem("theme", settings.theme);
        logger.info("Theme loaded and applied", { theme: settings.theme });
      } catch (err) {
        logger.error("Failed to load theme", err);
        // Default to light theme on error
        document.documentElement.setAttribute("data-theme", "light");
        localStorage.setItem("theme", "light");
      }
    };
    loadTheme();
  }, []);

  useEffect(() => {
    log.effect("window-visibility", "Setting up window visibility listener");

    // Try to load Tauri APIs dynamically
    import("@tauri-apps/api/window")
      .then((module) => {
        const getCurrentWindow = module.getCurrentWindow;
        logger.debug("App: Tauri window API loaded");

        getCurrentWindow()
          .isVisible()
          .then((isVisible: boolean) => {
            logger.debug("App: Window visibility", { isVisible });
            setVisible(isVisible);
          })
          .catch((err: any) => {
            logger.warn("App: Window visibility check failed", err);
            setVisible(true);
          });

        const unlistenShow = getCurrentWindow().onVisibilityChanged(
          (visible: boolean) => {
            logger.debug("App: Visibility changed", { visible });
            setVisible(visible);
          }
        );

        return () => {
          unlistenShow.then((f: () => void) => f()).catch(() => {});
        };
      })
      .catch((err) => {
        logger.debug("App: Tauri window API not available (normal in browser)", err);
      });
  }, []);

  useEffect(() => {
    log.effect("event-listeners", "Setting up event listeners");

    import("@tauri-apps/api/event")
      .then((module) => {
        const listen = module.listen;
        logger.debug("App: Tauri event API loaded");

        listen("snippet-saved", (event: any) => {
          logger.info("App: Snippet saved event received");
          const payload = event.payload as { id: number };
          showToast(`Snippet saved (ID: ${payload.id})`, "success");
        }).catch((err: any) => logger.error("App: Failed to listen to snippet-saved", err));

        listen("snippet-save-error", (event: any) => {
          logger.warn("App: Snippet save error event received");
          const error = event.payload as string;
          showToast(`Failed to save snippet: ${error}`, "error");
        }).catch((err: any) => logger.error("App: Failed to listen to snippet-save-error", err));
      })
      .catch((err) => {
        logger.debug("App: Tauri event API not available", err);
      });
  }, [showToast]);

  // Keyboard shortcuts for tab switching
  useKeyboardNavigation({
    enabled: true,
    preventDefault: false,
    onArrowUp: undefined,
    onArrowDown: undefined,
    onEnter: undefined,
    onEscape: undefined,
    onCtrlE: undefined,
    onCtrlD: undefined,
  });

  // Manual keyboard handling for Ctrl+1/2/3/4 and Ctrl+K (not in useKeyboardNavigation)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey) {
        if (e.key === "k" || e.key === "K") {
          e.preventDefault();
          setIsCommandPaletteOpen(true);
          logger.debug("App: Opened Command Palette via Ctrl+K");
        } else if (e.key === "1") {
          e.preventDefault();
          setActiveTab("home");
          logger.debug("App: Switched to Home tab via Ctrl+1");
        } else if (e.key === "2") {
          e.preventDefault();
          setActiveTab("search");
          logger.debug("App: Switched to Search tab via Ctrl+2");
        } else if (e.key === "3") {
          e.preventDefault();
          setActiveTab("analytics");
          logger.debug("App: Switched to Analytics tab via Ctrl+3");
        } else if (e.key === "4") {
          e.preventDefault();
          setActiveTab("settings");
          logger.debug("App: Switched to Settings tab via Ctrl+4");
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  const handleClose = () => {
    logger.debug("App: onClose called");
    setVisible(false);
  };

  const handleNavigateToSnippet = async (snippetId: number) => {
    logger.debug("App: Navigating to snippet", { snippetId });

    try {
      // Get the snippet's category
      const categoryId = await invoke<number | null>("get_snippet_category", { snippetId });
      logger.debug("App: Snippet category fetched", { snippetId, categoryId });

      // Update state to select the snippet and category
      setSelectedCategoryId(categoryId);
      setSelectedSnippetId(snippetId);

      // Navigate to home tab
      setActiveTab("home");

      logger.info("App: Navigated to snippet", { snippetId, categoryId });
    } catch (err) {
      logger.error("App: Failed to navigate to snippet", err);
      showToast("Failed to navigate to snippet", "error");
    }
  };

  // Listen for navigate events from Spotlight window
  useEffect(() => {
    const setupListener = async () => {
      try {
        const unlisten = await listen<{ snippetId: number }>("navigate-to-snippet", (event) => {
          logger.info("App: Received navigate-to-snippet event from Spotlight", { snippetId: event.payload.snippetId });
          handleNavigateToSnippet(event.payload.snippetId);
        });
        
        return () => {
          unlisten();
        };
      } catch (err) {
        logger.error("App: Failed to setup navigate-to-snippet listener", err);
      }
    };

    setupListener();
  }, []);

  return (
    <div className="app">
      <ToastContainer />
      <CommandPalette
        isOpen={isCommandPaletteOpen}
        onClose={() => setIsCommandPaletteOpen(false)}
        onNavigate={(tab) => setActiveTab(tab as TabType)}
      />
      <TabNavigation activeTab={activeTab} onTabChange={setActiveTab} />
      <div className="app-content">
        {activeTab === "home" && (
          <HomeView
            selectedSnippetId={selectedSnippetId}
            selectedCategoryId={selectedCategoryId}
            onSelectSnippet={setSelectedSnippetId}
            onSelectCategory={setSelectedCategoryId}
          />
        )}
        {activeTab === "search" && (
          <SearchWindow
            onClose={handleClose}
            onNavigateToSnippet={handleNavigateToSnippet}
          />
        )}
        {activeTab === "analytics" && <AnalyticsView />}
        {activeTab === "settings" && <SettingsWindow />}
      </div>
    </div>
  );
}

logger.debug("App: Component defined");
export default App;
