import { useState, useEffect, useCallback } from "react";
import {
  getCliPreference,
  startAgentSession,
  createSessionWindow,
  type CliBackend,
} from "../api";
import { useSessionStore } from "../stores/sessionStore";

export interface UseChatSessionsReturn {
  chatSessionMap: Record<string, string>;
  setChatSessionMap: React.Dispatch<React.SetStateAction<Record<string, string>>>;
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
 * Manages chat session state: session map and CLI preference.
 * Syncs dead sessions from sessionStore (evicts sessions no longer alive in backend).
 * Persists chatSessionMap to localStorage.
 * Session metadata (persona, task, beadId, etc.) is stored in the backend and
 * available via useSessionStore — no need to duplicate it here.
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

  const [currentCli, setCurrentCli] = useState<CliBackend>("gemini");

  // Load CLI preference on mount
  useEffect(() => {
    const loadCliPreference = async () => {
      try {
        const preference = await getCliPreference();
        setCurrentCli(preference as CliBackend);
      } catch (error) {
        console.error("Failed to load CLI preference:", error);
      }
    };
    loadCliPreference();
  }, []);

  // Read live sessions from Zustand store
  const sessions = useSessionStore((state) => state.sessions);

  // Evict dead sessions: drop chatSessionMap entries for sessions no longer alive in backend
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
  }, [sessions]);

  // Persist chatSessionMap to localStorage
  useEffect(() => {
    if (typeof localStorage === "undefined") return;
    localStorage.setItem("chatSessionMap", JSON.stringify(chatSessionMap));
  }, [chatSessionMap]);

  const handleOpenChat = useCallback(
    async (
      persona: string,
      task?: string,
      beadId?: string,
      role?: string,
      _beads?: { id: string; title?: string; description?: string }[],
      currentProjectPath?: string
    ) => {
      const key = `${persona}::${task || "chat"}::${beadId || "untracked"}::${role || "default"}::${currentCli}`;

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

        if (targetSessionId) {
          const updatedChatSessionMap = {
            ...chatSessionMap,
            [key]: targetSessionId,
          };

          setChatSessionMap(updatedChatSessionMap);

          // Synchronously persist to localStorage BEFORE creating window
          if (typeof localStorage !== "undefined") {
            localStorage.setItem(
              "chatSessionMap",
              JSON.stringify(updatedChatSessionMap)
            );
          }

          await createSessionWindow(targetSessionId);
        }
      } catch (error) {
        console.error("Failed to open chat window:", error);
      }
    },
    [chatSessionMap, sessions, currentCli]
  );

  return {
    chatSessionMap,
    setChatSessionMap,
    currentCli,
    setCurrentCli,
    handleOpenChat,
  };
}
