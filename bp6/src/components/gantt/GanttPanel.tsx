import { useMemo } from "react";
import type { BeadNode, GanttItem } from "../../api";
import type { Connector } from "./GanttConnectors";
import { GanttGrid, ROW_HEIGHT } from "./GanttGrid";
import { GanttConnectors } from "./GanttConnectors";
import { GanttBars } from "./GanttBars";
import { GanttSkeleton } from "../shared/Skeleton";

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

  // Compute the transitive dependency chain from the selected bead.
  // Walks backwards (predecessors) and forwards (successors) through connectors.
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
