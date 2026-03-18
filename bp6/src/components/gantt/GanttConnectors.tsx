const THREAD_OFFSET = 3; // px between parallel threads on the same edge

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

export function GanttConnectors({ connectors, zoom, totalWidth, totalHeight, chainIds, edgeThreads }: GanttConnectorsProps) {
  const hasChain = chainIds && chainIds.size > 0;

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
        const channelOffset = 10 * zoom;
        const verticalX = conn.fromX + channelOffset;
        const isChain = hasChain && chainIds!.has(conn.fromId) && chainIds!.has(conn.toId);
        const opacity = hasChain && !isChain ? 0.12 : 0.85;

        if (threads.length === 0) {
          // Connector not part of any thread (shouldn't normally happen) — render plain.
          const p = `M ${conn.fromX} ${conn.fromY} L ${verticalX} ${conn.fromY} L ${verticalX} ${conn.toY} L ${conn.toX} ${conn.toY}`;
          return [(
            <path
              key={`${conn.fromId}-${conn.toId}-${idx}-plain`}
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

        // Render one path per thread, offset vertically so overlapping strands are visible.
        return threads.map((color, ti) => {
          const dy = (ti - (threads.length - 1) / 2) * THREAD_OFFSET;
          const fy = conn.fromY + dy;
          const ty = conn.toY + dy;
          const p = `M ${conn.fromX} ${fy} L ${verticalX} ${fy} L ${verticalX} ${ty} L ${conn.toX} ${ty}`;
          return (
            <path
              key={`${conn.fromId}-${conn.toId}-${idx}-t${ti}`}
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
