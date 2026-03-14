import { useState } from 'react';
import type { Node, Phase, Edge } from '../types';

interface Props {
  phases: Phase[];
  nodes: Node[];
  edges: Edge[];
  onTaskClick: (node: Node) => void;
}

function statusBadge(node: Node): { label: string; cls: string } {
  switch (node.status) {
    case 'running':
      return { label: '● running', cls: 'text-emerald-400 bg-emerald-950/40 border-emerald-900' };
    case 'waiting':
      return {
        label: `⏸ ${node.skillId ?? 'waiting'}`,
        cls: 'text-amber-400 bg-amber-950/40 border-amber-900',
      };
    case 'complete':
      return { label: '✓ done', cls: 'text-neutral-600 bg-neutral-900/40 border-neutral-800' };
    case 'cancelled':
      return { label: '— cancelled', cls: 'text-neutral-700 bg-neutral-900/30 border-neutral-800 line-through' };
    case 'blocked':
      return { label: '⊘ blocked', cls: 'text-red-500 bg-red-950/30 border-red-900' };
    default:
      return { label: '░ pending', cls: 'text-neutral-600 bg-neutral-900/20 border-neutral-800' };
  }
}

function phaseStageBadge(status: string): { label: string; cls: string } {
  switch (status) {
    case 'running':
      return { label: 'Run', cls: 'text-emerald-500 border-emerald-900' };
    case 'gate':
      return { label: 'Gate', cls: 'text-amber-500 border-amber-900' };
    case 'paused':
      return { label: 'Paused', cls: 'text-orange-500 border-orange-900' };
    case 'complete':
      return { label: 'Done', cls: 'text-neutral-600 border-neutral-800' };
    default:
      return { label: 'Plan', cls: 'text-sky-500 border-sky-900' };
  }
}

interface RowData {
  node: Node;
  depth: number;
}

function flattenNodes(nodes: Node[]): RowData[] {
  const childrenMap = new Map<string | null, Node[]>();
  for (const n of nodes) {
    const key = n.parentId ?? null;
    if (!childrenMap.has(key)) childrenMap.set(key, []);
    childrenMap.get(key)!.push(n);
  }

  const typeOrder: Record<string, number> = { epic: 0, feature: 1, task: 2, bug: 2, chore: 2, subtask: 3 };

  function buildFlat(parentId: string | null, depth: number): RowData[] {
    const kids = (childrenMap.get(parentId) ?? []).slice();
    kids.sort((a, b) => {
      const ao = typeOrder[a.nodeType] ?? 9;
      const bo = typeOrder[b.nodeType] ?? 9;
      if (ao !== bo) return ao - bo;
      if (a.sortOrder !== null && b.sortOrder !== null) return a.sortOrder - b.sortOrder;
      return a.title.localeCompare(b.title);
    });
    return kids.flatMap(n => [{ node: n, depth }, ...buildFlat(n.id, depth + 1)]);
  }

  return buildFlat(null, 0);
}

export default function PhaseMatrix({ phases, nodes, edges, onTaskClick }: Props) {
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());

  // Build a set of upstream IDs for each node (nodes that have edges pointing TO this node)
  const upstreamMap = new Map<string, string[]>();
  for (const edge of edges) {
    if (!upstreamMap.has(edge.toId)) upstreamMap.set(edge.toId, []);
    upstreamMap.get(edge.toId)!.push(edge.fromId);
  }

  const allRows = flattenNodes(nodes);

  // Determine which rows to hide (because their ancestor is collapsed)
  function isHidden(node: Node): boolean {
    let id = node.parentId;
    while (id) {
      if (collapsed.has(id)) return true;
      const parent = nodes.find(n => n.id === id);
      id = parent?.parentId ?? null;
    }
    return false;
  }

  const visibleRows = allRows.filter(r => !isHidden(r.node));

  function toggleCollapse(nodeId: string) {
    setCollapsed(prev => {
      const next = new Set(prev);
      if (next.has(nodeId)) next.delete(nodeId);
      else next.add(nodeId);
      return next;
    });
  }

  const phaseIds = phases.map(p => p.id);

  function nodeForCell(rowNode: Node, phaseId: string): Node | null {
    // Find the node itself if it belongs to this phase, or look for child nodes
    if (rowNode.phaseId === phaseId) return rowNode;
    return null;
  }

  if (phases.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-neutral-700 text-xs">
        No phases defined
      </div>
    );
  }

  return (
    <div className="overflow-auto h-full">
      <table className="min-w-full border-collapse text-[11px] font-mono">
        <thead className="sticky top-0 z-10 bg-neutral-950">
          <tr>
            {/* Scope column header */}
            <th className="text-left px-3 py-1.5 text-[10px] uppercase tracking-widest text-neutral-600 border-b border-r border-neutral-800 bg-neutral-950 min-w-[200px] sticky left-0 z-20">
              Scope
            </th>
            {phases.map(phase => {
              const { label: stageLabel, cls: stageCls } = phaseStageBadge(phase.status);
              return (
                <th
                  key={phase.id}
                  className={`px-2 py-1.5 border-b border-r border-neutral-800 text-center min-w-[140px] bg-neutral-950 ${
                    phase.status === 'gate' || phase.status === 'paused' ? 'bg-amber-950/20' : ''
                  }`}
                >
                  <div className="flex flex-col items-center gap-0.5">
                    <span className="text-neutral-300 font-semibold">{phase.title}</span>
                    <div className="flex items-center gap-1">
                      <span className={`text-[9px] px-1 py-0 border rounded-sm ${stageCls}`}>
                        {stageLabel}
                      </span>
                      {(phase.status === 'gate' || phase.status === 'paused') && (
                        <span className="text-[9px] text-amber-500" title="Gate held — review required">
                          🔒
                        </span>
                      )}
                    </div>
                  </div>
                </th>
              );
            })}
          </tr>
        </thead>
        <tbody>
          {visibleRows.length === 0 ? (
            <tr>
              <td
                colSpan={phaseIds.length + 1}
                className="text-center py-8 text-neutral-700 text-xs"
              >
                No tasks
              </td>
            </tr>
          ) : (
            visibleRows.map(({ node, depth }) => {
              const hasChildren = nodes.some(n => n.parentId === node.id);
              const isCollapsible = hasChildren && (node.nodeType === 'epic' || node.nodeType === 'feature');
              const isCollapsed = collapsed.has(node.id);
              const typeLabel =
                node.nodeType === 'epic' ? 'EPIC' :
                node.nodeType === 'feature' ? 'FEAT' :
                node.nodeType === 'bug' ? 'BUG' :
                node.nodeType === 'chore' ? 'CHR' : null;

              return (
                <tr key={node.id} className="group hover:bg-neutral-900/30">
                  {/* Scope label cell — sticky left */}
                  <td
                    className="px-1 py-0.5 border-b border-r border-neutral-800/60 bg-neutral-950 sticky left-0"
                    style={{ paddingLeft: `${4 + depth * 16}px` }}
                  >
                    <div className="flex items-center gap-1 min-w-0">
                      {isCollapsible && (
                        <button
                          className="shrink-0 text-neutral-600 hover:text-neutral-400 w-3 text-center"
                          onClick={() => toggleCollapse(node.id)}
                        >
                          {isCollapsed ? '▶' : '▼'}
                        </button>
                      )}
                      {!isCollapsible && <span className="shrink-0 w-3" />}
                      {typeLabel && (
                        <span className="shrink-0 text-[9px] font-bold text-neutral-600 tracking-wider">
                          {typeLabel}
                        </span>
                      )}
                      <span
                        className={`truncate ${
                          node.status === 'complete' || node.status === 'cancelled'
                            ? 'text-neutral-600'
                            : 'text-neutral-300'
                        }`}
                      >
                        {node.title}
                      </span>
                    </div>
                  </td>

                  {/* Phase cells */}
                  {phases.map(phase => {
                    const cellNode = nodeForCell(node, phase.id);
                    const upstreams = cellNode ? (upstreamMap.get(cellNode.id) ?? []) : [];
                    const hasUpstream = upstreams.length > 0;

                    return (
                      <td
                        key={phase.id}
                        className={`px-1.5 py-0.5 border-b border-r border-neutral-800/60 text-center ${
                          phase.status === 'gate' || phase.status === 'paused'
                            ? 'opacity-50'
                            : ''
                        }`}
                      >
                        {cellNode ? (
                          <button
                            className="w-full"
                            onClick={() => onTaskClick(cellNode)}
                            title={cellNode.title}
                          >
                            <div className="flex items-center justify-center gap-1">
                              {hasUpstream && (
                                <span className="text-neutral-600 text-[9px]" title={`Depends on ${upstreams.length} task(s)`}>
                                  ↳
                                </span>
                              )}
                              {(() => {
                                const { label, cls } = statusBadge(cellNode);
                                return (
                                  <span
                                    className={`inline-block px-1.5 py-0.5 border rounded text-[9px] font-mono ${cls}`}
                                  >
                                    {label}
                                  </span>
                                );
                              })()}
                            </div>
                          </button>
                        ) : (
                          <span className="text-neutral-800">·</span>
                        )}
                      </td>
                    );
                  })}
                </tr>
              );
            })
          )}
        </tbody>
      </table>
    </div>
  );
}
