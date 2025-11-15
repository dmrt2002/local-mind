import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import SnippetCard, { Snippet } from "./SnippetCard";
import CommandCard from "./CommandCard";
import ScreenshotCard from "./ScreenshotCard";
import ScreenshotLightbox from "./ScreenshotLightbox";
import { ContentType } from "./ContentTypeTabs";
import "./ContentPanel.css";

interface ContentPanelProps {
  categoryId: number | null;
  snippetId?: number | null;
  contentType?: ContentType;
  onClearSnippet?: () => void;
}

const PAGE_SIZE = 50; // Load 50 snippets at a time

export default function ContentPanel({ categoryId, snippetId, contentType = "all", onClearSnippet }: ContentPanelProps) {
  const [snippets, setSnippets] = useState<Snippet[]>([]);
  const [selectedSnippet, setSelectedSnippet] = useState<Snippet | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lightboxSnippet, setLightboxSnippet] = useState<Snippet | null>(null);

  // Cache for loaded category data to avoid re-fetching
  // Cache key format: `${categoryId}-${contentType}`
  const categoryCache = useRef<Map<string, Snippet[]>>(new Map());
  const loadingRef = useRef(false);

  // Load snippets for the selected category
  const loadSnippets = useCallback(async (catId: number, offset: number = 0) => {
    // Prevent concurrent requests
    if (loadingRef.current) return;

    // Create cache key with categoryId and contentType
    const cacheKey = `${catId}-${contentType}`;

    // Check cache first (only for offset 0)
    if (offset === 0 && categoryCache.current.has(cacheKey)) {
      const cached = categoryCache.current.get(cacheKey)!;
      setSnippets(cached);
      setError(null);
      return;
    }

    loadingRef.current = true;
    setLoading(true);
    setError(null);

    try {
      const results = await invoke<Snippet[]>("get_snippets_by_category", {
        categoryId: catId,
        limit: PAGE_SIZE,
        offset,
        contentType: contentType === "all" ? null : contentType,
      });

      if (offset === 0) {
        setSnippets(results);
        // Cache the results with contentType in key
        categoryCache.current.set(cacheKey, results);
      } else {
        setSnippets((prev) => [...prev, ...results]);
      }
    } catch (err) {
      console.error("Failed to load snippets:", err);
      setError(err instanceof Error ? err.message : String(err));
      setSnippets([]);
    } finally {
      setLoading(false);
      loadingRef.current = false;
    }
  }, [contentType]);

  // Load a single snippet
  const loadSingleSnippet = useCallback(async (id: number) => {
    setLoading(true);
    setError(null);

    try {
      const result = await invoke<Snippet>("get_snippet", { id });
      setSelectedSnippet(result);
    } catch (err) {
      console.error("Failed to load snippet:", err);
      setError(err instanceof Error ? err.message : String(err));
      setSelectedSnippet(null);
    } finally {
      setLoading(false);
    }
  }, []);

  // Handle snippet update - invalidate cache and reload
  const handleSnippetUpdate = useCallback(() => {
    if (snippetId && selectedSnippet) {
      loadSingleSnippet(snippetId);
    } else if (categoryId !== null) {
      // Invalidate cache for this category and content type
      const cacheKey = `${categoryId}-${contentType}`;
      categoryCache.current.delete(cacheKey);
      loadSnippets(categoryId, 0);
    }
  }, [snippetId, selectedSnippet, categoryId, contentType, loadSingleSnippet, loadSnippets]);

  // Reset and load when category changes
  useEffect(() => {
    if (categoryId !== null && !snippetId) {
      setSelectedSnippet(null);
      loadSnippets(categoryId, 0);
    } else if (!categoryId && !snippetId) {
      setSnippets([]);
      setSelectedSnippet(null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [categoryId, snippetId]);

  // Reload when contentType changes if category is selected
  useEffect(() => {
    if (categoryId !== null && !snippetId) {
      // Clear cache for all content types of this category
      // (invalidate all possible cache keys for this category)
      const keysToDelete: string[] = [];
      categoryCache.current.forEach((_, key) => {
        if (key.startsWith(`${categoryId}-`)) {
          keysToDelete.push(key);
        }
      });
      keysToDelete.forEach((key) => categoryCache.current.delete(key));
      
      // Reload with new content type
      loadSnippets(categoryId, 0);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [contentType]);

  // Load single snippet when snippetId changes
  useEffect(() => {
    if (snippetId) {
      setSnippets([]);
      loadSingleSnippet(snippetId);
    } else {
      setSelectedSnippet(null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [snippetId]);

  // Show single snippet view
  if (selectedSnippet) {
    // Check if it's a screenshot - show lightbox instead of text view
    const snippetType = selectedSnippet.type || "text";
    if (snippetType === "screenshot") {
      return (
        <>
          <div className="content-panel">
            <div className="content-header">
              <button
                className="back-button"
                onClick={onClearSnippet}
                title="Back to category"
              >
                ← Back
              </button>
              <h2>{selectedSnippet.summary || "Screenshot"}</h2>
            </div>
          </div>
          <ScreenshotLightbox
            snippet={selectedSnippet}
            onClose={() => {
              if (onClearSnippet) {
                onClearSnippet();
              }
            }}
          />
        </>
      );
    }

    return (
      <div className="content-panel">
        <div className="content-header">
          <button
            className="back-button"
            onClick={onClearSnippet}
            title="Back to category"
          >
            ← Back
          </button>
          <h2>{selectedSnippet.summary || "Snippet"}</h2>
        </div>
        <div className="content-list">
          <div className="snippets-container">
            <SnippetCard snippet={selectedSnippet} onUpdate={handleSnippetUpdate} />
          </div>
        </div>
      </div>
    );
  }

  if (categoryId === null && !snippetId) {
    return (
      <div className="content-panel-empty">
        <div className="empty-state">
          <svg
            width="64"
            height="64"
            viewBox="0 0 24 24"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
          >
            <path
              d="M3 7V17C3 17.5304 3.21071 18.0391 3.58579 18.4142C3.96086 18.7893 4.46957 19 5 19H19C19.5304 19 20.0391 18.7893 20.4142 18.4142C20.7893 18.0391 21 17.5304 21 17V7C21 6.46957 20.7893 5.96086 20.4142 5.58579C20.0391 5.21071 19.5304 5 19 5H5C4.46957 5 3.96086 5.21071 3.58579 5.58579C3.21071 5.96086 3 6.46957 3 7Z"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
            <path
              d="M9 5V19"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
          <h3>Select a category</h3>
          <p>Choose a category from the sidebar to view its snippets</p>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="content-panel-empty">
        <div className="empty-state error">
          <svg
            width="64"
            height="64"
            viewBox="0 0 24 24"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
          >
            <path
              d="M12 9V11M12 15H12.01M5.07183 19H18.9282C20.4678 19 21.4301 17.3333 20.6603 16L13.7321 4C12.9623 2.66667 11.0378 2.66667 10.268 4L3.33978 16C2.56998 17.3333 3.53223 19 5.07183 19Z"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
          <h3>Error loading snippets</h3>
          <p>{error}</p>
        </div>
      </div>
    );
  }

  if (snippets.length === 0 && !loading) {
    return (
      <div className="content-panel-empty">
        <div className="empty-state">
          <svg
            width="64"
            height="64"
            viewBox="0 0 24 24"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
          >
            <path
              d="M9 12H15M9 16H15M17 21H7C6.46957 21 5.96086 20.7893 5.58579 20.4142C5.21071 20.0391 5 19.5304 5 19V5C5 4.46957 5.21071 3.96086 5.58579 3.58579C5.96086 3.21071 6.46957 3 7 3H12.586C12.8512 3.00006 13.1055 3.10545 13.293 3.293L18.707 8.707C18.8946 8.89449 18.9999 9.1488 19 9.414V19C19 19.5304 18.7893 20.0391 18.4142 20.4142C18.0391 20.7893 17.5304 21 17 21Z"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
          <h3>No snippets in this category</h3>
          <p>This category is empty. Snippets will appear here when added.</p>
        </div>
      </div>
    );
  }

  // Filter snippets by content type
  const filteredSnippets = snippets.filter((snippet) => {
    if (contentType === "all") return true;
    const snippetType = snippet.type || "text";
    return snippetType === contentType;
  });

  // Render different card types based on content type
  const renderSnippet = (snippet: Snippet) => {
    const snippetType = snippet.type || "text";

    if (snippetType === "command") {
      return <CommandCard key={snippet.id} snippet={snippet} onUpdate={handleSnippetUpdate} />;
    } else if (snippetType === "screenshot") {
      return (
        <ScreenshotCard
          key={snippet.id}
          snippet={snippet}
          onClick={() => setLightboxSnippet(snippet)}
        />
      );
    } else {
      return <SnippetCard key={snippet.id} snippet={snippet} onUpdate={handleSnippetUpdate} />;
    }
  };

  return (
    <>
      <div className="content-panel">
        <div className="content-header">
          <h2>
            {contentType === "command" && "Commands"}
            {contentType === "screenshot" && "Screenshots"}
            {(contentType === "text" || contentType === "all") && "Snippets"}
          </h2>
          <span className="snippet-count">{filteredSnippets.length} items</span>
        </div>
        <div className="content-list">
          <div
            className={`snippets-container ${
              contentType === "screenshot" ? "screenshots-grid" : ""
            }`}
          >
            {filteredSnippets.map(renderSnippet)}
          </div>
          {loading && (
            <div className="loading-indicator">
              <div className="spinner"></div>
              Loading...
            </div>
          )}
        </div>
      </div>
      {lightboxSnippet && (
        <ScreenshotLightbox
          snippet={lightboxSnippet}
          onClose={() => setLightboxSnippet(null)}
        />
      )}
    </>
  );
}
