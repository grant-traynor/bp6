import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from '@tauri-apps/api/window';
import App from "./App";
import "./index.css";

// Detect window type on initialization
const currentWindow = getCurrentWindow();
const windowLabel = currentWindow.label;

// Parse window context
// Main window label: "main"
// Session window label: "agent-session-{uuid}"
const isSessionWindow = windowLabel.startsWith('agent-session-');
const sessionId = isSessionWindow
  ? windowLabel.replace('agent-session-', '')
  : null;

console.log('🪟 Window context:', { windowLabel, isSessionWindow, sessionId });

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App
      isSessionWindow={isSessionWindow}
      sessionId={sessionId}
      windowLabel={windowLabel}
    />
  </React.StrictMode>,
);