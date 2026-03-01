import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from '@tauri-apps/api/window';
import App from "./App";
import "./index.css";

// Apply saved theme ASAP to avoid flash/mismatch
const savedTheme = (() => {
  try {
    return localStorage.getItem('theme');
  } catch {
    return null;
  }
})();

if (savedTheme === 'dark') document.documentElement.classList.add('dark');
else if (savedTheme === 'light') document.documentElement.classList.remove('dark');

// Request microphone access so macOS registers this app for dictation.
// getUserMedia triggers the TCC permission prompt; we immediately stop the
// track — we don't actually use the mic directly, the system dictation
// service does. Without this, the app never appears in Privacy > Microphone.
navigator.mediaDevices?.getUserMedia({ audio: true })
  .then((stream) => stream.getTracks().forEach((t) => t.stop()))
  .catch(() => { /* user denied or unavailable — dictation may not work */ });

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
