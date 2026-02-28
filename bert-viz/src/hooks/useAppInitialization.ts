import { useState, useEffect, useCallback, useRef } from "react";
import {
  fetchProjects,
  onBeadsUpdated,
  onProjectsUpdated,
  onProjectShellExited,
  loadStartupState,
  killProjectShell,
  openProject,
  type Project,
} from "../api";
import { getCurrentWindow, PhysicalPosition, PhysicalSize, currentMonitor } from "@tauri-apps/api/window";
import { useSessionStore } from "../stores/sessionStore";

// Time-based filter options for closed tasks
export type ClosedTimeFilter =
  | "all"
  | "1h"
  | "6h"
  | "24h"
  | "7d"
  | "30d"
  | "older_than_6h";

export interface StartupRestoreCallbacks {
  setFilterText: (v: string) => void;
  setHideClosed: (v: boolean) => void;
  setClosedTimeFilter: (v: ClosedTimeFilter) => void;
  setIncludeHierarchy: (v: boolean) => void;
  setSortBy: (v: "priority" | "title" | "type" | "id" | "none") => void;
  setSortOrder: (v: "asc" | "desc" | "none") => void;
  setZoom: (v: number) => void;
  setCollapsedIds: (v: Set<string>) => void;
  setPanelWidth: (v: number) => void;
  setProjects: (v: Project[]) => void;
  setCurrentProjectPath: (v: string) => void;
  setHasProject: (v: boolean) => void;
  setLoading: (v: boolean) => void;
  setProjectShellKey: React.Dispatch<React.SetStateAction<number>>;
}

export interface UseAppInitializationReturn {
  isReady: boolean;
  projectShellKey: number;
  loadProjects: () => Promise<void>;
  loadData: (showLoading?: boolean) => void;
}

const PROJECT_SHELL_ID = "project-shell";

/**
 * Manages app initialization: startup state restore, projects load, project shell exit listener.
 * Returns isReady (gates fetchProjectViewModel), projectShellKey, loadProjects, and loadData.
 *
 * loadData increments refetchTrigger which useFilterState watches.
 * The callbacks argument is a bag of setters from the other hooks — injected during wiring in App.tsx.
 */
export function useAppInitialization(
  callbacks: StartupRestoreCallbacks,
  onOpenProject: (path: string) => Promise<void>,
  onLoadData: () => void
): UseAppInitializationReturn {
  const [isReady, setIsReady] = useState(false);
  const [projectShellKey, setProjectShellKey] = useState(0);

  const hasInitialized = useRef(false);
  const isInitializing = useRef(true);

  const {
    setFilterText,
    setHideClosed,
    setClosedTimeFilter,
    setIncludeHierarchy,
    setSortBy,
    setSortOrder,
    setZoom,
    setCollapsedIds,
    setPanelWidth,
    setProjects,
    setCurrentProjectPath: _setCurrentProjectPath,
    setHasProject,
    setLoading,
  } = callbacks;

  const loadProjects = useCallback(async () => {
    const data = await fetchProjects();
    setProjects(data);
  }, [setProjects]);

  // loadData: signals useFilterState to refetch
  const loadData = useCallback(() => {
    onLoadData();
  }, [onLoadData]);

  // Listen for project shell exit — remounting Terminal component restores shell at correct dimensions
  useEffect(() => {
    const unlistenPromise = onProjectShellExited((_sessionId) => {
      setProjectShellKey((k) => k + 1);
    });
    return () => {
      unlistenPromise.then((u) => u());
    };
  }, []);

  // One-time initialization
  useEffect(() => {
    if (hasInitialized.current) return;
    hasInitialized.current = true;

    const init = async () => {
      try {
        const startupState = await loadStartupState();
        if (startupState) {
          console.log("Loaded startup state:", startupState);

          // Restore window position/size with safety clamps
          const win = getCurrentWindow();
          const monitor = await currentMonitor().catch(() => null);
          const minW = 960;
          const minH = 640;

          const savedW = startupState.window.width;
          const savedH = startupState.window.height;
          const savedX = startupState.window.x;
          const savedY = startupState.window.y;

          const monitorSize = monitor?.size;
          const monitorPos = (monitor as { position?: { x: number; y: number } })?.position || { x: 0, y: 0 };

          const maxW = monitorSize?.width || savedW || minW;
          const maxH = monitorSize?.height || savedH || minH;

          const targetW = Math.min(Math.max(savedW || maxW, minW), maxW);
          const targetH = Math.min(Math.max(savedH || maxH, minH), maxH);

          const maxX = monitorPos.x + (monitorSize?.width ?? targetW) - targetW;
          const maxY = monitorPos.y + (monitorSize?.height ?? targetH) - targetH;

          const targetX = Math.min(Math.max(savedX ?? monitorPos.x, monitorPos.x), maxX);
          const targetY = Math.min(Math.max(savedY ?? monitorPos.y, monitorPos.y), maxY);

          if (startupState.window.isMaximized) {
            await win.maximize();
          } else {
            await win.setSize(new PhysicalSize(targetW, targetH));
            await win.setPosition(new PhysicalPosition(targetX, targetY));
          }

          // Restore filter state
          setFilterText(startupState.filters.filterText);
          setHideClosed(startupState.filters.hideClosed);
          setClosedTimeFilter(startupState.filters.closedTimeFilter as ClosedTimeFilter);
          setIncludeHierarchy(startupState.filters.includeHierarchy);

          // Restore sort state
          setSortBy(startupState.sort.sortBy as "priority" | "title" | "type" | "id" | "none");
          setSortOrder(startupState.sort.sortOrder as "asc" | "desc" | "none");

          // Restore UI state
          setZoom(startupState.ui.zoom);
          setCollapsedIds(new Set(startupState.ui.collapsedIds));
          if (startupState.ui.wbsPanelWidth) {
            setPanelWidth(startupState.ui.wbsPanelWidth);
          }
        } else {
          console.log("No startup state file found, using defaults");
        }

        // Load projects list
        const projs = await fetchProjects();
        setProjects(projs);

        // Initialize session store
        const sessionUnlisten = await useSessionStore.getState().initializeStore();

        // Auto-open most recent project
        const mostRecent = [...projs].sort(
          (a, b) => (b.last_opened || "").localeCompare(a.last_opened || "")
        )[0];

        if (mostRecent) {
          await onOpenProject(mostRecent.path);
        } else {
          setHasProject(false);
        }

        return sessionUnlisten;
      } catch (error) {
        console.error("Initialization failed:", error);
        setHasProject(false);
      } finally {
        isInitializing.current = false;
        setLoading(false);
        setIsReady(true);
      }
    };

    const unlistenPromises = [
      onBeadsUpdated(() => {
        console.log("beads-updated event received");
        onLoadData();
      }),
      onProjectsUpdated(() => {
        console.log("projects-updated event received");
        loadProjects();
      }),
      init(),
    ];

    return () => {
      console.log("Cleaning up initialization event listeners");
      unlistenPromises.forEach(async (p) => {
        try {
          const unlisten = await p;
          unlisten?.();
        } catch (err) {
          console.error("Failed to unlisten:", err);
        }
      });
      useSessionStore.getState().cleanup();
      hasInitialized.current = false;
    };
  }, [
    loadProjects,
    onOpenProject,
    onLoadData,
    setFilterText,
    setHideClosed,
    setClosedTimeFilter,
    setIncludeHierarchy,
    setSortBy,
    setSortOrder,
    setZoom,
    setCollapsedIds,
    setPanelWidth,
    setProjects,
    setHasProject,
    setLoading,
  ]);

  return {
    isReady,
    projectShellKey,
    loadProjects,
    loadData,
  };
}

// Re-export for use in handleOpenProject outside the hook
export { PROJECT_SHELL_ID, killProjectShell, openProject };
