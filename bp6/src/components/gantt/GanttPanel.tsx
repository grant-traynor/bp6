import { useMemo } from "react";
import type { BeadNode, GanttItem } from "../../api";
import type { Connector } from "./GanttConnectors";
import { GanttGrid, ROW_HEIGHT } from "./GanttGrid";
import { GanttConnectors } from "./GanttConnectors";
import { GanttBars } from "./GanttBars";
import { GanttSkeleton } from "../shared/Skeleton";

// Distinct palette for dependency threads. Each root node (no predecessors
// in the connector graph) gets a color; that color propagates along every
// edge reachable from that root so you can follow each strand visually.
const THREAD_COLORS = [
  '#f97316', // orange
  '#3b82f6', // blue
  '#22c55e', // green
  '#a855f7', // purple
  '#14b8a6', // teal
  '#ef4444', // red
  '#f59e0b', // amber
  '#06b6d4', // cyan
  '#ec4899', // pink
  '#84cc16', // lime
];

// For each connector edge (fromId → toId), return the list of thread colors
// that flow through it. A "thread" originates at every source node (no
// predecessors) and propagates via DFS to all reachable successors.
function computeEdgeThreads(connectors: Connector[]): Map<string, string[]> {
  if (connectors.length === 0) return new Map();

  const succs = new Map<string, string[]>();
  const hasPred = new Set<string>();
  const allNodes = new Set<string>();

  for (const conn of connectors) {
    allNodes.add(conn.fromId);
    allNodes.add(conn.toId);
    if (!succs.has(conn.fromId)) succs.set(conn.fromId, []);
    succs.get(conn.fromId)!.push(conn.toId);
    hasPred.add(conn.toId);
  }

  // Source nodes: appear in the connector graph but have no predecessors.
  const sources = [...allNodes].filter(id => !hasPred.has(id));

  const edgeThreads = new Map<string, string[]>();

  sources.forEach((src, i) => {
    const color = THREAD_COLORS[i % THREAD_COLORS.length];
    const stack = [src];
    const visited = new Set<string>();

    while (stack.length > 0) {
      const node = stack.pop()!;
      if (visited.has(node)) continue;
      visited.add(node);
      for (const succ of succs.get(node) ?? []) {
        const key = `${node}-${succ}`;
        if (!edgeThreads.has(key)) edgeThreads.set(key, []);
        const colors = edgeThreads.get(key)!;
        if (!colors.includes(color)) colors.push(color);
        stack.push(succ);
      }
    }
  });

  return edgeThreads;
}

interface GanttPanelProps {
  items: GanttItem[];
  rowCount: number;
  rowDepths: number[];
  connectors: Connector[];
  zoom: number;
  totalWidth: number;
  selectedBead: BeadNode | null;
  onBeadClick: (bead: BeadNode) => void;
  onOpenChat: (persona: string, task?: string, beadId?: string, role?: string) => void;
  sessionsByBead: Record<string, string[]>;
  scrollRef: React.RefObject<HTMLDivElement | null>;
  onScroll: (e: React.UIEvent<HTMLDivElement>) => void;
  onMouseEnter: (e: React.MouseEvent<HTMLDivElement>) => void;
  loading: boolean;
}

export function GanttPanel({
  items,
  rowCount,
  rowDepths,
  connectors,
  zoom,
  totalWidth,
  selectedBead,
  onBeadClick,
  onOpenChat,
  sessionsByBead,
  scrollRef,
  onScroll,
  onMouseEnter,
  loading,
}: GanttPanelProps) {
  const totalHeight = Math.max(600, rowCount * ROW_HEIGHT);

  // Thread colors: one color per source node in the connector graph, propagated
  // along every reachable edge. Recompute only when connectors change.
  const edgeThreads = useMemo(() => computeEdgeThreads(connectors), [connectors]);

  // Transitive dependency chain from the selected bead (predecessors + successors).
  // Used to dim bars and connectors that aren't part of the selected chain.
  const chainIds = useMemo(() => {
    if (!selectedBead) return new Set<string>();

    const predecessorsOf = new Map<string, string[]>();
    const successorsOf = new Map<string, string[]>();
    for (const conn of connectors) {
      if (!predecessorsOf.has(conn.toId)) predecessorsOf.set(conn.toId, []);
      predecessorsOf.get(conn.toId)!.push(conn.fromId);
      if (!successorsOf.has(conn.fromId)) successorsOf.set(conn.fromId, []);
      successorsOf.get(conn.fromId)!.push(conn.toId);
    }

    const chain = new Set<string>();
    chain.add(selectedBead.id);
    const walkBack = (id: string) => {
      for (const p of predecessorsOf.get(id) ?? []) {
        if (!chain.has(p)) { chain.add(p); walkBack(p); }
      }
    };
    const walkForward = (id: string) => {
      for (const s of successorsOf.get(id) ?? []) {
        if (!chain.has(s)) { chain.add(s); walkForward(s); }
      }
    };
    walkBack(selectedBead.id);
    walkForward(selectedBead.id);
    return chain;
  }, [selectedBead, connectors]);

  return (
    <div
      ref={scrollRef}
      onScroll={onScroll}
      onMouseEnter={onMouseEnter}
      className="flex-1 relative bg-[var(--background-primary)] overflow-auto custom-scrollbar"
    >
      <div className="relative" style={{ height: totalHeight, width: totalWidth }}>
        {loading && <GanttSkeleton />}
        <GanttGrid zoom={zoom} rowDepths={rowDepths} />
        <GanttConnectors
          connectors={connectors}
          zoom={zoom}
          totalWidth={totalWidth}
          totalHeight={totalHeight}
          chainIds={chainIds}
          edgeThreads={edgeThreads}
        />
        <GanttBars
          items={items}
          selectedBead={selectedBead}
          onBeadClick={onBeadClick}
          onOpenChat={onOpenChat}
          sessionsByBead={sessionsByBead}
          chainIds={chainIds}
        />
      </div>
    </div>
  );
}
