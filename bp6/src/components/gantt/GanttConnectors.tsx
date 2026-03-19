export interface ThreadInfo {
  color: string;
  /** Global index of this source thread. Determines its fixed (dx, dy) lane. */
  index: number;
}

const LANE_DX = 1.5; // px — horizontal separation between thread vertical channels
const LANE_DY = 1.5; // px — vertical separation between thread horizontal runs

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
  edgeThreads: Map<string, ThreadInfo[]>;
  totalThreads: number;
}

export function GanttConnectors({
  connectors,
  zoom,
  totalWidth,
  totalHeight,
  chainIds,
  edgeThreads,
  totalThreads,
}: GanttConnectorsProps) {
  const hasChain = chainIds && chainIds.size > 0;

  // Each source thread gets a fixed global (dx, dy) offset it holds for every
  // segment it passes through.  Threads from the same source share one lane so
  // they merge into one visible line.  Threads from different sources each
  // occupy a distinct lane.
  //
  // dx is always NON-NEGATIVE: the vertical channel is shifted right of the
  // base channel, never left.  A negative dx would make the departure stub go
  // right-to-left which looks broken.  dy is centred for visual balance.
  const threadOffset = (index: number): { dx: number; dy: number } => {
    const centre = (totalThreads - 1) / 2;
    return {
      dx: index * LANE_DX,                    // 0, 3, 6, 9 … — always rightward
      dy: (index - centre) * LANE_DY,         // centred: negative above, positive below
    };
  };

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
        const opacity = hasChain && !isChain ? 0.35 : 0.85;
        const baseChannelOffset = 10 * zoom;

        if (threads.length === 0) {
          // Connector not assigned to any thread — render plain fallback.
          const vx = conn.fromX + baseChannelOffset;
          const p  = `M ${conn.fromX} ${conn.fromY} L ${vx} ${conn.fromY} L ${vx} ${conn.toY} L ${conn.toX} ${conn.toY}`;
          return [(
            <path
              key={`${edgeKey}-${idx}-plain`}
              d={p}
              stroke={conn.isCritical ? "var(--gantt-connector-critical)" : "var(--gantt-connector)"}
              strokeWidth={1.5}
              fill="none"
              opacity={opacity}
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          )];
        }

        // Render one path per thread.  Each thread uses its globally-fixed
        // (dx, dy) offset so the same strand holds one consistent lane across
        // every connector segment, making the flow easy to follow visually.
        return threads.map((thread) => {
          const { dx, dy } = threadOffset(thread.index);
          const vx = conn.fromX + baseChannelOffset + dx;
          const fy = conn.fromY + dy;
          const ty = conn.toY + dy;
          const p  = `M ${conn.fromX} ${fy} L ${vx} ${fy} L ${vx} ${ty} L ${conn.toX} ${ty}`;
          return (
            <path
              key={`${edgeKey}-${idx}-th${thread.index}`}
              d={p}
              stroke={thread.color}
              strokeWidth={1.5}
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
