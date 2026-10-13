// Transient notifications.

import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from "react";
import { CheckCircle2, Info, X, XCircle } from "lucide-react";

type Tone = "success" | "error" | "info";

interface Toast {
  id: number;
  tone: Tone;
  text: string;
}

const ToastContext = createContext<((tone: Tone, text: string) => void) | null>(null);

let nextId = 1;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const dismiss = useCallback((id: number) => setToasts((all) => all.filter((t) => t.id !== id)), []);
  const notify = useCallback(
    (tone: Tone, text: string) => {
      const id = nextId++;
      setToasts((all) => [...all.slice(-3), { id, tone, text }]);
      window.setTimeout(() => dismiss(id), tone === "error" ? 9000 : 5000);
    },
    [dismiss],
  );
  const value = useMemo(() => notify, [notify]);
  return (
    <ToastContext.Provider value={value}>
      {children}
      <div className="toasts">
        {toasts.map((toast) => (
          <div key={toast.id} className={`toast ${toast.tone}`} role="status">
            {toast.tone === "success" ? (
              <CheckCircle2 size={18} color="var(--success)" />
            ) : toast.tone === "error" ? (
              <XCircle size={18} color="var(--danger)" />
            ) : (
              <Info size={18} color="var(--accent)" />
            )}
            <div className="grow">{toast.text}</div>
            <button className="btn ghost icon" type="button" onClick={() => dismiss(toast.id)}>
              <X size={14} />
            </button>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}

export function useToast() {
  const notify = useContext(ToastContext);
  if (!notify) throw new Error("useToast outside ToastProvider");
  return notify;
}
