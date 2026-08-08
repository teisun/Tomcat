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

ReactDOM.createRoot(root).render(
  <WebviewErrorBoundary reportError={reportWebviewError}>
    <App vscodeApi={vscodeApi} />
  </WebviewErrorBoundary>,
);
