import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@vscode/codicons/dist/codicon.css";

import { PreviewPanel } from "./PreviewPanel";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <PreviewPanel />
  </StrictMode>,
);
