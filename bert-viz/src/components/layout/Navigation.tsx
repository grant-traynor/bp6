import { TableProperties, Settings, GanttChart } from "lucide-react";
import { cn } from "../../utils";

export type ViewType = 'gantt' | 'list';

interface NavigationProps {
  currentView: ViewType;
  onViewChange: (view: ViewType) => void;
}

export const Navigation = ({ currentView, onViewChange }: NavigationProps) => (
  <nav className="w-16 flex flex-col items-center py-6 border-r border-[var(--border-primary)] bg-[var(--background-secondary)] z-30 shadow-xl">
    <div className="flex flex-col gap-6 flex-1">
      <button 
        onClick={() => onViewChange('gantt')}
        className={cn(
          "p-3 rounded-xl transition-all border-2 shadow-md hover:shadow-lg active:scale-95",
          currentView === 'gantt' 
            ? "bg-indigo-600 text-white border-indigo-700 shadow-indigo-500/20" 
            : "bg-[var(--background-primary)] text-[var(--text-muted)] border-[var(--border-primary)] hover:text-indigo-600"
        )}
        title="Gantt View"
      >
        <GanttChart size={24} strokeWidth={2.5} />
      </button>

      <button 
        onClick={() => onViewChange('list')}
        className={cn(
          "p-3 rounded-xl transition-all border-2 shadow-md hover:shadow-lg active:scale-95",
          currentView === 'list' 
            ? "bg-indigo-600 text-white border-indigo-700 shadow-indigo-500/20" 
            : "bg-[var(--background-primary)] text-[var(--text-muted)] border-[var(--border-primary)] hover:text-indigo-600"
        )}
        title="List View"
      >
        <TableProperties size={24} strokeWidth={2.5} />
      </button>

      <div className="h-px w-8 bg-[var(--border-primary)] mx-auto" />
    </div>
    <div className="mt-auto border-t border-[var(--border-primary)] pt-4">
      <button className="p-3 text-[var(--text-primary)] hover:text-indigo-600 transition-colors">
        <Settings size={22} strokeWidth={2.5} />
      </button>
    </div>
  </nav>
);
