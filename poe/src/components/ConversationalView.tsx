/**
 * bp6-ims.9: ConversationalView
 *
 * Chat interface for lifecycle Steps 1-3 and Step 6 review.
 * Subscribes to agent:stdout events filtered by activeAgentId.
 * Supports artifact approval and revision request flows.
 */
import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useAtomValue } from "jotai";
import { nodesAtom } from "../store/dag";
import type { AgentStdoutLine, LifecycleStatus, DagNode } from "../types";

// ── Step label map ────────────────────────────────────────────────────────────

function stepLabel(status: LifecycleStatus): string {
  const { step, substep } = status;
  if (step === 1) return "Concept Development — Operational Analysis Expert";
  if (step === 2) {
    if (substep === "1") return "Guardrails — Architecture Constraints";
    if (substep === "2") return "Guardrails — Design System";
    if (substep === "3") return "Guardrails — User Analysis";
    if (substep === "4") return "Guardrails — Must-Nots";
    if (substep === "review") return "Guardrails — Engineering Manager Review";
    return "Guardrails";
  }
  if (step === 3) return "Stage Planning — Product Manager";
  if (step === 6) return "Replanning & QA — Review";
  return `Step ${step}`;
}

// ── Types ─────────────────────────────────────────────────────────────────────

interface ChatMessage {
  role: "user" | "agent";
  content: string;
  ts: string;
}

// ── StatusBanner ──────────────────────────────────────────────────────────────

function StatusBanner({
  lifecycleStatus,
  onStartLifecycle,
}: {
  lifecycleStatus: LifecycleStatus;
  onStartLifecycle: () => void;
}) {
  const { status, step } = lifecycleStatus;

  const statusColor =
    status === "running"
      ? "var(--status-running)"
      : status === "awaiting_approval"
      ? "var(--status-paused)"
      : status === "complete"
      ? "var(--status-completed)"
      : "var(--text-tertiary)";

  return (
    <div
      className="mac-toolbar flex items-center justify-between"
      style={{ padding: "0 16px", height: 40, minHeight: 40, borderBottom: "1px solid var(--divider)" }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
        <span
          style={{
            width: 7,
            height: 7,
            borderRadius: "50%",
            background: statusColor,
            flexShrink: 0,
            display: "inline-block",
            boxShadow: status === "running" ? `0 0 0 3px ${statusColor}30` : undefined,
          }}
        />
        <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)" }}>
          {stepLabel(lifecycleStatus)}
        </span>
        <span
          className="mono"
          style={{
            fontSize: 10,
            color: "var(--text-tertiary)",
            background: "var(--content-secondary-bg)",
            padding: "1px 8px",
            borderRadius: 4,
            border: "1px solid var(--border)",
          }}
        >
          Step {step} · {status}
        </span>
      </div>
      {status === "idle" && step === 0 && (
        <button
          onClick={onStartLifecycle}
          className="mac-btn mac-btn-primary"
          style={{ padding: "4px 14px", fontSize: 11 }}
        >
          Start Lifecycle
        </button>
      )}
    </div>
  );
}

// ── ArtifactCard ──────────────────────────────────────────────────────────────

function ArtifactCard({
  node,
  projectId,
  step,
  agentId,
}: {
  node: DagNode;
  projectId: string;
  step: number;
  agentId: string | null;
}) {
  const [revisionMode, setRevisionMode] = useState(false);
  const [revisionText, setRevisionText] = useState("");
  const [approving, setApproving] = useState(false);
  const [sendingRevision, setSendingRevision] = useState(false);

  const title = typeof node.data.title === "string" ? node.data.title : node.id.slice(0, 12);
  const content =
    typeof node.data.content === "string"
      ? node.data.content.slice(0, 500)
      : JSON.stringify(node.data).slice(0, 500);

  const handleApprove = async () => {
    setApproving(true);
    try {
      await invoke("approve_lifecycle_step", { projectId, step, decision: "approve" });
    } catch (err) {
      console.error("Approve error:", err);
    } finally {
      setApproving(false);
    }
  };

  const handleRevision = async () => {
    if (!revisionText.trim() || !agentId) return;
    setSendingRevision(true);
    try {
      await invoke("write_to_agent", { agentId, input: revisionText.trim() + "\n" });
      setRevisionText("");
      setRevisionMode(false);
    } catch (err) {
      console.error("Revision error:", err);
    } finally {
      setSendingRevision(false);
    }
  };

  return (
    <div
      className="mac-card"
      style={{ margin: "0 16px 12px", padding: "14px 16px" }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 8 }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)" }}>{title}</span>
        <span
          className="mono"
          style={{
            fontSize: 10,
            color: "var(--text-tertiary)",
            background: "var(--content-secondary-bg)",
            padding: "1px 6px",
            borderRadius: 4,
            border: "1px solid var(--border)",
          }}
        >
          KnowledgeArtifact
        </span>
      </div>
      <pre
        className="mono"
        style={{
          fontSize: 11,
          color: "var(--text-secondary)",
          background: "var(--content-secondary-bg)",
          border: "1px solid var(--border)",
          borderRadius: 6,
          padding: "8px 10px",
          overflow: "auto",
          maxHeight: 160,
          whiteSpace: "pre-wrap",
          wordBreak: "break-word",
          marginBottom: 10,
        }}
      >
        {content}
        {typeof node.data.content === "string" && node.data.content.length > 500 && "…"}
      </pre>

      {revisionMode ? (
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <textarea
            value={revisionText}
            onChange={(e) => setRevisionText(e.target.value)}
            placeholder="Describe what should be revised…"
            className="mac-input mono"
            style={{ fontSize: 11, height: 72, resize: "vertical" }}
            autoFocus
          />
          <div style={{ display: "flex", gap: 6 }}>
            <button
              onClick={handleRevision}
              disabled={sendingRevision || !revisionText.trim() || !agentId}
              className="mac-btn mac-btn-primary"
              style={{ flex: 1, justifyContent: "center", fontSize: 11, padding: "5px 10px" }}
            >
              {sendingRevision ? "Sending…" : "Send Revision Request"}
            </button>
            <button
              onClick={() => { setRevisionMode(false); setRevisionText(""); }}
              disabled={sendingRevision}
              className="mac-btn mac-btn-ghost"
              style={{ fontSize: 11, padding: "5px 10px" }}
            >
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <div style={{ display: "flex", gap: 6 }}>
          <button
            onClick={handleApprove}
            disabled={approving}
            className="mac-btn mac-btn-primary"
            style={{ flex: 1, justifyContent: "center", fontSize: 11, padding: "5px 10px" }}
          >
            {approving ? "Approving…" : "Approve"}
          </button>
          <button
            onClick={() => setRevisionMode(true)}
            className="mac-btn"
            style={{ fontSize: 11, padding: "5px 14px" }}
          >
            Request Revision
          </button>
        </div>
      )}
    </div>
  );
}

// ── ChatMessages ──────────────────────────────────────────────────────────────

function ChatMessages({
  messages,
  isResponding,
}: {
  messages: ChatMessage[];
  isResponding: boolean;
}) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, isResponding]);

  if (messages.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center" style={{ padding: 32 }}>
        <div style={{ textAlign: "center" }}>
          <p style={{ fontSize: 13, color: "var(--text-secondary)", margin: 0 }}>
            Agent is ready — messages will appear here.
          </p>
          <p style={{ fontSize: 11, color: "var(--text-placeholder)", marginTop: 6 }}>
            Use the input below to send instructions or respond to the agent.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div
      className="flex-1 overflow-auto"
      style={{ padding: "12px 16px", display: "flex", flexDirection: "column", gap: 10 }}
    >
      {messages.map((msg, i) => (
        <div
          key={i}
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: msg.role === "user" ? "flex-end" : "flex-start",
          }}
        >
          <div
            style={{
              maxWidth: "85%",
              padding: "8px 12px",
              borderRadius: msg.role === "user" ? "12px 12px 2px 12px" : "12px 12px 12px 2px",
              background: msg.role === "user" ? "var(--accent)" : "var(--content-secondary-bg)",
              border: msg.role === "agent" ? "1px solid var(--border)" : "none",
              fontSize: 12,
              lineHeight: 1.6,
              color: msg.role === "user" ? "#fff" : "var(--text-primary)",
              whiteSpace: "pre-wrap",
              wordBreak: "break-word",
            }}
          >
            {msg.content}
          </div>
        </div>
      ))}
      {isResponding && (
        <div style={{ display: "flex", alignItems: "flex-start" }}>
          <div
            style={{
              padding: "8px 12px",
              borderRadius: "12px 12px 12px 2px",
              background: "var(--content-secondary-bg)",
              border: "1px solid var(--border)",
            }}
          >
            <div style={{ display: "flex", gap: 4, alignItems: "center" }}>
              {[0, 150, 300].map((delay) => (
                <span
                  key={delay}
                  style={{
                    width: 5,
                    height: 5,
                    borderRadius: "50%",
                    background: "var(--text-tertiary)",
                    animation: "pulse 1.2s ease-in-out infinite",
                    animationDelay: `${delay}ms`,
                  }}
                />
              ))}
            </div>
          </div>
        </div>
      )}
      <div ref={bottomRef} />
    </div>
  );
}

// ── InputBar ──────────────────────────────────────────────────────────────────

function InputBar({
  onSend,
  disabled,
}: {
  onSend: (text: string) => void;
  disabled: boolean;
}) {
  const [text, setText] = useState("");

  function handleSubmit() {
    const trimmed = text.trim();
    if (!trimmed || disabled) return;
    onSend(trimmed);
    setText("");
  }

  return (
    <div
      style={{
        borderTop: "1px solid var(--divider)",
        padding: "10px 12px",
        display: "flex",
        gap: 8,
        alignItems: "flex-end",
      }}
    >
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.shiftKey) {
            e.preventDefault();
            handleSubmit();
          }
        }}
        placeholder={disabled ? "Waiting for agent…" : "Send a message to the agent…"}
        disabled={disabled}
        rows={1}
        style={{
          flex: 1,
          resize: "none",
          background: "var(--input-bg, var(--content-secondary-bg))",
          border: "1px solid var(--border)",
          borderRadius: 8,
          padding: "7px 10px",
          fontSize: 12,
          color: "var(--text-primary)",
          fontFamily: "inherit",
          outline: "none",
          lineHeight: 1.5,
          minHeight: 34,
          maxHeight: 120,
          opacity: disabled ? 0.5 : 1,
        }}
      />
      <button
        onClick={handleSubmit}
        disabled={disabled || !text.trim()}
        className="mac-btn"
        style={{ padding: "7px 14px", fontSize: 12, flexShrink: 0 }}
      >
        Send
      </button>
    </div>
  );
}

// ── ConversationalViewProps ───────────────────────────────────────────────────

export interface ConversationalViewProps {
  projectId: string;
  lifecycleStatus: LifecycleStatus;
  onStartLifecycle: () => void;
}

// ── ConversationalView ────────────────────────────────────────────────────────

export function ConversationalView({
  projectId,
  lifecycleStatus,
  onStartLifecycle,
}: ConversationalViewProps) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [isResponding, setIsResponding] = useState(false);
  const responseTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const nodes = useAtomValue(nodesAtom);

  const { activeAgentId, status } = lifecycleStatus;

  // Find latest KnowledgeArtifact node for this project
  const artifactNode: DagNode | undefined = Array.from(nodes.values())
    .filter((n) => n.nodeType === "KnowledgeArtifact" && n.projectId === projectId)
    .sort((a, b) => b.createdAt.localeCompare(a.createdAt))[0];

  // Subscribe to agent stdout for the active agent
  useEffect(() => {
    if (!activeAgentId) return;

    let unlistenFn: (() => void) | null = null;

    listen<AgentStdoutLine>("agent:stdout", (event) => {
      // Filter to only lines from the active agent (workflowId matches agentId convention)
      if (event.payload.workflowId !== activeAgentId) return;

      const line = event.payload.line;

      // Filter poe: control lines
      if (line.startsWith("poe:")) {
        if (line.startsWith("poe:done")) {
          setIsResponding(false);
          if (responseTimerRef.current) clearTimeout(responseTimerRef.current);
        }
        return;
      }

      setIsResponding(true);

      setMessages((prev) => {
        const last = prev[prev.length - 1];
        if (last?.role === "agent") {
          return [
            ...prev.slice(0, -1),
            { ...last, content: last.content + (last.content ? "\n" : "") + line },
          ];
        }
        return [
          ...prev,
          { role: "agent", content: line, ts: new Date().toISOString() },
        ];
      });

      // Reset done-detection timer (3s silence = done)
      if (responseTimerRef.current) clearTimeout(responseTimerRef.current);
      responseTimerRef.current = setTimeout(() => {
        setIsResponding(false);
      }, 3000);
    }).then((fn) => {
      unlistenFn = fn;
    });

    return () => {
      unlistenFn?.();
      if (responseTimerRef.current) clearTimeout(responseTimerRef.current);
    };
  }, [activeAgentId]);

  // Reset messages when the active agent changes
  useEffect(() => {
    setMessages([]);
    setIsResponding(false);
  }, [activeAgentId]);

  const handleSend = useCallback(
    async (text: string) => {
      if (!activeAgentId || status !== "running") return;

      setMessages((prev) => [
        ...prev,
        { role: "user", content: text, ts: new Date().toISOString() },
      ]);

      try {
        await invoke("write_to_agent", { agentId: activeAgentId, input: text + "\n" });
      } catch (err) {
        console.error("write_to_agent error:", err);
        setMessages((prev) => [
          ...prev,
          { role: "agent", content: `Error sending message: ${err}`, ts: new Date().toISOString() },
        ]);
      }
    },
    [activeAgentId, status]
  );

  const showArtifactCard =
    artifactNode !== undefined &&
    (status === "awaiting_approval" || status === "running");

  const showApprovalBanner =
    status === "awaiting_approval" && !showArtifactCard;

  const inputDisabled = status !== "running" || !activeAgentId;

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <StatusBanner lifecycleStatus={lifecycleStatus} onStartLifecycle={onStartLifecycle} />

      {/* Approval banner when awaiting but no artifact card */}
      {showApprovalBanner && (
        <div
          style={{
            background: "var(--status-paused-bg, #2d2200)",
            borderBottom: "1px solid var(--status-paused)",
            padding: "8px 16px",
            fontSize: 12,
            color: "var(--status-paused)",
            display: "flex",
            alignItems: "center",
            gap: 8,
          }}
        >
          <span style={{ fontWeight: 600 }}>Agent complete</span>
          <span style={{ color: "var(--text-secondary)" }}>
            — review above and approve or request revision.
          </span>
        </div>
      )}

      {/* Chat messages fill available space */}
      <ChatMessages messages={messages} isResponding={isResponding} />

      {/* Artifact approval card */}
      {showArtifactCard && artifactNode && (
        <ArtifactCard
          node={artifactNode}
          projectId={projectId}
          step={lifecycleStatus.step}
          agentId={activeAgentId}
        />
      )}

      <InputBar onSend={handleSend} disabled={inputDisabled} />
    </div>
  );
}
