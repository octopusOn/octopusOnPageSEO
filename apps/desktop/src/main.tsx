import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

import { App } from "./App";
import { ToastProvider } from "./components/Toasts";
import { I18nProvider } from "./i18n";
import { AppStateProvider } from "./state";
import "./styles.css";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // Data changes only through this app; refetch on demand rather than on focus.
      refetchOnWindowFocus: false,
      staleTime: 60_000,
      retry: false,
    },
  },
});

async function start() {
  // In a plain browser during development, answer IPC calls with demo data.
  if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
    const { installMockBackend } = await import("./dev/mockBackend");
    installMockBackend();
  }
  const root = document.getElementById("root");
  if (!root) return;
  createRoot(root).render(
    <StrictMode>
      <QueryClientProvider client={queryClient}>
        <I18nProvider>
          <AppStateProvider>
            <ToastProvider>
              <App />
            </ToastProvider>
          </AppStateProvider>
        </I18nProvider>
      </QueryClientProvider>
    </StrictMode>,
  );
}

void start();
