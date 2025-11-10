import "./StatusIndicator.css";

interface StatusIndicatorProps {
  message: string;
  type?: "info" | "warning" | "error" | "success";
}

export default function StatusIndicator({
  message,
  type = "info",
}: StatusIndicatorProps) {
  if (!message) {
    return null;
  }

  return <div className={`status-indicator status-${type}`}>{message}</div>;
}
