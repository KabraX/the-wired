import React from "react";
import ReactDOM from "react-dom/client";
import { error as logError } from "@tauri-apps/plugin-log";
import App from "./App";

// Los fallos de la interfaz también van al registro (Ajustes → Copiar registro)
window.addEventListener("error", (e) => logError(`interfaz: ${e.message} (${e.filename}:${e.lineno})`));
window.addEventListener("unhandledrejection", (e) => logError(`interfaz: ${String(e.reason)}`));

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
