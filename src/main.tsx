import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ToastProvider } from "./components";
import { followSystemTheme } from "./lib/theme";
// Passive UI first, then Plutus's own layout (App.css, imported by App).
import "./kit/passive-ui.css";

followSystemTheme();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ToastProvider>
      <App />
    </ToastProvider>
  </React.StrictMode>,
);
