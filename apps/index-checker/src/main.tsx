import { Component, StrictMode, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { App } from "./App";
import { I18nProvider } from "./i18n";
import "@octopusseo/design";
import "./styles.css";

class ErrorBoundary extends Component<
  { children: ReactNode },
  { error: string }
> {
  state = { error: "" };
  static getDerivedStateFromError(error: Error) {
    return { error: error.message };
  }
  render() {
    return this.state.error ? (
      <div className="oc-banner error" role="alert">
        {this.state.error}
      </div>
    ) : (
      this.props.children
    );
  }
}

const client = new QueryClient({
  defaultOptions: {
    queries: { retry: false, refetchOnWindowFocus: false, staleTime: 15000 },
  },
});

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <QueryClientProvider client={client}>
        <I18nProvider>
          <ErrorBoundary>
            <App />
          </ErrorBoundary>
        </I18nProvider>
      </QueryClientProvider>
    </StrictMode>,
  );
}
