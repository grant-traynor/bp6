import { useRef } from "react";
import { ChevronsDown, ChevronsUp } from "lucide-react";
import type { BeadNode, Project } from "../../api";
import type { ClosedTimeFilter } from "./WBSFilters";
import { WBSFilters } from "./WBSFilters";
import { FavoritesSection } from "./FavoritesSection";
import { WBSTreeList } from "./WBSTreeItem";
import { ResizeHandle } from "../shared/ResizeHandle";
import { WBSSkeleton } from "../shared/Skeleton";

interface WBSPanelProps {
  // Tree data
  nodes: BeadNode[];
  selectedBeadId: string | undefined;
  onToggle: (id: string) => void;
  onBeadClick: (bead: BeadNode) => void;

  // Filter state
  filterText: string;
  onFilterTextChange: (value: string) => void;
  hideClosed: boolean;
  onHideClosedChange: (value: boolean) => void;
  closedTimeFilter: ClosedTimeFilter;
  onClosedTimeFilterChange: (value: ClosedTimeFilter) => void;
  includeHierarchy: boolean;
  onIncludeHierarchyChange: (value: boolean) => void;

  // Expand/collapse
  onExpandAll: () => void;
  onCollapseAll: () => void;

  // Favorites
  favoriteBeads: BeadNode[];
  favoriteProjects: Project[];
  onOpenChat: (persona: string, task?: string, beadId?: string, role?: string) => void;
  onOpenProject: (path: string) => void;

  // Sessions
  sessionsByBead: Record<string, string[]>;
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
  filterText,
  onFilterTextChange,
  hideClosed,
  onHideClosedChange,
  closedTimeFilter,
  onClosedTimeFilterChange,
  includeHierarchy,
  onIncludeHierarchyChange,
  onExpandAll,
  onCollapseAll,
  favoriteBeads,
  favoriteProjects,
  onOpenChat,
  onOpenProject,
  sessionsByBead,
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
  const searchInputRef = useRef<HTMLInputElement>(null);

  return (
    <div
      ref={scrollRef}
      onScroll={onScroll}
      onMouseEnter={onMouseEnter}
      className="border-r-2 border-[var(--border-primary)] flex flex-col bg-[var(--background-secondary)] overflow-y-auto custom-scrollbar relative"
      style={{ width: `${panelWidth}px`, minWidth: `${minPanelWidth}px`, maxWidth: `${maxPanelWidth}px` }}
    >
      {/* Panel title + expand/collapse controls */}
      <div className="px-6 py-4 border-b-2 border-[var(--border-primary)]/50 flex items-center justify-between shrink-0">
        <h2 className="text-sm font-black text-[var(--text-primary)] uppercase tracking-[0.25em] flex items-center gap-2">
          Task Breakdown
        </h2>
        <div className="flex items-center gap-1">
          <button
            onClick={onExpandAll}
            title="Expand All"
            className="p-1.5 hover:bg-[var(--background-tertiary)] rounded-md text-[var(--text-muted)] hover:text-indigo-500 transition-colors"
          >
            <ChevronsDown size={16} strokeWidth={2.5} />
          </button>
          <button
            onClick={onCollapseAll}
            title="Collapse All"
            className="p-1.5 hover:bg-[var(--background-tertiary)] rounded-md text-[var(--text-muted)] hover:text-indigo-500 transition-colors"
          >
            <ChevronsUp size={16} strokeWidth={2.5} />
          </button>
        </div>
      </div>

      {/* Favorites */}
      <FavoritesSection
        favoriteBeads={favoriteBeads}
        favoriteProjects={favoriteProjects}
        onBeadClick={onBeadClick}
        onOpenChat={onOpenChat}
        onOpenProject={onOpenProject}
        sessionsByBead={sessionsByBead}
      />

      {/* Filters */}
      <WBSFilters
        filterText={filterText}
        onFilterTextChange={onFilterTextChange}
        hideClosed={hideClosed}
        onHideClosedChange={onHideClosedChange}
        closedTimeFilter={closedTimeFilter}
        onClosedTimeFilterChange={onClosedTimeFilterChange}
        includeHierarchy={includeHierarchy}
        onIncludeHierarchyChange={onIncludeHierarchyChange}
        searchInputRef={searchInputRef}
      />

      {/* Tree list */}
      <div className="p-0">
        {(loading || processingData) ? (
          <WBSSkeleton />
        ) : (
          <div className="flex flex-col">
            <WBSTreeList
              nodes={nodes}
              onToggle={onToggle}
              onClick={onBeadClick}
              selectedId={selectedBeadId}
              sessionsByBead={{}}
              onSessionClick={onSessionClick}
            />
          </div>
        )}
      </div>

      <ResizeHandle onMouseDown={onPanelResizeStart} />
    </div>
  );
}
