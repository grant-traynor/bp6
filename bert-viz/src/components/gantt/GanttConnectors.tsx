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
}

export function GanttConnectors({ connectors, zoom, totalWidth, totalHeight }: GanttConnectorsProps) {
  return (
    <svg
      className="absolute inset-0 pointer-events-none"
      style={{ zIndex: 30 }}
      width={totalWidth}
      height={totalHeight}
    >
      {connectors.map((conn, idx) => {
        // Keep vertical segment in connector channel (first 20px of each cell)
        // Place it 10px into the channel immediately after the blocker
        const channelOffset = 10 * zoom;
        const verticalX = conn.fromX + channelOffset;

        const path = `M ${conn.fromX} ${conn.fromY} L ${verticalX} ${conn.fromY} L ${verticalX} ${conn.toY} L ${conn.toX} ${conn.toY}`;
        return (
          <path
            key={`${conn.fromId}-${conn.toId}-${idx}`}
            d={path}
            stroke={conn.isCritical ? "var(--gantt-connector-critical)" : "var(--gantt-connector)"}
            strokeWidth="3"
            fill="none"
            opacity="0.9"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        );
      })}
    </svg>
  );
}

export type { Connector };
