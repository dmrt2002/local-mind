import { useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/tauri";
import { Snippet } from "./SnippetCard";
import "./ScreenshotCard.css";

interface ScreenshotCardProps {
  snippet: Snippet;
  onClick: () => void;
}

export default function ScreenshotCard({ snippet, onClick }: ScreenshotCardProps) {
  const [imageError, setImageError] = useState(false);

  // Helper to extract clean summary text from potentially malformed JSON
  const getSummaryText = (summary?: string): string => {
    if (!summary) return "Screenshot";

    // Detect if this looks like JSON
    if (summary.trim().startsWith('{') || summary.includes('"summary"')) {
      try {
        // Try to parse as JSON first
        const parsed = JSON.parse(summary);
        return parsed.summary || "Screenshot";
      } catch {
        // Malformed JSON - try to extract summary field with regex
        const match = summary.match(/"summary"\s*:\s*"([^"]+)"/);
        if (match && match[1]) {
          return match[1];
        }
        // If that fails, remove JSON characters and return cleaned text
        return summary
          .replace(/[{}"[\]]/g, '')
          .split(',')[0]
          .replace('summary:', '')
          .trim() || "Screenshot";
      }
    }

    return summary;
  };

  const formatDate = (dateStr: string) => {
    const date = new Date(dateStr);
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const days = Math.floor(diff / (1000 * 60 * 60 * 24));

    if (days === 0) {
      return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    } else if (days === 1) {
      return "Yesterday";
    } else if (days < 7) {
      return `${days}d ago`;
    } else {
      return date.toLocaleDateString();
    }
  };

  // Convert file path to Tauri asset URL
  const getImageUrl = () => {
    if (!snippet.file_path) {
      console.warn("No file_path for screenshot:", snippet.id);
      return "";
    }
    try {
      const url = convertFileSrc(snippet.file_path);
      console.log("Screenshot URL conversion:", {
        id: snippet.id,
        originalPath: snippet.file_path,
        convertedUrl: url
      });
      return url;
    } catch (err) {
      console.error("Failed to convert file path:", err, snippet.file_path);
      return "";
    }
  };

  const imageUrl = getImageUrl();

  return (
    <div className="screenshot-card" onClick={onClick}>
      <div className="screenshot-preview">
        {imageError || !imageUrl ? (
          <div className="screenshot-placeholder">
            <svg
              width="48"
              height="48"
              viewBox="0 0 16 16"
              fill="none"
              xmlns="http://www.w3.org/2000/svg"
            >
              <path
                d="M2 4.5C2 3.94772 2.44772 3.5 3 3.5H13C13.5523 3.5 14 3.94772 14 4.5V11.5C14 12.0523 13.5523 12.5 13 12.5H3C2.44772 12.5 2 12.0523 2 11.5V4.5Z"
                stroke="currentColor"
                strokeWidth="1.5"
              />
              <path
                d="M2 9L5.5 6L9 8.5L14 4.5"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
            <span>Image unavailable</span>
            {snippet.file_path && (
              <div style={{fontSize: '8px', color: '#666', marginTop: '4px', wordBreak: 'break-all', maxWidth: '200px'}}>
                {snippet.file_path}
              </div>
            )}
          </div>
        ) : (
          <img
            src={imageUrl}
            alt={getSummaryText(snippet.summary)}
            loading="lazy"
            onError={(e) => {
              console.error("Image load error for:", imageUrl, e);
              setImageError(true);
            }}
          />
        )}
      </div>
      <div className="screenshot-info">
        <div className="screenshot-title">
          {getSummaryText(snippet.summary) || snippet.website_title || "Screenshot"}
        </div>
        {snippet.website_url && (
          <div className="screenshot-url" title={snippet.website_url}>
            <svg
              width="10"
              height="10"
              viewBox="0 0 16 16"
              fill="none"
              xmlns="http://www.w3.org/2000/svg"
            >
              <path
                d="M8 15C11.866 15 15 11.866 15 8C15 4.13401 11.866 1 8 1C4.13401 1 1 4.13401 1 8C1 11.866 4.13401 15 8 15Z"
                stroke="currentColor"
                strokeWidth="1.5"
              />
              <path
                d="M1.5 8H14.5M10.5 8C10.5 11.866 9.433 15 8 15C6.567 15 5.5 11.866 5.5 8C5.5 4.13401 6.567 1 8 1C9.433 1 10.5 4.13401 10.5 8Z"
                stroke="currentColor"
                strokeWidth="1.5"
              />
            </svg>
            <span>{new URL(snippet.website_url).hostname}</span>
          </div>
        )}
        <div className="screenshot-meta">
          <span className="screenshot-date">{formatDate(snippet.created_at)}</span>
          {snippet.source_app && (
            <>
              <span className="separator">•</span>
              <span className="source-app">{snippet.source_app}</span>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
