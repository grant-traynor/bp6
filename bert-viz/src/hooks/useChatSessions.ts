import { useState, useEffect, useCallback } from "react";
import {
  getCliPreference,
  startAgentSession,
  createSessionWindow,
  type CliBackend,
} from "../api";
import { useSessionStore } from "../stores/sessionStore";

type SessionMetaEntry = {
  persona: string;
  task?: string | null;
  beadId?: string | null;
  beadTitle?: string | null;
  beadDescription?: string | null;
  backendId?: CliBackend;
  role?: string | null;
  projectPath?: string | null;
};

export interface UseChatSessionsReturn {
  chatSessionMap: Record<string, string>;
  setChatSessionMap: React.Dispatch<React.SetStateAction<Record<string, string>>>;
  sessionMetaIndex: Record<string, SessionMetaEntry>;
  setSessionMetaIndex: React.Dispatch<React.SetStateAction<Record<string, SessionMetaEntry>>>;
  currentCli: CliBackend;
  setCurrentCli: React.Dispatch<React.SetStateAction<CliBackend>>;
  handleOpenChat: (
    persona: string,
    task?: string,
    beadId?: string,
    role?: string,
    beads?: { id: string; title?: string; description?: string }[],
    currentProjectPath?: string
  ) => Promise<void>;
}

/**
 * Manages chat session state: session map, session meta index, CLI preference.
 * Syncs dead sessions from sessionStore (evicts sessions no longer alive in backend).
 * Persists chatSessionMap and sessionMetaIndex to localStorage.
 */
export function useChatSessions(): UseChatSessionsReturn {
  // Persisted map: key (persona::task::beadId::role::cli) -> sessionId
  const [chatSessionMap, setChatSessionMap] = useState<Record<string, string>>(
    () => {
      if (typeof localStorage === "undefined") return {};
      try {
        return JSON.parse(localStorage.getItem("chatSessionMap") || "{}");
      } catch {
        return {};
      }
    }
  );

  // Metadata about each session (for session window title, task display)
  const [sessionMetaIndex, setSessionMetaIndex] = useState<
    Record<string, SessionMetaEntry>
  >(() => {
    if (typeof localStorage === "undefined") return {};
    try {
      return JSON.parse(localStorage.getItem("chatSessionMeta") || "{}");
    } catch {
      return {};
    }
  });

  const [currentCli, setCurrentCli] = useState<CliBackend>("gemini");

  // Load CLI preference on mount
  useEffect(() => {
    const loadCliPreference = async () => {
      try {
        const preference = await getCliPreference();
        setCurrentCli(preference as CliBackend);
      } catch (error) {
        console.error("Failed to load CLI preference:", error);
        // Keep default value (gemini) on error
      }
    };
    loadCliPreference();
  }, []);

  // Read live sessions from Zustand store
  const sessions = useSessionStore((state) => state.sessions);

  // Evict dead sessions: drop chatSessionMap/sessionMetaIndex entries for sessions
  // that are no longer alive in the backend.
  useEffect(() => {
    const activeIds = new Set(sessions.map((s) => s.sessionId));

    setChatSessionMap((prev) => {
      const next = { ...prev };
      let changed = false;
      Object.entries(prev).forEach(([key, sid]) => {
        if (!activeIds.has(sid)) {
          delete next[key];
          changed = true;
        }
      });
      return changed ? next : prev;
    });

    setSessionMetaIndex((prev) => {
      const next = { ...prev };
      let changed = false;
      Object.keys(prev).forEach((sid) => {
        if (!activeIds.has(sid)) {
          delete next[sid];
          changed = true;
        }
      });
      return changed ? next : prev;
    });
  }, [sessions]);

  // Persist chatSessionMap and sessionMetaIndex to localStorage
  useEffect(() => {
    if (typeof localStorage === "undefined") return;
    localStorage.setItem("chatSessionMap", JSON.stringify(chatSessionMap));
    localStorage.setItem("chatSessionMeta", JSON.stringify(sessionMetaIndex));
  }, [chatSessionMap, sessionMetaIndex]);

  const handleOpenChat = useCallback(
    async (
      persona: string,
      task?: string,
      beadId?: string,
      role?: string,
      beads?: { id: string; title?: string; description?: string }[],
      currentProjectPath?: string
    ) => {
      const key = `${persona}::${task || "chat"}::${beadId || "untracked"}::${role || "default"}::${currentCli}`;
      const beadTitle =
        beadId && beads ? beads.find((b) => b.id === beadId)?.title || null : null;
      const beadDescription =
        beadId && beads
          ? beads.find((b) => b.id === beadId)?.description || null
          : null;

      try {
        let targetSessionId = chatSessionMap[key];

        const sessionExists = targetSessionId
          ? sessions.some((s) => s.sessionId === targetSessionId)
          : false;

        // Create new session if it doesn't exist
        if (!sessionExists) {
          targetSessionId = await startAgentSession(
            persona,
            task ?? undefined,
            beadId ?? undefined,
            currentCli,
            role,
            currentProjectPath || undefined
          );
          await useSessionStore.getState().refreshSessions();
        }

        // ALWAYS update metadata when opening chat (even for existing sessions)
        if (targetSessionId) {
          const updatedChatSessionMap = {
            ...chatSessionMap,
            [key]: targetSessionId,
          };
          const updatedSessionMetaIndex = {
            ...sessionMetaIndex,
            [targetSessionId]: {
              persona,
              task: task ?? null,
              beadId: beadId ?? null,
              beadTitle,
              beadDescription,
              backendId: currentCli,
              role: role ?? null,
              projectPath: currentProjectPath || null,
            },
          };

          // Update React state
          setChatSessionMap(updatedChatSessionMap);
          setSessionMetaIndex(updatedSessionMetaIndex);

          // CRITICAL: Synchronously persist to localStorage BEFORE creating window
          // This prevents a race condition where the new window reads stale data
          if (typeof localStorage !== "undefined") {
            localStorage.setItem(
              "chatSessionMap",
              JSON.stringify(updatedChatSessionMap)
            );
            localStorage.setItem(
              "chatSessionMeta",
              JSON.stringify(updatedSessionMetaIndex)
            );
          }

          await createSessionWindow(targetSessionId);
        }
      } catch (error) {
        console.error("Failed to open chat window:", error);
      }
    },
    [chatSessionMap, sessionMetaIndex, sessions, currentCli]
  );

  return {
    chatSessionMap,
    setChatSessionMap,
    sessionMetaIndex,
    setSessionMetaIndex,
    currentCli,
    setCurrentCli,
    handleOpenChat,
  };
}
