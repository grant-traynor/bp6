/**
 * bp6-ar2: useConversation — CLI-based conversational lifecycle turns
 *
 * Each turn spawns `claude -p --output-format stream-json` via
 * start_conversation_turn (first turn) or continue_conversation_turn
 * (subsequent turns). Session continuity is maintained via --session-id /
 * --resume exactly as in advisor.rs.
 *
 * The frontend subscribes to agent:stdout events, parses the stream-json NDJSON,
 * and handles poe:artifact events embedded in assistant text blocks.
 */

import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { AgentStdoutLine } from "../types";

// ── Types ─────────────────────────────────────────────────────────────────────

export interface ConversationMessage {
  role: "user" | "assistant";
  content: string;
}

export interface ArtifactEvent {
  filename: string;
  title: string;
  content: string;
  step: number;
}

export interface UseConversationReturn {
  messages: ConversationMessage[];
  streaming: boolean;
  pendingArtifact: ArtifactEvent | null;
  sendMessage: (userInput: string) => Promise<void>;
  clearPendingArtifact: () => void;
}

// ── stream-json line parser ────────────────────────────────────────────────────

type ParsedStreamEvent =
  | { type: "text"; text: string }
  | { type: "session_id"; sessionId: string }
  | { type: "done" }
  | { type: "ignored" };

function parseStreamLine(raw: string): ParsedStreamEvent[] {
  const line = raw.trim();
  if (!line.startsWith("{")) return [];
  let parsed: Record<string, unknown>;
  try {
    parsed = JSON.parse(line);
  } catch {
    return [];
  }

  switch (parsed.type) {
    case "system": {
      const sid = parsed.session_id;
      if (typeof sid === "string" && sid) {
        return [{ type: "session_id", sessionId: sid }];
      }
      return [];
    }
    case "assistant": {
      const content =
        (parsed.message as { content?: unknown[] } | undefined)?.content ?? [];
      if (!Array.isArray(content)) return [];
      return content.flatMap((block): ParsedStreamEvent[] => {
        const b = block as Record<string, unknown>;
        if (b.type === "text") {
          return [{ type: "text", text: String(b.text ?? "") }];
        }
        return [];
      });
    }
    case "result": {
      return [{ type: "done" }];
    }
    default:
      return [];
  }
}

// ── Hook ──────────────────────────────────────────────────────────────────────

export function useConversation(
  projectId: string,
  step: number,
  skillId: string,
): UseConversationReturn {
  const [messages, setMessages] = useState<ConversationMessage[]>([]);
  const [streaming, setStreaming] = useState(false);
  const [pendingArtifact, setPendingArtifact] =
    useState<ArtifactEvent | null>(null);

  const sessionIdRef = useRef<string | null>(null);
  const currentAgentIdRef = useRef<string | null>(null);

  // Reset when project/step/skill changes
  useEffect(() => {
    setMessages([]);
    setPendingArtifact(null);
    sessionIdRef.current = null;
    currentAgentIdRef.current = null;
  }, [projectId, step, skillId]);

  // Subscribe to agent:stdout events for the current turn
  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | null = null;
    let doneTimer: ReturnType<typeof setTimeout> | null = null;

    function markDone() {
      if (doneTimer) clearTimeout(doneTimer);
      setStreaming(false);
    }

    listen<AgentStdoutLine>("agent:stdout", (event) => {
      if (!active) return;
      if (event.payload.workflowId !== currentAgentIdRef.current) return;

      const events = parseStreamLine(event.payload.line);
      if (events.length === 0) return;

      if (doneTimer) clearTimeout(doneTimer);
      doneTimer = setTimeout(markDone, 5000);

      for (const ev of events) {
        if (ev.type === "session_id") {
          if (!sessionIdRef.current) {
            sessionIdRef.current = ev.sessionId;
          }
        } else if (ev.type === "text") {
          // Scan each line of the text block. Lines starting with poe:artifact
          // JSON are handled separately; all other non-empty lines are displayed.
          const displayLines: string[] = [];
          for (const textLine of ev.text.split("\n")) {
            const trimmed = textLine.trim();
            if (trimmed.startsWith('{"type":"poe:artifact"')) {
              try {
                handleArtifact(JSON.parse(trimmed) as Record<string, unknown>);
              } catch {
                displayLines.push(trimmed);
              }
            } else if (trimmed && !trimmed.startsWith("```")) {
              displayLines.push(trimmed);
            }
          }
          if (displayLines.length > 0) {
            appendAssistantText(displayLines.join("\n"));
          }
        } else if (ev.type === "done") {
          markDone();
        }
      }
    }).then((fn) => {
      if (!active) {
        fn(); // cleanup already ran — unsubscribe immediately
      } else {
        unlisten = fn;
      }
    });

    return () => {
      active = false;
      unlisten?.();
      if (doneTimer) clearTimeout(doneTimer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []); // stable; currentAgentIdRef checked inside listener

  const appendAssistantText = useCallback((text: string) => {
    setMessages((prev) => {
      const last = prev[prev.length - 1];
      if (last?.role === "assistant") {
        return [
          ...prev.slice(0, -1),
          { ...last, content: last.content + "\n" + text },
        ];
      }
      return [...prev, { role: "assistant" as const, content: text }];
    });
  }, []);

  const handleArtifact = useCallback(
    (event: Record<string, unknown>) => {
      const ae = event as {
        filename: string;
        title: string;
        content: string;
        step: number;
      };
      const artifact: ArtifactEvent = {
        filename: ae.filename,
        title: ae.title,
        content: ae.content,
        step: ae.step,
      };
      setPendingArtifact(artifact);
      invoke("create_lifecycle_artifact", {
        params: {
          projectId,
          filename: artifact.filename,
          title: artifact.title,
          content: artifact.content,
          step: artifact.step,
        },
      }).catch((err) =>
        console.error("[useConversation] create_lifecycle_artifact:", err),
      );
    },
    [projectId],
  );

  const sendMessage = useCallback(
    async (userInput: string) => {
      setMessages((prev) => [
        ...prev,
        { role: "user" as const, content: userInput },
      ]);
      setStreaming(true);

      try {
        if (!sessionIdRef.current) {
          const result = await invoke<{ agentId: string; sessionId: string }>(
            "start_conversation_turn",
            {
              params: {
                projectId,
                step,
                substep: null,
                skillId,
                initialMessage: userInput,
              },
            },
          );
          currentAgentIdRef.current = result.agentId;
          sessionIdRef.current = result.sessionId;
        } else {
          const result = await invoke<{ agentId: string; sessionId: string }>(
            "continue_conversation_turn",
            {
              params: {
                sessionId: sessionIdRef.current,
                message: userInput,
                projectId,
              },
            },
          );
          currentAgentIdRef.current = result.agentId;
        }
      } catch (err) {
        setStreaming(false);
        appendAssistantText(
          `Error communicating with Claude: ${String(err)}`,
        );
      }
    },
    [projectId, step, skillId, appendAssistantText],
  );

  const clearPendingArtifact = useCallback(() => {
    setPendingArtifact(null);
  }, []);

  return {
    messages,
    streaming,
    pendingArtifact,
    sendMessage,
    clearPendingArtifact,
  };
}
