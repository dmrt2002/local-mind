import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import ConfirmDialog from "./ConfirmDialog";
import SnippetEditDialog from "./SnippetEditDialog";
import { highlightMatches } from "../utils/highlight";
import { useListNavigation, useKeyboardNavigation } from "../hooks/useKeyboardNavigation";
import "./SearchResults.css";

interface SearchResult {
  id: number;
  content: string;
  created_at: string;
  source_app: string | null;
  rank: number;
  match_type: string;
}

interface SearchResultsProps {
  results: SearchResult[];
  onSelect: (id: number) => void;
  onDelete?: (id: number) => void;
  query?: string; // Current search query for highlighting
}

export default function SearchResults({
  results,
  onSelect,
  onDelete,
  query = "",
}: SearchResultsProps) {
  const [confirmingDelete, setConfirmingDelete] = useState<number | null>(null);
  const [editingSnippet, setEditingSnippet] = useState<number | null>(null);
  const resultRefs = useRef<(HTMLDivElement | null)[]>([]);

  // Keyboard navigation
  const {
    selectedIndex,
    moveUp,
    moveDown,
    selectCurrent,
    selectedItem,
  } = useListNavigation(results, {
    initialIndex: -1,
    loop: true,
    onSelect: (result) => {
      onSelect(result.id);
    },
  });

  // Handle keyboard shortcuts
  useKeyboardNavigation({
    enabled: results.length > 0,
    onArrowUp: moveUp,
    onArrowDown: moveDown,
    onEnter: selectCurrent,
    onCtrlE: () => {
      if (selectedItem) {
        setEditingSnippet(selectedItem.id);
      }
    },
    onCtrlD: () => {
      if (selectedItem) {
        setConfirmingDelete(selectedItem.id);
      }
    },
  });

  // Scroll selected item into view
  useEffect(() => {
    if (selectedIndex >= 0 && selectedIndex < resultRefs.current.length) {
      const element = resultRefs.current[selectedIndex];
      element?.scrollIntoView({ behavior: "smooth", block: "nearest" });
    }
  }, [selectedIndex]);

  const handleDeleteClick = (
    e: React.MouseEvent<HTMLButtonElement>,
    id: number
  ) => {
    e.preventDefault();
    e.stopPropagation(); // Prevent triggering onSelect
    console.log("Delete button clicked for snippet:", id);
    setConfirmingDelete(id);
  };

  const handleConfirmDelete = async () => {
    if (confirmingDelete === null) return;

    const id = confirmingDelete;
    console.log("User confirmed deletion for snippet:", id);
    setConfirmingDelete(null);

    try {
      const result = await invoke<boolean>("delete_snippet", { id });
      console.log("Delete result:", result);
      if (result) {
        console.log("✓ Snippet deleted successfully");
        onDelete?.(id);
      } else {
        console.warn("Snippet not found or already deleted:", id);
        // Still refresh UI to clear stale results
        onDelete?.(id);
      }
    } catch (error) {
      console.error("Failed to delete snippet:", error);
      // Error will be logged, UI will refresh anyway
      onDelete?.(id); // Refresh UI
    }
  };

  const handleCancelDelete = () => {
    console.log("User cancelled deletion");
    setConfirmingDelete(null);
  };

  const handleEditClick = (
    e: React.MouseEvent<HTMLButtonElement>,
    id: number
  ) => {
    e.preventDefault();
    e.stopPropagation();
    console.log("Edit button clicked for snippet:", id);
    setEditingSnippet(id);
  };

  const handleEditSave = () => {
    console.log("Snippet edit saved, refreshing results");
    // Trigger refresh by calling onSelect or onDelete (which refreshes results)
    onDelete?.(editingSnippet!);
  };

  if (results.length === 0) {
    return (
      <div className="no-results">
        No snippets found. Try a different search term.
      </div>
    );
  }

  return (
    <>
      {confirmingDelete !== null && (
        <ConfirmDialog
          message="Delete this snippet?"
          onConfirm={handleConfirmDelete}
          onCancel={handleCancelDelete}
        />
      )}
      {editingSnippet !== null && (
        <SnippetEditDialog
          snippetId={editingSnippet}
          onClose={() => setEditingSnippet(null)}
          onSave={handleEditSave}
        />
      )}
      <div className="search-results">
        {results.map((result, index) => (
          <div
            key={result.id}
            ref={(el) => (resultRefs.current[index] = el)}
            className={`result-item ${selectedIndex === index ? "selected" : ""}`}
            onClick={() => onSelect(result.id)}
          >
            <div className="result-header">
              <span className="match-type">{result.match_type}</span>
              {result.source_app && (
                <span className="source-app">{result.source_app}</span>
              )}
              <span className="date">
                {new Date(result.created_at).toLocaleDateString()}
              </span>
              <button
                type="button"
                className="edit-button"
                onClick={(e) => handleEditClick(e, result.id)}
                title="Edit snippet"
                aria-label="Edit snippet"
              >
                ✎
              </button>
              <button
                type="button"
                className="delete-button"
                onClick={(e) => handleDeleteClick(e, result.id)}
                title="Delete snippet"
                aria-label="Delete snippet"
              >
                ×
              </button>
            </div>
            <div
              className="result-content"
              dangerouslySetInnerHTML={{
                __html: highlightMatches(result.content, query, 200),
              }}
            />
          </div>
        ))}
      </div>
    </>
  );
}
