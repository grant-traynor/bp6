/**
 * bp6-80q.3 + bp6-80q.4: Probe & inspect + Stop & redirect controls
 *
 * Deep inspection of any DAG node. Fetches ProbeData from SQLite on mount.
 * Stop/redirect controls for nodes with a workflowId in their data.
 */
import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSetAtom } from "jotai";
import { selectedNodeIdAtom } from "../store/dag";
import type { ProbeData, DagNode, DagEdge } from "../types";

// ── Edge pill ──────────────────────────────────────────────────────────────────

const EDGE_COLORS: Record<string, string> = {
  "blocks": "bg-red-100 border-red-400 text-red-700",
  "depends-on": "bg-amber-100 border-amber-400 text-amber-700",
  "generated-by": "bg-blue-100 border-blue-400 text-blue-700",
  "approved-by": "bg-green-100 border-green-400 text-green-700",
  "discovered-from": "bg-purple-100 border-purple-400 text-purple-700",
  "implements": "bg-stone-100 border-stone-400 text-stone-700",
  "tests": "bg-cyan-100 border-cyan-400 text-cyan-700",
  "contradicts": "bg-rose-100 border-rose-400 text-rose-700",
};

function EdgePill({ edge, nodeLabel }: { edge: DagEdge; nodeLabel: string }) {
  const cls = EDGE_COLORS[edge.edgeType] ?? "bg-stone-100 border-stone-300 text-stone-600";
  return (
    <div className={`border-2 px-2 py-1 text-xs font-mono flex items-center gap-2 ${cls}`}>
      <span className="font-bold">{edge.edgeType}</span>
      <span className="opacity-70">→ {nodeLabel}</span>
    </div>
  );
}

function NodeChip({ node, onClick }: { node: DagNode; onClick: () => void }) {
  const title = String(node.data.title ?? node.data.name ?? node.id.slice(0, 8));
  return (
    <button
      onClick={onClick}
      className="border-2 border-black px-2 py-1 text-xs font-mono hover:bg-black hover:text-white transition-colors text-left"
    >
      <span className="font-bold">{node.nodeType}</span>{" "}
      <span className="opacity-70">{title}</span>
    </button>
  );
}

// ── Section ────────────────────────────────────────────────────────────────────

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="border-t-2 border-stone-200 pt-3 mt-3">
      <p className="font-mono text-xs font-bold text-stone-500 uppercase tracking-wider mb-2">{title}</p>
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
        <p className="font-mono text-xs border-2 border-black px-2 py-1 mb-2 bg-stone-50">{msg}</p>
      )}
      {/* Redirect */}
      <div className="space-y-1 mb-3">
        <textarea
          value={redirectText}
          onChange={(e) => setRedirectText(e.target.value)}
          placeholder="Redirect instruction…"
          className="w-full border-2 border-black font-mono text-xs p-2 resize-none h-16 focus:outline-none focus:border-blue-500"
        />
        <button
          onClick={handleRedirect}
          disabled={busy || !redirectText.trim()}
          className="w-full border-2 border-black font-mono text-xs py-1 hover:bg-black hover:text-white transition-colors disabled:opacity-40"
        >
          Redirect Agent
        </button>
      </div>
      {/* Stop */}
      <div className="space-y-1">
        <input
          value={stopReason}
          onChange={(e) => setStopReason(e.target.value)}
          placeholder="Stop reason (optional)…"
          className="w-full border-2 border-black font-mono text-xs p-2 focus:outline-none focus:border-red-400"
        />
        <button
          onClick={handleStop}
          disabled={busy}
          className="w-full border-2 border-red-600 text-red-700 font-mono text-xs py-1 hover:bg-red-600 hover:text-white transition-colors disabled:opacity-40"
        >
          Stop Workflow
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
    <div className="flex flex-col h-full bg-white">
      {/* Header */}
      <div className="border-b-4 border-black px-4 py-2 flex items-center justify-between shrink-0">
        <span className="font-black text-sm tracking-tight">Probe</span>
        <div className="flex gap-2">
          <button
            onClick={load}
            className="font-mono text-xs border-2 border-black px-2 py-0.5 hover:bg-stone-100"
          >
            ↻
          </button>
          <button
            onClick={() => setSelectedNodeId(null)}
            className="font-mono text-xs border-2 border-black px-2 py-0.5 hover:bg-black hover:text-white transition-colors"
          >
            ✕
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-auto p-4">
        {loading && (
          <p className="font-mono text-xs text-stone-400 animate-pulse">Loading…</p>
        )}
        {error && (
          <p className="font-mono text-xs text-red-600 border-2 border-red-400 p-2">{error}</p>
        )}

        {data && (
          <>
            {/* Node identity */}
            <div>
              <p className="font-mono text-xs text-stone-400 mb-0.5">{data.node.nodeType}</p>
              <p className="font-black text-base leading-tight">
                {String(data.node.data.title ?? data.node.data.name ?? data.node.id)}
              </p>
              <p className="font-mono text-xs text-stone-400 mt-0.5">{data.node.id}</p>
            </div>

            {/* Data fields */}
            <Section title="Data">
              <pre className="font-mono text-xs bg-stone-50 border-2 border-stone-200 p-2 overflow-auto max-h-32 whitespace-pre-wrap">
                {JSON.stringify(data.node.data, null, 2)}
              </pre>
            </Section>

            {/* Incoming edges */}
            {data.incomingEdges.length > 0 && (
              <Section title={`Incoming (${data.incomingEdges.length})`}>
                <div className="space-y-1">
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
                <div className="space-y-1">
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
                <div className="space-y-1">
                  {data.decisions.map((n) => (
                    <NodeChip key={n.id} node={n} onClick={() => setSelectedNodeId(n.id)} />
                  ))}
                </div>
              </Section>
            )}

            {/* Artifacts */}
            {data.artifacts.length > 0 && (
              <Section title={`Artifacts (${data.artifacts.length})`}>
                <div className="space-y-1">
                  {data.artifacts.map((n) => (
                    <NodeChip key={n.id} node={n} onClick={() => setSelectedNodeId(n.id)} />
                  ))}
                </div>
              </Section>
            )}

            {/* Stop/redirect */}
            <StopRedirectControls node={data.node} />
          </>
        )}
      </div>
    </div>
  );
}
