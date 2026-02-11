import { Sun, Moon, Plus, Package, FolderOpen, ChevronDown, Star, Trash2 } from "lucide-react";
import { cn } from "../../utils";
import type { Project } from "../../api";

interface HeaderProps {
  isDark: boolean;
  setIsDark: (dark: boolean) => void;
  handleStartCreate: () => void;
  loadData: () => void;
  onOpenChat: (persona: string) => void;
  projectMenuOpen: boolean;
  setProjectMenuOpen: (open: boolean) => void;
  favoriteProjects: Project[];
  recentProjects: Project[];
  currentProjectPath: string;
  handleOpenProject: (path: string) => Promise<void>;
  toggleFavoriteProject: (path: string) => Promise<void>;
  removeProject: (path: string) => Promise<void>;
  handleSelectProject: () => Promise<void>;
}

export const Header = ({
  isDark,
  setIsDark,
  handleStartCreate,
  loadData,
  onOpenChat,
  projectMenuOpen,
  setProjectMenuOpen,
  favoriteProjects,
  recentProjects,
  currentProjectPath,
  handleOpenProject,
  toggleFavoriteProject,
  removeProject,
  handleSelectProject,
}: HeaderProps) => {
  return (
    <header className="h-16 border-b-2 border-[var(--border-primary)] flex items-center px-6 justify-between bg-[var(--background-primary)]/90 backdrop-blur-xl z-30">
      <div className="flex items-center gap-6">
        <div className="flex items-center gap-3">
           <div className="w-8 h-8 bg-indigo-700 rounded-lg flex items-center justify-center font-black text-xs shadow-[var(--shadow-lg)] text-white">B</div>
           <h1 className="text-base font-black tracking-tighter uppercase text-[var(--text-primary)]">BERT <span className="text-indigo-700 dark:text-indigo-400 font-mono">BP6</span></h1>
        </div>
        <div className="h-6 w-px bg-[var(--border-primary)]" />
        <div className="flex items-center gap-4">
           <div className="flex items-center gap-2">
              <div className="w-2 h-2 rounded-full bg-emerald-600 shadow-[0_0_10px_rgba(5,150,105,0.5)] animate-pulse" />
              <span className="text-sm font-black text-[var(--text-primary)] uppercase tracking-widest">Live</span>
           </div>
           <span className="text-sm text-[var(--text-secondary)] font-bold tracking-tight">/ Workspace / <span className="text-indigo-700 dark:text-indigo-400 font-black">
             {(() => {
               const normalize = (p: string) => p.replace(/[/\\]$/, "");
               const current = normalize(currentProjectPath);
               const project = favoriteProjects.find(p => normalize(p.path) === current) || 
                              recentProjects.find(p => normalize(p.path) === current);
               if (project) return project.name;
               if (currentProjectPath) {
                 const parts = current.split(/[/\\]/);
                 return parts[parts.length - 1] || "Default";
               }
               return "Default";
             })()}
           </span></span>
        </div>
      </div>
      <div className="flex items-center gap-3">
        <button 
          onClick={() => onOpenChat('product-manager')}
          className="h-10 bg-[var(--background-secondary)] hover:bg-[var(--background-tertiary)] text-indigo-600 dark:text-indigo-400 px-5 rounded-xl text-xs font-black border-[var(--border-thick)] border-[var(--border-primary)] flex items-center gap-2 transition-all active:scale-95 shadow-[var(--shadow-sm)] uppercase tracking-widest"
        >
          <span className="text-base">🤖</span> PM
        </button>
        <button 
          onClick={() => setIsDark(!isDark)} 
          className="h-10 w-10 hover:bg-[var(--background-tertiary)] text-[var(--text-primary)] rounded-xl flex items-center justify-center transition-all border-[var(--border-thick)] border-[var(--border-primary)] shadow-[var(--shadow-sm)] active:scale-90"
        >
          {isDark ? <Sun size={18} strokeWidth={2.5} className="icon-rotate" /> : <Moon size={18} strokeWidth={2.5} className="icon-rotate" />}
        </button>
        <button 
          onClick={handleStartCreate} 
          className="h-10 bg-indigo-700 hover:bg-indigo-600 text-white px-5 rounded-xl text-xs font-black flex items-center gap-2 transition-all shadow-[var(--shadow-lg)] active:scale-95 border-[var(--border-thick)] border-indigo-800/20 uppercase tracking-widest"
        >
          <Plus size={16} strokeWidth={3} /> New Bead
        </button>
        <button 
          onClick={loadData} 
          className="h-10 bg-[var(--background-secondary)] hover:bg-[var(--background-tertiary)] text-[var(--text-primary)] px-5 rounded-xl text-xs font-black border-[var(--border-thick)] border-[var(--border-primary)] flex items-center gap-2 transition-all active:scale-95 shadow-[var(--shadow-sm)] uppercase tracking-widest"
        >
          <Package size={16} className="text-indigo-700 dark:text-indigo-400" strokeWidth={2.5} /> Sync
        </button>

        <div className="relative">
          <button 
            onClick={() => setProjectMenuOpen(!projectMenuOpen)}
            className="h-10 bg-[var(--background-secondary)] hover:bg-[var(--background-tertiary)] text-[var(--text-primary)] px-5 rounded-xl text-xs font-black border-[var(--border-thick)] border-[var(--border-primary)] flex items-center gap-2 transition-all active:scale-95 shadow-[var(--shadow-sm)] uppercase tracking-widest"
          >
            <FolderOpen size={16} className="text-indigo-700 dark:text-indigo-400" strokeWidth={2.5} />
            <span>Select Project</span>
            <ChevronDown size={16} className={cn("transition-transform opacity-100", projectMenuOpen && "rotate-180")} strokeWidth={3} />
          </button>
          
          {projectMenuOpen && (
            <div className="absolute right-0 mt-3 w-80 bg-[var(--background-primary)] border-[var(--border-thin)] border-[var(--border-primary)] rounded-2xl shadow-[var(--shadow-xl)] z-[60] py-3 animate-in fade-in zoom-in-95 duration-150 backdrop-blur-xl">
              {favoriteProjects.length > 0 && (
                <div className="px-2 pb-3 mb-2">
                  <div className="px-4 py-1.5 text-xs font-black text-[var(--text-muted)] uppercase tracking-[0.2em]">Favorites</div>
                  {favoriteProjects.map(p => (
                    <div 
                      key={p.path}
                      className={cn(
                        "group flex items-center gap-3 px-4 py-2.5 rounded-xl cursor-pointer transition-all",
                        currentProjectPath.replace(/[/\\]$/, "") === p.path.replace(/[/\\]$/, "") 
                          ? "bg-indigo-500/10 text-indigo-600 dark:text-indigo-400" 
                          : "hover:bg-[var(--background-secondary)] text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                      )}
                      onClick={() => { setProjectMenuOpen(false); handleOpenProject(p.path); }}
                    >
                      <div className="flex-1 min-w-0">
                        <div className="text-sm font-bold truncate tracking-tight">{p.name}</div>
                        <div className="text-xs opacity-50 truncate font-mono">{p.path}</div>
                      </div>
                      <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-all">
                        <button 
                          onClick={(e) => { e.stopPropagation(); toggleFavoriteProject(p.path); }}
                          className={cn("p-1.5 hover:bg-zinc-200 dark:hover:bg-zinc-800 rounded-lg transition-all", p.is_favorite ? "text-amber-500" : "text-[var(--text-muted)]")}
                        >
                          <Star size={14} className={cn(p.is_favorite && "fill-current")} />
                        </button>
                        <button 
                          onClick={(e) => { e.stopPropagation(); removeProject(p.path); }}
                          className="p-1.5 hover:bg-rose-500/10 hover:text-rose-500 rounded-lg transition-all text-[var(--text-muted)]"
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
              
              {recentProjects.length > 0 && (
                <div className="px-2 pb-3 mb-2">
                  <div className="px-4 py-1.5 text-xs font-black text-[var(--text-muted)] uppercase tracking-[0.2em]">Recent</div>
                  {recentProjects.map(p => (
                    <div 
                      key={p.path}
                      className={cn(
                        "group flex items-center gap-3 px-4 py-2.5 rounded-xl cursor-pointer transition-all",
                        currentProjectPath.replace(/[/\\]$/, "") === p.path.replace(/[/\\]$/, "") 
                          ? "bg-indigo-500/10 text-indigo-600 dark:text-indigo-400" 
                          : "hover:bg-[var(--background-secondary)] text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                      )}
                      onClick={() => { setProjectMenuOpen(false); handleOpenProject(p.path); }}
                    >
                      <div className="flex-1 min-w-0">
                        <div className="text-sm font-bold truncate tracking-tight">{p.name}</div>
                        <div className="text-xs opacity-50 truncate font-mono">{p.path}</div>
                      </div>
                      <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-all">
                        <button 
                          onClick={(e) => { e.stopPropagation(); toggleFavoriteProject(p.path); }}
                          className={cn("p-1.5 hover:bg-zinc-200 dark:hover:bg-zinc-800 rounded-lg transition-all", p.is_favorite ? "text-amber-500" : "text-[var(--text-muted)]")}
                        >
                          <Star size={14} className={cn(p.is_favorite && "fill-current")} />
                        </button>
                        <button 
                          onClick={(e) => { e.stopPropagation(); removeProject(p.path); }}
                          className="p-1.5 hover:bg-rose-500/10 hover:text-rose-500 rounded-lg transition-all text-[var(--text-muted)]"
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
              
              <div className="px-3 pt-2 border-t border-[var(--border-primary)]">
                <button 
                  onClick={() => { setProjectMenuOpen(false); handleSelectProject(); }}
                  className="w-full flex items-center gap-2 px-4 py-3 text-sm font-bold text-indigo-500 hover:bg-indigo-500/10 rounded-xl transition-all text-left"
                >
                  <Plus size={16} /> Add New Project
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </header>
  );
};
