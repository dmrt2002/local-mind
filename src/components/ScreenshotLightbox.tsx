import { useEffect } from "react";
import { convertFileSrc } from "@tauri-apps/api/tauri";
import { Snippet } from "./SnippetCard";
import "./ScreenshotLightbox.css";

interface ScreenshotLightboxProps {
  snippet: Snippet;
  onClose: () => void;
}

export default function ScreenshotLightbox({ snippet, onClose }: ScreenshotLightboxProps) {
  useEffect(() => {
    const handleEsc = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", handleEsc);
    return () => window.removeEventListener("keydown", handleEsc);
  }, [onClose]);

  const getImageUrl = () => {
    if (!snippet.file_path) {
      console.warn("Lightbox: No file_path for screenshot:", snippet.id);
      return "";
    }
    try {
      const url = convertFileSrc(snippet.file_path);
      console.log("Lightbox URL conversion:", {
        id: snippet.id,
        originalPath: snippet.file_path,
        convertedUrl: url
      });
      return url;
    } catch (err) {
      console.error("Lightbox: Failed to convert file path:", err, snippet.file_path);
      return "";
    }
  };

  const formatDate = (dateStr: string) => {
    const date = new Date(dateStr);
    return date.toLocaleString();
  };

  // Parse combined text to extract caption and OCR
  const parseContent = (content: string) => {
    const captionMatch = content.match(/\[Caption: (.*?)\]/);
    const textMatch = content.match(/\[Text: (.*?)\](?:\s*\[|$)/s);

    return {
      caption: captionMatch ? captionMatch[1] : null,
      ocrText: textMatch ? textMatch[1] : null,
    };
  };

  const { caption, ocrText } = parseContent(snippet.content);

  return (
    <div className="screenshot-lightbox" onClick={onClose}>
      <div className="lightbox-overlay" />
      <div className="lightbox-content" onClick={(e) => e.stopPropagation()}>
        <button className="lightbox-close" onClick={onClose} title="Close (Esc)">
          <svg
            width="20"
            height="20"
            viewBox="0 0 16 16"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
          >
            <path
              d="M12 4L4 12M4 4L12 12"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
            />
          </svg>
        </button>

        <div className="lightbox-body">
          <div className="lightbox-image">
            <img src={getImageUrl()} alt={snippet.summary || "Screenshot"} />
          </div>

          <div className="lightbox-sidebar">
            <div className="lightbox-header">
              <h3>{snippet.summary || "Screenshot"}</h3>
              <p className="lightbox-date">{formatDate(snippet.created_at)}</p>
            </div>

            {snippet.website_url && (
              <div className="lightbox-section">
                <div className="section-label">Website</div>
                <a
                  href={snippet.website_url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="website-link"
                  onClick={(e) => e.stopPropagation()}
                >
                  <svg
                    width="14"
                    height="14"
                    viewBox="0 0 16 16"
                    fill="none"
                    xmlns="http://www.w3.org/2000/svg"
                  >
                    <path
                      d="M12 8.66667V12.6667C12 13.0203 11.8595 13.3594 11.6095 13.6095C11.3594 13.8595 11.0203 14 10.6667 14H3.33333C2.97971 14 2.64057 13.8595 2.39052 13.6095C2.14048 13.3594 2 13.0203 2 12.6667V5.33333C2 4.97971 2.14048 4.64057 2.39052 4.39052C2.64057 4.14048 2.97971 4 3.33333 4H7.33333"
                      stroke="currentColor"
                      strokeWidth="1.5"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                    <path
                      d="M10 2H14V6"
                      stroke="currentColor"
                      strokeWidth="1.5"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                    <path
                      d="M6.66667 9.33333L14 2"
                      stroke="currentColor"
                      strokeWidth="1.5"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                  </svg>
                  {snippet.website_title || new URL(snippet.website_url).hostname}
                </a>
              </div>
            )}

            {caption && (
              <div className="lightbox-section">
                <div className="section-label">Caption</div>
                <p className="section-content">{caption}</p>
              </div>
            )}

            {ocrText && (
              <div className="lightbox-section">
                <div className="section-label">Extracted Text (OCR)</div>
                <pre className="ocr-text">{ocrText}</pre>
              </div>
            )}

            {snippet.source_app && (
              <div className="lightbox-section">
                <div className="section-label">Source</div>
                <p className="section-content">{snippet.source_app}</p>
              </div>
            )}

            {snippet.file_path && (
              <div className="lightbox-section">
                <div className="section-label">File Path</div>
                <p className="section-content file-path">{snippet.file_path}</p>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
