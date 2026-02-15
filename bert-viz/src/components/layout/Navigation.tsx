import { TableProperties, Settings, GanttChart, User, Palette } from "lucide-react";
import brandLogo from "../../assets/brand_logo_1.svg";
import { cn } from "../../utils";

export type ViewType = 'gantt' | 'list';

interface NavigationProps {
  currentView: ViewType;
  onViewChange: (view: ViewType) => void;
  onOpenChat: (persona: string, task?: string, beadId?: string, role?: string) => void;
  onOpenPalettePreview: () => void;
}

export const Navigation = ({ currentView, onViewChange, onOpenChat, onOpenPalettePreview }: NavigationProps) => (
  <nav className="w-16 flex flex-col items-center py-6 border-r border-[var(--border-primary)] bg-[var(--background-secondary)] z-30 shadow-xl">
    <div className="flex flex-col gap-6 flex-1 items-center">
      <div className="w-12 h-12 p-2 rounded-xl border-2 border-[var(--border-primary)] bg-[var(--background-primary)] shadow-md flex items-center justify-center" title="Pairti">
        <img src={brandLogo} alt="Pairti" className="w-full h-full object-contain" style={{ filter: 'var(--logo-filter)' }} />
      </div>

      <button
        onClick={() => onViewChange('gantt')}
        className={cn(
          "p-3 rounded-xl transition-all border-2 shadow-md hover:shadow-lg active:scale-95",
          currentView === 'gantt'
            ? "bg-[var(--accent-primary)] text-white border-[var(--accent-primary)] shadow-[0_8px_30px_rgba(15,139,255,0.25)]"
            : "bg-[var(--background-primary)] text-[var(--text-muted)] border-[var(--border-primary)] hover:text-[var(--accent-primary)]"
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
            ? "bg-[var(--accent-primary)] text-white border-[var(--accent-primary)] shadow-[0_8px_30px_rgba(15,139,255,0.25)]"
            : "bg-[var(--background-primary)] text-[var(--text-muted)] border-[var(--border-primary)] hover:text-[var(--accent-primary)]"
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
        className="w-full px-2 py-2.5 rounded-xl transition-all border-2 shadow-md hover:shadow-lg active:scale-95 bg-[var(--background-primary)] text-[var(--accent-primary)] border-[var(--accent-primary)] hover:bg-[var(--accent-primary)] hover:text-white flex flex-col items-center gap-0.5"
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
        className="w-full px-2 py-2.5 rounded-xl transition-all border-2 shadow-md hover:shadow-lg active:scale-95 bg-[var(--background-primary)] text-[var(--accent-violet)] border-[var(--accent-violet)] hover:bg-[var(--accent-violet)] hover:text-white flex flex-col items-center gap-0.5"
        title="Database Specialist"
      >
        <span className="text-xl leading-none">🐘</span>
          <span className="text-[8px] font-black uppercase tracking-wider">DB</span>
        </button>

      <button
        onClick={() => onOpenChat('specialist', undefined, undefined, 'web')}
        className="w-full px-2 py-2.5 rounded-xl transition-all border-2 shadow-md hover:shadow-lg active:scale-95 bg-[var(--background-primary)] text-[var(--accent-primary)] border-[var(--accent-primary)] hover:bg-[var(--accent-primary)] hover:text-white flex flex-col items-center gap-0.5"
        title="Web Specialist"
      >
        <span className="text-xl leading-none">🌐</span>
          <span className="text-[8px] font-black uppercase tracking-wider">WEB</span>
        </button>

      <button
        onClick={() => onOpenChat('specialist', undefined, undefined, 'flutter')}
        className="w-full px-2 py-2.5 rounded-xl transition-all border-2 shadow-md hover:shadow-lg active:scale-95 bg-[var(--background-primary)] text-blue-600 dark:text-blue-400 border-blue-500/30 hover:bg-blue-500/10 flex flex-col items-center gap-0.5"
        title="Flutter Specialist"
      >
        <span className="text-xl leading-none">📱</span>
        <span className="text-[8px] font-black uppercase tracking-wider">FLT</span>
      </button>

      <button
        onClick={() => onOpenChat('specialist', undefined, undefined, 'rust-tauri')}
        className="w-full px-2 py-2.5 rounded-xl transition-all border-2 shadow-md hover:shadow-lg active:scale-95 bg-[var(--background-primary)] text-orange-600 dark:text-orange-400 border-orange-500/30 hover:bg-orange-500/10 flex flex-col items-center gap-0.5"
        title="Rust/Tauri Specialist"
      >
        <span className="text-xl leading-none">🦀</span>
        <span className="text-[8px] font-black uppercase tracking-wider">RST</span>
      </button>

      <button
        onClick={() => onOpenChat('specialist', undefined, undefined, 'supabase-edge')}
        className="w-full px-2 py-2.5 rounded-xl transition-all border-2 shadow-md hover:shadow-lg active:scale-95 bg-[var(--background-primary)] text-yellow-600 dark:text-yellow-400 border-yellow-500/30 hover:bg-yellow-500/10 flex flex-col items-center gap-0.5"
        title="Supabase Edge Specialist"
      >
        <span className="text-xl leading-none">⚡</span>
        <span className="text-[8px] font-black uppercase tracking-wider">EDGE</span>
      </button>

      <button
        onClick={() => onOpenChat('architect', 'chat', undefined, undefined)}
        className="w-full px-2 py-2.5 rounded-xl transition-all border-2 shadow-md hover:shadow-lg active:scale-95 bg-[var(--background-primary)] text-purple-600 dark:text-purple-400 border-purple-500/30 hover:bg-purple-500/10 flex flex-col items-center gap-0.5"
        title="Architect"
      >
        <span className="text-xl leading-none">🏗️</span>
        <span className="text-[8px] font-black uppercase tracking-wider">ARCH</span>
      </button>

      <button
        onClick={() => onOpenChat('customer', 'chat', undefined, undefined)}
        className="w-full px-2 py-2.5 rounded-xl transition-all border-2 shadow-md hover:shadow-lg active:scale-95 bg-[var(--background-primary)] text-[var(--accent-violet)] border-[var(--accent-violet)] hover:bg-[var(--accent-violet)] hover:text-white flex flex-col items-center gap-0.5"
        title="Customer Voice"
      >
        <User size={20} strokeWidth={2.5} />
        <span className="text-[8px] font-black uppercase tracking-wider">CX</span>
      </button>
      </div>

      <div className="h-px w-8 bg-[var(--border-primary)] mx-auto" />
    </div>
    <div className="mt-auto border-t border-[var(--border-primary)] pt-4 flex flex-col items-center gap-3">
      <button
        onClick={onOpenPalettePreview}
        className="p-3 rounded-xl border-2 border-[var(--border-primary)] text-[var(--text-primary)] hover:text-[var(--accent-primary)] hover:border-[var(--accent-primary)] transition-colors shadow-sm"
        title="Open Palette Preview"
      >
        <Palette size={20} strokeWidth={2.5} />
      </button>
      <button className="p-3 text-[var(--text-primary)] hover:text-[var(--accent-primary)] transition-colors">
        <Settings size={22} strokeWidth={2.5} />
      </button>
    </div>
  </nav>
);
