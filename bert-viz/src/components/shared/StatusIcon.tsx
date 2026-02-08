import { CheckCircle2, Clock, Circle } from "lucide-react";
import { cn } from "../../utils";

export const StatusIcon = ({ status, size = 14, className }: { status: string; size?: number; className?: string }) => {
  switch (status) {
    case 'closed': return <CheckCircle2 size={size} className={cn("text-emerald-600 dark:text-emerald-400 stroke-[2.5]", className)} />;
    case 'in_progress': return <Clock size={size} className={cn("text-amber-600 dark:text-amber-400 stroke-[2.5]", className)} />;
    default: return <Circle size={size} className={cn("text-[var(--text-secondary)] stroke-[2]", className)} />;
  }
};
