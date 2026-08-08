import { Component, type ErrorInfo, type ReactNode, useEffect, useState } from "react";

type ReportError = (error: Error) => void;

type BoundaryProps = {
  children: ReactNode;
  reportError: ReportError;
};

type BoundaryState = {
  error: Error | null;
};

function WebviewErrorFallback({ error }: { error: Error }) {
  return (
    <main aria-label="Tomcat webview error" className="tc-webview-error" data-testid="webview-error-fallback">
      <h1>Tomcat could not render this view</h1>
      <p>The error was reported to the extension host. Reload this view to try again.</p>
      <pre>{error.message || "Unknown webview error"}</pre>
      <button onClick={() => window.location.reload()} type="button">
        Reload
      </button>
    </main>
  );
}

class RenderErrorBoundary extends Component<BoundaryProps, BoundaryState> {
  state: BoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): BoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, _info: ErrorInfo): void {
    this.props.reportError(error);
  }

  render() {
    return this.state.error ? (
      <WebviewErrorFallback error={this.state.error} />
    ) : (
      this.props.children
    );
  }
}

/**
 * Keeps a render exception, a synchronous browser error, and a rejected async
 * operation from degrading the whole webview to an unhelpful blank screen.
 */
export function WebviewErrorBoundary({ children, reportError }: BoundaryProps) {
  const [globalError, setGlobalError] = useState<Error | null>(null);

  useEffect(() => {
    const report = (error: unknown) => {
      const normalized = error instanceof Error ? error : new Error(String(error));
      reportError(normalized);
      setGlobalError(normalized);
    };
    const onError = (event: ErrorEvent) => report(event.error ?? event.message);
    const onUnhandledRejection = (event: PromiseRejectionEvent) => report(event.reason);

    window.addEventListener("error", onError);
    window.addEventListener("unhandledrejection", onUnhandledRejection);
    return () => {
      window.removeEventListener("error", onError);
      window.removeEventListener("unhandledrejection", onUnhandledRejection);
    };
  }, [reportError]);

  return globalError ? (
    <WebviewErrorFallback error={globalError} />
  ) : (
    <RenderErrorBoundary reportError={reportError}>{children}</RenderErrorBoundary>
  );
}
