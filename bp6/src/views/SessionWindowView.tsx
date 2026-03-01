import { useEffect } from "react";
import { type CliBackend, fetchBeads, saveWindowState } from "../api";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useSessionStore } from "../stores/sessionStore";
import ChatDialog from "../components/chat/ChatDialog";

interface SessionWindowViewProps {
  sessionId: string;
  sessionMetaIndex: Record<
    string,
    {
      persona: string;
      task?: string | null;
      beadId?: string | null;
      beadTitle?: string | null;
      beadDescription?: string | null;
      backendId?: CliBackend;
      role?: string | null;
      projectPath?: string | null;
    }
  >;
  setSessionMetaIndex: React.Dispatch<
    React.SetStateAction<
      Record<
        string,
        {
          persona: string;
          task?: string | null;
          beadId?: string | null;
          beadTitle?: string | null;
          beadDescription?: string | null;
          backendId?: CliBackend;
          role?: string | null;
          projectPath?: string | null;
        }
      >
    >
  >;
}

export function SessionWindowView({
  sessionId,
  sessionMetaIndex,
  setSessionMetaIndex,
}: SessionWindowViewProps) {
  const sessions = useSessionStore((state) => state.sessions);

  const sessionMeta = sessions.find((s) => s.sessionId === sessionId);
  const sessionPersona =
    sessionMeta?.persona ||
    sessionMetaIndex[sessionId]?.persona ||
    "product-manager";
  const sessionRole =
    sessionMeta?.role || sessionMetaIndex[sessionId]?.role || null;
  const sessionBeadId =
    sessionMeta?.beadId || sessionMetaIndex[sessionId]?.beadId || null;
  const sessionBackendId =
    (sessionMeta?.backendId as CliBackend) ||
    sessionMetaIndex[sessionId]?.backendId ||
    "gemini";
  const sessionTask =
    sessionMeta?.task || sessionMetaIndex[sessionId]?.task || "chat";
  const sessionBeadTitle = sessionMetaIndex[sessionId]?.beadTitle || null;
  const sessionBeadDescription =
    sessionMetaIndex[sessionId]?.beadDescription ?? null;

  // Effect 1: Fetch bead title/description if not yet cached
  useEffect(() => {
    if (
      !sessionBeadId ||
      (sessionBeadTitle && sessionBeadDescription !== undefined)
    )
      return;
    fetchBeads()
      .then((all) => {
        const bead = all.find((b) => b.id === sessionBeadId);
        if (bead?.title) {
          setSessionMetaIndex((prev) => ({
            ...prev,
            [sessionId]: {
              persona: sessionPersona,
              task: sessionTask,
              beadId: sessionBeadId,
              beadTitle: bead.title,
              beadDescription: bead.description ?? null,
              backendId: sessionBackendId,
              role: sessionRole,
            },
          }));
        }
      })
      .catch((err) =>
        console.error("Failed to fetch beads for title:", err)
      );
  }, [
    sessionBeadId,
    sessionBeadTitle,
    sessionBeadDescription,
    sessionId,
    sessionPersona,
    sessionTask,
    sessionBackendId,
    sessionRole,
    setSessionMetaIndex,
  ]);

  // Effect 2: Update window title
  useEffect(() => {
    const title = `${sessionBeadId || "Untracked"} · ${sessionBeadTitle || "Chat"} · ${sessionPersona}${sessionTask ? ` · ${sessionTask}` : ""} [${sessionBackendId}]`;
    getCurrentWindow()
      .setTitle(title)
      .catch((err) => console.error("Failed to set window title:", err));
  }, [
    sessionBeadId,
    sessionBeadTitle,
    sessionPersona,
    sessionTask,
    sessionBackendId,
  ]);

  // Effect 3: Window state persistence (bp6-643.005.5) — debounced save on resize/move
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

    // Debounced save — wait 500ms after last event
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

    // Cleanup: save final state on unmount (window close)
    return () => {
      if (saveTimeout) clearTimeout(saveTimeout);
      if (unlistenResize) unlistenResize();
      if (unlistenMove) unlistenMove();
      // Save final state synchronously
      saveCurrentState();
    };
  }, [sessionId]);

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[var(--background-primary)] text-[var(--text-primary)] font-sans">
      {/* Session window: fullscreen ChatDialog */}
      {/* TODO (bp6-643.005.4): Connect to existing session instead of creating new */}
      <ChatDialog
        isOpen={true}
        isSessionWindow={true}
        sessionIdOverride={sessionId}
        onClose={() => {
          // Session windows can't be closed from within - only via window close
          console.log("Session window ChatDialog close requested (no-op)");
        }}
        persona={sessionPersona}
        role={sessionRole}
        task={sessionTask || `Session window for session ${sessionId}`}
        beadId={sessionBeadId}
        beadTitle={sessionBeadTitle}
        beadDescription={sessionBeadDescription}
        cliBackend={sessionBackendId}
      />
    </div>
  );
}
