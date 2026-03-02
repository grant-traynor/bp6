import { useEffect } from "react";
import { saveWindowState } from "../api";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useSessionStore } from "../stores/sessionStore";
import { Terminal } from "../components/terminal/Terminal";

interface SessionWindowViewProps {
  sessionId: string;
}

export function SessionWindowView({ sessionId }: SessionWindowViewProps) {
  const sessions = useSessionStore((state) => state.sessions);
  const sessionMeta = sessions.find((s) => s.sessionId === sessionId);

  const sessionPersona = sessionMeta?.persona || "specialist";
  const sessionBeadId = sessionMeta?.beadId || null;
  const sessionBackendId = sessionMeta?.backendId || "claude";
  const sessionTask = sessionMeta?.task || null;
  const sessionProjectPath = sessionMeta?.projectPath || null;

  // Update window title from session metadata
  useEffect(() => {
    const title = [
      sessionBeadId || "Untracked",
      sessionPersona,
      sessionTask,
      sessionBackendId,
    ]
      .filter(Boolean)
      .join(" · ");
    getCurrentWindow()
      .setTitle(title)
      .catch((err) => console.error("Failed to set window title:", err));
  }, [sessionBeadId, sessionPersona, sessionTask, sessionBackendId]);

  // Window state persistence — debounced save on resize/move
  useEffect(() => {
    const saveCurrentState = async () => {
      try {
        const win = getCurrentWindow();
        const position = await win.outerPosition();
        const size = await win.outerSize();
        const isMaximized = await win.isMaximized();
        await saveWindowState(
          sessionId,
          position.x,
          position.y,
          size.width,
          size.height,
          isMaximized
        );
      } catch (error) {
        console.error("Failed to save window state:", error);
      }
    };

    let saveTimeout: ReturnType<typeof setTimeout> | null = null;
    const debouncedSave = () => {
      if (saveTimeout) clearTimeout(saveTimeout);
      saveTimeout = setTimeout(saveCurrentState, 500);
    };

    let unlistenResize: (() => void) | null = null;
    let unlistenMove: (() => void) | null = null;

    const setupListeners = async () => {
      const win = getCurrentWindow();
      unlistenResize = await win.onResized(debouncedSave);
      unlistenMove = await win.onMoved(debouncedSave);
    };

    setupListeners();

    return () => {
      if (saveTimeout) clearTimeout(saveTimeout);
      if (unlistenResize) unlistenResize();
      if (unlistenMove) unlistenMove();
      saveCurrentState();
    };
  }, [sessionId]);

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[var(--background-primary)]">
      <Terminal sessionId={sessionId} projectPath={sessionProjectPath || undefined} />
    </div>
  );
}
