import { CheckCircle2, Clock, Circle } from "lucide-react";
import { cn } from "../../utils";

export const StatusIcon = ({ status, isBlocked, size = 14, className }: { status: string; isBlocked?: boolean; size?: number; className?: string }) => {
  if (status === 'closed') {
    return <CheckCircle2 size={size} className={cn("text-[var(--status-done)] stroke-[2.5]", className)} />;
  }
  if (isBlocked) {
    return <Circle size={size} className={cn("text-[var(--status-blocked)] fill-[var(--status-blocked)]/20 stroke-[2.5]", className)} />;
  }
  if (status === 'in_progress') {
    return <Clock size={size} className={cn("text-[var(--status-active)] stroke-[2.5]", className)} />;
  }
  return <Circle size={size} className={cn("text-[var(--status-open)] stroke-[2.5]", className)} />;
};
