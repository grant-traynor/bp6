/**
 * bp6-ar2: useConversation — CLI-based conversational lifecycle turns
 *
 * Replaces the @anthropic-ai/sdk implementation with the same pattern used by
 * the Project Advisor: each turn spawns `claude -p --output-format stream-json`
 * via start_conversation_turn (first turn) or continue_conversation_turn
 * (subsequent turns).  Session continuity is maintained via --session-id /
 * --resume exactly as in advisor.rs.
 *
 * The frontend subscribes to agent:stdout events, parses the stream-json NDJSON,
 * and handles poe: JSON events embedded in assistant text blocks.
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

export interface DecisionOption {
  id: string;
  label: string;
  description: string;
}

export interface DecisionEvent {
  question: string;
  options: DecisionOption[];
  priority: number;
}

export interface UseConversationReturn {
  messages: ConversationMessage[];
  streaming: boolean;
  pendingArtifact: ArtifactEvent | null;
  sendMessage: (userInput: string) => Promise<void>;
  clearPendingArtifact: () => void;
}

// ── stream-json line parser (mirrors AdvisorView.tsx parseStreamLine) ──────────

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
      // {"type":"system","subtype":"init","session_id":"..."}
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
  onDecision: (event: DecisionEvent) => void,
  onDone: () => void,
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
    let unlisten: (() => void) | null = null;
    let doneTimer: ReturnType<typeof setTimeout> | null = null;

    function markDone() {
      if (doneTimer) clearTimeout(doneTimer);
      setStreaming(false);
    }

    listen<AgentStdoutLine>("agent:stdout", (event) => {
      if (event.payload.workflowId !== currentAgentIdRef.current) return;

      const events = parseStreamLine(event.payload.line);
      if (events.length === 0) return;

      // Fallback done-timer: if no "done" event arrives within 5s of last output
      if (doneTimer) clearTimeout(doneTimer);
      doneTimer = setTimeout(markDone, 5000);

      for (const ev of events) {
        if (ev.type === "session_id") {
          // Capture session_id from the init system event
          if (!sessionIdRef.current) {
            sessionIdRef.current = ev.sessionId;
          }
        } else if (ev.type === "text") {
          // Each text block may contain multiple lines; scan for poe: JSON events
          const lines = ev.text.split("\n");
          for (const textLine of lines) {
            const trimmed = textLine.trim();
            if (trimmed.startsWith('{"type":"poe:')) {
              try {
                const poeEvent = JSON.parse(trimmed) as Record<
                  string,
                  unknown
                >;
                handlePoeEvent(poeEvent);
              } catch {
                // Malformed poe: line — treat as regular text
                appendAssistantText(trimmed);
              }
            } else if (trimmed) {
              appendAssistantText(trimmed);
            }
          }
        } else if (ev.type === "done") {
          markDone();
        }
      }
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      unlisten?.();
      if (doneTimer) clearTimeout(doneTimer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []); // effect is stable; currentAgentIdRef checked inside listener

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

  const handlePoeEvent = useCallback(
    (event: Record<string, unknown>) => {
      const type = event.type as string;

      if (type === "poe:artifact") {
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
      } else if (type === "poe:decision") {
        const de = event as {
          question: string;
          options: DecisionOption[];
          priority?: number;
        };
        const decisionEvent: DecisionEvent = {
          question: de.question,
          options: de.options ?? [],
          priority: de.priority ?? 2,
        };
        invoke("create_queue_item", {
          agentId: `lifecycle-step-${step}`,
          workflowId: null,
          awakeableId: null,
          question: decisionEvent.question,
          options: decisionEvent.options,
          contextSnapshot: { step, projectId },
          priority: decisionEvent.priority,
        }).catch((err) =>
          console.error("[useConversation] create_queue_item:", err),
        );
        onDecision(decisionEvent);
      } else if (type === "poe:done") {
        onDone();
      }
    },
    [projectId, step, onDecision, onDone],
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
          // First message — start a new session
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
          // Continuation — resume existing session
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
