/**
 * bp6-80q.1: Live DAG / Gantt view
 *
 * WBS-style task list backed by the SQLite DAG. Status overlaid on each row.
 * Reacts to dag:node:upserted events via Jotai atoms. Click a row to probe.
 */
import { useAtomValue, useSetAtom } from "jotai";
import { nodesListAtom, edgesListAtom, selectedNodeIdAtom } from "../store/dag";
import type { DagNode, NodeType } from "../types";

const NODE_ORDER: NodeType[] = [
  "Project",
  "Epic",
  "Feature",
  "Task",
  "Decision",
  "AgentOutput",
  "KnowledgeArtifact",
  "Review",
];

const TYPE_COLORS: Record<string, string> = {
  Project: "bg-black text-white",
  Epic: "bg-stone-800 text-white",
  Feature: "bg-stone-600 text-white",
  Task: "bg-stone-400 text-black",
  Decision: "bg-amber-400 text-black",
  AgentOutput: "bg-blue-500 text-white",
  KnowledgeArtifact: "bg-green-600 text-white",
  Review: "bg-purple-500 text-white",
};

const STATUS_BADGE: Record<string, string> = {
  running: "border-2 border-blue-500 text-blue-600 bg-blue-50 animate-pulse",
  paused: "border-2 border-amber-400 text-amber-700 bg-amber-50",
  blocked: "border-2 border-red-400 text-red-700 bg-red-50",
  completed: "border-2 border-green-500 text-green-700 bg-green-50",
  cancelled: "border-2 border-stone-400 text-stone-500 bg-stone-100 line-through",
  failed: "border-2 border-red-600 text-red-700 bg-red-100",
};

function statusBadge(node: DagNode) {
  const status = String(node.data.status ?? "");
  if (!status) return null;
  const cls = STATUS_BADGE[status] ?? "border-2 border-stone-300 text-stone-500";
  return (
    <span className={`font-mono text-xs px-2 py-0.5 ${cls}`}>{status}</span>
  );
}

function indentForType(nodeType: string): number {
  const idx = NODE_ORDER.indexOf(nodeType as NodeType);
  return Math.max(0, idx) * 12;
}

export function GanttView() {
  const nodes = useAtomValue(nodesListAtom);
  const edges = useAtomValue(edgesListAtom);
  const setSelectedNodeId = useSetAtom(selectedNodeIdAtom);

  const sorted = [...nodes].sort((a, b) => {
    const ai = NODE_ORDER.indexOf(a.nodeType as NodeType);
    const bi = NODE_ORDER.indexOf(b.nodeType as NodeType);
    if (ai !== bi) return ai - bi;
    return a.createdAt.localeCompare(b.createdAt);
  });

  if (nodes.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="border-4 border-dashed border-stone-300 p-16 text-center">
          <p className="font-mono text-stone-400 text-sm">No DAG nodes yet.</p>
          <p className="font-mono text-stone-300 text-xs mt-1">Open a project to load the DAG.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-auto">
      {/* Header */}
      <div className="sticky top-0 bg-white border-b-4 border-black grid grid-cols-[1fr_auto_auto] gap-0 z-10">
        <span className="font-mono text-xs font-bold px-4 py-2 border-r-2 border-stone-200">NODE</span>
        <span className="font-mono text-xs font-bold px-4 py-2 border-r-2 border-stone-200 w-28 text-center">TYPE</span>
        <span className="font-mono text-xs font-bold px-4 py-2 w-28 text-center">STATUS</span>
      </div>

      {/* Rows */}
      <div>
        {sorted.map((node) => {
          const indent = indentForType(node.nodeType);
          const typeColor = TYPE_COLORS[node.nodeType] ?? "bg-stone-200 text-black";
          const title = String(node.data.title ?? node.data.name ?? node.id.slice(0, 8));
          const depCount = edges.filter((e) => e.toId === node.id).length;

          return (
            <button
              key={node.id}
              onClick={() => setSelectedNodeId(node.id)}
              className="w-full grid grid-cols-[1fr_auto_auto] gap-0 border-b-2 border-stone-200 hover:bg-stone-50 text-left transition-colors"
            >
              <div className="px-4 py-2 border-r-2 border-stone-200 flex items-center gap-2 min-w-0">
                <span style={{ paddingLeft: indent }} className="shrink-0" />
                <span className="font-medium text-sm truncate">{title}</span>
                {depCount > 0 && (
                  <span className="font-mono text-xs text-stone-400 shrink-0">
                    ←{depCount}
                  </span>
                )}
              </div>
              <div className="px-2 py-2 border-r-2 border-stone-200 flex items-center justify-center w-28">
                <span className={`font-mono text-xs px-1.5 py-0.5 ${typeColor}`}>
                  {node.nodeType}
                </span>
              </div>
              <div className="px-2 py-2 flex items-center justify-center w-28">
                {statusBadge(node) ?? (
                  <span className="font-mono text-xs text-stone-300">—</span>
                )}
              </div>
            </button>
          );
        })}
      </div>

      {/* Stats bar */}
      <div className="border-t-4 border-black px-4 py-2 flex gap-4 bg-stone-50">
        <span className="font-mono text-xs text-stone-500">{nodes.length} nodes</span>
        <span className="font-mono text-xs text-stone-500">{edges.length} edges</span>
        <span className="font-mono text-xs text-stone-400">click row to probe</span>
      </div>
    </div>
  );
}
