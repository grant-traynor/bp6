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

const TYPE_LABEL_COLORS: Record<string, { color: string; bg: string }> = {
  Project:           { color: "var(--text-primary)",   bg: "var(--card-bg)" },
  Epic:              { color: "var(--text-primary)",   bg: "var(--content-secondary-bg)" },
  Feature:           { color: "var(--text-secondary)", bg: "transparent" },
  Task:              { color: "var(--text-secondary)", bg: "transparent" },
  Decision:          { color: "var(--status-paused)",  bg: "var(--status-paused-bg)" },
  AgentOutput:       { color: "var(--status-running)", bg: "var(--status-running-bg)" },
  KnowledgeArtifact: { color: "var(--status-completed)", bg: "var(--status-completed-bg)" },
  Review:            { color: "#9B59B6",               bg: "rgba(155,89,182,0.08)" },
};

function StatusBadge({ node }: { node: DagNode }) {
  const status = String(node.data.status ?? "");
  if (!status) return <span style={{ color: "var(--text-placeholder)", fontSize: 11 }}>—</span>;
  return (
    <span className={`status-badge status-${status}`} style={{ fontSize: 10 }}>
      {status}
    </span>
  );
}

function indentForType(nodeType: string): number {
  const idx = NODE_ORDER.indexOf(nodeType as NodeType);
  return Math.max(0, idx) * 10;
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
        <div style={{
          border: "1px dashed var(--border-strong)", borderRadius: 10,
          padding: "48px 32px", textAlign: "center",
        }}>
          <p style={{ fontSize: 13, color: "var(--text-tertiary)", margin: 0 }}>No DAG nodes yet.</p>
          <p style={{ fontSize: 11, color: "var(--text-placeholder)", marginTop: 4 }}>Open a project to load the DAG.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-auto">
      {/* Header */}
      <div className="mac-toolbar sticky top-0 z-10" style={{ display: "grid", gridTemplateColumns: "1fr 90px 100px" }}>
        <span className="mono" style={{ padding: "8px 16px", fontSize: 10, fontWeight: 600, color: "var(--text-tertiary)", textTransform: "uppercase", letterSpacing: "0.06em", borderRight: "1px solid var(--divider)" }}>
          Node
        </span>
        <span className="mono" style={{ padding: "8px 10px", fontSize: 10, fontWeight: 600, color: "var(--text-tertiary)", textTransform: "uppercase", letterSpacing: "0.06em", textAlign: "center", borderRight: "1px solid var(--divider)" }}>
          Type
        </span>
        <span className="mono" style={{ padding: "8px 10px", fontSize: 10, fontWeight: 600, color: "var(--text-tertiary)", textTransform: "uppercase", letterSpacing: "0.06em", textAlign: "center" }}>
          Status
        </span>
      </div>

      {/* Rows */}
      <div>
        {sorted.map((node) => {
          const indent = indentForType(node.nodeType);
          const typeMeta = TYPE_LABEL_COLORS[node.nodeType] ?? { color: "var(--text-secondary)", bg: "transparent" };
          const title = String(node.data.title ?? node.data.name ?? node.id.slice(0, 8));
          const depCount = edges.filter((e) => e.toId === node.id).length;

          return (
            <button
              key={node.id}
              onClick={() => setSelectedNodeId(node.id)}
              style={{
                width: "100%", display: "grid", gridTemplateColumns: "1fr 90px 100px",
                borderBottom: "1px solid var(--divider)", background: "transparent",
                cursor: "pointer", textAlign: "left", border: "none", borderBottomColor: "var(--divider)",
                borderBottomWidth: 1, borderBottomStyle: "solid",
              }}
              onMouseEnter={(e) => (e.currentTarget.style.background = "var(--sidebar-hover-bg)")}
              onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
            >
              <div style={{ padding: "7px 16px", borderRight: "1px solid var(--divider)", display: "flex", alignItems: "center", gap: 6, minWidth: 0 }}>
                <span style={{ paddingLeft: indent, flexShrink: 0 }} />
                <span style={{ fontSize: 12, fontWeight: node.nodeType === "Project" || node.nodeType === "Epic" ? 600 : 400, color: "var(--text-primary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {title}
                </span>
                {depCount > 0 && (
                  <span className="mono" style={{ fontSize: 10, color: "var(--text-placeholder)", flexShrink: 0 }}>
                    ←{depCount}
                  </span>
                )}
              </div>
              <div style={{ padding: "7px 10px", borderRight: "1px solid var(--divider)", display: "flex", alignItems: "center", justifyContent: "center" }}>
                <span className="mono" style={{
                  fontSize: 10, padding: "2px 6px", borderRadius: 4,
                  color: typeMeta.color, background: typeMeta.bg,
                }}>
                  {node.nodeType}
                </span>
              </div>
              <div style={{ padding: "7px 10px", display: "flex", alignItems: "center", justifyContent: "center" }}>
                <StatusBadge node={node} />
              </div>
            </button>
          );
        })}
      </div>

      {/* Footer */}
      <div style={{
        padding: "6px 16px", display: "flex", gap: 16,
        borderTop: "1px solid var(--divider)",
      }}>
        <span className="mono" style={{ fontSize: 11, color: "var(--text-tertiary)" }}>{nodes.length} nodes</span>
        <span className="mono" style={{ fontSize: 11, color: "var(--text-tertiary)" }}>{edges.length} edges</span>
        <span className="mono" style={{ fontSize: 11, color: "var(--text-placeholder)" }}>click row to inspect</span>
      </div>
    </div>
  );
}
