/**
 * bp6-80q.5: Provenance traversal
 *
 * Full lineage view for any DAG node. Recursive CTE walks N hops from the
 * selected node in both directions. Renders subgraph as a navigable list.
 */
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSetAtom } from "jotai";
import { selectedNodeIdAtom } from "../store/dag";
import type { DagSnapshot, DagNode } from "../types";

const NODE_COLORS: Record<string, { color: string; bg: string; border: string }> = {
  Decision:          { color: "var(--status-paused)",    bg: "var(--status-paused-bg)",    border: "var(--status-paused)" },
  AgentOutput:       { color: "var(--status-running)",   bg: "var(--status-running-bg)",   border: "var(--status-running)" },
  KnowledgeArtifact: { color: "var(--status-completed)", bg: "var(--status-completed-bg)", border: "var(--status-completed)" },
  Project:           { color: "var(--text-primary)",     bg: "var(--card-bg)",             border: "var(--border-strong)" },
  Epic:              { color: "var(--text-primary)",     bg: "var(--content-secondary-bg)", border: "var(--border-strong)" },
  Feature:           { color: "var(--text-secondary)",   bg: "transparent",                border: "var(--border)" },
  Task:              { color: "var(--text-secondary)",   bg: "transparent",                border: "var(--border)" },
  Review:            { color: "#9B59B6",                 bg: "rgba(155,89,182,0.08)",      border: "#9B59B6" },
};

function nodeLabel(n: DagNode): string {
  return String(n.data.title ?? n.data.name ?? n.id.slice(0, 8));
}

interface ProvenanceViewProps {
  nodeId: string;
}

export function ProvenanceView({ nodeId }: ProvenanceViewProps) {
  const [depth, setDepth] = useState(2);
  const [snap, setSnap] = useState<DagSnapshot | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const setSelectedNodeId = useSetAtom(selectedNodeIdAtom);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    invoke<DagSnapshot>("get_provenance", { nodeId, depth })
      .then((s) => { if (!cancelled) setSnap(s); })
      .catch((e) => { if (!cancelled) setError(String(e)); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [nodeId, depth]);

  // Build edge lookup for rendering connections
  const edgesByFrom = new Map<string, typeof snap extends null ? never : DagSnapshot["edges"][number][]>();
  const edgesByTo = new Map<string, typeof snap extends null ? never : DagSnapshot["edges"][number][]>();
  if (snap) {
    for (const e of snap.edges) {
      if (!edgesByFrom.has(e.fromId)) edgesByFrom.set(e.fromId, []);
      if (!edgesByTo.has(e.toId)) edgesByTo.set(e.toId, []);
      edgesByFrom.get(e.fromId)!.push(e);
      edgesByTo.get(e.toId)!.push(e);
    }
  }

  const nodeMap = new Map(snap?.nodes.map((n) => [n.id, n]) ?? []);
  const seed = nodeMap.get(nodeId);

  return (
    <div className="flex flex-col h-full" style={{ background: "var(--inspector-bg)" }}>
      {/* Controls */}
      <div className="mac-toolbar flex items-center shrink-0" style={{ padding: "0 12px", height: 36, gap: 10 }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: "var(--text-primary)", flex: 1 }}>Provenance</span>
        <span className="mono" style={{ fontSize: 10, color: "var(--text-tertiary)" }}>depth</span>
        {[1, 2, 3, 4, 5].map((d) => (
          <button
            key={d}
            onClick={() => setDepth(d)}
            className="mac-btn mac-btn-ghost"
            style={{
              padding: "2px 0", width: 22, height: 22, justifyContent: "center",
              fontSize: 11,
              ...(depth === d ? { background: "var(--accent)", color: "var(--accent-text)", borderRadius: 5 } : {}),
            }}
          >
            {d}
          </button>
        ))}
      </div>

      <div className="flex-1 overflow-auto" style={{ padding: "12px 14px" }}>
        {loading && (
          <p className="mono" style={{ fontSize: 11, color: "var(--text-tertiary)" }}>Traversing graph…</p>
        )}
        {error && (
          <p className="mono" style={{
            fontSize: 11, color: "var(--status-failed)", background: "var(--status-failed-bg)",
            padding: "6px 10px", borderRadius: 5,
          }}>{error}</p>
        )}

        {snap && !loading && (
          <>
            <p className="mono" style={{ fontSize: 10, color: "var(--text-tertiary)", marginBottom: 10 }}>
              {snap.nodes.length} nodes · {snap.edges.length} edges · {depth} hop{depth !== 1 ? "s" : ""}
            </p>

            {/* Seed node */}
            {seed && (() => {
              const meta = NODE_COLORS[seed.nodeType] ?? { color: "var(--text-primary)", bg: "var(--card-bg)", border: "var(--border-strong)" };
              return (
                <div style={{ marginBottom: 12 }}>
                  <p className="mac-section-header" style={{ padding: "0 0 4px 0", margin: 0 }}>Seed</p>
                  <div style={{
                    border: `1px solid ${meta.border}`, borderRadius: 6, padding: "8px 12px",
                    background: meta.bg,
                  }}>
                    <p style={{ fontSize: 12, fontWeight: 600, color: meta.color, margin: 0 }}>
                      {nodeLabel(seed)}
                    </p>
                    <p className="mono" style={{ fontSize: 10, color: "var(--text-tertiary)", margin: "2px 0 0" }}>
                      {seed.nodeType}
                    </p>
                  </div>
                </div>
              );
            })()}

            {/* All nodes in subgraph */}
            <div style={{ display: "flex", flexDirection: "column", gap: 5 }}>
              {snap.nodes
                .filter((n) => n.id !== nodeId)
                .map((n) => {
                  const inEdges = edgesByTo.get(n.id) ?? [];
                  const outEdges = edgesByFrom.get(n.id) ?? [];
                  const meta = NODE_COLORS[n.nodeType] ?? { color: "var(--text-secondary)", bg: "transparent", border: "var(--border)" };

                  return (
                    <button
                      key={n.id}
                      onClick={() => setSelectedNodeId(n.id)}
                      style={{
                        width: "100%", border: `1px solid ${meta.border}`, borderRadius: 6,
                        padding: "7px 10px", textAlign: "left", cursor: "pointer",
                        background: meta.bg, display: "flex", alignItems: "flex-start",
                        justifyContent: "space-between", gap: 8,
                      }}
                      onMouseEnter={(e) => (e.currentTarget.style.opacity = "0.75")}
                      onMouseLeave={(e) => (e.currentTarget.style.opacity = "1")}
                    >
                      <div style={{ minWidth: 0, flex: 1 }}>
                        <p style={{ fontSize: 11, fontWeight: 500, color: meta.color, margin: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                          {nodeLabel(n)}
                        </p>
                        <p className="mono" style={{ fontSize: 10, color: "var(--text-tertiary)", margin: "2px 0 0" }}>
                          {n.nodeType}
                        </p>
                      </div>
                      <div className="mono" style={{ display: "flex", flexDirection: "column", gap: 2, flexShrink: 0, textAlign: "right" }}>
                        {inEdges.map((e) => {
                          const src = nodeMap.get(e.fromId);
                          return (
                            <span key={e.id} style={{ fontSize: 10, color: "var(--text-tertiary)" }}>
                              ← {e.edgeType} {src ? nodeLabel(src).slice(0, 10) : ""}
                            </span>
                          );
                        })}
                        {outEdges.map((e) => {
                          const tgt = nodeMap.get(e.toId);
                          return (
                            <span key={e.id} style={{ fontSize: 10, color: "var(--text-tertiary)" }}>
                              → {e.edgeType} {tgt ? nodeLabel(tgt).slice(0, 10) : ""}
                            </span>
                          );
                        })}
                      </div>
                    </button>
                  );
                })}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
