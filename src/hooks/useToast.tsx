import { useState, useCallback } from "react";
import Toast from "../components/Toast";
import "./useToast.css";

interface ToastState {
  id: number;
  message: string;
  type: "success" | "error" | "info";
}

let toastIdCounter = 0;

export function useToast() {
  const [toasts, setToasts] = useState<ToastState[]>([]);

  const showToast = useCallback(
    (message: string, type: "success" | "error" | "info" = "info") => {
      const id = toastIdCounter++;
      setToasts((prev) => [...prev, { id, message, type }]);

      // Auto-remove after a delay
      setTimeout(() => {
        setToasts((prev) => prev.filter((t) => t.id !== id));
      }, 3000);
    },
    []
  );

  const removeToast = useCallback((id: number) => {
    setToasts((prev) => prev.filter((t) => t.id !== id));
  }, []);

  const ToastContainer = () => (
    <div className="toast-container">
      {toasts.map((toast) => (
        <Toast
          key={toast.id}
          message={toast.message}
          type={toast.type}
          onClose={() => removeToast(toast.id)}
        />
      ))}
    </div>
  );

  return { showToast, ToastContainer };
}
