import { useEffect, useState } from "react";
import ReactDOM from "react-dom/client";

import { App } from "./App";
import { WebviewErrorBoundary } from "./WebviewErrorBoundary";
import "@vscode/codicons/dist/codicon.css";
import "./styles.css";
import type { VsCodeApiLike } from "./types";

declare global {
  interface Window {
    acquireVsCodeApi?: () => VsCodeApiLike;
  }
}

const vscodeApi: VsCodeApiLike =
  window.acquireVsCodeApi?.() ?? {
    postMessage() {},
    setState() {},
  };

const root = document.getElementById("root");
if (!root) {
  throw new Error("Tomcat webview root element was not found");
}

function reportWebviewError(error: Error): void {
  vscodeApi.postMessage({
    data: {
      message: error.message || "Unknown webview error",
      stack: error.stack,
    },
    messageId: `webview-error-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    type: "webviewError",
  });
}

const ERROR_BOUNDARY_CRASH_FIXTURE_TYPE = "__test.webview_error_boundary_crash";
const ERROR_BOUNDARY_CRASH_MESSAGE =
  "E2E fixture intentionally crashed the Tomcat webview";

function isErrorBoundaryCrashFixture(value: unknown): boolean {
  if (!value || typeof value !== "object") {
    return false;
  }
  const frame = value as {
    channel?: unknown;
    content?: { enabled?: unknown; type?: unknown };
  };
  return (
    frame.channel === "event" &&
    frame.content?.type === ERROR_BOUNDARY_CRASH_FIXTURE_TYPE &&
    frame.content.enabled === true
  );
}

/**
 * This hook only reacts to the extension host's test-only event channel. Production
 * hosts never emit that frame, so the component stays inert outside the packaged E2E.
 */
function ErrorBoundaryCrashFixture() {
  const [enabled, setEnabled] = useState(false);

  useEffect(() => {
    const onMessage = (event: MessageEvent<unknown>) => {
      if (isErrorBoundaryCrashFixture(event.data)) {
        setEnabled(true);
      }
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, []);

  if (enabled) {
    throw new Error(ERROR_BOUNDARY_CRASH_MESSAGE);
  }
  return null;
}

ReactDOM.createRoot(root).render(
  <WebviewErrorBoundary reportError={reportWebviewError}>
    <ErrorBoundaryCrashFixture />
    <App vscodeApi={vscodeApi} />
  </WebviewErrorBoundary>,
);
