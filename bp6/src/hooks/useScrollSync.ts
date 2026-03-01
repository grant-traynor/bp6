import { useRef, useEffect } from "react";

export interface UseScrollSyncReturn {
  wbsScrollRef: React.RefObject<HTMLDivElement | null>;
  ganttScrollRef: React.RefObject<HTMLDivElement | null>;
  ganttHeaderScrollRef: React.RefObject<HTMLDivElement | null>;
  activeScrollSourceRef: React.MutableRefObject<HTMLDivElement | null>;
  handleWBSScroll: (e: React.UIEvent<HTMLDivElement>) => void;
  handleGanttScroll: (e: React.UIEvent<HTMLDivElement>) => void;
  handleGanttHeaderScroll: (e: React.UIEvent<HTMLDivElement>) => void;
  handleMouseEnter: (e: React.MouseEvent<HTMLDivElement>) => void;
}

/**
 * Manages synchronized horizontal scrolling between WBS and Gantt panes,
 * and restores scroll position from localStorage when loading completes.
 *
 * The three scroll containers are:
 *   wbsScrollRef        - WBS tree list (vertical scroll only)
 *   ganttScrollRef      - Gantt body (horizontal + vertical scroll)
 *   ganttHeaderScrollRef - Gantt header (horizontal scroll only)
 */
export function useScrollSync(loading: boolean): UseScrollSyncReturn {
  const wbsScrollRef = useRef<HTMLDivElement | null>(null);
  const ganttScrollRef = useRef<HTMLDivElement | null>(null);
  const ganttHeaderScrollRef = useRef<HTMLDivElement | null>(null);
  const activeScrollSourceRef = useRef<HTMLDivElement | null>(null);
  const scrollRAF = useRef<number | null>(null);

  // Restore scroll position after initial load completes
  useEffect(() => {
    if (!loading) {
      const savedScroll = localStorage.getItem("scrollTop");
      if (savedScroll) {
        const top = parseInt(savedScroll, 10);
        if (wbsScrollRef.current) wbsScrollRef.current.scrollTop = top;
        if (ganttScrollRef.current) ganttScrollRef.current.scrollTop = top;
      }
    }
  }, [loading]);

  const handleWBSScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const target = e.currentTarget;
    if (activeScrollSourceRef.current && activeScrollSourceRef.current !== target) return;

    // Sync vertical scroll to Gantt body
    if (ganttScrollRef.current) {
      ganttScrollRef.current.scrollTop = target.scrollTop;
    }

    localStorage.setItem("scrollTop", target.scrollTop.toString());
  };

  const handleGanttScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const target = e.currentTarget;
    if (activeScrollSourceRef.current && activeScrollSourceRef.current !== target) return;

    // Sync vertical scroll back to WBS
    if (wbsScrollRef.current) {
      wbsScrollRef.current.scrollTop = target.scrollTop;
    }
    // Sync horizontal scroll to Gantt header
    if (ganttHeaderScrollRef.current) {
      ganttHeaderScrollRef.current.scrollLeft = target.scrollLeft;
    }

    localStorage.setItem("scrollTop", target.scrollTop.toString());
  };

  const handleGanttHeaderScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const target = e.currentTarget;
    if (activeScrollSourceRef.current && activeScrollSourceRef.current !== target) return;

    // Sync horizontal scroll to Gantt body
    if (ganttScrollRef.current) {
      ganttScrollRef.current.scrollLeft = target.scrollLeft;
    }
  };

  const handleMouseEnter = (e: React.MouseEvent<HTMLDivElement>) => {
    activeScrollSourceRef.current = e.currentTarget;
  };

  // Cleanup any pending RAF on unmount
  useEffect(() => {
    return () => {
      if (scrollRAF.current !== null) {
        cancelAnimationFrame(scrollRAF.current);
      }
    };
  }, []);

  return {
    wbsScrollRef,
    ganttScrollRef,
    ganttHeaderScrollRef,
    activeScrollSourceRef,
    handleWBSScroll,
    handleGanttScroll,
    handleGanttHeaderScroll,
    handleMouseEnter,
  };
}
