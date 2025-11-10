import "./LoadingIndicator.css";

interface LoadingIndicatorProps {
  message?: string;
  size?: "small" | "medium" | "large";
}

export default function LoadingIndicator({
  message = "Loading...",
  size = "medium",
}: LoadingIndicatorProps) {
  return (
    <div className={`loading-indicator loading-${size}`}>
      <div className="spinner"></div>
      {message && <span className="loading-message">{message}</span>}
    </div>
  );
}
