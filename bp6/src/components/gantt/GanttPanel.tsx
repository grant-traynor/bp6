import { useRef } from "react";
import type { BeadNode, GanttItem, BucketDistribution } from "../../api";
import type { Connector } from "./GanttConnectors";
import { GanttStateHeader } from "./GanttStateHeader";
import { GanttGrid, ROW_HEIGHT } from "./GanttGrid";
import { GanttConnectors } from "./GanttConnectors";
import { GanttBars } from "./GanttBars";
import { GanttSkeleton } from "../shared/Skeleton";

interface GanttPanelProps {
  items: GanttItem[];
  rowCount: number;
  rowDepths: number[];
  connectors: Connector[];
  distributions: BucketDistribution[];
  zoom: number;
  onZoomChange: (zoom: number) => void;
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
  distributions,
  zoom,
  selectedBead,
  onBeadClick,
  onOpenChat,
  sessionsByBead,
  scrollRef,
  onScroll,
  onMouseEnter,
  loading,
}: GanttPanelProps) {
  const headerScrollRef = useRef<HTMLDivElement>(null);

  const totalWidth = 5000 * zoom;
  const totalHeight = Math.max(600, rowCount * ROW_HEIGHT);
  const stateHeaderWidth = Math.max(totalWidth, (distributions.length * 100 * zoom));

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* State header row */}
      <div className="shrink-0 flex-1 overflow-hidden bg-[var(--background-tertiary)]">
        <div
          ref={headerScrollRef}
          className="overflow-x-auto overflow-y-hidden no-scrollbar"
          onScroll={(e) => {
            // Sync horizontal scroll with gantt body
            if (scrollRef.current) {
              scrollRef.current.scrollLeft = e.currentTarget.scrollLeft;
            }
          }}
        >
          <div style={{ width: stateHeaderWidth }}>
            <GanttStateHeader distributions={distributions} zoom={zoom} />
          </div>
        </div>
      </div>

      {/* Gantt body */}
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
          />
          <GanttBars
            items={items}
            selectedBead={selectedBead}
            onBeadClick={onBeadClick}
            onOpenChat={onOpenChat}
            sessionsByBead={sessionsByBead}
          />
        </div>
      </div>
    </div>
  );
}
