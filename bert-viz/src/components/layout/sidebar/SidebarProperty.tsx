import { LucideIcon } from "lucide-react";
import { cn } from "../../../utils";

interface SidebarPropertyProps {
  label: string;
  value: React.ReactNode;
  icon: LucideIcon;
  iconColor?: string;
  className?: string;
}

export const SidebarProperty = ({ 
  label, 
  value, 
  icon: Icon, 
  iconColor = "text-[var(--text-muted)]",
  className 
}: SidebarPropertyProps) => {
  return (
    <div className={cn("flex items-center gap-4 bg-[var(--background-secondary)] p-4 rounded-2xl border-2 border-[var(--border-primary)] shadow-sm hover:shadow-md transition-all group", className)}>
      <div className={cn("p-2 rounded-xl bg-[var(--background-tertiary)] border-2 border-[var(--border-primary)] shadow-sm transition-transform group-hover:scale-105", iconColor)}>
        <Icon size={18} strokeWidth={2.5} />
      </div>
      <div className="flex flex-col">
        <span className="text-[10px] font-black text-[var(--text-muted)] uppercase tracking-[0.2em] mb-0.5">{label}</span>
        <div className="text-sm font-black text-[var(--text-primary)]">{value}</div>
      </div>
    </div>
  );
};
