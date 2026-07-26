import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { PreviewPanel } from "./PreviewPanel";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <PreviewPanel />
  </StrictMode>,
);
