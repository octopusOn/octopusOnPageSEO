// Shows unexpected rendering errors instead of a blank window.

import { Component, type ErrorInfo, type ReactNode } from "react";

interface State {
  error: Error | null;
}

export class ErrorBoundary extends Component<{ children: ReactNode; resetKey?: string }, State> {
  override state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  override componentDidCatch(error: Error, info: ErrorInfo) {
    console.error(error, info.componentStack);
  }

  override componentDidUpdate(previous: { resetKey?: string }) {
    if (previous.resetKey !== this.props.resetKey && this.state.error) this.setState({ error: null });
  }

  override render() {
    if (!this.state.error) return this.props.children;
    return (
      <div className="page">
        <div className="banner error" role="alert">
          <div className="grow">
            <strong>{this.state.error.name}</strong>: {this.state.error.message}
            <pre>{this.state.error.stack}</pre>
          </div>
        </div>
      </div>
    );
  }
}
