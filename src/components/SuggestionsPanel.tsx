import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { logger } from "../utils/logger";
import "./SuggestionsPanel.css";

interface Suggestion {
  id: string;
  suggestion_type: { type: string };
  title: string;
  description: string;
  action: any;
  priority: string;
  metadata?: any;
}

export default function SuggestionsPanel() {
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [loading, setLoading] = useState(true);
  const [dismissing, setDismissing] = useState<string | null>(null);
  const [showAll, setShowAll] = useState(false);

  useEffect(() => {
    loadSuggestions();
  }, []);

  const loadSuggestions = async () => {
    setLoading(true);
    try {
      const data = await invoke<Suggestion[]>("get_suggestions");
      // Filter out dismissed suggestions from localStorage
      const dismissed = JSON.parse(localStorage.getItem("dismissedSuggestions") || "[]");
      const filtered = data.filter((s) => !dismissed.includes(s.id));
      setSuggestions(filtered);
    } catch (err) {
      logger.error("Failed to load suggestions", err);
    } finally {
      setLoading(false);
    }
  };

  const handleDismiss = async (suggestionId: string) => {
    setDismissing(suggestionId);
    try {
      await invoke("dismiss_suggestion", { suggestionId });

      // Save to localStorage
      const dismissed = JSON.parse(localStorage.getItem("dismissedSuggestions") || "[]");
      dismissed.push(suggestionId);
      localStorage.setItem("dismissedSuggestions", JSON.stringify(dismissed));

      // Remove from UI
      setSuggestions((prev) => prev.filter((s) => s.id !== suggestionId));
    } catch (err) {
      logger.error("Failed to dismiss suggestion", err);
    } finally {
      setDismissing(null);
    }
  };

  const getPriorityColor = (priority: string): string => {
    switch (priority) {
      case "High":
        return "high";
      case "Medium":
        return "medium";
      default:
        return "low";
    }
  };

  const getIcon = (type: string): string => {
    switch (type) {
      case "Duplicate":
        return "⚠️";
      case "AutoTag":
        return "🏷️";
      case "RelatedSnippets":
        return "🔗";
      case "Archive":
        return "📦";
      case "SmartCollection":
        return "📚";
      default:
        return "💡";
    }
  };

  if (loading) {
    return (
      <div className="suggestions-panel">
        <div className="suggestions-loading">Loading suggestions...</div>
      </div>
    );
  }

  if (suggestions.length === 0) {
    return (
      <div className="suggestions-panel">
        <div className="no-suggestions">
          <div className="no-suggestions-icon">✨</div>
          <div className="no-suggestions-text">No suggestions at the moment</div>
        </div>
      </div>
    );
  }

  const displayedSuggestions = showAll ? suggestions : suggestions.slice(0, 3);

  return (
    <div className="suggestions-panel">
      <div className="suggestions-header">
        <h3>💡 Smart Suggestions</h3>
        <button className="refresh-button" onClick={loadSuggestions}>
          Refresh
        </button>
      </div>

      <div className="suggestions-list">
        {displayedSuggestions.map((suggestion) => (
          <div
            key={suggestion.id}
            className={`suggestion-card priority-${getPriorityColor(suggestion.priority)}`}
          >
            <div className="suggestion-icon">
              {getIcon(suggestion.suggestion_type.type)}
            </div>
            <div className="suggestion-content">
              <div className="suggestion-title">{suggestion.title}</div>
              <div className="suggestion-description">{suggestion.description}</div>
            </div>
            <button
              className="dismiss-button"
              onClick={() => handleDismiss(suggestion.id)}
              disabled={dismissing === suggestion.id}
              title="Dismiss"
            >
              ×
            </button>
          </div>
        ))}
      </div>

      {suggestions.length > 3 && (
        <button
          className="show-more-button"
          onClick={() => setShowAll(!showAll)}
        >
          {showAll ? "Show Less" : `Show ${suggestions.length - 3} More`}
        </button>
      )}
    </div>
  );
}
