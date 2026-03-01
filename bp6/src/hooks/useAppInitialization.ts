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
import { getCurrentWindow, PhysicalPosition, PhysicalSize, availableMonitors, primaryMonitor } from "@tauri-apps/api/window";
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
}

export interface UseAppInitializationReturn {
  isReady: boolean;
  projectShellKey: number;
  setProjectShellKey: React.Dispatch<React.SetStateAction<number>>;
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

          // Restore window position/size, handling monitor topology changes.
          //
          // The original bug: using currentMonitor() returns the monitor where Tauri
          // created the window (always the primary / monitor A), so any saved position
          // from a secondary monitor got clamped into monitor A's bounds.
          //
          // Fix: apply the saved position directly. Only fall back to centering on the
          // primary monitor if the window would be entirely off-screen (e.g. a display
          // was disconnected since last run). macOS does NOT do this automatically.
          const win = getCurrentWindow();
          const minW = 960;
          const minH = 640;

          const savedW = startupState.window.width;
          const savedH = startupState.window.height;
          const savedX = startupState.window.x ?? 0;
          const savedY = startupState.window.y ?? 0;

          const targetW = Math.max(savedW ?? minW, minW);
          const targetH = Math.max(savedH ?? minH, minH);

          // Check that at least 100×100 px of the window would land on some monitor.
          const monitors = await availableMonitors().catch(() => []);
          const isOnScreen = monitors.length === 0 || monitors.some((m) => {
            const mPos = m.position ?? { x: 0, y: 0 };
            const overlapX = Math.min(savedX + targetW, mPos.x + m.size.width) - Math.max(savedX, mPos.x);
            const overlapY = Math.min(savedY + targetH, mPos.y + m.size.height) - Math.max(savedY, mPos.y);
            return overlapX > 100 && overlapY > 100;
          });

          let targetX: number;
          let targetY: number;

          if (isOnScreen) {
            // Use the saved position exactly — preserves multi-monitor placement.
            targetX = savedX;
            targetY = savedY;
          } else {
            // Saved monitor is gone — center on primary.
            const primary = await primaryMonitor().catch(() => null) ?? monitors[0] ?? null;
            const pPos = primary?.position ?? { x: 0, y: 0 };
            const pSize = primary?.size ?? { width: 1920, height: 1080 };
            targetX = pPos.x + Math.round((pSize.width - targetW) / 2);
            targetY = pPos.y + Math.round((pSize.height - targetH) / 2);
          }

          if (startupState.window.isMaximized) {
            // Move to the target monitor before maximizing — macOS maximizes on
            // whichever screen the window is currently on (default: primary / monitor A).
            // A 1px nudge to any point on the target monitor is enough to steer it.
            await win.setPosition(new PhysicalPosition(targetX, targetY));
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
      // Do NOT reset hasInitialized — init must run exactly once for the app lifetime.
      // Resetting it here would cause re-initialization on every dependency change.
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
    setProjectShellKey,
    loadProjects,
    loadData,
  };
}

// Re-export for use in handleOpenProject outside the hook
export { PROJECT_SHELL_ID, killProjectShell, openProject };
