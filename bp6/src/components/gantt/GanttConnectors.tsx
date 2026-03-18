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
}

export function GanttConnectors({ connectors, zoom, totalWidth, totalHeight, chainIds }: GanttConnectorsProps) {
  const hasChain = chainIds && chainIds.size > 0;

  return (
    <svg
      className="absolute inset-0 pointer-events-none"
      style={{ zIndex: 30 }}
      width={totalWidth}
      height={totalHeight}
    >
      {connectors.map((conn, idx) => {
        const channelOffset = 10 * zoom;
        const verticalX = conn.fromX + channelOffset;
        const path = `M ${conn.fromX} ${conn.fromY} L ${verticalX} ${conn.fromY} L ${verticalX} ${conn.toY} L ${conn.toX} ${conn.toY}`;

        const isChain = hasChain && chainIds!.has(conn.fromId) && chainIds!.has(conn.toId);

        return (
          <path
            key={`${conn.fromId}-${conn.toId}-${idx}`}
            d={path}
            stroke={
              isChain
                ? "#f97316"
                : conn.isCritical
                ? "var(--gantt-connector-critical)"
                : "var(--gantt-connector)"
            }
            strokeWidth={isChain ? 4 : 3}
            fill="none"
            opacity={hasChain && !isChain ? 0.2 : 0.9}
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        );
      })}
    </svg>
  );
}

export type { Connector };
