/**
 * bp6-41g: ConversationalView
 *
 * Chat interface for lifecycle Steps 1-3 and Step 6 review.
 * Uses the Claude API directly (via useConversation) instead of a PTY agent.
 * Supports artifact approval and revision request flows.
 */
import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAtomValue } from "jotai";
import { nodesAtom } from "../store/dag";
import type { LifecycleStatus, DagNode } from "../types";
import {
  useConversation,
  type ConversationMessage,
  type ArtifactEvent,
  type DecisionEvent,
} from "../hooks/useConversation";

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

// ── Skill ID lookup ───────────────────────────────────────────────────────────

const STEP_SPECIALIST_MAP: Record<string, string> = {
  "1": "operational-analyst",
  "2.1": "architecture-analyst",
  "2.2": "design-system-analyst",
  "2.3": "user-analyst",
  "2.4": "must-not-analyst",
  "2.review": "engineering-manager",
  "3": "product-manager",
  "6.validity": "validity-analyst",
  "6.rca": "rca-analyst",
};

function resolveSkillId(lifecycleStatus: LifecycleStatus): string {
  const { step, substep } = lifecycleStatus;
  const key = substep ? `${step}.${substep}` : String(step);
  return STEP_SPECIALIST_MAP[key] ?? `step-${step}`;
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
  pendingArtifact,
  projectId,
  step,
  onRevisionSend,
}: {
  node: DagNode | null;
  pendingArtifact: ArtifactEvent | null;
  projectId: string;
  step: number;
  onRevisionSend: (text: string) => Promise<void>;
}) {
  const [revisionMode, setRevisionMode] = useState(false);
  const [revisionText, setRevisionText] = useState("");
  const [approving, setApproving] = useState(false);
  const [sendingRevision, setSendingRevision] = useState(false);

  // Display from DAG node if available, otherwise from pendingArtifact
  const title =
    node != null
      ? typeof node.data.title === "string"
        ? node.data.title
        : node.id.slice(0, 12)
      : (pendingArtifact?.title ?? "Artifact");

  const rawContent =
    node != null
      ? typeof node.data.content === "string"
        ? node.data.content
        : JSON.stringify(node.data)
      : (pendingArtifact?.content ?? "");

  const content = rawContent.slice(0, 500);
  const truncated = rawContent.length > 500;

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
    if (!revisionText.trim()) return;
    setSendingRevision(true);
    try {
      await onRevisionSend(revisionText.trim());
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
        {truncated && "…"}
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
              onClick={() => void handleRevision()}
              disabled={sendingRevision || !revisionText.trim()}
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
            onClick={() => void handleApprove()}
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
  messages: ConversationMessage[];
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
              border: msg.role === "assistant" ? "1px solid var(--border)" : "none",
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
  const nodes = useAtomValue(nodesAtom);
  const { status, step } = lifecycleStatus;

  const skillId = resolveSkillId(lifecycleStatus);

  const handleDecision = (_event: DecisionEvent) => {
    // Decision items surface in the queue panel; no local UI action needed here.
  };

  const handleDone = () => {
    // The agent has finished talking. The user still presses Approve explicitly.
    // Nothing to do here — the lifecycleStatus.status will update via DAG events.
  };

  const { messages, streaming, pendingArtifact, sendMessage, clearPendingArtifact } =
    useConversation(projectId, step, skillId, handleDecision, handleDone);

  // Find latest KnowledgeArtifact node for this project (from DAG)
  const artifactNode: DagNode | undefined = Array.from(nodes.values())
    .filter((n) => n.nodeType === "KnowledgeArtifact" && n.projectId === projectId)
    .sort((a, b) => b.createdAt.localeCompare(a.createdAt))[0];

  const showArtifactCard =
    (artifactNode !== undefined || pendingArtifact !== null) &&
    (status === "awaiting_approval" || status === "running");

  const showApprovalBanner = status === "awaiting_approval" && !showArtifactCard;

  const inputDisabled = streaming || status === "complete";

  const handleRevisionSend = async (text: string) => {
    await sendMessage(text);
  };

  const handleSend = (text: string) => {
    if (pendingArtifact) clearPendingArtifact();
    void sendMessage(text);
  };

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
      <ChatMessages messages={messages} isResponding={streaming} />

      {/* Artifact approval card */}
      {showArtifactCard && (
        <ArtifactCard
          node={artifactNode ?? null}
          pendingArtifact={pendingArtifact}
          projectId={projectId}
          step={step}
          onRevisionSend={handleRevisionSend}
        />
      )}

      <InputBar onSend={handleSend} disabled={inputDisabled} />
    </div>
  );
}
