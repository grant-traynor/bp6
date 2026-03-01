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
  );
}
