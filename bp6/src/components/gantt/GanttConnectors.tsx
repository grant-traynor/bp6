import { useMemo } from "react";

// Vertical spacing between parallel thread strands on the same edge.
const THREAD_DY = 3; // px

// Horizontal spacing between connector lanes sharing the same departure/arrival X.
const LANE_DX = 4; // px

interface Connector {
  fromId: string;
  toId: string;
  fromX: number;
  fromY: number;
  toX: number;
  toY: number;
  isCritical: boolean;
}

interface GanttConnectorsProps {
  connectors: Connector[];
  zoom: number;
  totalWidth: number;
  totalHeight: number;
  chainIds?: Set<string>;
  edgeThreads: Map<string, string[]>;
}

// For each connector, compute the horizontal offset (dx) applied to its
// vertical routing channel so that connectors fanning out from the same
// source bead (or converging on the same target bead) are visually separated.
function computeLaneOffsets(connectors: Connector[]): Map<string, number> {
  const offsets = new Map<string, number>(); // `${fromId}-${toId}` -> dx

  // --- Fan-out: group by fromId, offset by departure column ---
  const byFrom = new Map<string, Connector[]>();
  for (const conn of connectors) {
    if (!byFrom.has(conn.fromId)) byFrom.set(conn.fromId, []);
    byFrom.get(conn.fromId)!.push(conn);
  }
  for (const group of byFrom.values()) {
    if (group.length <= 1) continue;
    // Sort by destination row so lanes flow naturally top-to-bottom.
    const sorted = [...group].sort((a, b) => a.toY - b.toY);
    const n = sorted.length;
    sorted.forEach((conn, i) => {
      const key = `${conn.fromId}-${conn.toId}`;
      const dx = (i - (n - 1) / 2) * LANE_DX;
      offsets.set(key, (offsets.get(key) ?? 0) + dx);
    });
  }

  // --- Fan-in: group by toId, offset by arrival column ---
  // Multiple predecessors arriving at the same task also need separation
  // so their horizontal arrival segments don't stack.
  const byTo = new Map<string, Connector[]>();
  for (const conn of connectors) {
    if (!byTo.has(conn.toId)) byTo.set(conn.toId, []);
    byTo.get(conn.toId)!.push(conn);
  }
  for (const group of byTo.values()) {
    if (group.length <= 1) continue;
    const sorted = [...group].sort((a, b) => a.fromY - b.fromY);
    const n = sorted.length;
    sorted.forEach((conn, i) => {
      const key = `${conn.fromId}-${conn.toId}`;
      // Blend arrival offset with any existing departure offset (average).
      const dx = (i - (n - 1) / 2) * LANE_DX;
      offsets.set(key, ((offsets.get(key) ?? 0) + dx) / 2);
    });
  }

  return offsets;
}

export function GanttConnectors({ connectors, zoom, totalWidth, totalHeight, chainIds, edgeThreads }: GanttConnectorsProps) {
  const hasChain = chainIds && chainIds.size > 0;

  const laneOffsets = useMemo(() => computeLaneOffsets(connectors), [connectors]);

  return (
    <svg
      className="absolute inset-0 pointer-events-none"
      style={{ zIndex: 30 }}
      width={totalWidth}
      height={totalHeight}
    >
      {connectors.flatMap((conn, idx) => {
        const edgeKey = `${conn.fromId}-${conn.toId}`;
        const threads = edgeThreads.get(edgeKey) ?? [];
        const isChain = hasChain && chainIds!.has(conn.fromId) && chainIds!.has(conn.toId);
        const opacity = hasChain && !isChain ? 0.12 : 0.85;

        // Horizontal lane offset for this connector's vertical channel.
        const dx = laneOffsets.get(edgeKey) ?? 0;
        const baseChannelOffset = 10 * zoom;
        const verticalX = conn.fromX + baseChannelOffset + dx;

        if (threads.length === 0) {
          const p = `M ${conn.fromX} ${conn.fromY} L ${verticalX} ${conn.fromY} L ${verticalX} ${conn.toY} L ${conn.toX} ${conn.toY}`;
          return [(
            <path
              key={`${edgeKey}-${idx}-plain`}
              d={p}
              stroke={conn.isCritical ? "var(--gantt-connector-critical)" : "var(--gantt-connector)"}
              strokeWidth={3}
              fill="none"
              opacity={opacity}
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          )];
        }

        // One path per thread colour, each offset vertically by THREAD_DY
        // so that shared edges (two independent strands, same fromId→toId) are
        // both legible rather than painted over each other.
        return threads.map((color, ti) => {
          const dy = (ti - (threads.length - 1) / 2) * THREAD_DY;
          const fy = conn.fromY + dy;
          const ty = conn.toY + dy;
          const p = `M ${conn.fromX} ${fy} L ${verticalX} ${fy} L ${verticalX} ${ty} L ${conn.toX} ${ty}`;
          return (
            <path
              key={`${edgeKey}-${idx}-t${ti}`}
              d={p}
              stroke={color}
              strokeWidth={threads.length > 1 ? 2.5 : 3}
              fill="none"
              opacity={opacity}
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          );
        });
      })}
    </svg>
  );
}

export type { Connector };
