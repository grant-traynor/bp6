import { Flame } from "lucide-react";
import { cn } from "../../utils";
import type { Bead, GanttItem } from "../../api";

interface GanttBarProps {
  item: GanttItem;
  onClick: (bead: Bead) => void;
}

export const GanttBar = ({ item, onClick }: GanttBarProps) => {
  const { bead, isCritical, isBlocked, x, width } = item;
  const isSummary = bead.issue_type === 'epic' || bead.issue_type === 'feature';
  const isMilestone = bead.estimate === 0 && !isSummary;

  const getStatusColor = () => {
    if (bead.status === 'closed') return "bg-[var(--status-done)] border-[var(--status-done)] opacity-40";
    if (isBlocked) return "bg-[var(--status-blocked)] border-[var(--status-blocked)]";
    if (bead.status === 'in_progress') return "bg-[var(--status-active)] border-[var(--status-active)] shadow-[0_0_12px_rgba(245,158,11,0.3)]";
    return "bg-[var(--status-open)] border-[var(--status-open)]";
  };

  const getSummaryColor = () => {
     if (bead.status === 'closed') return "bg-[var(--status-done)] border-[var(--status-done)] opacity-60";
     return "bg-[var(--text-muted)] border-[var(--text-primary)]";
  };

  return (
    <div 
      className="absolute h-full flex items-center group cursor-pointer"
      style={{ left: x, width: isMilestone ? 12 : width }}
      onClick={() => onClick(bead)}
    >
      <div className="flex-1 flex items-center h-full relative">
        {isSummary ? (
          <div className={cn(
            "w-full h-2 relative rounded-sm border-b-2",
            getSummaryColor()
          )}>
             <div className={cn("absolute left-0 -bottom-1 top-0 w-1.5 rounded-sm", getSummaryColor())} />
             <div className={cn("absolute right-0 -bottom-1 top-0 w-1.5 rounded-sm", getSummaryColor())} />
          </div>
        ) : isMilestone ? (
          <div className={cn("w-3.5 h-3.5 border-2 shadow-md transition-all group-hover:scale-125", getStatusColor())} />
        ) : (
          <div className={cn(
            "h-6 rounded-md border-2 shadow-md transition-all w-full group-hover:shadow-lg group-hover:border-indigo-500",
            getStatusColor()
          )} />
        )}

        <div className="absolute left-full ml-4 flex items-center gap-2 whitespace-nowrap pointer-events-none z-10 opacity-90 group-hover:opacity-100 transition-opacity">
          <span className={cn(
            "text-[13px] font-black tracking-tight",
            bead.status === 'closed' ? "text-[var(--text-muted)] line-through font-bold" : "text-[var(--text-primary)] drop-shadow-md"
          )}>{bead.title}</span>
          {isCritical && !isMilestone && <Flame size={14} className="text-rose-600 dark:text-rose-400 fill-current" />}
        </div>
      </div>
    </div>
  );
};
