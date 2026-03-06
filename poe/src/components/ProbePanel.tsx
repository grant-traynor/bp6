/**
 * bp6-80q.3 + bp6-80q.4 + bp6-3d1.5: Probe & inspect + Stop & redirect + Work assignment
 *
 * Deep inspection of any DAG node. Fetches ProbeData from SQLite on mount.
 * Stop/redirect controls for nodes with a workflowId in their data.
 * Work assignment for unassigned nodes (bp6-3d1.5).
 */
import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSetAtom } from "jotai";
import { selectedNodeIdAtom } from "../store/dag";
import type { ProbeData, DagNode, DagEdge } from "../types";

// ── V-lifecycle workflow types ──────────────────────────────────────────────────

const WORKFLOW_TYPES = [
  "ImplementationWorkflow",
  "DesignWorkflow",
  "RequirementsWorkflow",
  "ValidationWorkflow",
  "DeploymentWorkflow",
] as const;

type WorkflowType = (typeof WORKFLOW_TYPES)[number];

function inferWorkflowType(nodeType: string): WorkflowType {
  if (nodeType === "Epic") return "DesignWorkflow";
  if (nodeType === "Review") return "ValidationWorkflow";
  return "ImplementationWorkflow";
}

// ── Edge pill ──────────────────────────────────────────────────────────────────

const EDGE_COLORS: Record<string, { color: string; bg: string }> = {
  "blocks":           { color: "var(--status-failed)",    bg: "var(--status-failed-bg)" },
  "depends-on":       { color: "var(--status-paused)",    bg: "var(--status-paused-bg)" },
  "generated-by":     { color: "var(--status-running)",   bg: "var(--status-running-bg)" },
  "approved-by":      { color: "var(--status-completed)", bg: "var(--status-completed-bg)" },
  "discovered-from":  { color: "#9B59B6",                 bg: "rgba(155,89,182,0.08)" },
  "implements":       { color: "var(--text-secondary)",   bg: "var(--content-secondary-bg)" },
  "tests":            { color: "#17A2B8",                 bg: "rgba(23,162,184,0.08)" },
  "contradicts":      { color: "var(--status-failed)",    bg: "var(--status-failed-bg)" },
};

function EdgePill({ edge, nodeLabel }: { edge: DagEdge; nodeLabel: string }) {
  const meta = EDGE_COLORS[edge.edgeType] ?? { color: "var(--text-secondary)", bg: "var(--content-secondary-bg)" };
  return (
    <div className="mono" style={{
      fontSize: 11, padding: "3px 8px", borderRadius: 4,
      color: meta.color, background: meta.bg,
      display: "flex", alignItems: "center", gap: 6,
    }}>
      <span style={{ fontWeight: 600 }}>{edge.edgeType}</span>
      <span style={{ opacity: 0.7 }}>→ {nodeLabel}</span>
    </div>
  );
}

function NodeChip({ node, onClick }: { node: DagNode; onClick: () => void }) {
  const title = String(node.data.title ?? node.data.name ?? node.id.slice(0, 8));
  return (
    <button
      onClick={onClick}
      className="mac-btn"
      style={{ fontSize: 11, padding: "3px 8px", textAlign: "left", justifyContent: "flex-start" }}
    >
      <span style={{ fontWeight: 600 }}>{node.nodeType}</span>{" "}
      <span style={{ opacity: 0.7 }}>{title}</span>
    </button>
  );
}

// ── Section ────────────────────────────────────────────────────────────────────

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div style={{ borderTop: "1px solid var(--divider)", paddingTop: 10, marginTop: 10 }}>
      <p className="mac-section-header" style={{ padding: "0 0 6px 0", margin: 0 }}>{title}</p>
      {children}
    </div>
  );
}

// ── Stop/Redirect controls ─────────────────────────────────────────────────────

function StopRedirectControls({ node }: { node: DagNode }) {
  const workflowId = String(node.data.workflowId ?? "");
  const [redirectText, setRedirectText] = useState("");
  const [stopReason, setStopReason] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  if (!workflowId) return null;

  async function handleStop() {
    setBusy(true);
    setMsg(null);
    try {
      await invoke("stop_workflow", {
        params: { workflowId, nodeId: node.id, reason: stopReason || undefined },
      });
      setMsg("Workflow stopped.");
    } catch (e) {
      setMsg(`Error: ${e}`);
    } finally {
      setBusy(false);
    }
  }

  async function handleRedirect() {
    if (!redirectText.trim()) return;
    setBusy(true);
    setMsg(null);
    try {
      await invoke("redirect_workflow", {
        params: { workflowId, nodeId: node.id, instruction: redirectText },
      });
      setMsg("Redirect recorded.");
      setRedirectText("");
    } catch (e) {
      setMsg(`Error: ${e}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Section title="Controls">
      {msg && (
        <p className="mono" style={{
          fontSize: 11, color: "var(--text-secondary)", background: "var(--content-secondary-bg)",
          padding: "5px 8px", borderRadius: 5, marginBottom: 8, border: "1px solid var(--border)",
        }}>{msg}</p>
      )}
      {/* Redirect */}
      <div style={{ marginBottom: 10 }}>
        <textarea
          value={redirectText}
          onChange={(e) => setRedirectText(e.target.value)}
          placeholder="Redirect instruction…"
          className="mac-input mono"
          style={{ height: 56, resize: "none", fontSize: 11, marginBottom: 5 }}
        />
        <button
          onClick={handleRedirect}
          disabled={busy || !redirectText.trim()}
          className="mac-btn"
          style={{ width: "100%", justifyContent: "center", fontSize: 11, padding: "5px 10px" }}
        >
          Redirect Agent
        </button>
      </div>
      {/* Stop */}
      <div>
        <input
          value={stopReason}
          onChange={(e) => setStopReason(e.target.value)}
          placeholder="Stop reason (optional)…"
          className="mac-input mono"
          style={{ fontSize: 11, marginBottom: 5 }}
        />
        <button
          onClick={handleStop}
          disabled={busy}
          className="mac-btn mac-btn-destructive"
          style={{ width: "100%", justifyContent: "center", fontSize: 11, padding: "5px 10px" }}
        >
          Stop Workflow
        </button>
      </div>
    </Section>
  );
}

// ── Assign to Agent (bp6-3d1.5) ───────────────────────────────────────────────

function AssignToAgentControls({ node }: { node: DagNode }) {
  const workflowId = String(node.data.workflowId ?? "");
  const [cmd, setCmd] = useState("claude");
  const [args, setArgs] = useState("--dangerously-skip-permissions -p");
  const [workflowType, setWorkflowType] = useState<WorkflowType>(
    inferWorkflowType(node.nodeType)
  );
  const [fanout, setFanout] = useState(false);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);

  // Already has an active workflow — don't show assign controls
  if (workflowId) return null;
  if (!["Task", "Feature", "Epic", "Review"].includes(node.nodeType)) return null;

  async function handleAssign() {
    setBusy(true);
    setMsg(null);
    try {
      const nodeTitle = String(node.data.title ?? node.data.name ?? node.id);
      const prompt = `You are working on a ${node.nodeType}: "${nodeTitle}". Report progress using poe: JSON events.`;
      const parsedArgs = [...args.split(/\s+/).filter(Boolean), prompt];
      const result = await invoke<{ workflow: { id: string }; agentId: string }>(
        "create_workflow",
        {
          params: {
            nodeId: node.id,
            workflowType,
            cmd,
            args: parsedArgs,
            fanout,
          },
        }
      );
      setMsg(`Started — wf: ${result.workflow.id.slice(0, 12)}…`);
    } catch (e) {
      setMsg(`Error: ${e}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Section title="Assign to Agent">
      {msg && (
        <p className="mono" style={{
          fontSize: 11, color: "var(--text-secondary)", background: "var(--content-secondary-bg)",
          padding: "5px 8px", borderRadius: 5, marginBottom: 8, wordBreak: "break-all",
          border: "1px solid var(--border)",
        }}>{msg}</p>
      )}
      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        <div>
          <label className="mono" style={{ fontSize: 10, color: "var(--text-tertiary)", display: "block", marginBottom: 3 }}>Command</label>
          <input value={cmd} onChange={(e) => setCmd(e.target.value)} className="mac-input mono" style={{ fontSize: 11 }} />
        </div>
        <div>
          <label className="mono" style={{ fontSize: 10, color: "var(--text-tertiary)", display: "block", marginBottom: 3 }}>
            Args (prompt appended)
          </label>
          <input value={args} onChange={(e) => setArgs(e.target.value)} className="mac-input mono" style={{ fontSize: 11 }} />
        </div>
        <div>
          <label className="mono" style={{ fontSize: 10, color: "var(--text-tertiary)", display: "block", marginBottom: 3 }}>Workflow Type</label>
          <select
            value={workflowType}
            onChange={(e) => setWorkflowType(e.target.value as WorkflowType)}
            className="mac-input mono"
            style={{ fontSize: 11 }}
          >
            {WORKFLOW_TYPES.map((t) => (
              <option key={t} value={t}>{t}</option>
            ))}
          </select>
        </div>
        {(node.nodeType === "Epic" || node.nodeType === "Feature") && (
          <label style={{ display: "flex", alignItems: "center", gap: 6, cursor: "pointer", fontSize: 11, color: "var(--text-secondary)" }}>
            <input
              type="checkbox"
              checked={fanout}
              onChange={(e) => setFanout(e.target.checked)}
            />
            Fan-out to child nodes
          </label>
        )}
        <button
          onClick={handleAssign}
          disabled={busy || !cmd.trim()}
          className="mac-btn mac-btn-primary"
          style={{ width: "100%", justifyContent: "center", fontSize: 11, padding: "6px 12px" }}
        >
          {busy ? "Starting…" : "▶ Assign to Agent"}
        </button>
      </div>
    </Section>
  );
}

// ── Main panel ─────────────────────────────────────────────────────────────────

interface ProbePanelProps {
  nodeId: string;
}

export function ProbePanel({ nodeId }: ProbePanelProps) {
  const setSelectedNodeId = useSetAtom(selectedNodeIdAtom);
  const [data, setData] = useState<ProbeData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<ProbeData>("probe_node", { nodeId });
      setData(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [nodeId]);

  useEffect(() => { load(); }, [load]);

  const nodeLabel = (n: DagNode) =>
    String(n.data.title ?? n.data.name ?? n.id.slice(0, 8));

  return (
    <div className="flex flex-col h-full" style={{ background: "var(--inspector-bg)" }}>
      {/* Header */}
      <div className="mac-toolbar flex items-center justify-between shrink-0" style={{ padding: "0 12px", height: 36 }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)" }}>Inspector</span>
        <div className="flex gap-1">
          <button onClick={load} className="mac-btn mac-btn-ghost" style={{ padding: "2px 6px" }} title="Refresh">
            ↻
          </button>
          <button onClick={() => setSelectedNodeId(null)} className="mac-btn mac-btn-ghost" style={{ padding: "2px 6px" }} title="Close">
            ✕
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-auto" style={{ padding: "12px 14px" }}>
        {loading && (
          <p className="mono" style={{ fontSize: 11, color: "var(--text-tertiary)" }}>Loading…</p>
        )}
        {error && (
          <p className="mono" style={{
            fontSize: 11, color: "var(--status-failed)", background: "var(--status-failed-bg)",
            padding: "6px 10px", borderRadius: 5,
          }}>{error}</p>
        )}

        {data && (
          <>
            {/* Node identity */}
            <div>
              <p className="mono" style={{ fontSize: 10, color: "var(--text-tertiary)", marginBottom: 2 }}>
                {data.node.nodeType}
              </p>
              <p style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)", lineHeight: 1.4, margin: 0 }}>
                {String(data.node.data.title ?? data.node.data.name ?? data.node.id)}
              </p>
              <p className="mono" style={{ fontSize: 10, color: "var(--text-placeholder)", marginTop: 3 }}>
                {data.node.id}
              </p>
            </div>

            {/* Data fields */}
            <Section title="Data">
              <pre className="mono" style={{
                fontSize: 10, background: "var(--content-secondary-bg)", padding: "8px 10px",
                borderRadius: 5, overflow: "auto", maxHeight: 120, whiteSpace: "pre-wrap",
                border: "1px solid var(--border)", color: "var(--text-secondary)",
              }}>
                {JSON.stringify(data.node.data, null, 2)}
              </pre>
            </Section>

            {/* Incoming edges */}
            {data.incomingEdges.length > 0 && (
              <Section title={`Incoming (${data.incomingEdges.length})`}>
                <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                  {data.incomingEdges.map((e) => {
                    const src = data.connectedNodes.find((n) => n.id === e.fromId);
                    return (
                      <EdgePill key={e.id} edge={e} nodeLabel={src ? nodeLabel(src) : e.fromId.slice(0, 8)} />
                    );
                  })}
                </div>
              </Section>
            )}

            {/* Outgoing edges */}
            {data.outgoingEdges.length > 0 && (
              <Section title={`Outgoing (${data.outgoingEdges.length})`}>
                <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                  {data.outgoingEdges.map((e) => {
                    const tgt = data.connectedNodes.find((n) => n.id === e.toId);
                    return (
                      <EdgePill key={e.id} edge={e} nodeLabel={tgt ? nodeLabel(tgt) : e.toId.slice(0, 8)} />
                    );
                  })}
                </div>
              </Section>
            )}

            {/* Decision history */}
            {data.decisions.length > 0 && (
              <Section title={`Decisions (${data.decisions.length})`}>
                <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                  {data.decisions.map((n) => (
                    <NodeChip key={n.id} node={n} onClick={() => setSelectedNodeId(n.id)} />
                  ))}
                </div>
              </Section>
            )}

            {/* Artifacts */}
            {data.artifacts.length > 0 && (
              <Section title={`Artifacts (${data.artifacts.length})`}>
                <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                  {data.artifacts.map((n) => (
                    <NodeChip key={n.id} node={n} onClick={() => setSelectedNodeId(n.id)} />
                  ))}
                </div>
              </Section>
            )}

            {/* Stop/redirect */}
            <StopRedirectControls node={data.node} />

            {/* Assign to agent (Phase 3) */}
            <AssignToAgentControls node={data.node} />
          </>
        )}
      </div>
    </div>
  );
}
