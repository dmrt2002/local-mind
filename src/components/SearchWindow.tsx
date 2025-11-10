import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { listen } from "@tauri-apps/api/event";
import SearchResults from "./SearchResults";
import SearchFiltersComponent, { SearchFilters } from "./SearchFilters";
import { logComponent, logger } from "../utils/logger";
import "./SearchWindow.css";

const log = logComponent("SearchWindow");

interface SearchResult {
  id: number;
  content: string;
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

interface SearchWindowProps {
  onClose: () => void;
  onNavigateToSnippet?: (snippetId: number) => void;
}

export default function SearchWindow({ onClose, onNavigateToSnippet }: SearchWindowProps) {
  const [query, setQuery] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState("");
  const [results, setResults] = useState<SearchResults | null>(null);
  const [loading, setLoading] = useState(false);
  const [filters, setFilters] = useState<SearchFilters>({});

  const handleDelete = (id: number) => {
    logger.info("SearchWindow: Snippet deleted, refreshing results", { id });
    // Always refresh search to clear stale results
    if (debouncedQuery.trim().length > 0) {
      // Force a refresh by re-triggering the search
      const currentQuery = debouncedQuery;
      // Clear and re-set to trigger useEffect
      setQuery("");
      setTimeout(() => {
        setQuery(currentQuery);
      }, 50);
    } else {
      // Clear results if no query
      setResults(null);
    }
  };

  // Debounce query input
  useEffect(() => {
    log.effect("debounce", "Setting up debounce timer", { query });
    const timer = setTimeout(() => {
      log.effect("debounce", "Debounced query updated", { query });
      setDebouncedQuery(query);
    }, 300);

    return () => {
      log.effect("debounce", "Clearing debounce timer");
      clearTimeout(timer);
    };
  }, [query]);

  // Search when debounced query or filters change
  useEffect(() => {
    if (debouncedQuery.trim().length === 0) {
      log.effect("search", "Empty query, clearing results");
      setResults(null);
      return;
    }

    log.effect("search", "Starting search", { query: debouncedQuery, filters });
    setLoading(true);

    // Prepare filters - only include defined values
    const searchFilters = Object.keys(filters).length > 0 ? filters : null;

    invoke<SearchResults>("search", {
      query: debouncedQuery,
      filters: searchFilters
    })
      .then((data) => {
        logger.info("SearchWindow: Search completed", {
          resultCount: data.combined.length,
          filters: searchFilters,
        });
        setResults(data);
        setLoading(false);
      })
      .catch((err) => {
        logger.error("SearchWindow: Search failed", err);
        setLoading(false);
      });
  }, [debouncedQuery, filters]);

  // Handle keyboard shortcuts
  useEffect(() => {
    log.effect("keyboard", "Setting up keyboard listener");
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        logger.debug("SearchWindow: Escape key pressed");
        onClose();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      log.effect("keyboard", "Removing keyboard listener");
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [onClose]);

  // Listen for snippet saved and deleted events
  useEffect(() => {
    log.effect("events", "Setting up event listeners");

    const unlistenSaved = listen("snippet-saved", (event) => {
      logger.info("SearchWindow: Snippet saved event received");
      // Refresh search if needed
      if (debouncedQuery.trim().length > 0) {
        setQuery(query); // Trigger search refresh
      }
    });

    const unlistenDeleted = listen("snippet-deleted", (event: any) => {
      const deletedId = event.payload as number;
      logger.info("SearchWindow: Snippet deleted event received", { id: deletedId });

      // Immediately remove the deleted snippet from results (optimistic update)
      if (results) {
        setResults({
          keyword_results: results.keyword_results.filter(r => r.id !== deletedId),
          semantic_results: results.semantic_results.filter(r => r.id !== deletedId),
          combined: results.combined.filter(r => r.id !== deletedId),
        });
      }
    });

    return () => {
      log.effect("events", "Cleaning up event listeners");
      unlistenSaved.then((f) => f());
      unlistenDeleted.then((f) => f());
    };
  }, [query, debouncedQuery, results]);

  const handleFiltersChange = (newFilters: SearchFilters) => {
    logger.debug("SearchWindow: Filters changed", newFilters);
    setFilters(newFilters);
  };

  const handleFiltersReset = () => {
    logger.debug("SearchWindow: Filters reset");
    setFilters({});
  };

  return (
    <div className="search-window">
      <div className="search-container">
        <input
          type="text"
          className="search-input"
          placeholder="Search snippets..."
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
          }}
          autoFocus
        />
        <SearchFiltersComponent
          filters={filters}
          onChange={handleFiltersChange}
          onReset={handleFiltersReset}
        />
        {loading && (
          <div className="loading">
            <div className="loading-spinner"></div>
            <span>Searching...</span>
          </div>
        )}
        {!loading && results && (
          <SearchResults
            results={results.combined}
            query={debouncedQuery}
            onSelect={(id) => {
              logger.debug("SearchWindow: Snippet selected", { id });
              if (onNavigateToSnippet) {
                onNavigateToSnippet(id);
              }
            }}
            onDelete={handleDelete}
          />
        )}
        {!loading && debouncedQuery && !results && (
          <div className="no-results">No results found</div>
        )}
      </div>
    </div>
  );
}
