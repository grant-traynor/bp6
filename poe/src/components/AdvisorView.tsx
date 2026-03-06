/**
 * bp6-coj + bp6-8nr: Project Advisor panel
 *
 * Renders responses from the advisor agent. Each query spawns a one-shot
 * claude -p --output-format stream-json process; we parse the NDJSON stream
 * to extract text content, tool activity, and diffs cleanly.
 */
import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useAtomValue } from "jotai";
import {
  MessageSquare, RefreshCw, ChevronDown, ChevronRight,
  FileText, Search, Terminal, FolderOpen, Edit3,
} from "lucide-react";
import { projectAtom } from "../store/dag";
import type { AgentStdoutLine } from "../types";

// ── Tauri types ───────────────────────────────────────────────────────────────

interface AdvisorResult {
  agentId: string;
  sessionId: string;
}

interface AdvisorStateResult {
  sessionId: string | null;
  isActive: boolean;
}

// ── Message model ─────────────────────────────────────────────────────────────

interface ToolActivity {
  id: string;
  name: string;
  input: Record<string, unknown>;
  output?: string;
  isError?: boolean;
}

interface AssistantMessage {
  role: "assistant";
  text: string;
  tools: ToolActivity[];
  ts: string;
}

interface UserMessage {
  role: "user";
  text: string;
  ts: string;
}

type Message = UserMessage | AssistantMessage;

// ── stream-json parsing ───────────────────────────────────────────────────────

type StreamEvent =
  | { type: "text"; text: string }
  | { type: "tool_use"; id: string; name: string; input: Record<string, unknown> }
  | { type: "tool_result"; toolUseId: string; output: string; isError: boolean }
  | { type: "done"; sessionId: string }
  | { type: "session_error"; message: string; isNoSession: boolean }
  | { type: "ignored" };

/**
 * Parse a single NDJSON line from --output-format stream-json into zero or
 * more structured events. Returns [] for lines we don't care about.
 */
function parseStreamLine(raw: string): StreamEvent[] {
  const line = raw.trim();
  if (!line.startsWith("{")) return [];
  let parsed: Record<string, unknown>;
  try {
    parsed = JSON.parse(line);
  } catch {
    return [];
  }

  switch (parsed.type) {
    case "assistant": {
      const content = (parsed.message as { content?: unknown[] } | undefined)?.content ?? [];
      if (!Array.isArray(content)) return [];
      return content.flatMap((block): StreamEvent[] => {
        const b = block as Record<string, unknown>;
        if (b.type === "text") return [{ type: "text", text: String(b.text ?? "") }];
        if (b.type === "tool_use") {
          return [{
            type: "tool_use",
            id: String(b.id ?? ""),
            name: String(b.name ?? ""),
            input: (b.input ?? {}) as Record<string, unknown>,
          }];
        }
        return [];
      });
    }
    case "user": {
      // Tool results arrive as "user" turn content
      const content = (parsed.message as { content?: unknown[] } | undefined)?.content ?? [];
      if (!Array.isArray(content)) return [];
      return content.flatMap((block): StreamEvent[] => {
        const b = block as Record<string, unknown>;
        if (b.type === "tool_result") {
          const raw2 = b.content;
          const output = typeof raw2 === "string" ? raw2 : JSON.stringify(raw2, null, 2);
          return [{
            type: "tool_result",
            toolUseId: String(b.tool_use_id ?? ""),
            output,
            isError: Boolean(b.is_error),
          }];
        }
        return [];
      });
    }
    case "result": {
      const isErr = Boolean(parsed.is_error);
      const sessionId = String(parsed.session_id ?? "");
      if (isErr) {
        const errors = Array.isArray(parsed.errors) ? parsed.errors as string[] : [];
        const message = errors.join("; ");
        const isNoSession = message.includes("No conversation found");
        return [{ type: "session_error", message, isNoSession }];
      }
      return [{ type: "done", sessionId }];
    }
    default:
      return [];
  }
}

// ── Rich text rendering ───────────────────────────────────────────────────────

type TextPart =
  | { kind: "text"; content: string }
  | { kind: "code"; lang: string; content: string };

function splitCodeBlocks(text: string): TextPart[] {
  const parts: TextPart[] = [];
  const re = /```(\w*)\n?([\s\S]*?)```/g;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) parts.push({ kind: "text", content: text.slice(last, m.index) });
    parts.push({ kind: "code", lang: m[1] || "", content: m[2] });
    last = m.index + m[0].length;
  }
  if (last < text.length) parts.push({ kind: "text", content: text.slice(last) });
  return parts;
}

function CodeBlock({ lang, code }: { lang: string; code: string }) {
  // Treat as diff if lang=diff, or if content has unified diff markers
  const isDiff = lang === "diff" || (!lang && /^@@/m.test(code));
  return (
    <pre
      className="mono"
      style={{
        background: "#0D1117", border: "1px solid var(--border)", borderRadius: 6,
        padding: "8px 12px", fontSize: 11, lineHeight: 1.6,
        overflowX: "auto", margin: "6px 0", whiteSpace: "pre",
      }}
    >
      {isDiff
        ? code.split("\n").map((line, i) => (
            <span key={i} style={{
              display: "block",
              color: line.startsWith("+") && !line.startsWith("+++") ? "#3FB950"
                : line.startsWith("-") && !line.startsWith("---") ? "#F85149"
                : line.startsWith("@@") ? "#79C0FF"
                : "#8B949E",
            }}>
              {line}
            </span>
          ))
        : <span style={{ color: "#E6EDF3" }}>{code}</span>
      }
    </pre>
  );
}

function RichText({ text }: { text: string }) {
  return (
    <>
      {splitCodeBlocks(text).map((p, i) =>
        p.kind === "text"
          ? <span key={i} style={{ whiteSpace: "pre-wrap", wordBreak: "break-word" }}>{p.content}</span>
          : <CodeBlock key={i} lang={p.lang} code={p.content} />
      )}
    </>
  );
}

// ── Tool activity row ─────────────────────────────────────────────────────────

const TOOL_ICONS: Record<string, React.ReactNode> = {
  Read:  <FileText size={11} />,
  Grep:  <Search size={11} />,
  Glob:  <FolderOpen size={11} />,
  Bash:  <Terminal size={11} />,
  Edit:  <Edit3 size={11} />,
  Write: <Edit3 size={11} />,
};

function toolSummary(name: string, input: Record<string, unknown>): string {
  switch (name) {
    case "Read":
    case "Write":
    case "Edit":   return String(input.file_path ?? input.path ?? "");
    case "Grep":   return `${String(input.pattern ?? "")} in ${String(input.path ?? ".")}`;
    case "Glob":   return String(input.pattern ?? "");
    case "Bash":   return String(input.command ?? "").slice(0, 80);
    default:       return JSON.stringify(input).slice(0, 80);
  }
}

function ToolRow({ tool }: { tool: ToolActivity }) {
  const [open, setOpen] = useState(false);
  const icon = TOOL_ICONS[tool.name] ?? <Terminal size={11} />;
  const summary = toolSummary(tool.name, tool.input);
  const pending = tool.output === undefined;

  return (
    <div style={{
      borderRadius: 6, border: "1px solid var(--border)",
      background: "var(--content-secondary-bg)", overflow: "hidden", margin: "3px 0",
    }}>
      <button
        onClick={() => !pending && setOpen((v) => !v)}
        style={{
          width: "100%", display: "flex", alignItems: "center", gap: 6,
          padding: "5px 8px", background: "transparent", border: "none",
          cursor: pending ? "default" : "pointer",
          color: tool.isError ? "var(--status-failed)" : "var(--text-secondary)",
          fontSize: 11, fontFamily: "inherit", textAlign: "left",
        }}
      >
        <span style={{ color: "var(--text-tertiary)", flexShrink: 0 }}>{icon}</span>
        <span style={{ fontWeight: 500, flexShrink: 0 }}>{tool.name}</span>
        <span className="mono" style={{
          color: "var(--text-tertiary)", overflow: "hidden",
          textOverflow: "ellipsis", whiteSpace: "nowrap", flex: 1,
        }}>
          {summary}
        </span>
        {pending && (
          <span className="mono" style={{ fontSize: 10, color: "var(--text-placeholder)", flexShrink: 0 }}>
            running…
          </span>
        )}
        {!pending && tool.output && (
          <span style={{ color: "var(--text-placeholder)", flexShrink: 0 }}>
            {open ? <ChevronDown size={10} /> : <ChevronRight size={10} />}
          </span>
        )}
      </button>
      {open && tool.output && (
        <div className="mono" style={{
          borderTop: "1px solid var(--border)", padding: "6px 8px",
          fontSize: 10, lineHeight: 1.6, color: "var(--text-secondary)",
          whiteSpace: "pre-wrap", wordBreak: "break-all",
          maxHeight: 200, overflowY: "auto", background: "#0D1117",
        }}>
          {tool.output}
        </div>
      )}
    </div>
  );
}

// ── Message list ──────────────────────────────────────────────────────────────

function MessageList({ messages, isResponding }: { messages: Message[]; isResponding: boolean }) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages.length, isResponding]);

  if (messages.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-3" style={{ padding: 32 }}>
        <MessageSquare size={28} style={{ color: "var(--text-placeholder)" }} />
        <p style={{ fontSize: 13, color: "var(--text-secondary)", margin: 0, textAlign: "center" }}>
          Ask anything about your project
        </p>
        <div style={{ marginTop: 8, display: "flex", flexDirection: "column", gap: 6, width: "100%", maxWidth: 340 }}>
          {[
            "What is currently blocked?",
            "Summarise what was built in the last stage",
            "Which agents have been running the longest?",
          ].map((q) => (
            <p key={q} className="mono" style={{
              fontSize: 11, color: "var(--text-tertiary)", margin: 0, textAlign: "center",
              border: "1px dashed var(--border)", borderRadius: 6, padding: "6px 10px",
            }}>
              Try: &ldquo;{q}&rdquo;
            </p>
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-auto" style={{
      padding: "12px 16px", display: "flex", flexDirection: "column", gap: 10,
    }}>
      {messages.map((msg, i) =>
        msg.role === "user" ? (
          <div key={i} style={{ display: "flex", justifyContent: "flex-end" }}>
            <div style={{
              maxWidth: "80%", padding: "8px 12px",
              borderRadius: "12px 12px 2px 12px",
              background: "var(--accent)", color: "#fff",
              fontSize: 12, lineHeight: 1.6,
              whiteSpace: "pre-wrap", wordBreak: "break-word",
            }}>
              {msg.text}
            </div>
          </div>
        ) : (
          <div key={i} style={{ display: "flex", flexDirection: "column", alignItems: "flex-start", maxWidth: "92%" }}>
            {msg.tools.length > 0 && (
              <div style={{ width: "100%", marginBottom: 4 }}>
                {msg.tools.map((t) => <ToolRow key={t.id} tool={t} />)}
              </div>
            )}
            {msg.text && (
              <div style={{
                padding: "8px 12px",
                borderRadius: msg.tools.length > 0 ? "2px 12px 12px 12px" : "12px 12px 12px 2px",
                background: "var(--content-secondary-bg)",
                border: "1px solid var(--border)",
                fontSize: 12, lineHeight: 1.6, color: "var(--text-primary)",
                maxWidth: "100%",
              }}>
                <RichText text={msg.text} />
              </div>
            )}
          </div>
        )
      )}

      {isResponding && (
        <div style={{ display: "flex" }}>
          <div style={{
            padding: "10px 14px",
            borderRadius: "12px 12px 12px 2px",
            background: "var(--content-secondary-bg)",
            border: "1px solid var(--border)",
            display: "flex", gap: 4, alignItems: "center",
          }}>
            {[0, 150, 300].map((d) => (
              <span key={d} style={{
                width: 5, height: 5, borderRadius: "50%",
                background: "var(--text-tertiary)",
                animation: "pulse 1.2s ease-in-out infinite",
                animationDelay: `${d}ms`,
              }} />
            ))}
          </div>
        </div>
      )}

      <div ref={bottomRef} />
    </div>
  );
}

// ── Input bar ─────────────────────────────────────────────────────────────────

function InputBar({ onSend, disabled }: { onSend: (text: string) => void; disabled: boolean }) {
  const [text, setText] = useState("");

  function submit() {
    const t = text.trim();
    if (!t || disabled) return;
    onSend(t);
    setText("");
  }

  return (
    <div style={{
      borderTop: "1px solid var(--divider)", padding: "10px 12px",
      display: "flex", gap: 8, alignItems: "flex-end",
    }}>
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); submit(); } }}
        placeholder="Ask about your project…"
        disabled={disabled}
        rows={1}
        style={{
          flex: 1, resize: "none",
          background: "var(--content-secondary-bg)",
          border: "1px solid var(--border)",
          borderRadius: 8, padding: "7px 10px",
          fontSize: 12, color: "var(--text-primary)", fontFamily: "inherit",
          outline: "none", lineHeight: 1.5, minHeight: 34, maxHeight: 120,
          opacity: disabled ? 0.5 : 1,
        }}
      />
      <button
        onClick={submit}
        disabled={disabled || !text.trim()}
        className="mac-btn"
        style={{ padding: "7px 14px", fontSize: 12, flexShrink: 0 }}
      >
        Send
      </button>
    </div>
  );
}

// ── Session controls ──────────────────────────────────────────────────────────

function timeSince(iso: string): string {
  const ms = Date.now() - new Date(iso).getTime();
  if (isNaN(ms)) return "";
  const m = Math.floor(ms / 60000);
  if (m < 1) return "just started";
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  return h < 24 ? `${h}h ${m % 60}m ago` : `${Math.floor(h / 24)}d ago`;
}

function SessionControls({
  sessionStartedAt, turnCount, onReset,
}: {
  sessionStartedAt: string | null;
  turnCount: number;
  onReset: () => void;
}) {
  const [, setTick] = useState(0);
  useEffect(() => {
    const id = setInterval(() => setTick((t) => t + 1), 30000);
    return () => clearInterval(id);
  }, []);

  return (
    <div className="mac-toolbar flex items-center justify-between" style={{ padding: "0 12px", height: 36, minHeight: 36 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <MessageSquare size={13} style={{ color: "var(--text-tertiary)" }} />
        <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-secondary)" }}>Project Advisor</span>
        {sessionStartedAt && (
          <span className="mono" style={{ fontSize: 10, color: "var(--text-placeholder)" }}>
            {turnCount} {turnCount === 1 ? "turn" : "turns"} · {timeSince(sessionStartedAt)}
          </span>
        )}
      </div>
      <button
        onClick={onReset}
        className="mac-btn mac-btn-ghost"
        style={{ padding: "3px 8px", fontSize: 11, display: "flex", alignItems: "center", gap: 4 }}
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
  const [currentAgentId, setCurrentAgentId] = useState<string | null>(null);
  const [isResponding, setIsResponding] = useState(false);
  const [sessionStartedAt, setSessionStartedAt] = useState<string | null>(null);

  // Refs so event listeners always see fresh values without re-subscribing
  const projectDirRef = useRef<string | null>(null);
  useEffect(() => { projectDirRef.current = project?.projectDir ?? null; }, [project?.projectDir]);

  // Check for existing session on mount
  useEffect(() => {
    if (!project) return;
    invoke<AdvisorStateResult>("get_advisor_state", { projectDir: project.projectDir })
      .catch(console.error);
  }, [project?.projectDir]);

  // Subscribe to agent:stdout for the current turn's agent
  useEffect(() => {
    if (!currentAgentId) return;
    let unlisten: (() => void) | null = null;
    let doneTimer: ReturnType<typeof setTimeout> | null = null;

    function markDone() {
      if (doneTimer) clearTimeout(doneTimer);
      setIsResponding(false);
    }

    listen<AgentStdoutLine>("agent:stdout", (event) => {
      if (event.payload.workflowId !== currentAgentId) return;

      const events = parseStreamLine(event.payload.line);
      if (events.length === 0) return;

      // Fallback done-timer: if no "done" event arrives within 5s of last output
      if (doneTimer) clearTimeout(doneTimer);
      doneTimer = setTimeout(markDone, 5000);

      setMessages((prev) => {
        let msgs = [...prev];

        function lastAssistant(): AssistantMessage {
          const last = msgs[msgs.length - 1];
          if (last?.role === "assistant") return last as AssistantMessage;
          const msg: AssistantMessage = { role: "assistant", text: "", tools: [], ts: new Date().toISOString() };
          msgs = [...msgs, msg];
          return msg;
        }

        for (const ev of events) {
          if (ev.type === "text") {
            const msg = lastAssistant();
            msgs = [...msgs.slice(0, -1), { ...msg, text: msg.text + ev.text }];
          } else if (ev.type === "tool_use") {
            const msg = lastAssistant();
            const tool: ToolActivity = { id: ev.id, name: ev.name, input: ev.input };
            msgs = [...msgs.slice(0, -1), { ...msg, tools: [...msg.tools, tool] }];
          } else if (ev.type === "tool_result") {
            msgs = msgs.map((m) => {
              if (m.role !== "assistant") return m;
              const am = m as AssistantMessage;
              return {
                ...am,
                tools: am.tools.map((t) =>
                  t.id === ev.toolUseId ? { ...t, output: ev.output, isError: ev.isError } : t
                ),
              };
            });
          } else if (ev.type === "done") {
            // Confirm session only now that we know Claude accepted it
            const dir = projectDirRef.current;
            if (dir && ev.sessionId) {
              invoke("confirm_advisor_session", {
                projectDir: dir,
                sessionId: ev.sessionId,
              }).catch(console.error);
            }
            markDone();
          } else if (ev.type === "session_error") {
            const dir = projectDirRef.current;
            if (ev.isNoSession && dir) {
              // Stale session ID — clear it so next turn starts fresh
              invoke("reset_advisor_session", { projectDir: dir }).catch(console.error);
              msgs = [...msgs, {
                role: "assistant" as const,
                text: "_(Session expired — conversation reset. Please send your message again.)_",
                tools: [],
                ts: new Date().toISOString(),
              }];
            } else {
              msgs = [...msgs, {
                role: "assistant" as const,
                text: `Error: ${ev.message}`,
                tools: [],
                ts: new Date().toISOString(),
              }];
            }
            markDone();
          }
        }

        return msgs;
      });
    }).then((fn) => { unlisten = fn; });

    return () => {
      unlisten?.();
      if (doneTimer) clearTimeout(doneTimer);
    };
  }, [currentAgentId]);

  const handleSend = useCallback(async (text: string) => {
    if (!project || isResponding) return;
    const ts = new Date().toISOString();
    setMessages((prev) => [...prev, { role: "user", text, ts }]);
    if (!sessionStartedAt) setSessionStartedAt(ts);
    setIsResponding(true);
    try {
      const result = await invoke<AdvisorResult>("start_advisor_query", {
        projectDir: project.projectDir,
        query: text,
      });
      setCurrentAgentId(result.agentId);
    } catch (err) {
      setMessages((prev) => [...prev, {
        role: "assistant", text: `Error: ${String(err)}`, tools: [], ts: new Date().toISOString(),
      }]);
      setIsResponding(false);
    }
  }, [project, isResponding, sessionStartedAt]);

  const handleReset = useCallback(async () => {
    if (!project) return;
    if (!confirm("Start a new advisor session? The current conversation history will be cleared.")) return;
    try {
      await invoke<void>("reset_advisor_session", { projectDir: project.projectDir });
    } catch (err) {
      console.error("Reset error:", err);
    }
    setMessages([]);
    setCurrentAgentId(null);
    setSessionStartedAt(null);
    setIsResponding(false);
  }, [project]);

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
        turnCount={messages.filter((m) => m.role === "user").length}
        onReset={handleReset}
      />
      <MessageList messages={messages} isResponding={isResponding} />
      <InputBar onSend={handleSend} disabled={isResponding} />
    </div>
  );
}
