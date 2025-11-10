import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import "./SearchFilters.css";

export interface SearchFilters {
  date_from?: string;
  date_to?: string;
  source_app?: string;
  has_embedding?: boolean;
}

interface SearchFiltersProps {
  filters: SearchFilters;
  onChange: (filters: SearchFilters) => void;
  onReset: () => void;
}

export default function SearchFilters({ filters, onChange, onReset }: SearchFiltersProps) {
  const [isExpanded, setIsExpanded] = useState(false);
  const [sourceApps, setSourceApps] = useState<string[]>([]);

  // Fetch available source apps on mount
  useEffect(() => {
    invoke<string[]>("get_source_apps")
      .then((apps) => {
        setSourceApps(apps);
      })
      .catch((err) => {
        console.error("Failed to fetch source apps:", err);
      });
  }, []);

  const handleDateFromChange = (value: string) => {
    onChange({
      ...filters,
      date_from: value ? new Date(value).toISOString() : undefined,
    });
  };

  const handleDateToChange = (value: string) => {
    onChange({
      ...filters,
      date_to: value ? new Date(value).toISOString() : undefined,
    });
  };

  const handleSourceAppChange = (value: string) => {
    onChange({
      ...filters,
      source_app: value === "" ? undefined : value,
    });
  };

  const handleEmbeddingFilterChange = (value: string) => {
    onChange({
      ...filters,
      has_embedding: value === "all" ? undefined : value === "with",
    });
  };

  const hasActiveFilters = !!(
    filters.date_from ||
    filters.date_to ||
    filters.source_app ||
    filters.has_embedding !== undefined
  );

  // Convert ISO string back to date input format (YYYY-MM-DD)
  const dateFromValue = filters.date_from
    ? filters.date_from.split("T")[0]
    : "";
  const dateToValue = filters.date_to ? filters.date_to.split("T")[0] : "";

  return (
    <div className="search-filters">
      <button
        className={`filter-toggle ${hasActiveFilters ? "active" : ""}`}
        onClick={() => setIsExpanded(!isExpanded)}
        title="Filter search results"
      >
        <svg
          width="16"
          height="16"
          viewBox="0 0 16 16"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <path
            d="M2 4h12M4 8h8M6 12h4"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
          />
        </svg>
        Filters
        {hasActiveFilters && <span className="filter-badge"></span>}
      </button>

      {isExpanded && (
        <div className="filter-panel">
          <div className="filter-header">
            <h3>Search Filters</h3>
            {hasActiveFilters && (
              <button className="reset-filters" onClick={onReset}>
                Reset All
              </button>
            )}
          </div>

          <div className="filter-groups-container">
            <div className="filter-group">
              <label htmlFor="date-from">From Date</label>
              <input
                id="date-from"
                type="date"
                value={dateFromValue}
                onChange={(e) => handleDateFromChange(e.target.value)}
                max={dateToValue || undefined}
              />
            </div>

            <div className="filter-group">
              <label htmlFor="date-to">To Date</label>
              <input
                id="date-to"
                type="date"
                value={dateToValue}
                onChange={(e) => handleDateToChange(e.target.value)}
                min={dateFromValue || undefined}
              />
            </div>

            <div className="filter-group">
              <label htmlFor="source-app">Source App</label>
              <select
                id="source-app"
                value={filters.source_app || ""}
                onChange={(e) => handleSourceAppChange(e.target.value)}
              >
                <option value="">All apps</option>
                {sourceApps.map((app) => (
                  <option key={app} value={app}>
                    {app}
                  </option>
                ))}
              </select>
              <small className="filter-hint">
                Filter by the app where text was copied from
              </small>
            </div>

            <div className="filter-group">
              <label htmlFor="embedding-filter">Embedding Status</label>
              <select
                id="embedding-filter"
                value={
                  filters.has_embedding === undefined
                    ? "all"
                    : filters.has_embedding
                    ? "with"
                    : "without"
                }
                onChange={(e) => handleEmbeddingFilterChange(e.target.value)}
              >
                <option value="all">All snippets</option>
                <option value="with">With embeddings only</option>
                <option value="without">Without embeddings only</option>
              </select>
              <small className="filter-hint">
                Embeddings enable semantic search (meaning-based)
              </small>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
