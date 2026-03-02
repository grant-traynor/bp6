import { useEffect, useRef, useMemo, useCallback, useState } from "react";
import { useSessionStore, groupSessionsByBead } from "./stores/sessionStore";
import "./stores/sessionStoreDiagnostics"; // Enable diagnostics

// Hooks
import { useAppInitialization } from "./hooks/useAppInitialization";
import { useProjectState } from "./hooks/useProjectState";
import { useFilterState } from "./hooks/useFilterState";
import { useGanttTree } from "./hooks/useGanttTree";
import { useGanttLayout } from "./hooks/useGanttLayout";
import { useBeadEditing } from "./hooks/useBeadEditing";
import { useChatSessions } from "./hooks/useChatSessions";
import { usePersistentUI } from "./hooks/usePersistentUI";
import { useScrollSync } from "./hooks/useScrollSync";
import { useKeyboardShortcuts } from "./hooks/useKeyboardShortcuts";

// Views
import { SessionWindowView } from "./views/SessionWindowView";
import { MainAppView } from "./views/MainAppView";

// Context providers (optional wrapping — no component uses them yet)
import { ProjectProvider } from "./contexts/ProjectContext";
import { BeadProvider } from "./contexts/BeadContext";
import { UIProvider } from "./contexts/UIContext";

interface AppProps {
  isSessionWindow?: boolean;
  sessionId?: string | null;
  windowLabel?: string;
}

function App({
  isSessionWindow = false,
  sessionId = null,
  windowLabel: _windowLabel = "main",
}: AppProps = {}) {
  // -------------------------------------------------------------------
  // Session window: initialize store then render SessionWindowView
  // -------------------------------------------------------------------
  useEffect(() => {
    if (!isSessionWindow) return;

    let cleanupFn: (() => void) | undefined;
    useSessionStore
      .getState()
      .initializeStore()
      .then((unlisten) => {
        cleanupFn = unlisten;
      })
      .catch((err) =>
        console.error("Failed to init session store in session window:", err)
      );

    return () => {
      cleanupFn?.();
      useSessionStore.getState().cleanup();
    };
  }, [isSessionWindow]);

  // -------------------------------------------------------------------
  // Gantt tree: collapse state + zoom (needed by filterState for deps)
  // -------------------------------------------------------------------
  const ganttTree = useGanttTree();

  // -------------------------------------------------------------------
  // Filter / sort / data-fetching state (gated by isReady)
  // Needs a project path and isReady; these come from initialization
  // so we must declare filterState before init but feed it isReady after.
  // We'll initialise with placeholder values and the hooks self-correct
  // once isReady flips true.
  // -------------------------------------------------------------------

  // isInitializingRef — fed into usePersistentUI to guard saves during startup
  const isInitializingRef = useRef(true);

  // Persistent UI: theme, panel width, view
  // We'll wire the real filter state into it after declaring filterState.
  // To break the circular dependency we forward refs to filter state via
  // a stable ref container that is populated after the hooks run.
  const filterStateRef = useRef<{
    filterText: string;
    hideClosed: boolean;
    closedTimeFilter: string;
    includeHierarchy: boolean;
    sortBy: string;
    sortOrder: string;
  }>({
    filterText: "",
    hideClosed: false,
    closedTimeFilter: "all",
    includeHierarchy: true,
    sortBy: "none",
    sortOrder: "none",
  });

  // We need project state before filterState to wire `hasProject`
  // -------------------------------------------------------------------
  // We create filterState first with placeholder projectPath/isReady;
  // the hooks handle their own initialization.
  // -------------------------------------------------------------------

  // Chat sessions (standalone)
  const chatSessions = useChatSessions();

  // Sessions from Zustand store
  const sessions = useSessionStore((state) => state.sessions);
  const sessionsByBead = useMemo(
    () => groupSessionsByBead(sessions),
    [sessions]
  );

  // Project state: needs loadData + setLoading + setProjectShellKey
  // These come from filterState and appInit, but we have a circular dep.
  // Solution: use a stable callback ref for loadData, resolved after filterState.
  const loadDataRef = useRef<() => void>(() => {});
  const setLoadingRef = useRef<React.Dispatch<React.SetStateAction<boolean>>>(
    () => {}
  );
  const setProjectShellKeyRef = useRef<
    React.Dispatch<React.SetStateAction<number>>
  >(() => {});

  // Stable ref-forwarding wrappers — must not change identity on re-render,
  // otherwise handleOpenProject becomes unstable and causes init to loop.
  const stableLoadData = useCallback(() => loadDataRef.current(), []);
  const stableSetLoading = useCallback(
    (...args: Parameters<React.Dispatch<React.SetStateAction<boolean>>>) =>
      setLoadingRef.current(...args),
    []
  );
  const stableSetProjectShellKey = useCallback(
    (...args: Parameters<React.Dispatch<React.SetStateAction<number>>>) =>
      setProjectShellKeyRef.current(...args),
    []
  );

  const projectState = useProjectState(
    stableLoadData,
    stableSetLoading,
    stableSetProjectShellKey
  );

  // Reactive isReady gate — set to true once appInit completes
  const [isReady, setIsReady] = useState(false);

  // Filter state — owns isReady gate, data fetching, sorting, etc.
  const filterState = useFilterState({
    isReady,
    currentProjectPath: projectState.currentProjectPath,
    zoom: ganttTree.zoom,
    collapsedIds: ganttTree.collapsedIds,
    panelWidth: 300, // placeholder; real value from persistentUI
    hasProject: projectState.hasProject,
  });

  // Patch the loadDataRef so projectState.handleOpenProject can call it
  loadDataRef.current = filterState.incrementRefetchTrigger;
  setLoadingRef.current = filterState.setLoading;

  // Persistent UI — needs filter state snapshot for saving startup state
  const persistentUI = usePersistentUI({
    hasProject: projectState.hasProject,
    isInitializing: isInitializingRef,
    filterText: filterState.filterText,
    hideClosed: filterState.hideClosed,
    closedTimeFilter: filterState.closedTimeFilter,
    includeHierarchy: filterState.includeHierarchy,
    sortBy: filterState.sortBy,
    sortOrder: filterState.sortOrder,
    zoom: ganttTree.zoom,
    collapsedIds: ganttTree.collapsedIds,
    isSessionWindow,
  });

  // Keep filterStateRef in sync so persistentUI can observe latest values
  filterStateRef.current = {
    filterText: filterState.filterText,
    hideClosed: filterState.hideClosed,
    closedTimeFilter: filterState.closedTimeFilter,
    includeHierarchy: filterState.includeHierarchy,
    sortBy: filterState.sortBy,
    sortOrder: filterState.sortOrder,
  };

  // App initialization: startup state restore, session store init, project auto-open
  const appInit = useAppInitialization(
    {
      setFilterText: filterState.setFilterText,
      setHideClosed: filterState.setHideClosed,
      setClosedTimeFilter: filterState.setClosedTimeFilter,
      setIncludeHierarchy: filterState.setIncludeHierarchy,
      setSortBy: filterState.setSortBy,
      setSortOrder: filterState.setSortOrder,
      setZoom: ganttTree.setZoom,
      setCollapsedIds: ganttTree.setCollapsedIds,
      setPanelWidth: persistentUI.setPanelWidth,
      setProjects: projectState.setProjects,
      setCurrentProjectPath: projectState.setCurrentProjectPath,
      setHasProject: projectState.setHasProject,
      setLoading: filterState.setLoading,
    },
    projectState.handleOpenProject,
    filterState.incrementRefetchTrigger,
    isSessionWindow
  );

  // Wire the setter from useAppInitialization so projectState.handleOpenProject can increment it
  setProjectShellKeyRef.current = appInit.setProjectShellKey;

  // Mark initialization complete when isReady flips
  useEffect(() => {
    if (appInit.isReady) {
      isInitializingRef.current = false;
    }
  }, [appInit.isReady]);

  // Ungate data fetching once initialization completes
  useEffect(() => {
    if (appInit.isReady) {
      setIsReady(true);
    }
  }, [appInit.isReady]);

  // Gantt layout computed from viewModel + zoom + collapsedIds
  const ganttLayoutData = useGanttLayout(
    filterState.viewModel,
    ganttTree.zoom,
    ganttTree.collapsedIds
  );

  // Bead editing
  const beadEditing = useBeadEditing(
    filterState.incrementRefetchTrigger,
    ganttLayoutData.beads
  );

  // Scroll sync
  const scrollSync = useScrollSync(filterState.loading);

  // Keyboard shortcuts
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  useKeyboardShortcuts({
    expandAll: ganttTree.expandAll,
    collapseAll: ganttTree.collapseAll,
    loadData: filterState.incrementRefetchTrigger,
    handleOpenChat: (persona, task, beadId, role) =>
      chatSessions.handleOpenChat(
        persona,
        task,
        beadId,
        role,
        ganttLayoutData.beads,
        projectState.currentProjectPath
      ),
    setFilterText: filterState.setFilterText,
    searchInputRef,
    setSelectedBead: beadEditing.setSelectedBead,
    setIsCreating: beadEditing.setIsCreating,
    setBeadDetailOpen: beadEditing.setBeadDetailOpen,
    handleStartCreate: beadEditing.handleStartCreate,
  });

  // Stats memo
  const stats = useMemo(() => {
    const beads = ganttLayoutData.beads;
    const total = beads.length;
    const open = beads.filter((b) => b.status === "open").length;
    const inProgress = beads.filter((b) => b.status === "in_progress").length;
    const closed = beads.filter((b) => b.status === "closed").length;
    const blocked = beads.filter((b) => {
      if (b.status === "closed") return false;
      const deps = b.dependencies || [];
      return deps.some((d) => {
        if (d.type !== "blocks") return false;
        const pred = beads.find((p) => p.id === d.depends_on_id);
        return pred && pred.status !== "closed";
      });
    }).length;
    return { total, open, inProgress, closed, blocked };
  }, [ganttLayoutData.beads]);

  // Favorite beads
  const favoriteBeads = useMemo(
    () => ganttLayoutData.beads.filter((b) => b.is_favorite),
    [ganttLayoutData.beads]
  );

  // Convert sessionsByBead to string[] for components that need only IDs
  const sessionsByBeadIds = useMemo(
    () =>
      Object.fromEntries(
        Object.entries(sessionsByBead).map(([k, v]) => [
          k,
          v.map((s) => s.sessionId),
        ])
      ),
    [sessionsByBead]
  );

  // -------------------------------------------------------------------
  // Session window early return
  // -------------------------------------------------------------------
  if (isSessionWindow && sessionId) {
    return (
      <SessionWindowView
        sessionId={sessionId}
      />
    );
  }

  // -------------------------------------------------------------------
  // Main app — context providers + MainAppView
  // -------------------------------------------------------------------
  return (
    <ProjectProvider
      value={{
        viewModel: filterState.viewModel,
        currentProjectPath: projectState.currentProjectPath,
        hasProject: projectState.hasProject,
        loading: filterState.loading,
        processingData: filterState.processingData,
        loadData: filterState.incrementRefetchTrigger,
      }}
    >
      <BeadProvider
        value={{
          selectedBead: beadEditing.selectedBead,
          handleBeadClick: beadEditing.handleBeadClick,
          sessionsByBead: sessionsByBeadIds,
        }}
      >
        <UIProvider
          value={{
            isDark: persistentUI.isDark,
            view: persistentUI.view,
            panelWidth: persistentUI.panelWidth,
          }}
        >
          <MainAppView
            // Project
            currentProjectPath={projectState.currentProjectPath}
            hasProject={projectState.hasProject}
            projects={projectState.projects}
            favoriteProjects={projectState.favoriteProjects}
            recentProjects={projectState.recentProjects}
            projectMenuOpen={projectState.projectMenuOpen}
            setProjectMenuOpen={projectState.setProjectMenuOpen}
            showPalettePreview={projectState.showPalettePreview}
            setShowPalettePreview={projectState.setShowPalettePreview}
            handleOpenProject={projectState.handleOpenProject}
            handleRemoveProject={projectState.handleRemoveProject}
            handleToggleFavoriteProject={projectState.handleToggleFavoriteProject}
            handleSelectProject={projectState.handleSelectProject}
            // View / UI
            view={persistentUI.view}
            setView={persistentUI.setView}
            isDark={persistentUI.isDark}
            setIsDark={persistentUI.setIsDark}
            panelWidth={persistentUI.panelWidth}
            handlePanelResizeStart={persistentUI.handlePanelResizeStart}
            // Filter
            filterText={filterState.filterText}
            setFilterText={filterState.setFilterText}
            hideClosed={filterState.hideClosed}
            setHideClosed={filterState.setHideClosed}
            closedTimeFilter={filterState.closedTimeFilter}
            setClosedTimeFilter={filterState.setClosedTimeFilter}
            includeHierarchy={filterState.includeHierarchy}
            setIncludeHierarchy={filterState.setIncludeHierarchy}
            sortBy={filterState.sortBy}
            sortOrder={filterState.sortOrder}
            handleHeaderClick={filterState.handleHeaderClick}
            loading={filterState.loading}
            processingData={filterState.processingData}
            viewModel={filterState.viewModel}
            loadData={filterState.incrementRefetchTrigger}
            // Gantt tree
            collapsedIds={ganttTree.collapsedIds}
            zoom={ganttTree.zoom}
            setZoom={ganttTree.setZoom}
            toggleNode={ganttTree.toggleNode}
            expandAll={ganttTree.expandAll}
            collapseAll={ganttTree.collapseAll}
            beads={ganttLayoutData.beads}
            ganttLayout={ganttLayoutData.ganttLayout}
            // Bead editing
            selectedBead={beadEditing.selectedBead}
            setSelectedBead={beadEditing.setSelectedBead}
            isCreating={beadEditing.isCreating}
            setIsCreating={beadEditing.setIsCreating}
            editForm={beadEditing.editForm}
            setEditForm={beadEditing.setEditForm}
            beadDetailOpen={beadEditing.beadDetailOpen}
            setBeadDetailOpen={beadEditing.setBeadDetailOpen}
            handleBeadClick={beadEditing.handleBeadClick}
            handleStartCreate={beadEditing.handleStartCreate}
            handleSaveCreate={beadEditing.handleSaveCreate}
            handleSaveEdit={beadEditing.handleSaveEdit}
            handleCloseBead={beadEditing.handleCloseBead}
            handleReopenBead={beadEditing.handleReopenBead}
            handleClaimBead={beadEditing.handleClaimBead}
            toggleFavorite={beadEditing.toggleFavorite}
            // Chat
            currentCli={chatSessions.currentCli}
            setCurrentCli={chatSessions.setCurrentCli}
            handleOpenChat={(persona, task, beadId, role) =>
              chatSessions.handleOpenChat(
                persona,
                task,
                beadId,
                role,
                ganttLayoutData.beads,
                projectState.currentProjectPath
              )
            }
            sessionsByBead={sessionsByBead}
            sessions={sessions}
            // Shell
            projectShellKey={appInit.projectShellKey}
            // Stats
            stats={stats}
            favoriteBeads={favoriteBeads}
            // Scroll sync
            wbsScrollRef={scrollSync.wbsScrollRef}
            ganttScrollRef={scrollSync.ganttScrollRef}
            ganttHeaderScrollRef={scrollSync.ganttHeaderScrollRef}
            handleWBSScroll={scrollSync.handleWBSScroll}
            handleGanttScroll={scrollSync.handleGanttScroll}
            handleGanttHeaderScroll={scrollSync.handleGanttHeaderScroll}
            handleMouseEnter={scrollSync.handleMouseEnter}
          />
        </UIProvider>
      </BeadProvider>
    </ProjectProvider>
  );
}

export default App;
