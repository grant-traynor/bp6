/**
 * bp6-41g: useConversation
 *
 * Drives conversational lifecycle steps (1, 2, 3, 6) via the Claude API SDK,
 * replacing the PTY agent:stdout subscription with clean streaming text.
 *
 * Tool calls handled:
 *   emit_artifact  → create_lifecycle_artifact Tauri command + pendingArtifact state
 *   emit_decision  → create_queue_item Tauri command + onDecision callback
 *   done           → onDone callback (step is complete; user still presses Approve)
 */

import { useState, useEffect, useRef, useCallback } from "react";
import Anthropic from "@anthropic-ai/sdk";
import { invoke } from "@tauri-apps/api/core";

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

interface ConversationContext {
  skillContent: string;
  artefactContext: string;
}

export interface UseConversationReturn {
  messages: ConversationMessage[];
  streaming: boolean;
  pendingArtifact: ArtifactEvent | null;
  apiKeyError: string | null;
  sendMessage: (userInput: string) => Promise<void>;
  clearPendingArtifact: () => void;
}

// ── Tool definitions ──────────────────────────────────────────────────────────

const TOOLS: Anthropic.Tool[] = [
  {
    name: "emit_artifact",
    description: "Produce a document artefact for the current lifecycle step",
    input_schema: {
      type: "object" as const,
      properties: {
        filename: { type: "string", description: "Filename, e.g. conops.md" },
        title: { type: "string", description: "Human-readable title" },
        content: { type: "string", description: "Full markdown content" },
        step: { type: "number", description: "Lifecycle step number" },
      },
      required: ["filename", "title", "content", "step"],
    },
  },
  {
    name: "emit_decision",
    description: "Surface a question to the human that needs a decision",
    input_schema: {
      type: "object" as const,
      properties: {
        question: { type: "string" },
        options: { type: "array", items: { type: "object" } },
        priority: { type: "number" },
      },
      required: ["question", "options"],
    },
  },
  {
    name: "done",
    description:
      "Signal that this step is complete and ready for human review",
    input_schema: {
      type: "object" as const,
      properties: {
        summary: { type: "string" },
      },
      required: ["summary"],
    },
  },
];

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
  const [pendingArtifact, setPendingArtifact] = useState<ArtifactEvent | null>(null);
  const [apiKeyError, setApiKeyError] = useState<string | null>(null);

  const apiKeyRef = useRef<string | null>(null);
  const contextRef = useRef<ConversationContext | null>(null);

  // On mount (or when step/skillId changes), load API key and conversation context
  useEffect(() => {
    let cancelled = false;

    async function init() {
      // Get API key
      try {
        const key = await invoke<string>("get_anthropic_api_key");
        if (!cancelled) {
          apiKeyRef.current = key;
          setApiKeyError(null);
        }
      } catch (err) {
        if (!cancelled) {
          setApiKeyError(String(err));
        }
        return;
      }

      // Get conversation context (skill + artefacts)
      try {
        const ctx = await invoke<ConversationContext>("get_conversation_context", {
          projectId,
          step,
          skillId,
        });
        if (!cancelled) {
          contextRef.current = ctx;
        }
      } catch (err) {
        // Non-fatal: context may be empty for first step
        if (!cancelled) {
          contextRef.current = { skillContent: "", artefactContext: "" };
        }
        console.warn("[useConversation] get_conversation_context failed:", err);
      }
    }

    // Reset state when key inputs change
    setMessages([]);
    setPendingArtifact(null);

    void init();

    return () => {
      cancelled = true;
    };
  }, [projectId, step, skillId]);

  const sendMessage = useCallback(
    async (userInput: string) => {
      if (!apiKeyRef.current) {
        setApiKeyError("ANTHROPIC_API_KEY environment variable not set");
        return;
      }

      const userMsg: ConversationMessage = { role: "user", content: userInput };
      const nextMessages = [...messages, userMsg];
      setMessages(nextMessages);
      setStreaming(true);

      try {
        const client = new Anthropic({
          apiKey: apiKeyRef.current,
          dangerouslyAllowBrowser: true,
        });

        const ctx = contextRef.current;
        const systemParts: string[] = [];
        if (ctx?.skillContent) systemParts.push(ctx.skillContent);
        if (ctx?.artefactContext) {
          systemParts.push("\n\n## Prior artefacts from earlier steps\n\n" + ctx.artefactContext);
        }
        const system = systemParts.join("\n\n");

        // Build messages array for Anthropic SDK (exclude trailing streaming assistant msg)
        const apiMessages: Anthropic.MessageParam[] = nextMessages.map((m) => ({
          role: m.role === "assistant" ? "assistant" : "user",
          content: m.content,
        }));

        // Accumulate the current assistant response
        let assistantText = "";

        const stream = client.messages.stream({
          model: "claude-opus-4-6",
          max_tokens: 8192,
          system: system || undefined,
          messages: apiMessages,
          tools: TOOLS,
        });

        // Stream text deltas
        stream.on("text", (delta) => {
          assistantText += delta;
          setMessages((prev) => {
            const last = prev[prev.length - 1];
            if (last?.role === "assistant") {
              return [
                ...prev.slice(0, -1),
                { role: "assistant" as const, content: assistantText },
              ];
            }
            return [...prev, { role: "assistant" as const, content: assistantText }];
          });
        });

        // Wait for the full response
        const finalMessage = await stream.finalMessage();

        // Process tool use blocks
        for (const block of finalMessage.content) {
          if (block.type !== "tool_use") continue;

          const input = block.input as Record<string, unknown>;

          if (block.name === "emit_artifact") {
            const artifact: ArtifactEvent = {
              filename: String(input.filename ?? "artifact.md"),
              title: String(input.title ?? "Untitled"),
              content: String(input.content ?? ""),
              step: Number(input.step ?? step),
            };

            try {
              await invoke<string>("create_lifecycle_artifact", {
                projectId,
                filename: artifact.filename,
                title: artifact.title,
                content: artifact.content,
                step: artifact.step,
              });
              setPendingArtifact(artifact);
            } catch (err) {
              console.error("[useConversation] create_lifecycle_artifact failed:", err);
            }
          } else if (block.name === "emit_decision") {
            const decisionInput = input as {
              question: string;
              options: DecisionOption[];
              priority?: number;
            };
            const decisionEvent: DecisionEvent = {
              question: decisionInput.question,
              options: decisionInput.options ?? [],
              priority: decisionInput.priority ?? 2,
            };

            try {
              await invoke("create_queue_item", {
                agentId: `lifecycle-step-${step}`,
                workflowId: null,
                awakeableId: null,
                question: decisionEvent.question,
                options: decisionEvent.options,
                contextSnapshot: { step, projectId },
                priority: decisionEvent.priority,
              });
            } catch (err) {
              console.error("[useConversation] create_queue_item failed:", err);
            }

            onDecision(decisionEvent);
          } else if (block.name === "done") {
            // Agent signals completion; user still presses Approve explicitly
            onDone();
          }
        }
      } catch (err) {
        console.error("[useConversation] streaming error:", err);
        setMessages((prev) => [
          ...prev,
          {
            role: "assistant" as const,
            content: `Error communicating with Claude API: ${String(err)}`,
          },
        ]);
      } finally {
        setStreaming(false);
      }
    },
    [messages, projectId, step, onDecision, onDone],
  );

  const clearPendingArtifact = useCallback(() => {
    setPendingArtifact(null);
  }, []);

  return {
    messages,
    streaming,
    pendingArtifact,
    apiKeyError,
    sendMessage,
    clearPendingArtifact,
  };
}
