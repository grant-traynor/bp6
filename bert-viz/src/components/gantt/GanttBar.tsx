import { Flame } from "lucide-react";
import { cn } from "../../utils";
import type { BeadNode, GanttItem } from "../../api";

interface GanttBarProps {
  item: GanttItem;
  onClick: (bead: BeadNode) => void;
  isSelected?: boolean;
}

export const GanttBar = ({ item, onClick, isSelected }: GanttBarProps) => {
  const { bead, isCritical, isBlocked, x, width } = item;
  const isSummary = bead.issueType === 'epic' || bead.issueType === 'feature';
  const isMilestone = bead.estimate === 0 && !isSummary;
  const isInProgress = bead.status === 'in_progress';

  const getStatusColor = () => {
    if (bead.status === 'closed') return "bg-[var(--status-done)] border-[var(--status-done)] opacity-40";
    if (isBlocked) return "bg-[var(--status-blocked)] border-[var(--status-blocked)]";
    if (bead.status === 'in_progress') return "bg-[var(--status-active)] border-[var(--status-active)] shadow-[0_0_12px_rgba(15,139,255,0.35)]";
    return "bg-[var(--status-open)] border-[var(--status-open)]";
  };

  const getSummaryColor = () => {
     if (bead.status === 'closed') return "bg-[var(--gantt-summary)] border-[var(--gantt-summary)] opacity-60";
     return "bg-[var(--gantt-summary)] border-[var(--gantt-summary)]";
  };

  const milestoneBase = "bg-[var(--background-primary)] border-[var(--gantt-milestone)]";

  return (
    <div
      className={cn(
        "absolute h-full flex items-center group cursor-pointer transition-transform duration-200",
        isSelected ? "z-20 scale-y-110" : "z-10 hover:z-20"
      )}
      style={{ left: x, width: isMilestone ? 12 : width, paddingLeft: isMilestone ? 0 : '8px' }}
      onClick={() => onClick(bead)}
    >
      <div className="flex-1 flex items-center h-full relative" style={{ paddingLeft: isMilestone ? 0 : '12px' }}>
        {isSummary ? (
          <div className={cn(
            "w-full h-2 relative rounded-sm border-b-2 transition-all",
            isSelected ? "shadow-[0_0_15px_rgba(59,163,255,0.45)] ring-2 ring-[rgba(59,163,255,0.35)]" : "group-hover:shadow-md",
            getSummaryColor()
          )}>
             <div className={cn("absolute left-0 -bottom-1 top-0 w-1.5 rounded-sm", getSummaryColor())} />
             <div className={cn("absolute right-0 -bottom-1 top-0 w-1.5 rounded-sm", getSummaryColor())} />
          </div>
        ) : isMilestone ? (
          <div className={cn(
            "w-3.5 h-3.5 border-2 shadow-md transition-all group-hover:scale-125 rotate-45",
            milestoneBase,
            isSelected ? "shadow-[0_0_15px_rgba(59,163,255,0.5)] ring-4 ring-[rgba(59,163,255,0.35)] scale-125" : "group-hover:border-[var(--accent-primary)]"
          )} />
        ) : (
          <div className={cn(
            "h-6 rounded-sm border-2 shadow-md transition-all w-full relative overflow-hidden",
            isSelected ? "ring-4 ring-[rgba(59,163,255,0.35)] border-[var(--accent-primary)] shadow-[var(--shadow-lg)] scale-[1.02]" : "group-hover:shadow-[var(--shadow-md)] group-hover:border-[var(--accent-primary)] group-hover:translate-y-[-1px]",
            getStatusColor()
          )}>
            {isInProgress && (
              <div className="absolute inset-0 gantt-progress" />
            )}
          </div>
        )}

        <div className={cn(
          "absolute left-full ml-6 flex items-center gap-2 whitespace-nowrap pointer-events-none z-10 transition-all",
          isSelected ? "opacity-100 scale-105 origin-left" : "opacity-80 group-hover:opacity-100 group-hover:translate-x-1"
        )}>
          <span className={cn(
            "text-base font-black tracking-tight transition-colors",
            isSelected ? "text-[var(--accent-primary)] drop-shadow-md" : 
            bead.status === 'closed' ? "text-[var(--text-muted)] italic font-bold" : "text-[var(--text-primary)] drop-shadow-sm"
          )}>{bead.title}</span>
          {isCritical && !isMilestone && <Flame size={14} className={cn("fill-current", isSelected ? "text-[var(--accent-primary)]" : "text-rose-500 dark:text-rose-400")} />}
        </div>
      </div>
    </div>
  );
};
