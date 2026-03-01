import { useState, useEffect, useCallback, useRef } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { saveStartupState } from "../api";
import { type ViewType } from "../components/layout/Navigation";
import type { ClosedTimeFilter } from "./useAppInitialization";

const MIN_PANEL_WIDTH = 200;
const MAX_PANEL_WIDTH = 1200;

export interface UsePersistentUIReturn {
  isDark: boolean;
  setIsDark: React.Dispatch<React.SetStateAction<boolean>>;
  panelWidth: number;
  setPanelWidth: React.Dispatch<React.SetStateAction<number>>;
  view: ViewType;
  setView: React.Dispatch<React.SetStateAction<ViewType>>;
  handlePanelResizeStart: (e: React.MouseEvent) => void;
}

/**
 * Additional state required for the save-panel-width effect and save-filter effect.
 * These are passed in from other hooks so usePersistentUI can write a complete StartupState.
 */
export interface UsePersistentUIInput {
  hasProject: boolean;
  isInitializing: React.MutableRefObject<boolean>;
  filterText: string;
  hideClosed: boolean;
  closedTimeFilter: ClosedTimeFilter;
  includeHierarchy: boolean;
  sortBy: string;
  sortOrder: string;
  zoom: number;
  collapsedIds: Set<string>;
  /** When true, skip all startup.json persistence (session/chat windows) */
  isSessionWindow?: boolean;
}

/**
 * Owns isDark (theme), panelWidth, and view type.
 * Handles:
 * - DOM dark class + localStorage persistence for theme.
 * - Debounced save of panelWidth + full startup state on panel resize.
 * - Debounced main window position/size save on resize/move events.
 */
export function usePersistentUI(input: UsePersistentUIInput): UsePersistentUIReturn {
  const {
    hasProject,
    isInitializing,
    filterText,
    hideClosed,
    closedTimeFilter,
    includeHierarchy,
    sortBy,
    sortOrder,
    zoom,
    collapsedIds,
    isSessionWindow = false,
  } = input;

  const [isDark, setIsDark] = useState<boolean>(() => {
    if (typeof localStorage !== "undefined") {
      const stored = localStorage.getItem("theme");
      if (stored === "dark") return true;
      if (stored === "light") return false;
    }
    return true;
  });

  const [panelWidth, setPanelWidth] = useState(300);
  const [view, setView] = useState<ViewType>("gantt");

  // Keep a stable ref to panelWidth for the resize handler
  const panelWidthRef = useRef(panelWidth);
  useEffect(() => {
    panelWidthRef.current = panelWidth;
  }, [panelWidth]);

  // Apply dark mode class + persist theme to localStorage
  useEffect(() => {
    if (isDark) document.documentElement.classList.add("dark");
    else document.documentElement.classList.remove("dark");

    if (typeof localStorage !== "undefined") {
      localStorage.setItem("theme", isDark ? "dark" : "light");
    }
  }, [isDark]);

  // Main window position/size persistence: debounced save on resize/move
  useEffect(() => {
    // Session/chat windows must not overwrite the main app's startup.json
    if (isSessionWindow) return;

    const saveCurrentWindowState = async () => {
      if (isInitializing.current) return;
      try {
        const win = getCurrentWindow();
        const position = await win.outerPosition();
        const size = await win.outerSize();
        const isMaximized = await win.isMaximized();

        // Reject bogus coordinates (caught during window transition)
        if (Math.abs(position.x) > 10000 || Math.abs(position.y) > 10000) {
          console.warn("Skipping save: suspicious window position", position);
          return;
        }

        await saveStartupState({
          window: {
            width: size.width,
            height: size.height,
            x: position.x,
            y: position.y,
            isMaximized,
          },
          filters: {
            filterText,
            hideClosed,
            closedTimeFilter,
            includeHierarchy,
          },
          sort: {
            sortBy,
            sortOrder,
          },
          ui: {
            zoom,
            collapsedIds: Array.from(collapsedIds),
            wbsPanelWidth: panelWidthRef.current,
          },
        });
      } catch (error) {
        console.error("Failed to save main window state:", error);
      }
    };

    let saveTimeout: ReturnType<typeof setTimeout> | null = null;
    const debouncedSave = () => {
      if (saveTimeout) clearTimeout(saveTimeout);
      saveTimeout = setTimeout(saveCurrentWindowState, 500);
    };

    let unlistenResize: (() => void) | null = null;
    let unlistenMove: (() => void) | null = null;

    const setupListeners = async () => {
      const win = getCurrentWindow();
      unlistenResize = await win.onResized(debouncedSave);
      unlistenMove = await win.onMoved(debouncedSave);
    };

    setupListeners();

    return () => {
      if (saveTimeout) clearTimeout(saveTimeout);
      if (unlistenResize) unlistenResize();
      if (unlistenMove) unlistenMove();
    };
  }, [
    isSessionWindow,
    filterText,
    hideClosed,
    closedTimeFilter,
    includeHierarchy,
    sortBy,
    sortOrder,
    zoom,
    collapsedIds,
    isInitializing,
  ]);

  // Save WBS panel width to startup.json with debouncing
  useEffect(() => {
    if (isSessionWindow) return;
    if (!hasProject) return;
    if (isInitializing.current) return;

    const saveTimeout = setTimeout(async () => {
      try {
        const win = getCurrentWindow();
        const position = await win.outerPosition();
        const size = await win.outerSize();
        const isMaximized = await win.isMaximized();

        await saveStartupState({
          window: {
            width: size.width,
            height: size.height,
            x: position.x,
            y: position.y,
            isMaximized,
          },
          filters: {
            filterText,
            hideClosed,
            closedTimeFilter,
            includeHierarchy,
          },
          sort: {
            sortBy,
            sortOrder,
          },
          ui: {
            zoom,
            collapsedIds: Array.from(collapsedIds),
            wbsPanelWidth: panelWidth,
          },
        });
      } catch (error) {
        console.error("Failed to save panel width:", error);
      }
    }, 500);

    return () => clearTimeout(saveTimeout);
  }, [
    isSessionWindow,
    panelWidth,
    hasProject,
    filterText,
    hideClosed,
    closedTimeFilter,
    includeHierarchy,
    sortBy,
    sortOrder,
    zoom,
    collapsedIds,
    isInitializing,
  ]);

  // WBS panel drag resize handler
  const handlePanelResizeStart = useCallback(
    (e: React.MouseEvent) => {
      const startX = e.clientX;
      const startWidth = panelWidthRef.current;

      const handleMouseMove = (moveEvent: MouseEvent) => {
        const delta = moveEvent.clientX - startX;
        const newWidth = Math.min(
          MAX_PANEL_WIDTH,
          Math.max(MIN_PANEL_WIDTH, startWidth + delta)
        );
        setPanelWidth(newWidth);
      };

      const handleMouseUp = () => {
        document.removeEventListener("mousemove", handleMouseMove);
        document.removeEventListener("mouseup", handleMouseUp);
      };

      document.addEventListener("mousemove", handleMouseMove);
      document.addEventListener("mouseup", handleMouseUp);
    },
    []
  );

  return {
    isDark,
    setIsDark,
    panelWidth,
    setPanelWidth,
    view,
    setView,
    handlePanelResizeStart,
  };
}
