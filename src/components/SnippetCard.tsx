import { useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import ConfirmDialog from "./ConfirmDialog";
import "./SnippetCard.css";

export interface Snippet {
  id: number;
  content: string;
  summary?: string;
  created_at: string;
  source_app?: string;
  metadata?: string;
}

interface SnippetCardProps {
  snippet: Snippet;
  style?: React.CSSProperties;
  onUpdate?: () => void;
}

export default function SnippetCard({ snippet, style, onUpdate }: SnippetCardProps) {
  const [copied, setCopied] = useState(false);
  const [isEditing, setIsEditing] = useState(false);
  const [editedContent, setEditedContent] = useState(snippet.content);
  const [isSaving, setIsSaving] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(snippet.content);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  };

  const handleEdit = () => {
    setIsEditing(true);
    setEditedContent(snippet.content);
  };

  const handleCancelEdit = () => {
    setIsEditing(false);
    setEditedContent(snippet.content);
  };

  const handleSave = async () => {
    if (editedContent.trim() === snippet.content.trim()) {
      setIsEditing(false);
      return;
    }

    setIsSaving(true);
    try {
      await invoke("edit_snippet", {
        id: snippet.id,
        newContent: editedContent,
      });
      setIsEditing(false);
      snippet.content = editedContent;
      onUpdate?.();
    } catch (err) {
      console.error("Failed to save snippet:", err);
      alert(`Failed to save: ${err}`);
    } finally {
      setIsSaving(false);
    }
  };

  const handleDeleteClick = () => {
    setShowDeleteConfirm(true);
  };

  const handleConfirmDelete = async () => {
    setShowDeleteConfirm(false);
    setIsDeleting(true);

    try {
      await invoke("delete_snippet", {
        id: snippet.id,
      });
      onUpdate?.();
    } catch (err) {
      console.error("Failed to delete snippet:", err);
      alert(`Failed to delete: ${err}`);
      setIsDeleting(false);
    }
  };

  const handleCancelDelete = () => {
    setShowDeleteConfirm(false);
  };

  const formatDate = (dateStr: string) => {
    try {
      const date = new Date(dateStr);
      const now = new Date();
      const diffMs = now.getTime() - date.getTime();
      const diffMins = Math.floor(diffMs / 60000);
      const diffHours = Math.floor(diffMs / 3600000);
      const diffDays = Math.floor(diffMs / 86400000);

      if (diffMins < 1) return "Just now";
      if (diffMins < 60) return `${diffMins}m ago`;
      if (diffHours < 24) return `${diffHours}h ago`;
      if (diffDays < 7) return `${diffDays}d ago`;

      return date.toLocaleDateString();
    } catch {
      return dateStr;
    }
  };

  const parseMetadata = () => {
    if (!snippet.metadata) return null;
    try {
      return JSON.parse(snippet.metadata);
    } catch {
      return null;
    }
  };

  const metadata = parseMetadata();

  return (
    <div className="snippet-card" style={style}>
      <div className="snippet-header">
        <div className="snippet-meta">
          {snippet.source_app && (
            <span className="snippet-source">{snippet.source_app}</span>
          )}
          <span className="snippet-date">{formatDate(snippet.created_at)}</span>
        </div>
        <div className="snippet-actions">
          <button
            className="edit-button"
            onClick={handleEdit}
            title="Edit snippet"
            disabled={isEditing}
          >
            <svg
              width="16"
              height="16"
              viewBox="0 0 16 16"
              fill="none"
              xmlns="http://www.w3.org/2000/svg"
            >
              <path
                d="M11.5 2.5L13.5 4.5L5.5 12.5H3.5V10.5L11.5 2.5Z"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
              <path
                d="M10 4L12 6"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          </button>
          <button
            className={`copy-button ${copied ? "copied" : ""}`}
            onClick={handleCopy}
            title={copied ? "Copied!" : "Copy to clipboard"}
          >
            {copied ? (
              <svg
                width="16"
                height="16"
                viewBox="0 0 16 16"
                fill="none"
                xmlns="http://www.w3.org/2000/svg"
              >
                <path
                  d="M13 4L6 11L3 8"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            ) : (
              <svg
                width="16"
                height="16"
                viewBox="0 0 16 16"
                fill="none"
                xmlns="http://www.w3.org/2000/svg"
              >
                <path
                  d="M5.5 4.5H4C3.46957 4.5 2.96086 4.71071 2.58579 5.08579C2.21071 5.46086 2 5.96957 2 6.5V12C2 12.5304 2.21071 13.0391 2.58579 13.4142C2.96086 13.7893 3.46957 14 4 14H9.5C10.0304 14 10.5391 13.7893 10.9142 13.4142C11.2893 13.0391 11.5 12.5304 11.5 12V10.5"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
                <path
                  d="M6.5 2H12C12.5304 2 13.0391 2.21071 13.4142 2.58579C13.7893 2.96086 14 3.46957 14 4V9.5C14 10.0304 13.7893 10.5391 13.4142 10.9142C13.0391 11.2893 12.5304 11.5 12 11.5H6.5C5.96957 11.5 5.46086 11.2893 5.08579 10.9142C4.71071 10.5391 4.5 10.0304 4.5 9.5V4C4.5 3.46957 4.71071 2.96086 5.08579 2.58579C5.46086 2.21071 5.96957 2 6.5 2V2Z"
                  stroke="currentColor"
                  strokeWidth="1.5"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            )}
          </button>
          <button
            className="delete-button"
            onClick={handleDeleteClick}
            title="Delete snippet"
            disabled={isDeleting || isEditing}
          >
            <svg
              width="16"
              height="16"
              viewBox="0 0 16 16"
              fill="none"
              xmlns="http://www.w3.org/2000/svg"
            >
              <path
                d="M2 4H14"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
              <path
                d="M12.6667 4V13.3333C12.6667 13.7 12.5 14 12.3333 14H3.66667C3.5 14 3.33333 13.7 3.33333 13.3333V4"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
              <path
                d="M5.33331 4V2.66667C5.33331 2.3 5.49998 2 5.66665 2H10.3333C10.5 2 10.6666 2.3 10.6666 2.66667V4"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
              <path
                d="M6.66669 7.33331V10.6666"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
              <path
                d="M9.33331 7.33331V10.6666"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          </button>
        </div>
      </div>
      {isEditing ? (
        <div className="snippet-edit-container">
          <textarea
            className="snippet-edit-textarea"
            value={editedContent}
            onChange={(e) => setEditedContent(e.target.value)}
            autoFocus
          />
          <div className="snippet-edit-actions">
            <button
              className="edit-cancel-button"
              onClick={handleCancelEdit}
              disabled={isSaving}
            >
              Cancel
            </button>
            <button
              className="edit-save-button"
              onClick={handleSave}
              disabled={isSaving || editedContent.trim() === snippet.content.trim()}
            >
              {isSaving ? "Saving..." : "Save"}
            </button>
          </div>
        </div>
      ) : (
        <>
          <div className="snippet-content">
            {snippet.content}
          </div>
          {metadata && (
            <div className="snippet-context">
              {metadata.url && (
                <div className="context-item">
                  <span className="context-label">🔗 URL:</span>
                  <a href={metadata.url} className="context-link" target="_blank" rel="noopener noreferrer">
                    {metadata.url}
                  </a>
                </div>
              )}
              {metadata.file_path && (
                <div className="context-item">
                  <span className="context-label">📄 File:</span>
                  <span className="context-value">{metadata.file_path}</span>
                </div>
              )}
              {metadata.window_title && (
                <div className="context-item">
                  <span className="context-label">🪟 Window:</span>
                  <span className="context-value">{metadata.window_title}</span>
                </div>
              )}
            </div>
          )}
        </>
      )}
      {showDeleteConfirm && (
        <ConfirmDialog
          title="Delete Snippet"
          message="Are you sure you want to delete this snippet? This action cannot be undone."
          confirmText="Delete"
          cancelText="Cancel"
          onConfirm={handleConfirmDelete}
          onCancel={handleCancelDelete}
          variant="danger"
        />
      )}
    </div>
  );
}
