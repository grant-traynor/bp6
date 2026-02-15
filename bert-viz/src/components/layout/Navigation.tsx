import { TableProperties, Settings, GanttChart, User } from "lucide-react";
import { cn } from "../../utils";

export type ViewType = 'gantt' | 'list';

interface NavigationProps {
  currentView: ViewType;
  onViewChange: (view: ViewType) => void;
  onOpenChat: (persona: string, task?: string, beadId?: string, role?: string) => void;
}

export const Navigation = ({ currentView, onViewChange, onOpenChat }: NavigationProps) => (
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

      {/* Persona Buttons Section */}
      <div className="flex flex-col gap-3 items-center">
        <button
          onClick={() => onOpenChat('product-manager', undefined, undefined)}
          className="w-full px-2 py-2.5 rounded-xl transition-all border-2 shadow-md hover:shadow-lg active:scale-95 bg-[var(--background-primary)] text-indigo-600 dark:text-indigo-400 border-indigo-500/30 hover:bg-indigo-500/10 flex flex-col items-center gap-0.5"
          title="Product Manager"
        >
          <span className="text-xl leading-none">🤖</span>
          <span className="text-[8px] font-black uppercase tracking-wider">PM</span>
        </button>

        <button
          onClick={() => onOpenChat('qa-engineer', 'fix_dependencies', undefined)}
          className="w-full px-2 py-2.5 rounded-xl transition-all border-2 shadow-md hover:shadow-lg active:scale-95 bg-[var(--background-primary)] text-emerald-600 dark:text-emerald-400 border-emerald-500/30 hover:bg-emerald-500/10 flex flex-col items-center gap-0.5"
          title="QA Engineer"
        >
          <span className="text-xl leading-none">🛡️</span>
          <span className="text-[8px] font-black uppercase tracking-wider">QA</span>
        </button>

        <button
          onClick={() => onOpenChat('specialist', undefined, undefined, 'supabase-db')}
          className="w-full px-2 py-2.5 rounded-xl transition-all border-2 shadow-md hover:shadow-lg active:scale-95 bg-[var(--background-primary)] text-purple-600 dark:text-purple-400 border-purple-500/30 hover:bg-purple-500/10 flex flex-col items-center gap-0.5"
          title="Database Specialist"
        >
          <span className="text-xl leading-none">🐘</span>
          <span className="text-[8px] font-black uppercase tracking-wider">DB</span>
        </button>

        <button
          onClick={() => onOpenChat('specialist', undefined, undefined, 'web')}
          className="w-full px-2 py-2.5 rounded-xl transition-all border-2 shadow-md hover:shadow-lg active:scale-95 bg-[var(--background-primary)] text-blue-600 dark:text-blue-400 border-blue-500/30 hover:bg-blue-500/10 flex flex-col items-center gap-0.5"
          title="Web Specialist"
        >
          <span className="text-xl leading-none">🌐</span>
          <span className="text-[8px] font-black uppercase tracking-wider">WEB</span>
        </button>

        <button
          onClick={() => onOpenChat('customer', 'chat', undefined, undefined)}
          className="w-full px-2 py-2.5 rounded-xl transition-all border-2 shadow-md hover:shadow-lg active:scale-95 bg-[var(--background-primary)] text-violet-600 dark:text-violet-400 border-violet-500/30 hover:bg-violet-500/10 flex flex-col items-center gap-0.5"
          title="Customer Voice"
        >
          <User size={20} strokeWidth={2.5} />
          <span className="text-[8px] font-black uppercase tracking-wider">CX</span>
        </button>
      </div>

      <div className="h-px w-8 bg-[var(--border-primary)] mx-auto" />
    </div>
    <div className="mt-auto border-t border-[var(--border-primary)] pt-4">
      <button className="p-3 text-[var(--text-primary)] hover:text-indigo-600 transition-colors">
        <Settings size={22} strokeWidth={2.5} />
      </button>
    </div>
  </nav>
);
