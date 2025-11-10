import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import "./SnippetEditDialog.css";

interface Snippet {
  id: number;
  content: string;
  summary: string | null;
  created_at: string;
  updated_at: string | null;
  source_app: string | null;
  metadata: string | null;
}

interface SnippetEditDialogProps {
  snippetId: number;
  onClose: () => void;
  onSave: () => void;
}

export default function SnippetEditDialog({
  snippetId,
  onClose,
  onSave,
}: SnippetEditDialogProps) {
  const [snippet, setSnippet] = useState<Snippet | null>(null);
  const [editedContent, setEditedContent] = useState("");
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // Fetch snippet details
    invoke<Snippet>("get_snippet", { id: snippetId })
      .then((data) => {
        setSnippet(data);
        setEditedContent(data.content);
        setLoading(false);
      })
      .catch((err) => {
        console.error("Failed to load snippet:", err);
        setError("Failed to load snippet");
        setLoading(false);
      });
  }, [snippetId]);

  const handleSave = async () => {
    if (editedContent.trim() === snippet?.content.trim()) {
      // No changes made
      onClose();
      return;
    }

    setSaving(true);
    setError(null);

    try {
      await invoke("edit_snippet", {
        id: snippetId,
        newContent: editedContent,
      });
      console.log("✓ Snippet edited successfully");
      onSave();
      onClose();
    } catch (err) {
      console.error("Failed to save snippet:", err);
      setError(`Failed to save: ${err}`);
      setSaving(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      onClose();
    } else if (e.key === "s" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      handleSave();
    }
  };

  if (loading) {
    return (
      <div className="dialog-overlay">
        <div className="edit-dialog">
          <div className="loading">Loading...</div>
        </div>
      </div>
    );
  }

  if (error && !snippet) {
    return (
      <div className="dialog-overlay">
        <div className="edit-dialog">
          <div className="error">{error}</div>
          <button onClick={onClose}>Close</button>
        </div>
      </div>
    );
  }

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="edit-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-header">
          <h2>Edit Snippet</h2>
          <button className="close-button" onClick={onClose}>
            ×
          </button>
        </div>

        <div className="dialog-body">
          {snippet && (
            <div className="snippet-meta">
              <span>
                Created: {new Date(snippet.created_at).toLocaleString()}
              </span>
              {snippet.updated_at && (
                <span>
                  {" "}
                  | Updated: {new Date(snippet.updated_at).toLocaleString()}
                </span>
              )}
              {snippet.source_app && <span> | From: {snippet.source_app}</span>}
            </div>
          )}

          <textarea
            className="edit-textarea"
            value={editedContent}
            onChange={(e) => setEditedContent(e.target.value)}
            onKeyDown={handleKeyDown}
            autoFocus
            rows={15}
          />

          {error && <div className="error-message">{error}</div>}

          <div className="dialog-footer">
            <button className="cancel-button" onClick={onClose}>
              Cancel (Esc)
            </button>
            <button
              className="save-button"
              onClick={handleSave}
              disabled={saving || editedContent.trim() === snippet?.content.trim()}
            >
              {saving ? "Saving..." : "Save (⌘S)"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
