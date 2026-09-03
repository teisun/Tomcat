import ReactDOM from "react-dom/client";

import "@vscode/codicons/dist/codicon.css";
import { acquireVsCodeApiLike } from "../../../src/shared/settingsProtocol";
import "../styles.css";
import { SettingsApp } from "./SettingsApp";

const root = document.getElementById("root");
if (!root) {
  throw new Error("Tomcat settings root element was not found");
}

ReactDOM.createRoot(root).render(
  <SettingsApp
    initialRoute={
      new URLSearchParams(window.location.search).get("route") === "connectors"
        ? "connectors"
        : "models"
    }
    vscodeApi={acquireVsCodeApiLike()}
  />,
);
