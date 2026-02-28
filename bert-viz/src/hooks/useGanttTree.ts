import { useState, useEffect, useCallback } from "react";
import type { BeadNode } from "../api";

export interface UseGanttTreeReturn {
  collapsedIds: Set<string>;
  setCollapsedIds: React.Dispatch<React.SetStateAction<Set<string>>>;
  zoom: number;
  setZoom: React.Dispatch<React.SetStateAction<number>>;
  toggleNode: (id: string, scrollContainerRef?: React.RefObject<HTMLDivElement | null>) => void;
  expandAll: () => void;
  collapseAll: (viewModelTree?: BeadNode[]) => void;
}

/**
 * Owns collapse state and zoom for the Gantt/WBS views.
 * useGanttLayout (bp6-gnze) takes these as inputs to compute the rendered layout.
 */
export function useGanttTree(): UseGanttTreeReturn {
  const [collapsedIds, setCollapsedIds] = useState<Set<string>>(() => {
    const saved = localStorage.getItem("collapsedIds");
    return saved ? new Set(JSON.parse(saved)) : new Set();
  });

  const [zoom, setZoom] = useState(1);

  // Persist collapsedIds to localStorage whenever they change
  useEffect(() => {
    localStorage.setItem("collapsedIds", JSON.stringify(Array.from(collapsedIds)));
  }, [collapsedIds]);

  const toggleNode = useCallback(
    (id: string, scrollContainerRef?: React.RefObject<HTMLDivElement | null>) => {
      // Track the element's visual position before toggling so we can re-adjust scroll
      let savedOffsetTop: number | null = null;
      if (scrollContainerRef?.current) {
        const element = document.querySelector(`[data-bead-id="${id}"]`);
        if (element) {
          const elementRect = element.getBoundingClientRect();
          const containerRect = scrollContainerRef.current.getBoundingClientRect();
          savedOffsetTop = elementRect.top - containerRect.top;
        }
      }

      setCollapsedIds((prev) => {
        const next = new Set(prev);
        if (next.has(id)) {
          next.delete(id);
        } else {
          next.add(id);
        }
        return next;
      });

      // After state update, restore the toggled node's visual position
      if (savedOffsetTop !== null && scrollContainerRef?.current) {
        const containerRef = scrollContainerRef.current;
        setTimeout(() => {
          const element = document.querySelector(`[data-bead-id="${id}"]`);
          if (element && containerRef) {
            const elementRect = element.getBoundingClientRect();
            const containerRect = containerRef.getBoundingClientRect();
            const currentOffsetTop = elementRect.top - containerRect.top;
            const scrollAdjustment = currentOffsetTop - savedOffsetTop!;
            containerRef.scrollTop += scrollAdjustment;
          }
        }, 0);
      }
    },
    []
  );

  const expandAll = useCallback(() => {
    setCollapsedIds(new Set());
  }, []);

  const collapseAll = useCallback((viewModelTree?: BeadNode[]) => {
    const allParentIds = new Set<string>();
    const findParents = (nodes: BeadNode[]) => {
      nodes.forEach((node) => {
        if (node.children.length > 0) {
          allParentIds.add(node.id);
          findParents(node.children);
        }
      });
    };
    if (viewModelTree) {
      findParents(viewModelTree);
    }
    setCollapsedIds(allParentIds);
  }, []);

  return {
    collapsedIds,
    setCollapsedIds,
    zoom,
    setZoom,
    toggleNode,
    expandAll,
    collapseAll,
  };
}
