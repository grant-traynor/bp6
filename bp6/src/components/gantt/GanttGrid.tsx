import { useMemo } from "react";

interface GanttGridProps {
  zoom: number;
  rowDepths: number[];
}

const ROW_HEIGHT = 32;

export function GanttGrid({ zoom, rowDepths }: GanttGridProps) {
  const ganttGridColumns = useMemo(() =>
    Array.from({ length: 50 }).map((_, i) => (
      <div
        key={i}
        className="h-full border-r-2"
        style={{ width: 100 * zoom, borderColor: 'var(--gantt-gridline)' }}
      />
    )), [zoom]);

  return (
    <div className="absolute inset-0 pointer-events-none">
      {rowDepths.map((depth, i) => (
        <div
          key={i}
          className="w-full border-b-2"
          style={{
            height: `${ROW_HEIGHT}px`,
            backgroundColor: `var(--level-${Math.min(depth, 4)})`,
            borderColor: 'var(--gantt-gridline)'
          }}
        />
      ))}
      <div className="absolute inset-0 flex">
        {ganttGridColumns}
      </div>
    </div>
  );
}

export { ROW_HEIGHT };
