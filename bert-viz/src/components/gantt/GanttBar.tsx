import { Flame } from "lucide-react";
import { cn } from "../../utils";
import type { Bead, GanttItem } from "../../api";

interface GanttBarProps {
  item: GanttItem;
  onClick: (bead: Bead) => void;
}

export const GanttBar = ({ item, onClick }: GanttBarProps) => {
  const { bead, isCritical, isBlocked, x, width } = item;
  const isEpic = bead.issue_type === 'epic';
  const isMilestone = bead.estimate === 0;

  const getStatusColor = () => {
    if (isEpic) return "bg-zinc-500 dark:bg-zinc-500 border-zinc-600 dark:border-zinc-400";
    if (isMilestone) return "bg-indigo-700 dark:bg-indigo-400 border-indigo-800 rotate-45";
    if (bead.status === 'closed') return "bg-indigo-600 dark:bg-indigo-500 border-indigo-700 opacity-50";
    if (isCritical) return "bg-rose-600 dark:bg-rose-500 border-rose-700 shadow-[0_0_12px_rgba(225,29,72,0.4)]";
    if (isBlocked) return "bg-amber-600 dark:bg-amber-500 border-amber-700";
    if (bead.status === 'in_progress') return "bg-emerald-600 dark:bg-emerald-500 border-emerald-700 shadow-[0_0_12px_rgba(16,185,129,0.3)]";
    return "bg-indigo-600 dark:bg-indigo-500 border-indigo-700";
  };

  return (
    <div 
      className="absolute h-full flex items-center group cursor-pointer"
      style={{ left: x, width: isMilestone ? 12 : width }}
      onClick={() => onClick(bead)}
    >
      <div className="flex-1 flex items-center h-full relative">
        {isEpic ? (
          <div className="w-full h-3 bg-zinc-300 dark:bg-zinc-700 relative rounded-full overflow-hidden border-2 border-zinc-400 dark:border-zinc-500">
             <div className="absolute left-0 top-0 bottom-0 w-2 bg-indigo-700 dark:bg-indigo-400" />
             <div className="absolute right-0 top-0 bottom-0 w-2 bg-indigo-700 dark:bg-indigo-400" />
          </div>
        ) : isMilestone ? (
          <div className={cn("w-3.5 h-3.5 border-2 shadow-md transition-all group-hover:scale-125", getStatusColor())} />
        ) : (
          <div className={cn(
            "h-6 rounded-md border-2 shadow-md transition-all w-full group-hover:shadow-lg group-hover:border-indigo-500",
            getStatusColor(),
            bead.status === 'closed' ? "opacity-40" : "opacity-100"
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
