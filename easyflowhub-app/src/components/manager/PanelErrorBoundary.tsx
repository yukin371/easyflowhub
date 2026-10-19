/**
 * PanelErrorBoundary - keeps one crashing manager panel from taking down the
 * whole shell (sidebar included).
 *
 * Manager panels are rendered from a dynamic registry, so a single panel can
 * throw on unexpected service data. Without a boundary React unmounts the
 * entire tree, which is what produced the blank screen with no sidebar.
 */

import { Component, type ErrorInfo, type ReactNode } from 'react';

interface PanelErrorBoundaryProps {
  panelId: string;
  children: ReactNode;
}

interface PanelErrorBoundaryState {
  error: Error | null;
}

export class PanelErrorBoundary extends Component<
  PanelErrorBoundaryProps,
  PanelErrorBoundaryState
> {
  state: PanelErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): PanelErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    // Keep the diagnostic available for developers without echoing raw service
    // payloads into the UI.
    console.error(`Manager panel "${this.props.panelId}" crashed:`, error, info.componentStack);
  }

  private handleRetry = () => {
    this.setState({ error: null });
  };

  render() {
    const { error } = this.state;
    if (!error) {
      return this.props.children;
    }

    return (
      <section className="flex h-full items-center justify-center px-6 py-10">
        <div role="alert" className="max-w-md rounded-[18px] border border-[color:var(--manager-border)] bg-[color:var(--manager-panel)] px-6 py-5">
          <p className="text-sm font-medium text-[color:var(--manager-ink-strong)]">
            该面板加载失败，其他页面仍可正常使用。
          </p>
          <p className="mt-2 break-words text-xs leading-6 text-[color:var(--manager-ink-muted)]">
            {error.message}
          </p>
          <button
            type="button"
            onClick={this.handleRetry}
            className="mt-4 rounded-full border border-[color:var(--manager-border)] bg-[color:var(--manager-panel-strong)] px-4 py-2 text-sm text-[color:var(--manager-ink-soft)] transition hover:border-[color:var(--manager-accent)] hover:text-[color:var(--manager-ink-strong)]"
          >
            重试
          </button>
        </div>
      </section>
    );
  }
}
