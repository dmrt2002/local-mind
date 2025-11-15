import { useState } from "react";
import { Snippet } from "./SnippetCard";
import "./CommandCard.css";

interface CommandCardProps {
  snippet: Snippet;
  onUpdate?: () => void;
}

export default function CommandCard({ snippet }: CommandCardProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(snippet.content);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error("Failed to copy:", err);
    }
  };

  const formatDate = (dateStr: string) => {
    const date = new Date(dateStr);
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const days = Math.floor(diff / (1000 * 60 * 60 * 24));

    if (days === 0) {
      const hours = Math.floor(diff / (1000 * 60 * 60));
      if (hours === 0) {
        const minutes = Math.floor(diff / (1000 * 60));
        return `${minutes}m ago`;
      }
      return `${hours}h ago`;
    } else if (days === 1) {
      return "Yesterday";
    } else if (days < 7) {
      return `${days}d ago`;
    } else {
      return date.toLocaleDateString();
    }
  };

  const getExitCodeBadge = (exitCode?: number) => {
    if (exitCode === undefined || exitCode === null) return null;

    return (
      <span className={`exit-code ${exitCode === 0 ? "success" : "error"}`}>
        {exitCode === 0 ? "✓" : "✗"} {exitCode}
      </span>
    );
  };

  return (
    <div className="command-card">
      <div className="command-header">
        <div className="command-meta">
          {snippet.working_directory && (
            <div className="working-dir" title="Working Directory">
              <svg
                width="12"
                height="12"
                viewBox="0 0 16 16"
                fill="none"
                xmlns="http://www.w3.org/2000/svg"
              >
                <path
                  d="M2 3.5C2 2.94772 2.44772 2.5 3 2.5H6.58579C6.851 2.5 7.10536 2.60536 7.29289 2.79289L9.20711 4.70711C9.39464 4.89464 9.649 5 9.91421 5H13C13.5523 5 14 5.44772 14 6V12.5C14 13.0523 13.5523 13.5 13 13.5H3C2.44772 13.5 2 13.0523 2 12.5V3.5Z"
                  stroke="currentColor"
                  strokeWidth="1.5"
                />
              </svg>
              <span className="dir-path">{snippet.working_directory}</span>
            </div>
          )}
          <span className="timestamp">{formatDate(snippet.created_at)}</span>
          {getExitCodeBadge(snippet.exit_code)}
        </div>
        <div className="command-actions">
          <button
            className="icon-button"
            onClick={handleCopy}
            title="Copy command"
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
                  strokeWidth="1.5"
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
                  d="M5.5 4.5H4C3.44772 4.5 3 4.94772 3 5.5V12C3 12.5523 3.44772 13 4 13H10.5C11.0523 13 11.5 12.5523 11.5 12V5.5C11.5 4.94772 11.0523 4.5 10.5 4.5H9"
                  stroke="currentColor"
                  strokeWidth="1.5"
                />
                <path
                  d="M5.5 4.5C5.5 3.94772 5.94772 3.5 6.5 3.5H8C8.55228 3.5 9 3.94772 9 4.5V5C9 5.55228 8.55228 6 8 6H6.5C5.94772 6 5.5 5.55228 5.5 5V4.5Z"
                  stroke="currentColor"
                  strokeWidth="1.5"
                />
              </svg>
            )}
          </button>
        </div>
      </div>
      <div className="command-content">
        <pre>
          <code className="language-bash">{snippet.content}</code>
        </pre>
      </div>
      {snippet.source_app && (
        <div className="command-footer">
          <span className="source-app">via {snippet.source_app}</span>
        </div>
      )}
    </div>
  );
}
