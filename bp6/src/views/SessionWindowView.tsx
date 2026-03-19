import { useEffect, useState } from "react";
import { saveWindowState, PERSONA_ICONS, startAgentSession, createSessionWindow, terminateSession, type CliBackend } from "../api";
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
  const sessionBeadTitle = sessionMeta?.beadTitle || null;
  const sessionBeadDescriptionPreview = sessionMeta?.beadDescriptionPreview || null;
  const projectName = sessionProjectPath
    ? sessionProjectPath.split("/").filter(Boolean).pop() ?? null
    : null;

  const personaIcon = PERSONA_ICONS[sessionPersona] || PERSONA_ICONS[sessionMeta?.role || ""] || "🤖";
  // For specialists, show "{role} specialist" (e.g. "flutter specialist"); otherwise show persona as-is
  const sessionRole = sessionMeta?.role || null;
  const personaLabel = sessionPersona === "specialist" && sessionRole
    ? `${sessionRole} specialist`
    : sessionPersona;

  const [isRestarting, setIsRestarting] = useState(false);
  const [launchInBackground, setLaunchInBackground] = useState(
    () => localStorage.getItem("session-launch-in-bg") === "true"
  );

  const toggleBackground = () => {
    const next = !launchInBackground;
    setLaunchInBackground(next);
    localStorage.setItem("session-launch-in-bg", String(next));
  };

  const handleNewSession = async () => {
    if (isRestarting) return;
    setIsRestarting(true);
    try {
      const newSessionId = await startAgentSession(
        sessionPersona,
        sessionTask || undefined,
        sessionBeadId || undefined,
        sessionBackendId as CliBackend,
        sessionMeta?.role || undefined,
        sessionProjectPath || undefined,
        true // force_new — always a clean slate from the "+ New Session" button
      );
      await useSessionStore.getState().refreshSessions();
      if (!launchInBackground) {
        await createSessionWindow(newSessionId, getCurrentWindow().label);
      }
      await terminateSession(sessionId);
    } catch (error) {
      console.error("Failed to create new session:", error);
      setIsRestarting(false);
    }
  };

  // Update window title from session metadata
  useEffect(() => {
    const title = [
      sessionBeadId || "Untracked",
      personaLabel,
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
        // outerPosition/outerSize return physical pixels; convert to logical
        // so they match what WebviewWindowBuilder::position/inner_size expect.
        const scaleFactor = await win.scaleFactor();
        const physPos = await win.outerPosition();
        const physSize = await win.outerSize();
        const position = physPos.toLogical(scaleFactor);
        const size = physSize.toLogical(scaleFactor);
        const isMaximized = await win.isMaximized();
        await saveWindowState(
          sessionId,
          Math.round(position.x),
          Math.round(position.y),
          Math.round(size.width),
          Math.round(size.height),
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
    <div className="flex flex-col h-screen w-screen overflow-hidden bg-[var(--background-primary)]">
      {/* Session context header */}
      <div className="flex-shrink-0 border-b border-[var(--border-primary)] bg-[var(--background-secondary)] px-3 py-2 text-xs">
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-base leading-none">{personaIcon}</span>
          <span className="font-semibold text-[var(--text-primary)] capitalize whitespace-nowrap">
            {personaLabel}
          </span>
          {sessionTask && (
            <>
              <span className="text-[var(--text-muted)]">·</span>
              <span className="text-[var(--text-secondary)] truncate">{sessionTask}</span>
            </>
          )}
          <div className="ml-auto flex items-center gap-2 text-[var(--text-muted)] whitespace-nowrap">
            {projectName && (
              <span className="text-[var(--text-secondary)]">📁 {projectName}</span>
            )}
            <span>{sessionBackendId}</span>
          </div>
        </div>
        <div className="flex justify-end mt-1">
          <button
            onClick={handleNewSession}
            disabled={isRestarting}
            title="Close this session and start a fresh one with the same context"
            className="px-2 py-0.5 rounded text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-[var(--background-tertiary)] disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
          >
            {isRestarting ? "↻ Starting…" : "+ New Session"}
          </button>
        </div>
        {(sessionBeadId || sessionBeadTitle || sessionBeadDescriptionPreview) && (
          <div className="mt-1 pl-6 text-[var(--text-muted)]">
            {(sessionBeadId || sessionBeadTitle) && (
              <div className="flex items-center gap-1.5">
                {sessionBeadId && (
                  <span className="font-mono text-[var(--accent-primary)] opacity-80">{sessionBeadId}</span>
                )}
                {sessionBeadTitle && (
                  <span className="text-[var(--text-secondary)] truncate">{sessionBeadTitle}</span>
                )}
              </div>
            )}
            {sessionBeadDescriptionPreview && (
              <div className="mt-0.5 text-[10px] leading-snug text-[var(--text-muted)] opacity-70 line-clamp-2">
                {sessionBeadDescriptionPreview}
              </div>
            )}
          </div>
        )}
      </div>

      <div className="flex-1 min-h-0">
        <Terminal sessionId={sessionId} projectPath={sessionProjectPath || undefined} />
      </div>
    </div>
  );
}
