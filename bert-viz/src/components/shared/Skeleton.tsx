import { cn } from "../../utils";

export const Skeleton = ({ className, style }: { className?: string; style?: React.CSSProperties }) => (
  <div className={cn("animate-pulse bg-[var(--background-tertiary)] rounded-md", className)} style={style} />
);

export const WBSSkeleton = () => (
  <div className="flex flex-col">
    {Array.from({ length: 15 }).map((_, i) => (
      <div key={i} className="h-[48px] flex items-center px-4 border-b border-[var(--border-primary)]/50">
        <div className="w-10 shrink-0 flex items-center justify-center">
            <Skeleton className="w-4 h-4" />
        </div>
        <div className="w-24 shrink-0 px-2 flex items-center h-full">
            <Skeleton className="w-16 h-5" />
        </div>
        <div className="flex-1 px-4 flex items-center gap-3">
            <Skeleton className="w-8 h-5" />
            <Skeleton className="h-4 w-2/3" />
        </div>
      </div>
    ))}
  </div>
);

export const GanttSkeleton = () => (
  <div className="absolute inset-0 flex flex-col">
    {Array.from({ length: 15 }).map((_, i) => (
      <div key={i} className="h-[48px] w-full border-b border-[var(--border-primary)]/30 relative">
        <Skeleton 
          className="absolute h-6 rounded-md opacity-20" 
          style={{ 
            left: `${20 + (i * 15) % 40}%`, 
            width: `${10 + (i * 7) % 30}%`,
            top: '11px'
          }} 
        />
      </div>
    ))}
  </div>
);
