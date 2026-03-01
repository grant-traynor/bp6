import type { BeadNode } from "../../api";
import { WBSTreeList } from "./WBSTreeItem";
import { ResizeHandle } from "../shared/ResizeHandle";
import { WBSSkeleton } from "../shared/Skeleton";

interface WBSPanelProps {
  // Tree data
  nodes: BeadNode[];
  selectedBeadId: string | undefined;
  onToggle: (id: string) => void;
  onBeadClick: (bead: BeadNode) => void;

  // Sessions
  onSessionClick: (sessionId: string) => void;

  // Panel sizing
  panelWidth: number;
  minPanelWidth: number;
  maxPanelWidth: number;
  onPanelResizeStart: (e: React.MouseEvent) => void;

  // Scroll sync
  scrollRef: React.RefObject<HTMLDivElement | null>;
  onScroll: (e: React.UIEvent<HTMLDivElement>) => void;
  onMouseEnter: (e: React.MouseEvent<HTMLDivElement>) => void;

  // Loading state
  loading: boolean;
  processingData: boolean;
}

export function WBSPanel({
  nodes,
  selectedBeadId,
  onToggle,
  onBeadClick,
  onSessionClick,
  panelWidth,
  minPanelWidth,
  maxPanelWidth,
  onPanelResizeStart,
  scrollRef,
  onScroll,
  onMouseEnter,
  loading,
  processingData,
}: WBSPanelProps) {
  return (
    <div
      className="border-r-2 border-[var(--border-primary)] flex flex-col bg-[var(--background-secondary)] relative"
      style={{ width: `${panelWidth}px`, minWidth: `${minPanelWidth}px`, maxWidth: `${maxPanelWidth}px` }}
    >
      {/* Scrolling tree — scroll-synced with Gantt */}
      <div
        ref={scrollRef}
        onScroll={onScroll}
        onMouseEnter={onMouseEnter}
        className="flex-1 overflow-y-auto custom-scrollbar"
      >
        {(loading || processingData) ? (
          <WBSSkeleton />
        ) : (
          <WBSTreeList
            nodes={nodes}
            onToggle={onToggle}
            onClick={onBeadClick}
            selectedId={selectedBeadId}
            sessionsByBead={{}}
            onSessionClick={onSessionClick}
          />
        )}
      </div>

      <ResizeHandle onMouseDown={onPanelResizeStart} />
    </div>
  );
}
