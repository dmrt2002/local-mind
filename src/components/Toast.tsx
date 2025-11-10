import { useEffect, useState } from "react";
import "./Toast.css";

interface ToastProps {
  message: string;
  type?: "success" | "error" | "info";
  duration?: number;
  onClose?: () => void;
}

export default function Toast({
  message,
  type = "info",
  duration = 3000,
  onClose,
}: ToastProps) {
  const [visible, setVisible] = useState(true);

  useEffect(() => {
    const timer = setTimeout(() => {
      setVisible(false);
      setTimeout(() => onClose?.(), 300); // Wait for fade out
    }, duration);

    return () => clearTimeout(timer);
  }, [duration, onClose]);

  if (!visible) {
    return null;
  }

  return (
    <div className={`toast toast-${type} ${visible ? "toast-visible" : ""}`}>
      <span className="toast-message">{message}</span>
    </div>
  );
}
