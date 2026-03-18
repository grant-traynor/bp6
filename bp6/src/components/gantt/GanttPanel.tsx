import { useMemo } from "react";
import type { BeadNode, GanttItem } from "../../api";
import type { Connector, ThreadInfo } from "./GanttConnectors";
import { GanttGrid, ROW_HEIGHT } from "./GanttGrid";
import { GanttConnectors } from "./GanttConnectors";
import { GanttBars } from "./GanttBars";
import { GanttSkeleton } from "../shared/Skeleton";

// One colour per independent source thread.
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

/** For each connector edge (fromId→toId), the list of source threads whose
 *  colour propagates through it.  Also returns the total thread count so
 *  GanttConnectors can centre the lane spread. */
function computeEdgeThreads(connectors: Connector[]): {
  edgeThreads: Map<string, ThreadInfo[]>;
  totalThreads: number;
} {
  if (connectors.length === 0) {
    return { edgeThreads: new Map(), totalThreads: 0 };
  }

  const succs  = new Map<string, string[]>();
  const hasPred = new Set<string>();
  const allNodes = new Set<string>();

  for (const conn of connectors) {
    allNodes.add(conn.fromId);
    allNodes.add(conn.toId);
    if (!succs.has(conn.fromId)) succs.set(conn.fromId, []);
    succs.get(conn.fromId)!.push(conn.toId);
    hasPred.add(conn.toId);
  }

  const sources = [...allNodes].filter(id => !hasPred.has(id));
  const edgeThreads = new Map<string, ThreadInfo[]>();

  sources.forEach((src, i) => {
    const info: ThreadInfo = { color: THREAD_COLORS[i % THREAD_COLORS.length], index: i };
    const stack   = [src];
    const visited = new Set<string>();

    while (stack.length > 0) {
      const node = stack.pop()!;
      if (visited.has(node)) continue;
      visited.add(node);
      for (const succ of succs.get(node) ?? []) {
        const key = `${node}-${succ}`;
        if (!edgeThreads.has(key)) edgeThreads.set(key, []);
        const list = edgeThreads.get(key)!;
        if (!list.some(t => t.index === i)) list.push(info);
        stack.push(succ);
      }
    }
  });

  return { edgeThreads, totalThreads: sources.length };
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

  const { edgeThreads, totalThreads } = useMemo(
    () => computeEdgeThreads(connectors),
    [connectors]
  );

  const chainIds = useMemo(() => {
    if (!selectedBead) return new Set<string>();

    const predecessorsOf = new Map<string, string[]>();
    const successorsOf   = new Map<string, string[]>();
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
          totalThreads={totalThreads}
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
