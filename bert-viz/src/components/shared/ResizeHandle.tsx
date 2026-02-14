import { cn } from "../../utils";

interface ResizeHandleProps {
  className?: string;
}

export const ResizeHandle = ({ className }: ResizeHandleProps) => {
  return (
    <div
      className={cn(
        "absolute right-0 top-0 bottom-0 cursor-col-resize bg-[var(--border-primary)] transition-colors hover:bg-[var(--accent-primary)] z-10",
        className
      )}
      style={{ width: '4px' }}
      title="Drag to resize"
    >
      {/* Visual indicator dots on hover */}
      <div className="absolute inset-0 flex flex-col items-center justify-center gap-1 opacity-0 hover:opacity-100 transition-opacity">
        <div className="w-0.5 h-0.5 bg-current rounded-full" />
        <div className="w-0.5 h-0.5 bg-current rounded-full" />
        <div className="w-0.5 h-0.5 bg-current rounded-full" />
      </div>
    </div>
  );
};
