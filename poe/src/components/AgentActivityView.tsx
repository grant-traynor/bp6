/**
 * bp6-80q.2: Agent activity view
 *
 * Per-agent real-time view. Subscribes to workflow:status and agent:stdout
 * events (ready for Phase 3). Shows live stdout stream, current step, elapsed time.
 */
import { useState, useEffect, useRef } from "react";
import { useAtomValue, useSetAtom } from "jotai";
import { workflowsListAtom, agentStdoutAtom, selectedNodeIdAtom } from "../store/dag";
import type { WorkflowInfo } from "../types";

function elapsed(startedAt: string | undefined): string {
  if (!startedAt) return "";
  const ms = Date.now() - new Date(startedAt).getTime();
  if (isNaN(ms)) return "";
  const s = Math.floor(ms / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${s % 60}s`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}

const STATUS_COLORS: Record<string, string> = {
  running:   "var(--status-running)",
  paused:    "var(--status-paused)",
  blocked:   "var(--status-blocked)",
  completed: "var(--status-completed)",
  cancelled: "var(--status-cancelled)",
  failed:    "var(--status-failed)",
};

function AgentPanel({ workflow }: { workflow: WorkflowInfo }) {
  const agentStdout = useAtomValue(agentStdoutAtom);
  const setSelectedNodeId = useSetAtom(selectedNodeIdAtom);
  const lines = agentStdout.get(workflow.workflowId) ?? [];
  const logRef = useRef<HTMLDivElement>(null);
  const [, setTick] = useState(0);

  // Re-render elapsed every 5s
  useEffect(() => {
    const id = setInterval(() => setTick((t) => t + 1), 5000);
    return () => clearInterval(id);
  }, []);

  // Auto-scroll stdout
  useEffect(() => {
    if (logRef.current) {
      logRef.current.scrollTop = logRef.current.scrollHeight;
    }
  }, [lines.length]);

  const dotColor = STATUS_COLORS[workflow.status] ?? "var(--text-placeholder)";

  return (
    <div className="mac-card" style={{ overflow: "hidden", padding: 0 }}>
      {/* Agent header */}
      <div className="mac-toolbar flex items-center justify-between" style={{ padding: "8px 14px" }}>
        <div className="flex items-center gap-8">
          <span style={{
            width: 7, height: 7, borderRadius: "50%", background: dotColor,
            flexShrink: 0, display: "inline-block",
            boxShadow: workflow.status === "running" ? `0 0 0 3px ${dotColor}30` : undefined,
          }} />
          <div>
            <p style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)", margin: 0 }}>
              {workflow.agentId}
            </p>
            <p className="mono" style={{ fontSize: 10, color: "var(--text-tertiary)", margin: 0 }}>
              {workflow.workflowId.slice(0, 14)}…
            </p>
          </div>
        </div>
        <div className="flex items-center gap-6">
          {workflow.currentStep && (
            <span className="mono" style={{
              fontSize: 10, color: "var(--text-secondary)",
              background: "var(--content-secondary-bg)", padding: "2px 8px", borderRadius: 4,
              border: "1px solid var(--border)",
            }}>
              {workflow.currentStep}
            </span>
          )}
          <span className="mono" style={{ fontSize: 11, color: "var(--text-tertiary)" }}>
            {elapsed(workflow.startedAt)}
          </span>
          <button
            onClick={() => setSelectedNodeId(workflow.nodeId)}
            className="mac-btn"
            style={{ padding: "3px 10px", fontSize: 11 }}
          >
            Inspect
          </button>
        </div>
      </div>

      {/* Stdout stream */}
      <div
        ref={logRef}
        className="mono"
        style={{
          overflow: "auto", background: "#0D1117", color: "#3FB950",
          fontSize: 11, padding: "10px 14px", minHeight: 100, maxHeight: 180,
          lineHeight: 1.6,
        }}
      >
        {lines.length === 0 ? (
          <span style={{ color: "#3D444D" }}>waiting for output…</span>
        ) : (
          lines.map((l, i) => (
            <div key={i} style={{ whiteSpace: "pre-wrap", wordBreak: "break-all" }}>
              {l.line}
            </div>
          ))
        )}
      </div>
    </div>
  );
}

export function AgentActivityView() {
  const workflows = useAtomValue(workflowsListAtom);
  // Show all workflows — completed/failed stay visible for inspection.
  // Sort: running first, then by recency.
  const active = [...workflows].sort((a, b) => {
    const rank = (s: string) => s === "running" ? 0 : s === "paused" ? 1 : s === "blocked" ? 2 : 3;
    return rank(a.status) - rank(b.status);
  });

  if (active.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div style={{
          border: "1px dashed var(--border-strong)", borderRadius: 10,
          padding: "48px 32px", textAlign: "center",
        }}>
          <p style={{ fontSize: 13, color: "var(--text-tertiary)", margin: 0 }}>No agents running.</p>
          <p style={{ fontSize: 11, color: "var(--text-placeholder)", marginTop: 4 }}>
            Agents will appear here when workflows start.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-auto" style={{ padding: 16, display: "flex", flexDirection: "column", gap: 12 }}>
      {active.map((wf) => (
        <AgentPanel key={wf.workflowId} workflow={wf} />
      ))}
    </div>
  );
}
