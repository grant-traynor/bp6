import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useAtomValue } from "jotai";
import { MessageSquare, RefreshCw } from "lucide-react";
import { projectAtom } from "../store/dag";
import type { AgentStdoutLine } from "../types";

interface Message {
  role: "user" | "assistant";
  content: string;
  ts: string;
}

interface AdvisorStartResult {
  agentId: string;
  sessionId: string;
}

interface AdvisorStateResult {
  sessionId: string | null;
  agentId: string | null;
  isActive: boolean;
}

function timeSince(iso: string): string {
  const ms = Date.now() - new Date(iso).getTime();
  if (isNaN(ms)) return "";
  const m = Math.floor(ms / 60000);
  if (m < 1) return "just started";
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ${m % 60}m ago`;
  return `${Math.floor(h / 24)}d ago`;
}

// ── MessageList ──────────────────────────────────────────────────────────────

function MessageList({ messages, isResponding }: { messages: Message[]; isResponding: boolean }) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, isResponding]);

  if (messages.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-3" style={{ padding: 32 }}>
        <MessageSquare size={28} style={{ color: "var(--text-placeholder)" }} />
        <p style={{ fontSize: 13, color: "var(--text-secondary)", margin: 0, textAlign: "center" }}>
          Ask anything about your project
        </p>
        <div style={{ marginTop: 8, display: "flex", flexDirection: "column", gap: 6, width: "100%", maxWidth: 320 }}>
          {["What is currently blocked?", "Summarise what was built in the last stage", "Which agents have been running the longest?"].map((q) => (
            <p
              key={q}
              className="mono"
              style={{
                fontSize: 11, color: "var(--text-tertiary)", margin: 0, textAlign: "center",
                border: "1px dashed var(--border)", borderRadius: 6, padding: "6px 10px",
              }}
            >
              Try: &ldquo;{q}&rdquo;
            </p>
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-auto" style={{ padding: "12px 16px", display: "flex", flexDirection: "column", gap: 10 }}>
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
                    width: 5, height: 5, borderRadius: "50%",
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

// ── InputBar ─────────────────────────────────────────────────────────────────

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
        placeholder="Ask about your project…"
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

// ── SessionControls ───────────────────────────────────────────────────────────

function SessionControls({
  sessionStartedAt,
  messageCount,
  onReset,
}: {
  sessionStartedAt: string | null;
  messageCount: number;
  onReset: () => void;
}) {
  const [, setTick] = useState(0);
  useEffect(() => {
    const id = setInterval(() => setTick((t) => t + 1), 30000);
    return () => clearInterval(id);
  }, []);

  return (
    <div
      className="mac-toolbar flex items-center justify-between"
      style={{ padding: "0 12px", height: 36, minHeight: 36 }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <MessageSquare size={13} style={{ color: "var(--text-tertiary)" }} />
        <span style={{ fontSize: 12, color: "var(--text-secondary)", fontWeight: 600 }}>
          Project Advisor
        </span>
        {sessionStartedAt && (
          <span className="mono" style={{ fontSize: 10, color: "var(--text-placeholder)" }}>
            {messageCount} {messageCount === 1 ? "turn" : "turns"} · {timeSince(sessionStartedAt)}
          </span>
        )}
      </div>
      <button
        onClick={onReset}
        className="mac-btn mac-btn-ghost"
        style={{ padding: "3px 8px", fontSize: 11, display: "flex", alignItems: "center", gap: 4 }}
        title="New Session"
      >
        <RefreshCw size={11} />
        New Session
      </button>
    </div>
  );
}

// ── AdvisorView ───────────────────────────────────────────────────────────────

export function AdvisorView() {
  const project = useAtomValue(projectAtom);
  const [messages, setMessages] = useState<Message[]>([]);
  const [advisorAgentId, setAdvisorAgentId] = useState<string | null>(null);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [isResponding, setIsResponding] = useState(false);
  const [sessionStartedAt, setSessionStartedAt] = useState<string | null>(null);
  const responseTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // On mount, check for existing session
  useEffect(() => {
    if (!project) return;
    invoke<AdvisorStateResult>("get_advisor_state", { projectDir: project.projectDir })
      .then((state) => {
        if (state.agentId && state.isActive) {
          setAdvisorAgentId(state.agentId);
        }
        if (state.sessionId) setSessionId(state.sessionId);
      })
      .catch(console.error);
  }, [project?.projectDir]);

  // Subscribe to agent:stdout events for streaming
  useEffect(() => {
    if (!advisorAgentId) return;
    let unlistenFn: (() => void) | null = null;

    listen<AgentStdoutLine>("agent:stdout", (event) => {
      if (event.payload.workflowId !== advisorAgentId) return;

      const line = event.payload.line;

      // Filter out poe: control lines
      if (line.startsWith("poe:")) {
        if (line.startsWith("poe:done")) {
          setIsResponding(false);
          if (responseTimerRef.current) clearTimeout(responseTimerRef.current);
        }
        return;
      }

      setIsResponding(true);

      // Append to last assistant message or create new one
      setMessages((prev) => {
        const last = prev[prev.length - 1];
        if (last?.role === "assistant") {
          return [
            ...prev.slice(0, -1),
            { ...last, content: last.content + (last.content ? "\n" : "") + line },
          ];
        }
        return [
          ...prev,
          { role: "assistant", content: line, ts: new Date().toISOString() },
        ];
      });

      // Reset done-detection timer
      if (responseTimerRef.current) clearTimeout(responseTimerRef.current);
      responseTimerRef.current = setTimeout(() => {
        setIsResponding(false);
      }, 3000);
    }).then((fn) => { unlistenFn = fn; });

    return () => {
      unlistenFn?.();
      if (responseTimerRef.current) clearTimeout(responseTimerRef.current);
    };
  }, [advisorAgentId]);

  const handleSend = useCallback(async (text: string) => {
    if (!project || isResponding) return;

    const userMsg: Message = { role: "user", content: text, ts: new Date().toISOString() };
    setMessages((prev) => [...prev, userMsg]);
    if (!sessionStartedAt) setSessionStartedAt(userMsg.ts);
    setIsResponding(true);

    try {
      if (!advisorAgentId) {
        // First message — spawn the advisor
        const result = await invoke<AdvisorStartResult>("start_advisor_query", {
          projectDir: project.projectDir,
          query: text,
        });
        setAdvisorAgentId(result.agentId);
        setSessionId(result.sessionId);
      } else {
        // Follow-up message
        await invoke<void>("send_advisor_message", {
          agentId: advisorAgentId,
          message: text,
          projectDir: project.projectDir,
        });
      }
    } catch (err) {
      console.error("Advisor error:", err);
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: `Error: ${err}`, ts: new Date().toISOString() },
      ]);
      setIsResponding(false);
    }
  }, [project, advisorAgentId, isResponding, sessionStartedAt]);

  const handleReset = useCallback(async () => {
    if (!project) return;
    if (!confirm("Start a new advisor session? The current conversation history will be cleared.")) return;

    try {
      await invoke<void>("reset_advisor_session", { projectDir: project.projectDir });
    } catch (err) {
      console.error("Reset error:", err);
    }

    setMessages([]);
    setAdvisorAgentId(null);
    setSessionId(null);
    setSessionStartedAt(null);
    setIsResponding(false);
  }, [project]);

  // Suppress unused variable warning — sessionId is stored for potential future use
  void sessionId;

  if (!project) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <p style={{ fontSize: 13, color: "var(--text-tertiary)" }}>No project open.</p>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      <SessionControls
        sessionStartedAt={sessionStartedAt}
        messageCount={messages.filter((m) => m.role === "user").length}
        onReset={handleReset}
      />
      <MessageList messages={messages} isResponding={isResponding} />
      <InputBar onSend={handleSend} disabled={isResponding} />
    </div>
  );
}
