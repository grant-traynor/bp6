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
            isSelected ? "shadow-[0_0_15px_rgba(79,70,229,0.5)] ring-2 ring-indigo-500/30" : "group-hover:shadow-md",
            getSummaryColor()
          )}>
             <div className={cn("absolute left-0 -bottom-1 top-0 w-1.5 rounded-sm", getSummaryColor())} />
             <div className={cn("absolute right-0 -bottom-1 top-0 w-1.5 rounded-sm", getSummaryColor())} />
          </div>
        ) : isMilestone ? (
          <div className={cn(
            "w-3.5 h-3.5 border-2 shadow-md transition-all group-hover:scale-125 rotate-45",
            isSelected ? "shadow-[0_0_15px_rgba(79,70,229,0.6)] ring-4 ring-indigo-500/40 scale-125 border-indigo-500" : "group-hover:border-indigo-500",
            getStatusColor()
          )} />
        ) : (
          <div className={cn(
            "h-6 rounded-md border-2 shadow-md transition-all w-full",
            isSelected ? "ring-4 ring-indigo-500/40 border-indigo-500 shadow-[var(--shadow-lg)] scale-[1.02]" : "group-hover:shadow-[var(--shadow-md)] group-hover:border-indigo-500 group-hover:translate-y-[-1px]",
            getStatusColor()
          )} />
        )}

        <div className={cn(
          "absolute left-full ml-6 flex items-center gap-2 whitespace-nowrap pointer-events-none z-10 transition-all",
          isSelected ? "opacity-100 scale-105 origin-left" : "opacity-80 group-hover:opacity-100 group-hover:translate-x-1"
        )}>
          <span className={cn(
            "text-base font-black tracking-tight transition-colors",
            isSelected ? "text-indigo-700 dark:text-indigo-300 drop-shadow-md" : 
            bead.status === 'closed' ? "text-[var(--text-muted)] italic font-bold" : "text-[var(--text-primary)] drop-shadow-sm"
          )}>{bead.title}</span>
          {isCritical && !isMilestone && <Flame size={14} className={cn("fill-current", isSelected ? "text-indigo-600 dark:text-indigo-400" : "text-rose-600 dark:text-rose-400")} />}
        </div>
      </div>
    </div>
  );
};
