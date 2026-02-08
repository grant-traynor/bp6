import { ChevronDown } from "lucide-react";
import { cn } from "../../../utils";

interface CollapsibleSectionProps {
  title: string;
  isCollapsed: boolean;
  onToggle: () => void;
  children: React.ReactNode;
  rightElement?: React.ReactNode;
}

export const CollapsibleSection = ({ 
  title, 
  isCollapsed, 
  onToggle, 
  children,
  rightElement 
}: CollapsibleSectionProps) => {
  return (
    <div className="border-b-[var(--border-thick)] border-[var(--border-primary)] last:border-0">
      <div 
        onClick={onToggle}
        className="flex items-center justify-between w-full group py-4 px-6 cursor-pointer hover:bg-[var(--background-tertiary)] transition-colors"
      >
        <h3 className="text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.3em] flex items-center gap-2">
          <ChevronDown 
            size={14} 
            className={cn(
              "transition-transform duration-300 text-indigo-500",
              isCollapsed ? "-rotate-90" : "rotate-0"
            )} 
          />
          {title}
        </h3>
        {rightElement}
      </div>
      <div className={cn(
        "transition-all duration-300 overflow-hidden",
        isCollapsed ? "max-h-0 opacity-0" : "max-h-[2000px] opacity-100 p-6 pt-0"
      )}>
        {children}
      </div>
    </div>
  );
};
