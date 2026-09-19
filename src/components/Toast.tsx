import { useCallback, useRef, useState } from "react";
import type { ReactNode } from "react";
import { Icon } from "./Icon";
import { ToastContext } from "./toastContext";
import type { Notify, ToastTone } from "./toastContext";
import "./components.css";

interface ToastItem {
  id: number;
  message: string;
  tone: ToastTone;
}

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<ToastItem[]>([]);
  const nextId = useRef(0);

  const dismiss = useCallback((id: number) => {
    setToasts((current) => current.filter((t) => t.id !== id));
  }, []);

  const notify = useCallback<Notify>(
    (message, tone = "success") => {
      const id = nextId.current++;
      setToasts((current) => [...current.slice(-2), { id, message, tone }]);
      window.setTimeout(() => dismiss(id), tone === "error" ? 8000 : 4000);
    },
    [dismiss],
  );

  return (
    <ToastContext.Provider value={notify}>
      {children}
      <div className="hd-toasts" role="status" aria-live="polite">
        {toasts.map((toast) => (
          <div key={toast.id} className={`hd-toast hd-toast--${toast.tone}`}>
            <Icon name={toast.tone === "error" ? "alert" : "check"} size={16} strokeWidth={2} />
            <span>{toast.message}</span>
            <button
              type="button"
              className="hd-toast__close"
              aria-label="Dismiss"
              onClick={() => dismiss(toast.id)}
            >
              <Icon name="close" size={14} />
            </button>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}
