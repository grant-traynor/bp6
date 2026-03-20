import React, { useState, useRef, useCallback } from "react";
import { cn } from "../utils";
import { Info, Maximize2, Minimize2 } from "lucide-react";
import type { BeadNode, SessionInfo } from "../api";
import { createSessionWindow, terminateSession } from "../api";
import { Navigation } from "../components/layout/Navigation";
import { Header } from "../components/layout/Header";
import { BeadDetailModal } from "../components/shared/BeadDetailModal";
import { PalettePreviewDialog } from "../components/palette/PalettePreviewDialog";
import SettingsView from "../components/settings/SettingsView";
import { Terminal } from "../components/terminal/Terminal";
import { SessionsView } from "../components/sessions/SessionsView";
import { ListView } from "../components/list/ListView";
import { WBSPanel } from "../components/wbs/WBSPanel";
import { WBSHeader } from "../components/wbs/WBSHeader";
import { WBSFilters } from "../components/wbs/WBSFilters";
import { FavoritesSection } from "../components/wbs/FavoritesSection";
import { GanttPanel } from "../components/gantt/GanttPanel";
import { GanttStateHeader } from "../components/gantt/GanttStateHeader";
import { StatsBar } from "../components/layout/StatsBar";
import { ProjectSelectionView } from "./ProjectSelectionView";
import type { ViewType } from "../components/layout/Navigation";
import type { ClosedTimeFilter } from "../hooks/useAppInitialization";
import type { GanttItem } from "../hooks/useGanttLayout";
import type { Connector } from "../components/gantt/GanttConnectors";
import type { Project, ProjectViewModel, CliBackend, BucketDistribution } from "../api";

const MIN_PANEL_WIDTH = 200;
const MAX_PANEL_WIDTH = 1200;
const PROJECT_SHELL_ID = "project-shell";

export interface MainAppViewProps {
  // Project state
  currentProjectPath: string;
  hasProject: boolean;
  projects: Project[];
  favoriteProjects: Project[];
  recentProjects: Project[];
  projectMenuOpen: boolean;
  setProjectMenuOpen: React.Dispatch<React.SetStateAction<boolean>>;
  showPalettePreview: boolean;
  setShowPalettePreview: React.Dispatch<React.SetStateAction<boolean>>;
  handleOpenProject: (path: string) => Promise<void>;
  handleRemoveProject: (path: string) => Promise<void>;
  handleToggleFavoriteProject: (path: string) => Promise<void>;
  handleSelectProject: () => Promise<void>;

  // View state
  view: ViewType;
  setView: React.Dispatch<React.SetStateAction<ViewType>>;
  isDark: boolean;
  setIsDark: React.Dispatch<React.SetStateAction<boolean>>;
  panelWidth: number;
  handlePanelResizeStart: (e: React.MouseEvent) => void;

  // Filter state
  filterText: string;
  setFilterText: React.Dispatch<React.SetStateAction<string>>;
  hideClosed: boolean;
  setHideClosed: React.Dispatch<React.SetStateAction<boolean>>;
  closedTimeFilter: ClosedTimeFilter;
  setClosedTimeFilter: React.Dispatch<React.SetStateAction<ClosedTimeFilter>>;
  includeHierarchy: boolean;
  setIncludeHierarchy: React.Dispatch<React.SetStateAction<boolean>>;
  sortBy: "priority" | "title" | "type" | "id" | "none";
  sortOrder: "asc" | "desc" | "none";
  handleHeaderClick: (column: "priority" | "title" | "type" | "id") => void;
  loading: boolean;
  processingData: boolean;
  viewModel: ProjectViewModel | null;
  loadData: () => void;

  // Gantt / tree
  collapsedIds: Set<string>;
  zoom: number;
  setZoom: React.Dispatch<React.SetStateAction<number>>;
  toggleNode: (id: string, scrollContainerRef?: React.RefObject<HTMLDivElement | null>) => void;
  expandAll: () => void;
  collapseAll: (viewModelTree?: BeadNode[]) => void;
  beads: BeadNode[];
  ganttLayout: {
    items: GanttItem[];
    rowCount: number;
    rowDepths: number[];
    connectors: Connector[];
  };

  // Bead editing
  selectedBead: BeadNode | null;
  setSelectedBead: React.Dispatch<React.SetStateAction<BeadNode | null>>;
  isCreating: boolean;
  setIsCreating: React.Dispatch<React.SetStateAction<boolean>>;
  editForm: Partial<BeadNode>;
  setEditForm: React.Dispatch<React.SetStateAction<Partial<BeadNode>>>;
  beadDetailOpen: boolean;
  setBeadDetailOpen: React.Dispatch<React.SetStateAction<boolean>>;
  handleBeadClick: (bead: BeadNode) => void;
  handleStartCreate: () => void;
  handleSaveCreate: (viewModel: ProjectViewModel | null) => Promise<void>;
  handleSaveEdit: (viewModel: ProjectViewModel | null) => Promise<void>;
  handleCloseBead: (beadId: string) => Promise<void>;
  handleReopenBead: (beadId: string) => Promise<void>;
  handleClaimBead: (beadId: string) => Promise<void>;
  toggleFavorite: (bead: BeadNode) => Promise<void>;

  // Chat sessions
  currentCli: CliBackend;
  setCurrentCli: React.Dispatch<React.SetStateAction<CliBackend>>;
  launchInBackground: boolean;
  toggleLaunchInBackground: () => void;
  handleOpenChat: (persona: string, task?: string, beadId?: string, role?: string) => Promise<void>;
  sessionsByBead: Record<string, SessionInfo[]>;
  sessions: SessionInfo[];

  // Shell
  projectShellKey: number;

  // Stats
  stats: {
    total: number;
    open: number;
    inProgress: number;
    closed: number;
    blocked: number;
  };

  // Scroll sync
  wbsScrollRef: React.RefObject<HTMLDivElement | null>;
  ganttScrollRef: React.RefObject<HTMLDivElement | null>;
  ganttHeaderScrollRef: React.RefObject<HTMLDivElement | null>;
  handleWBSScroll: (e: React.UIEvent<HTMLDivElement>) => void;
  handleGanttScroll: (e: React.UIEvent<HTMLDivElement>) => void;
  handleGanttHeaderScroll: (e: React.UIEvent<HTMLDivElement>) => void;
  handleMouseEnter: (e: React.MouseEvent<HTMLDivElement>) => void;

  // Favorites
  favoriteBeads: BeadNode[];
}

export function MainAppView({
  currentProjectPath,
  hasProject,
  projects,
  favoriteProjects,
  recentProjects,
  projectMenuOpen,
  setProjectMenuOpen,
  showPalettePreview,
  setShowPalettePreview,
  handleOpenProject,
  handleRemoveProject,
  handleToggleFavoriteProject,
  handleSelectProject,
  view,
  setView,
  isDark,
  setIsDark,
  panelWidth,
  handlePanelResizeStart,
  filterText,
  setFilterText,
  hideClosed,
  setHideClosed,
  closedTimeFilter,
  setClosedTimeFilter,
  includeHierarchy,
  setIncludeHierarchy,
  sortBy,
  sortOrder,
  handleHeaderClick,
  loading,
  processingData,
  viewModel,
  loadData,
  zoom,
  setZoom,
  toggleNode,
  expandAll,
  collapseAll,
  beads,
  ganttLayout,
  selectedBead,
  setSelectedBead,
  isCreating,
  setIsCreating,
  editForm,
  setEditForm,
  beadDetailOpen,
  setBeadDetailOpen,
  handleBeadClick,
  handleStartCreate,
  handleSaveCreate,
  handleSaveEdit,
  handleCloseBead,
  handleReopenBead,
  handleClaimBead,
  toggleFavorite,
  currentCli,
  setCurrentCli,
  launchInBackground,
  toggleLaunchInBackground,
  handleOpenChat,
  sessionsByBead,
  sessions,
  projectShellKey,
  stats,
  wbsScrollRef,
  ganttScrollRef,
  ganttHeaderScrollRef,
  handleWBSScroll,
  handleGanttScroll,
  handleGanttHeaderScroll,
  handleMouseEnter,
  favoriteBeads,
}: MainAppViewProps) {
  // ── Bottom terminal panel ──────────────────────────────────────────────────
  const defaultTerminalHeight = Math.floor(window.innerHeight * 0.30);
  const [terminalHeight, setTerminalHeight] = useState(defaultTerminalHeight);
  const lastManualHeightRef = useRef(defaultTerminalHeight);
  const [terminalExpanded, setTerminalExpanded] = useState(false);

  const handleTerminalResizeStart = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    const startY = e.clientY;
    const startHeight = terminalHeight;
    const onMove = (ev: MouseEvent) => {
      const delta = startY - ev.clientY; // drag up = taller
      const newH = Math.max(60, Math.min(window.innerHeight - 120, startHeight + delta));
      setTerminalHeight(newH);
      lastManualHeightRef.current = newH;
      setTerminalExpanded(false);
    };
    const onUp = () => {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  }, [terminalHeight]);

  const toggleTerminalExpand = useCallback(() => {
    if (terminalExpanded) {
      setTerminalHeight(lastManualHeightRef.current);
      setTerminalExpanded(false);
    } else {
      setTerminalHeight(window.innerHeight - 80); // fill minus header
      setTerminalExpanded(true);
    }
  }, [terminalExpanded]);
  // ──────────────────────────────────────────────────────────────────────────

  // Distributions from viewModel metadata
  const distributions: BucketDistribution[] = viewModel?.metadata?.distributions ?? [];
  // Shared width for gantt header and body — must match for scroll sync to work
  const ganttTotalWidth = Math.max(5000 * zoom, (distributions.length || 0) * 100 * zoom);

  // Wrap handleOpenChat to match Navigation's expected signature (no beads/path args)
  const handleOpenChatForNav = (persona: string, task?: string, beadId?: string, role?: string) =>
    handleOpenChat(persona, task, beadId, role);

  const renderMainContent = () => {
    if (!hasProject) {
      return (
        <ProjectSelectionView
          projects={projects}
          favoriteProjects={favoriteProjects}
          recentProjects={recentProjects}
          onOpenProject={handleOpenProject}
          onRemoveProject={handleRemoveProject}
          onToggleFavorite={handleToggleFavoriteProject}
          loading={loading}
        />
      );
    }

    if (view === "settings") {
      return <SettingsView />;
    }

    if (view === "terminal") {
      return (
        <div className="flex-1 overflow-hidden">
          <Terminal
            key={`${currentProjectPath}-${projectShellKey}`}
            sessionId={PROJECT_SHELL_ID}
            projectPath={currentProjectPath}
          />
        </div>
      );
    }

    if (view === "sessions") {
      return (
        <SessionsView
          sessions={sessions}
          currentProjectPath={currentProjectPath}
          onSessionClick={createSessionWindow}
          onTerminate={(id) => terminateSession(id).catch(console.error)}
        />
      );
    }

    // gantt or list view: split layout
    return (
      <div className="flex-1 flex overflow-hidden">
        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Top filter/stats bar */}
          <div className="flex shrink-0 border-b-2 border-[var(--border-primary)] bg-[var(--background-secondary)] z-20">
            {/* WBS column: title + expand/collapse + favorites + filters */}
            <div
              className="border-r-2 border-[var(--border-primary)] flex flex-col shrink-0"
              style={{
                width: `${panelWidth}px`,
                minWidth: `${MIN_PANEL_WIDTH}px`,
                maxWidth: `${MAX_PANEL_WIDTH}px`,
              }}
            >
              <div className="px-6 py-4 border-b-2 border-[var(--border-primary)]/50 flex items-center justify-between shrink-0">
                <h2 className="text-sm font-black text-[var(--text-primary)] uppercase tracking-[0.25em] flex items-center gap-2">
                  Task Breakdown
                </h2>
                <div className="flex items-center gap-1">
                  <button onClick={expandAll} title="Expand All" className="p-1.5 hover:bg-[var(--background-tertiary)] rounded-md text-[var(--text-muted)] hover:text-indigo-500 transition-colors">
                    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><polyline points="7 13 12 18 17 13"/><polyline points="7 6 12 11 17 6"/></svg>
                  </button>
                  <button onClick={() => collapseAll(viewModel?.tree)} title="Collapse All" className="p-1.5 hover:bg-[var(--background-tertiary)] rounded-md text-[var(--text-muted)] hover:text-indigo-500 transition-colors">
                    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><polyline points="17 11 12 6 7 11"/><polyline points="17 18 12 13 7 18"/></svg>
                  </button>
                </div>
              </div>
              <FavoritesSection
                favoriteBeads={favoriteBeads}
                favoriteProjects={favoriteProjects}
                onBeadClick={handleBeadClick}
                onOpenChat={handleOpenChatForNav}
                onOpenProject={handleOpenProject}
                sessionsByBead={Object.fromEntries(
                  Object.entries(sessionsByBead).map(([k, v]) => [k, v.map((s) => s.sessionId)])
                )}
              />
              <WBSFilters
                filterText={filterText}
                onFilterTextChange={setFilterText}
                hideClosed={hideClosed}
                onHideClosedChange={setHideClosed}
                closedTimeFilter={closedTimeFilter}
                onClosedTimeFilterChange={setClosedTimeFilter}
                includeHierarchy={includeHierarchy}
                onIncludeHierarchyChange={setIncludeHierarchy}
              />
            </div>

            {/* Stats + zoom bar */}
            <div className="flex-1 flex items-end justify-between px-8 py-3 bg-[var(--background-primary)]">
              <StatsBar stats={stats} />
              <div className="flex items-center gap-1 bg-[var(--background-secondary)] p-1.5 rounded-2xl border-[var(--border-thick)] border-[var(--border-primary)] shadow-[var(--shadow-md)] mb-1">
                <button
                  onClick={() => setZoom(Math.max(0.25, zoom - 0.25))}
                  className="p-2 hover:bg-[var(--background-tertiary)] rounded-xl text-[var(--text-primary)] transition-all text-xs font-black px-4 active:scale-90"
                >
                  -
                </button>
                <span className="text-sm font-mono font-black text-[var(--text-primary)] min-w-[50px] text-center">
                  {Math.round(zoom * 100)}%
                </span>
                <button
                  onClick={() => setZoom(Math.min(3, zoom + 0.25))}
                  className="p-2 hover:bg-[var(--background-tertiary)] rounded-xl text-[var(--text-primary)] transition-all text-xs font-black px-4 active:scale-90"
                >
                  +
                </button>
                <div className="w-px h-5 bg-[var(--border-primary)] mx-2" />
                <button
                  onClick={() => setZoom(1)}
                  className="px-4 py-2 hover:bg-[var(--background-tertiary)] rounded-xl text-[var(--text-primary)] transition-all text-xs font-black uppercase tracking-[0.2em] active:scale-95"
                >
                  Reset
                </button>
                <div className="w-px h-5 bg-[var(--border-primary)] mx-2" />
                <label
                  className="flex items-center gap-2 cursor-pointer select-none px-2"
                  title="When enabled, launching an agent starts it headlessly — no window opens. Pick it up later from the WBS tree."
                >
                  <span className="text-[10px] font-black uppercase tracking-[0.2em] text-[var(--text-muted)] whitespace-nowrap">
                    BG Launch
                  </span>
                  <div
                    onClick={toggleLaunchInBackground}
                    className={`relative w-8 h-4 rounded-full transition-colors cursor-pointer flex-shrink-0 ${launchInBackground ? "bg-[var(--accent-primary)]" : "bg-[var(--border-primary)]"}`}
                  >
                    <div className={`absolute top-0.5 w-3 h-3 rounded-full bg-white shadow transition-transform ${launchInBackground ? "translate-x-4" : "translate-x-0.5"}`} />
                  </div>
                </label>
                <div className="w-px h-5 bg-[var(--border-primary)] mx-2" />
                <button
                  onClick={() => setBeadDetailOpen(!beadDetailOpen)}
                  className={cn(
                    "p-2 hover:bg-[var(--background-tertiary)] rounded-xl transition-all active:scale-90",
                    beadDetailOpen
                      ? "text-[var(--accent-primary)]"
                      : "text-[var(--text-primary)]"
                  )}
                  style={
                    beadDetailOpen
                      ? { backgroundColor: "rgba(15,139,255,0.12)" }
                      : undefined
                  }
                  title={beadDetailOpen ? "Close Bead Detail" : "Open Bead Detail"}
                >
                  <Info size={18} strokeWidth={2.5} />
                </button>
              </div>
            </div>
          </div>

          {/* Column header row (gantt only) */}
          {view === "gantt" && (
            <div className="flex shrink-0 border-b-2 border-[var(--border-primary)] bg-[var(--background-secondary)] z-10">
              <div
                className="border-r-2 border-[var(--border-primary)] shrink-0"
                style={{
                  width: `${panelWidth}px`,
                  minWidth: `${MIN_PANEL_WIDTH}px`,
                  maxWidth: `${MAX_PANEL_WIDTH}px`,
                }}
              >
                <WBSHeader
                  sortBy={sortBy}
                  sortOrder={sortOrder === "none" ? "none" : sortOrder}
                  onHeaderClick={handleHeaderClick}
                />
              </div>
              {/* Gantt state header — scroll-synced with gantt body */}
              <div className="flex-1 overflow-hidden bg-[var(--background-tertiary)]">
                <div
                  ref={ganttHeaderScrollRef}
                  onScroll={handleGanttHeaderScroll}
                  onMouseEnter={handleMouseEnter}
                  className="overflow-x-auto overflow-y-hidden no-scrollbar"
                >
                  <div style={{ width: ganttTotalWidth }}>
                    <GanttStateHeader distributions={distributions} zoom={zoom} />
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* Main body row */}
          <div className="flex-1 flex overflow-hidden">
            {/* WBS Panel — just the scrolling tree */}
            <WBSPanel
              nodes={viewModel?.tree ?? []}
              selectedBeadId={selectedBead?.id}
              onToggle={(id) => toggleNode(id, wbsScrollRef)}
              onBeadClick={handleBeadClick}
              sessionsByBead={sessionsByBead}
              onSessionClick={createSessionWindow}
              panelWidth={panelWidth}
              minPanelWidth={MIN_PANEL_WIDTH}
              maxPanelWidth={MAX_PANEL_WIDTH}
              onPanelResizeStart={handlePanelResizeStart}
              scrollRef={wbsScrollRef}
              onScroll={handleWBSScroll}
              onMouseEnter={handleMouseEnter}
              loading={loading}
              processingData={processingData}
            />

            {/* Right panel: Gantt or List */}
            {view === "gantt" ? (
              <GanttPanel
                items={ganttLayout.items}
                rowCount={ganttLayout.rowCount}
                rowDepths={ganttLayout.rowDepths}
                connectors={ganttLayout.connectors}
                zoom={zoom}
                totalWidth={ganttTotalWidth}
                selectedBead={selectedBead}
                onBeadClick={handleBeadClick}
                onOpenChat={handleOpenChatForNav}
                sessionsByBead={Object.fromEntries(
                  Object.entries(sessionsByBead).map(([k, v]) => [
                    k,
                    v.map((s) => s.sessionId),
                  ])
                )}
                scrollRef={ganttScrollRef}
                onScroll={handleGanttScroll}
                onMouseEnter={handleMouseEnter}
                loading={loading}
              />
            ) : (
              <ListView
                beads={beads}
                onBeadClick={handleBeadClick}
                selectedBeadId={selectedBead?.id}
                sortBy={sortBy}
                sortOrder={sortOrder}
                onHeaderClick={handleHeaderClick}
                sessionsByBead={sessionsByBead}
                onSessionClick={createSessionWindow}
              />
            )}
          </div>
        </div>
      </div>
    );
  };

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[var(--background-primary)] text-[var(--text-primary)] font-sans">
      <Navigation
        currentView={view}
        onViewChange={setView}
        onOpenChat={handleOpenChatForNav}
        onOpenPalettePreview={() => setShowPalettePreview(true)}
        selectedBeadId={selectedBead?.id}
      />
      <main className="flex-1 flex flex-col min-w-0 bg-[var(--background-primary)] relative overflow-hidden">
        <Header
          isDark={isDark}
          setIsDark={setIsDark}
          handleStartCreate={handleStartCreate}
          loadData={loadData}
          projectMenuOpen={projectMenuOpen}
          setProjectMenuOpen={setProjectMenuOpen}
          favoriteProjects={favoriteProjects}
          recentProjects={recentProjects}
          currentProjectPath={currentProjectPath}
          handleOpenProject={handleOpenProject}
          toggleFavoriteProject={handleToggleFavoriteProject}
          removeProject={handleRemoveProject}
          handleSelectProject={handleSelectProject}
          currentCli={currentCli}
          setCurrentCli={setCurrentCli}
        />
        {/* Content + bottom terminal panel.
            CSS grid gives each row an exact pixel allocation — 1fr shrinks
            precisely as terminalHeight grows, with no flex inference issues. */}
        <div
          className="flex-1 min-h-0"
          style={{
            display: "grid",
            gridTemplateRows: hasProject ? `1fr ${terminalHeight}px` : "1fr",
            gridTemplateColumns: "1fr",
          }}
        >
          <div className="flex flex-col overflow-hidden min-h-0 h-full">
            {renderMainContent()}
          </div>

          {/* ── Bottom terminal panel ── */}
          {hasProject && (
            <div
              className="flex flex-col border-t-2 border-[var(--border-primary)] bg-[var(--background-primary)] min-h-0"
            >
              {/* Drag handle + toolbar */}
              <div
                className="flex-shrink-0 flex items-center justify-between px-3 h-7 bg-[var(--background-secondary)] border-b border-[var(--border-primary)]/50 cursor-ns-resize select-none"
                onMouseDown={handleTerminalResizeStart}
              >
                <div className="flex items-center gap-1.5 pointer-events-none">
                  <div className="w-6 h-0.5 rounded-full bg-[var(--border-primary)]" />
                  <span className="text-[10px] font-black uppercase tracking-[0.2em] text-[var(--text-muted)]">
                    Shell
                  </span>
                </div>
                <button
                  onMouseDown={(e) => e.stopPropagation()}
                  onClick={toggleTerminalExpand}
                  className="pointer-events-auto p-1 rounded hover:bg-[var(--background-tertiary)] text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-colors"
                  title={terminalExpanded ? "Restore" : "Expand"}
                >
                  {terminalExpanded
                    ? <Minimize2 size={13} strokeWidth={2.5} />
                    : <Maximize2 size={13} strokeWidth={2.5} />}
                </button>
              </div>

              {/* Terminal body */}
              <div className="flex-1 min-h-0">
                <Terminal
                  key={`${currentProjectPath}-${projectShellKey}`}
                  sessionId={PROJECT_SHELL_ID}
                  projectPath={currentProjectPath}
                />
              </div>
            </div>
          )}
        </div>
      </main>

      {showPalettePreview && (
        <PalettePreviewDialog onClose={() => setShowPalettePreview(false)} />
      )}

      <BeadDetailModal
        bead={selectedBead}
        open={beadDetailOpen}
        onClose={() => setBeadDetailOpen(false)}
        isCreating={isCreating}
        editForm={editForm}
        beads={beads}
        setIsCreating={setIsCreating}
        setSelectedBead={setSelectedBead}
        setEditForm={setEditForm}
        handleSaveEdit={() => handleSaveEdit(viewModel)}
        handleSaveCreate={() => handleSaveCreate(viewModel)}
        handleCloseBead={handleCloseBead}
        handleReopenBead={handleReopenBead}
        handleClaimBead={handleClaimBead}
        toggleFavorite={toggleFavorite}
        onOpenChat={handleOpenChatForNav}
      />
    </div>
  );
}
