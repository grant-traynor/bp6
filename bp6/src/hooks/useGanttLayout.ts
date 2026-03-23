import { useCallback, useMemo } from "react";
import React from "react";
import { type BeadNode, type ProjectViewModel } from "../api";

export interface GanttItem {
  bead: BeadNode;
  x: number;
  width: number;
  row: number;
  depth: number;
  isCritical: boolean;
  isBlocked: boolean;
}

export interface Connector {
  fromId: string;
  toId: string;
  fromX: number;
  fromY: number;
  toX: number;
  toY: number;
  isCritical: boolean;
}

const ROW_HEIGHT = 32;

export function useGanttLayout(
  viewModel: ProjectViewModel | null,
  zoom: number,
  _collapsedIds: Set<string>
): {
  beads: BeadNode[];
  ganttLayout: {
    items: GanttItem[];
    rowCount: number;
    rowDepths: number[];
    connectors: Connector[];
  };
  ganttGridColumns: React.ReactNode;
} {
  const flattenTree = useCallback((nodes: BeadNode[]): BeadNode[] => {
    const result: BeadNode[] = [];
    const traverse = (nodeList: BeadNode[]) => {
      nodeList.forEach(node => {
        result.push(node);
        if (node.children.length > 0) {
          traverse(node.children);
        }
      });
    };
    traverse(nodes);
    return result;
  }, []);

  const beads = useMemo(
    () => (viewModel ? flattenTree(viewModel.tree) : []),
    [viewModel, flattenTree]
  );

  const ganttLayout = useMemo(() => {
    if (!viewModel) {
      return { items: [], rowCount: 0, rowDepths: [], connectors: [] };
    }

    const items: GanttItem[] = [];
    const rowDepths: number[] = [];
    const idToItem = new Map<string, { x: number; width: number; row: number }>();
    let rowIndex = 0;

    // Traverse tree and build visible items with row numbers
    const traverse = (nodes: BeadNode[], depth: number = 0) => {
      nodes.forEach(node => {
        // Convert cell offset/count to pixels.
        // Task bars fill 72% of their cell span so connector lines have a clear
        // right-side channel. Rollup bars (epic/feature) span the full extent
        // of their children so the bar aligns exactly with the child range.
        const cellSize = 100 * zoom;
        const x = node.cellOffset * cellSize;
        const isSummary = node.issueType === 'epic' || node.issueType === 'feature';
        const width = node.cellCount * cellSize * (isSummary ? 1.0 : 0.72);

        const item: GanttItem = {
          bead: node,
          x,
          width,
          row: rowIndex,
          depth,
          isCritical: node.isCritical,
          isBlocked: node.isBlocked,
        };
        items.push(item);
        idToItem.set(node.id, { x, width, row: rowIndex });
        rowDepths.push(depth);
        rowIndex++;

        // Recurse to children if node is expanded
        if (node.isExpanded && node.children.length > 0) {
          traverse(node.children, depth + 1);
        }
      });
    };

    traverse(viewModel.tree);

    // Build connectors from "blocks" dependencies
    const connectors: Connector[] = [];

    items.forEach(item => {
      // Find "blocks" dependencies (other tasks block this task)
      // If item has {type: "blocks", depends_on_id: "B"}, it means B blocks item
      // So we draw: B → item (from blocker to blocked)
      const blocksDeps =
        item.bead.dependencies?.filter((d: { type: string }) => d.type === 'blocks') || [];

      blocksDeps.forEach((dep: { depends_on_id: string; type: string }) => {
        const blocker = idToItem.get(dep.depends_on_id);
        if (blocker) {
          // Account for the 20px left padding (8px + 12px) on bars
          const CONNECTOR_PADDING = 20;

          // Connector from right edge of BLOCKER to left edge of BLOCKED task
          const connector: Connector = {
            fromId: dep.depends_on_id,
            toId: item.bead.id,
            fromX: blocker.x + blocker.width,       // Right edge of blocker cell
            fromY: blocker.row * ROW_HEIGHT + ROW_HEIGHT / 2,
            toX: item.x + CONNECTOR_PADDING,        // Left edge of blocked bar (after padding)
            toY: item.row * ROW_HEIGHT + ROW_HEIGHT / 2,
            isCritical:
              (items.find(i => i.bead.id === dep.depends_on_id)?.isCritical &&
                item.isCritical) ||
              false,
          };
          connectors.push(connector);
        }
      });
    });

    return { items, rowCount: rowIndex, rowDepths, connectors };
  }, [viewModel, zoom]);

  const ganttGridColumns = useMemo(
    () =>
      Array.from({ length: 50 }).map((_, i) =>
        React.createElement("div", {
          key: i,
          className: "h-full border-r-2",
          style: { width: 100 * zoom, borderColor: "var(--gantt-gridline)" },
        })
      ),
    [zoom]
  );

  return { beads, ganttLayout, ganttGridColumns };
}
