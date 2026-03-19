import { useState, useEffect, useRef, useCallback, startTransition } from "react";
import {
  fetchProjectViewModel,
  saveStartupState,
  type ProjectViewModel,
} from "../api";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useSessionStore } from "../stores/sessionStore";
import type { ClosedTimeFilter } from "./useAppInitialization";

export type { ClosedTimeFilter };

export interface UseFilterStateReturn {
  filterText: string;
  setFilterText: React.Dispatch<React.SetStateAction<string>>;
  hideClosed: boolean;
  setHideClosed: React.Dispatch<React.SetStateAction<boolean>>;
  closedTimeFilter: ClosedTimeFilter;
  setClosedTimeFilter: React.Dispatch<React.SetStateAction<ClosedTimeFilter>>;
  includeHierarchy: boolean;
  setIncludeHierarchy: React.Dispatch<React.SetStateAction<boolean>>;
  sortBy: "priority" | "title" | "type" | "id" | "none";
  setSortBy: React.Dispatch<
    React.SetStateAction<"priority" | "title" | "type" | "id" | "none">
  >;
  sortOrder: "asc" | "desc" | "none";
  setSortOrder: React.Dispatch<
    React.SetStateAction<"asc" | "desc" | "none">
  >;
  loading: boolean;
  setLoading: React.Dispatch<React.SetStateAction<boolean>>;
  processingData: boolean;
  viewModel: ProjectViewModel | null;
  handleHeaderClick: (column: "priority" | "title" | "type" | "id") => void;
  // Trigger a refetch manually (called by loadData)
  incrementRefetchTrigger: () => void;
}

export interface UseFilterStateInput {
  isReady: boolean;
  currentProjectPath: string;
  // Extra state to write back (panelWidth, zoom, collapsedIds used in saveStartupState)
  zoom: number;
  collapsedIds: Set<string>;
  panelWidth: number;
  hasProject: boolean;
}

/**
 * Owns all filter/sort state and the data fetching effect.
 * Uses a refetchTrigger counter to allow external callers (loadData) to force a refetch.
 */
export function useFilterState(input: UseFilterStateInput): UseFilterStateReturn {
  const { isReady, currentProjectPath, zoom, collapsedIds, panelWidth, hasProject } = input;

  const [filterText, setFilterText] = useState("");
  const [hideClosed, setHideClosed] = useState(false);
  const [closedTimeFilter, setClosedTimeFilter] = useState<ClosedTimeFilter>(
    () => {
      const saved = localStorage.getItem("closedTimeFilter");
      return (saved as ClosedTimeFilter) || "all";
    }
  );
  const [includeHierarchy, setIncludeHierarchy] = useState(true);
  const [sortBy, setSortBy] = useState<
    "priority" | "title" | "type" | "id" | "none"
  >("none");
  const [sortOrder, setSortOrder] = useState<"asc" | "desc" | "none">("none");
  const [loading, setLoading] = useState(true);
  const [processingData, setProcessingData] = useState(false);
  const [viewModel, setViewModel] = useState<ProjectViewModel | null>(null);
  const [refetchTrigger, setRefetchTrigger] = useState(0);

  const isInitializingRef = useRef(true);

  // Mark initialization complete once isReady
  useEffect(() => {
    if (isReady) {
      isInitializingRef.current = false;
    }
  }, [isReady]);

  // Persist closedTimeFilter to localStorage
  useEffect(() => {
    localStorage.setItem("closedTimeFilter", closedTimeFilter);
  }, [closedTimeFilter]);

  // Fetch view model when filters/sort/trigger change (gated by isReady)
  useEffect(() => {
    if (!isReady) return;

    let cancelled = false;
    const startTime = performance.now();
    setProcessingData(true);

    fetchProjectViewModel({
      filter_text: filterText,
      hide_closed: hideClosed,
      closed_time_filter: closedTimeFilter,
      include_hierarchy: includeHierarchy,
      zoom: zoom,
      collapsed_ids: Array.from(collapsedIds),
      sort_by: sortBy,
      sort_order: sortOrder,
    })
      .then((data) => {
        if (cancelled) return;
        console.log(`Frontend: IPC call took ${(performance.now() - startTime).toFixed(2)}ms`);
        startTransition(() => setViewModel(data));
        setProcessingData(false);
        useSessionStore.getState().refreshSessions();
      })
      .catch((error) => {
        if (cancelled) return;
        console.error("Failed to fetch view model:", error);
        setProcessingData(false);
      });

    return () => { cancelled = true; };
  }, [
    isReady,
    filterText,
    zoom,
    hideClosed,
    includeHierarchy,
    closedTimeFilter,
    collapsedIds,
    currentProjectPath,
    refetchTrigger,
    sortBy,
    sortOrder,
  ]);

  // Save filter and sort state to startup.json (debounced when filters change)
  useEffect(() => {
    if (!hasProject) return;
    if (isInitializingRef.current) return;

    const saveState = async () => {
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
        console.error("Failed to save startup state:", error);
      }
    };

    saveState();
  }, [hideClosed, closedTimeFilter, includeHierarchy, sortBy, sortOrder, filterText, zoom, collapsedIds, hasProject]);

  const handleHeaderClick = (column: "priority" | "title" | "type" | "id") => {
    if (sortBy === column) {
      if (sortOrder === "asc") setSortOrder("desc");
      else if (sortOrder === "desc") {
        setSortOrder("none");
        setSortBy("none");
      } else {
        setSortOrder("asc");
      }
    } else {
      setSortBy(column);
      setSortOrder("asc");
    }
  };

  // Stable across renders — setRefetchTrigger from useState never changes identity.
  // Must be stable so useAppInitialization's useEffect dep array doesn't cause it
  // to teardown and re-run (which drops the beads-updated listener permanently).
  const incrementRefetchTrigger = useCallback(() => {
    setRefetchTrigger((prev) => prev + 1);
  }, []);

  return {
    filterText,
    setFilterText,
    hideClosed,
    setHideClosed,
    closedTimeFilter,
    setClosedTimeFilter,
    includeHierarchy,
    setIncludeHierarchy,
    sortBy,
    setSortBy,
    sortOrder,
    setSortOrder,
    loading,
    setLoading,
    processingData,
    viewModel,
    handleHeaderClick,
    incrementRefetchTrigger,
  };
}
