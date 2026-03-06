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

const NODE_COLORS: Record<string, string> = {
  Decision: "border-amber-400 bg-amber-50 text-amber-900",
  AgentOutput: "border-blue-400 bg-blue-50 text-blue-900",
  KnowledgeArtifact: "border-green-400 bg-green-50 text-green-900",
  Project: "border-black bg-black text-white",
  Epic: "border-stone-700 bg-stone-700 text-white",
  Feature: "border-stone-500 bg-stone-100 text-stone-800",
  Task: "border-stone-400 bg-white text-stone-800",
  Review: "border-purple-400 bg-purple-50 text-purple-900",
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
    <div className="flex flex-col h-full">
      {/* Controls */}
      <div className="border-b-4 border-black px-4 py-2 flex items-center gap-4 shrink-0 bg-white">
        <span className="font-mono text-xs font-bold">PROVENANCE</span>
        <div className="flex items-center gap-2 ml-auto">
          <span className="font-mono text-xs text-stone-500">depth</span>
          {[1, 2, 3, 4, 5].map((d) => (
            <button
              key={d}
              onClick={() => setDepth(d)}
              className={`w-6 h-6 font-mono text-xs border-2 border-black transition-colors ${
                depth === d ? "bg-black text-white" : "hover:bg-stone-100"
              }`}
            >
              {d}
            </button>
          ))}
        </div>
      </div>

      <div className="flex-1 overflow-auto p-4">
        {loading && (
          <p className="font-mono text-xs text-stone-400 animate-pulse">Traversing graph…</p>
        )}
        {error && (
          <p className="font-mono text-xs text-red-600 border-2 border-red-400 p-2">{error}</p>
        )}

        {snap && !loading && (
          <>
            <p className="font-mono text-xs text-stone-400 mb-3">
              {snap.nodes.length} nodes · {snap.edges.length} edges within {depth} hop{depth !== 1 ? "s" : ""}
            </p>

            {/* Seed node */}
            {seed && (
              <div className="mb-4">
                <p className="font-mono text-xs text-stone-400 mb-1">SEED</p>
                <div
                  className={`border-4 px-3 py-2 ${NODE_COLORS[seed.nodeType] ?? "border-stone-400 bg-white"}`}
                >
                  <p className="font-bold text-sm">{nodeLabel(seed)}</p>
                  <p className="font-mono text-xs opacity-60">{seed.nodeType}</p>
                </div>
              </div>
            )}

            {/* All nodes in subgraph */}
            <div className="space-y-1">
              {snap.nodes
                .filter((n) => n.id !== nodeId)
                .map((n) => {
                  const inEdges = edgesByTo.get(n.id) ?? [];
                  const outEdges = edgesByFrom.get(n.id) ?? [];
                  const cls = NODE_COLORS[n.nodeType] ?? "border-stone-300 bg-white text-stone-800";

                  return (
                    <button
                      key={n.id}
                      onClick={() => setSelectedNodeId(n.id)}
                      className={`w-full border-2 px-3 py-2 text-left hover:opacity-80 transition-opacity ${cls}`}
                    >
                      <div className="flex items-start justify-between gap-2">
                        <div className="min-w-0">
                          <p className="font-bold text-xs truncate">{nodeLabel(n)}</p>
                          <p className="font-mono text-xs opacity-60">{n.nodeType}</p>
                        </div>
                        <div className="flex flex-col gap-0.5 shrink-0 text-right">
                          {inEdges.map((e) => {
                            const src = nodeMap.get(e.fromId);
                            return (
                              <span key={e.id} className="font-mono text-xs opacity-70">
                                ← {e.edgeType} {src ? nodeLabel(src).slice(0, 12) : ""}
                              </span>
                            );
                          })}
                          {outEdges.map((e) => {
                            const tgt = nodeMap.get(e.toId);
                            return (
                              <span key={e.id} className="font-mono text-xs opacity-70">
                                → {e.edgeType} {tgt ? nodeLabel(tgt).slice(0, 12) : ""}
                              </span>
                            );
                          })}
                        </div>
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
