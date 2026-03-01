import { BucketDistribution } from "../../api";

interface GanttStateHeaderProps {
  distributions: BucketDistribution[];
  zoom: number;
}

export const GanttStateHeader = ({ distributions, zoom }: GanttStateHeaderProps) => {
  return (
    <div className="flex h-10 bg-[var(--background-tertiary)]">
      {distributions.map((dist, i) => (
        <div 
          key={i} 
          className="h-full border-r-2 border-[var(--border-primary)]/50 flex flex-col justify-center px-1 overflow-hidden" 
          style={{ width: 100 * zoom, minWidth: 100 * zoom }}
        >
          <div className="flex flex-col gap-0.5 px-1.5 translate-y-[1px]">
            <div className="flex items-center gap-4">
              {dist.open > 0 && (
                <div className="flex items-center gap-1 min-w-0">
                  <div className="w-1.5 h-1.5 rounded-full bg-[var(--status-open)] shrink-0" />
                  <span className="text-[10px] font-black text-[var(--status-open)] leading-none">{dist.open}</span>
                </div>
              )}
              {dist.inProgress > 0 && (
                <div className="flex items-center gap-1 min-w-0">
                  <div className="w-1.5 h-1.5 rounded-full bg-[var(--status-active)] shrink-0" />
                  <span className="text-[10px] font-black text-[var(--status-active)] leading-none">{dist.inProgress}</span>
                </div>
              )}
              {dist.blocked > 0 && (
                <div className="flex items-center gap-1 min-w-0">
                  <div className="w-1.5 h-1.5 rounded-full bg-[var(--status-blocked)] shrink-0" />
                  <span className="text-[10px] font-black text-[var(--status-blocked)] leading-none">{dist.blocked}</span>
                </div>
              )}
              {dist.closed > 0 && (
                <div className="flex items-center gap-1 min-w-0">
                  <div className="w-1.5 h-1.5 rounded-full bg-[var(--status-done)] shrink-0" />
                  <span className="text-[10px] font-black text-[var(--status-done)] leading-none">{dist.closed}</span>
                </div>
              )}
            </div>
             {dist.open === 0 && dist.inProgress === 0 && dist.blocked === 0 && dist.closed === 0 && (
              <span className="text-[10px] font-black text-[var(--text-muted)] opacity-20 text-center">-</span>
            )}
          </div>
        </div>
      ))}
    </div>
  );
};
