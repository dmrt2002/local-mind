import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { writeText } from "@tauri-apps/api/clipboard";
// Don't import getCurrentWindow statically - use dynamic import instead
import "./SpotlightSearch.css";

// Helper function to get current window using dynamic import
async function getCurrentWindowInstance() {
  const windowModule = await import("@tauri-apps/api/window");
  return windowModule.getCurrent();
}

interface SearchResult {
  id: number;
  content: string;
  summary: string | null;
  type?: string | null;
  created_at: string;
  source_app: string | null;
  rank: number;
  match_type: string;
}

interface SearchResults {
  keyword_results: SearchResult[];
  semantic_results: SearchResult[];
  combined: SearchResult[];
}

export default function SpotlightSearch() {
  const [query, setQuery] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const resultsRef = useRef<HTMLDivElement>(null);

  // Auto-focus input when component mounts
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Listen for focus event from backend
  useEffect(() => {
    let unlistenPromise: Promise<() => void> | null = null;

    getCurrentWindowInstance()
      .then((appWindow) => {
        unlistenPromise = appWindow.listen("spotlight-focus", () => {
          inputRef.current?.focus();
          setQuery("");
          setResults([]);
          setSelectedIndex(0);
        });
      })
      .catch((error) => {
        console.warn("[SpotlightSearch] Failed to setup focus listener:", error);
        // Continue without focus listener - non-critical
      });

    return () => {
      if (unlistenPromise) {
        unlistenPromise.then(f => f()).catch(() => {});
      }
    };
  }, []);

  // Clear search when window is hidden (via Escape or backdrop click)
  useEffect(() => {
    const handleVisibilityChange = () => {
      getCurrentWindowInstance()
        .then((appWindow) => {
          appWindow.isVisible().then((visible: boolean) => {
            if (!visible) {
              setQuery("");
              setResults([]);
              setSelectedIndex(0);
            }
          }).catch(() => {});
        })
        .catch((error) => {
          // Tauri API not available - continue without visibility checking
          console.warn("[SpotlightSearch] Visibility check failed:", error);
        });
    };

    // Check visibility periodically when component is mounted
    const interval = setInterval(handleVisibilityChange, 500);
    
    return () => clearInterval(interval);
  }, []);

  // Debounce query input (optimized for fast feel)
  useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedQuery(query);
    }, 120); // Fast debounce for responsive feel

    return () => clearTimeout(timer);
  }, [query]);

  // Search when debounced query changes
  useEffect(() => {
    if (debouncedQuery.trim().length === 0) {
      setResults([]);
      setSelectedIndex(0);
      return;
    }

    setLoading(true);
    invoke<SearchResults>("search", {
      query: debouncedQuery,
      filters: null
    })
      .then((data) => {
        // Limit to 8 results for minimal display
        setResults(data.combined.slice(0, 8));
        setSelectedIndex(0);
        setLoading(false);
      })
      .catch((err) => {
        console.error("Spotlight search failed:", err);
        setLoading(false);
      });
  }, [debouncedQuery]);

  // Define callback functions before they're used in useEffect dependencies
  const copyToClipboard = useCallback(async (content: string) => {
    try {
      console.log("[SpotlightSearch] Copying to clipboard, content length:", content.length);
      await writeText(content);
      console.log("[SpotlightSearch] Successfully copied to clipboard");
      // Optional: Show brief feedback
    } catch (err) {
      console.error("[SpotlightSearch] Failed to copy to clipboard:", err);
    }
  }, []);

  const handleSelectResult = useCallback(async (result: SearchResult, openInApp: boolean) => {
    console.log("[SpotlightSearch] Result selected:", { id: result.id, openInApp });
    try {
      const appWindow = await getCurrentWindowInstance();

      if (openInApp) {
        // Open in main app - emit event to main window
        console.log("[SpotlightSearch] Opening result in main app");
        try {
          const { WebviewWindow } = await import("@tauri-apps/api/window");
          const mainWindow = WebviewWindow.getByLabel("main");
          if (mainWindow) {
            await mainWindow.show();
            await mainWindow.setFocus();
            await mainWindow.emit("navigate-to-snippet", { snippetId: result.id });
          }
        } catch (err) {
          console.error("Failed to open in main app:", err);
        }
        await appWindow.hide();
      } else {
        // Copy to clipboard
        console.log("[SpotlightSearch] Copying result to clipboard");
        await copyToClipboard(result.content);
        console.log("[SpotlightSearch] Result copied, hiding window");
        await appWindow.hide();
      }
    } catch (error) {
      console.warn("[SpotlightSearch] Failed to handle result selection:", error);
      // Still try to copy to clipboard even if window API fails
      if (!openInApp) {
        console.log("[SpotlightSearch] Fallback: copying to clipboard without window API");
        await copyToClipboard(result.content);
      }
    }
  }, [copyToClipboard]);

  // Keyboard navigation
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        console.log("[SpotlightSearch] ESC key pressed, hiding window");
        getCurrentWindowInstance()
          .then((appWindow) => {
            console.log("[SpotlightSearch] Successfully got window, hiding...");
            return appWindow.hide();
          })
          .then(() => {
            console.log("[SpotlightSearch] Window hidden successfully");
          })
          .catch((error) => {
            console.error("[SpotlightSearch] Failed to hide window on ESC:", error);
            console.error("[SpotlightSearch] Error details:", JSON.stringify(error));
          });
        return;
      }

      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((prev) =>
          prev < results.length - 1 ? prev + 1 : prev
        );
        return;
      }

      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((prev) => (prev > 0 ? prev - 1 : 0));
        return;
      }

      // Home/End keys for first/last result
      if (e.key === "Home") {
        e.preventDefault();
        setSelectedIndex(0);
        return;
      }

      if (e.key === "End") {
        e.preventDefault();
        setSelectedIndex(results.length - 1);
        return;
      }

      if (e.key === "Enter") {
        e.preventDefault();
        if (results[selectedIndex]) {
          handleSelectResult(results[selectedIndex], e.metaKey || e.ctrlKey);
        }
        return;
      }

      // Cmd/Ctrl+C to copy selected result
      if ((e.metaKey || e.ctrlKey) && e.key === "c" && results[selectedIndex]) {
        e.preventDefault();
        copyToClipboard(results[selectedIndex].content);
        return;
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [results, selectedIndex, handleSelectResult, copyToClipboard]);

  // Scroll selected item into view
  useEffect(() => {
    if (resultsRef.current) {
      const selectedElement = resultsRef.current.children[selectedIndex] as HTMLElement;
      if (selectedElement) {
        selectedElement.scrollIntoView({ block: "nearest", behavior: "smooth" });
      }
    }
  }, [selectedIndex, results]);

  const getTypeIcon = (type: string | null | undefined) => {
    if (!type) return "📄";
    switch (type.toLowerCase()) {
      case "screenshot":
        return "📸";
      case "command":
        return "💻";
      default:
        return "📄";
    }
  };

  const getTypeLabel = (type: string | null | undefined) => {
    if (!type) return "Snippet";
    switch (type.toLowerCase()) {
      case "screenshot":
        return "Screenshot";
      case "command":
        return "Command";
      default:
        return "Snippet";
    }
  };

  const truncateText = (text: string, maxLength: number) => {
    if (text.length <= maxLength) return text;
    return text.slice(0, maxLength) + "...";
  };

  // Debug: Log when component renders and verify DOM elements
  useEffect(() => {
    console.log("[SpotlightSearch] Component mounted");
    
    // Wait a bit for DOM to update
    setTimeout(() => {
      const windowEl = document.querySelector('.spotlight-window');
      const containerEl = document.querySelector('.spotlight-container');
      const inputEl = inputRef.current;
      
      console.log("[SpotlightSearch] DOM Elements:", {
        container: containerEl,
        window: windowEl,
        input: inputEl
      });
      
      if (windowEl) {
        const windowStyles = window.getComputedStyle(windowEl as Element);
        console.log("[SpotlightSearch] Window computed styles:", {
          display: windowStyles.display,
          visibility: windowStyles.visibility,
          opacity: windowStyles.opacity,
          width: windowStyles.width,
          height: windowStyles.height,
          zIndex: windowStyles.zIndex,
          position: windowStyles.position
        });
        
        // Check if window is actually visible
        const rect = (windowEl as Element).getBoundingClientRect();
        console.log("[SpotlightSearch] Window bounding rect:", {
          top: rect.top,
          left: rect.left,
          width: rect.width,
          height: rect.height,
          visible: rect.width > 0 && rect.height > 0
        });
      }
      
      // Verify input is visible
      if (inputEl) {
        const inputStyles = window.getComputedStyle(inputEl);
        console.log("[SpotlightSearch] Input styles:", {
          display: inputStyles.display,
          visibility: inputStyles.visibility,
          opacity: inputStyles.opacity,
          width: inputStyles.width,
          height: inputStyles.height
        });
      } else {
        console.warn("[SpotlightSearch] Input ref is null!");
      }
    }, 100);
  }, []);

  const handleBackdropClick = async (e: React.MouseEvent<HTMLDivElement>) => {
    // Close when clicking directly on the container (backdrop), not on window
    // The window click handler stops propagation, so if we get here it's a backdrop click
    const target = e.target as HTMLElement;

    console.log("[SpotlightSearch] Backdrop click event fired", {
      target: target.className,
      currentTarget: e.currentTarget.className,
      isBackdrop: target === e.currentTarget,
      hasContainerClass: target.classList?.contains('spotlight-container')
    });

    // Check if clicking directly on the container (not bubbled from children)
    if (target === e.currentTarget || target.classList?.contains('spotlight-container')) {
      console.log("[SpotlightSearch] ✓ Backdrop clicked, hiding window");
      getCurrentWindowInstance()
        .then((appWindow) => {
          console.log("[SpotlightSearch] Successfully got window, hiding...");
          return appWindow.hide();
        })
        .then(() => {
          console.log("[SpotlightSearch] Window hidden successfully");
        })
        .catch((error) => {
          console.error("[SpotlightSearch] Failed to hide window:", error);
          console.error("[SpotlightSearch] Error details:", JSON.stringify(error));
        });
    } else {
      console.log("[SpotlightSearch] ✗ Click was on child element, not closing");
    }
  };

  const handleBackdropMouseDown = async (e: React.MouseEvent<HTMLDivElement>) => {
    // Also handle mousedown as backup in case click doesn't work
    const target = e.target as HTMLElement;
    if (target === e.currentTarget) {
      // Don't close on mousedown, wait for click - but this ensures we capture the event
      console.log("[SpotlightSearch] Backdrop mousedown detected (pointer-events working)");
    }
  };

  const handleWindowClick = (e: React.MouseEvent) => {
    // Prevent clicks on window from propagating to backdrop
    console.log("[SpotlightSearch] Window click detected, stopping propagation");
    e.stopPropagation();
  };

  return (
    <div
      className="spotlight-container"
      onClick={handleBackdropClick}
      onMouseDown={handleBackdropMouseDown}
      style={{ pointerEvents: 'all' }}
    >
      <div className="spotlight-window" onClick={handleWindowClick}>
        <div className="spotlight-search-box">
          <span className="spotlight-search-icon">🔍</span>
          <input
            ref={inputRef}
            type="text"
            className="spotlight-input"
            placeholder="Search LocalMind..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            autoFocus
          />
          {loading && <span className="spotlight-loading">⏳</span>}
        </div>

        {results.length > 0 && (
          <div className="spotlight-results" ref={resultsRef}>
            {results.map((result, index) => (
              <div
                key={result.id}
                className={`spotlight-result ${index === selectedIndex ? "selected" : ""}`}
                onClick={(e) => {
                  e.stopPropagation(); // Prevent window click handler from interfering
                  handleSelectResult(result, false);
                }}
                onDoubleClick={(e) => {
                  e.stopPropagation(); // Prevent window click handler from interfering
                  handleSelectResult(result, true);
                }}
              >
                <div className="spotlight-result-icon">
                  {getTypeIcon(result.type)}
                </div>
                <div className="spotlight-result-content">
                  <div className="spotlight-result-title">
                    {result.summary || truncateText(result.content, 55)}
                  </div>
                  {result.summary && (
                    <div className="spotlight-result-preview">
                      {truncateText(result.content, 90)}
                    </div>
                  )}
                  <div className="spotlight-result-meta">
                    <span className="spotlight-result-type">{getTypeLabel(result.type)}</span>
                    {result.source_app && (
                      <span className="spotlight-result-app">• {result.source_app}</span>
                    )}
                    {result.match_type && (
                      <span className="spotlight-result-match">• {result.match_type}</span>
                    )}
                  </div>
                </div>
                {index === selectedIndex && (
                  <div className="spotlight-result-actions">
                    <span className="spotlight-action-hint">Enter to copy • Cmd+Enter to open</span>
                  </div>
                )}
              </div>
            ))}
          </div>
        )}

        {query.length > 0 && results.length === 0 && !loading && (
          <div className="spotlight-empty">
            <div className="spotlight-empty-icon">🔍</div>
            <div className="spotlight-empty-text">No results found</div>
            <div className="spotlight-empty-hint">Try different keywords or check spelling</div>
          </div>
        )}

        {query.length === 0 && (
          <div className="spotlight-empty">
            <div className="spotlight-empty-icon">✨</div>
            <div className="spotlight-empty-text">Search LocalMind</div>
            <div className="spotlight-empty-hint">Type to search snippets, screenshots, and commands</div>
          </div>
        )}
      </div>
    </div>
  );
}

