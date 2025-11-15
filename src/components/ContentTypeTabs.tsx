import "./ContentTypeTabs.css";

export type ContentType = "all" | "text" | "command" | "screenshot";

interface ContentTypeTabsProps {
  activeType: ContentType;
  onTypeChange: (type: ContentType) => void;
  counts?: {
    all: number;
    text: number;
    command: number;
    screenshot: number;
  };
}

export default function ContentTypeTabs({
  activeType,
  onTypeChange,
  counts,
}: ContentTypeTabsProps) {
  return (
    <div className="content-type-tabs">
      <button
        className={`content-type-tab ${activeType === "all" ? "active" : ""}`}
        onClick={() => onTypeChange("all")}
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 16 16"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <path
            d="M2 4H14M2 8H14M2 12H14"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
          />
        </svg>
        <span>All</span>
        {counts && <span className="count">{counts.all}</span>}
      </button>
      <button
        className={`content-type-tab ${activeType === "text" ? "active" : ""}`}
        onClick={() => onTypeChange("text")}
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 16 16"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <path
            d="M3 3H13V5H11V13H9V5H7V13H5V5H3V3Z"
            fill="currentColor"
          />
        </svg>
        <span>Snippets</span>
        {counts && <span className="count">{counts.text}</span>}
      </button>
      <button
        className={`content-type-tab ${activeType === "command" ? "active" : ""}`}
        onClick={() => onTypeChange("command")}
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 16 16"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <path
            d="M2 3.5L5.5 7L2 10.5M7 11H14"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
        <span>Commands</span>
        {counts && <span className="count">{counts.command}</span>}
      </button>
      <button
        className={`content-type-tab ${activeType === "screenshot" ? "active" : ""}`}
        onClick={() => onTypeChange("screenshot")}
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 16 16"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <path
            d="M14 4.5H11.5L10.5 2.5H5.5L4.5 4.5H2C1.72386 4.5 1.5 4.72386 1.5 5V13C1.5 13.2761 1.72386 13.5 2 13.5H14C14.2761 13.5 14.5 13.2761 14.5 13V5C14.5 4.72386 14.2761 4.5 14 4.5Z"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
          <circle
            cx="8"
            cy="9"
            r="2"
            stroke="currentColor"
            strokeWidth="1.5"
          />
        </svg>
        <span>Screenshots</span>
        {counts && <span className="count">{counts.screenshot}</span>}
      </button>
    </div>
  );
}
