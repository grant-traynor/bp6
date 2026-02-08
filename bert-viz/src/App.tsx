import { useState, useEffect, useCallback, useRef, useMemo } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { ListTree, Settings, ChevronRight, ChevronDown, Package, CheckCircle2, Circle, Clock, X, User, Tag, Save, Edit3, Trash2, Plus, Flame, Star, Sun, Moon, FolderOpen } from "lucide-react";
import { twMerge } from "tailwind-merge";
import { clsx, type ClassValue } from "clsx";
import { fetchBeads, buildWBSTree, calculateGanttLayout, updateBead, createBead, type WBSNode, type Bead, type GanttItem, type GanttLayout, type Project, fetchProjects, addProject, removeProject, openProject } from "./api";

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

// --- Custom Components ---

const getChipStyles = (label: string) => {
  const l = label.toLowerCase();
  if (l === 'epic') return "bg-purple-500/20 text-purple-800 dark:text-purple-300 border-purple-500/40";
  if (l === 'bug') return "bg-rose-500/20 text-rose-800 dark:text-rose-300 border-rose-500/40";
  if (l === 'feature') return "bg-emerald-500/20 text-emerald-800 dark:text-emerald-300 border-emerald-500/40";
  if (l === 'task') return "bg-indigo-500/20 text-indigo-800 dark:text-indigo-300 border-indigo-500/40";
  if (l.includes('infra')) return "bg-amber-500/20 text-amber-800 dark:text-amber-300 border-amber-500/40";
  if (l.includes('doc')) return "bg-cyan-500/20 text-cyan-800 dark:text-cyan-300 border-cyan-500/40";
  return "bg-[var(--background-tertiary)] text-[var(--text-primary)] border-[var(--border-primary)]";
};

const Chip = ({ label }: { label: string }) => (
  <span className={cn("px-2 py-0.5 rounded-md text-[10px] font-black uppercase tracking-widest border transition-all", getChipStyles(label))}>
    {label}
  </span>
);

const StatusIcon = ({ status, size = 14 }: { status: string; size?: number }) => {
  switch (status) {
    case 'closed': return <CheckCircle2 size={size} className="text-emerald-600 dark:text-emerald-400 stroke-[2.5]" />;
    case 'in_progress': return <Clock size={size} className="text-amber-600 dark:text-amber-400 stroke-[2.5]" />;
    default: return <Circle size={size} className="text-[var(--text-secondary)] stroke-[2]" />;
  }
};

const GanttBar = ({ item, onClick }: { item: GanttItem; onClick: (bead: Bead) => void }) => {
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

const WBSTreeItem = ({ 
  node, 
  depth = 0, 
  onToggle,
  onClick
}: { 
  node: WBSNode; 
  depth?: number; 
  onToggle: (id: string) => void;
  onClick: (bead: Bead) => void;
}) => {
  const hasChildren = node.children.length > 0;

  return (
    <div className="select-none h-[48px] flex flex-col justify-center border-b border-[var(--border-primary)]">
      <div 
        className={cn(
          "flex items-center hover:bg-[var(--background-primary)] cursor-pointer group transition-all h-full",
          node.isCritical && "bg-rose-500/[0.08]"
        )}
        onClick={() => onClick(node as Bead)}
      >
        <div 
          className="w-10 shrink-0 flex items-center justify-center h-full text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors" 
          onClick={(e) => { e.stopPropagation(); onToggle(node.id); }}
        >
          {hasChildren ? (
            node.isExpanded ? <ChevronDown size={18} className="stroke-[3]" /> : <ChevronRight size={18} className="stroke-[3]" />
          ) : null}
        </div>

        <div className="w-28 shrink-0 px-2 flex items-center h-full border-r border-[var(--border-primary)]">
           <span className={cn(
             "font-mono text-[11px] font-black px-2.5 py-1 rounded-md tracking-tighter border-2 shadow-sm",
             node.isCritical ? "bg-rose-500/20 text-rose-900 dark:text-rose-100 border-rose-500/40" : "bg-[var(--background-tertiary)] text-[var(--text-primary)] border-[var(--border-primary)]"
           )}>
             {node.id}
           </span>
        </div>

        <div 
          className="flex-1 px-4 flex items-center gap-4 truncate h-full"
          style={{ paddingLeft: `${depth * 0.75 + 0.75}rem` }}
        >
          <StatusIcon status={node.status} size={16} />
          <span className={cn(
            "text-[13px] truncate font-black tracking-tight",
            node.status === 'closed' ? "text-[var(--text-muted)] line-through font-bold" : "text-[var(--text-primary)] group-hover:text-indigo-700 dark:group-hover:text-indigo-300"
          )}>
            {node.title}
          </span>
          {node.issue_type !== 'task' && (
            <span className={cn(
              "text-[9px] font-black px-2 py-0.5 rounded-md uppercase tracking-widest border-2 shadow-sm",
              getChipStyles(node.issue_type)
            )}>
              {node.issue_type}
            </span>
          )}
        </div>
      </div>
    </div>
  );
};

const WBSTreeList = ({ nodes, depth = 0, onToggle, onClick }: { nodes: WBSNode[], depth?: number, onToggle: (id: string) => void, onClick: (bead: Bead) => void }) => {
  return (
    <>
      {nodes.map(node => (
        <div key={node.id}>
          <WBSTreeItem node={node} depth={depth} onToggle={onToggle} onClick={onClick} />
          {node.isExpanded && node.children.length > 0 && (
            <WBSTreeList nodes={node.children} depth={depth + 1} onToggle={onToggle} onClick={onClick} />
          )}
        </div>
      ))}
    </>
  );
};

function App() {
  const [beads, setBeads] = useState<Bead[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [currentProjectPath, setCurrentProjectPath] = useState<string>("");
  const [tree, setTree] = useState<WBSNode[]>([]);
  const [ganttLayout, setGanttLayout] = useState<GanttLayout>({ items: [], connectors: [], rowCount: 0 });
  const [loading, setLoading] = useState(true);
  const [selectedBead, setSelectedBead] = useState<Bead | null>(null);
  const [isEditing, setIsEditing] = useState(false);
  const [isCreating, setIsCreating] = useState(false);
  const [editForm, setEditForm] = useState<Partial<Bead>>({});
  const [filterText, setFilterText] = useState("");
  const [zoom, setZoom] = useState(1);
  const [isDark, setIsDark] = useState(true);
  const [projectMenuOpen, setProjectMenuOpen] = useState(false);

  useEffect(() => {
    if (isDark) {
      document.documentElement.classList.add('dark');
    } else {
      document.documentElement.classList.remove('dark');
    }
  }, [isDark]);

  const scrollRefWBS = useRef<HTMLDivElement>(null);
  const scrollRefBERT = useRef<HTMLDivElement>(null);
  const activeScrollSource = useRef<HTMLDivElement | null>(null);

  const handleSelectProject = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Select BERT Project Directory"
    });
    if (selected && typeof selected === 'string') {
      await handleOpenProject(selected);
    }
  };

  const loadProjects = useCallback(async () => {
    const data = await fetchProjects();
    setProjects(data);
  }, []);

  const handleOpenProject = async (path: string) => {
    await openProject(path);
    setCurrentProjectPath(path);
    loadData();
  };

  const handleAddCurrentProject = async () => {
    const path = await invoke<string>("get_current_dir");
    const existing = projects.find(p => p.path === path);
    
    if (existing) {
      await toggleFavoriteProject(path);
    } else {
      const parts = path.split(/[/\\]/);
      const dirName = parts[parts.length - 1] || "Project";
      const name = prompt("Project Name:", dirName);
      if (name) {
        await addProject({ name, path, is_favorite: true });
      }
    }
    loadProjects();
  };

  const isCurrentProjectFavorite = useMemo(() => {
    return projects.some(p => p.path === currentProjectPath && p.is_favorite);
  }, [projects, currentProjectPath]);

  const loadData = useCallback(async () => {
    try {
      console.log("Fetching beads...");
      const data = await fetchBeads();
      console.log(`Fetched ${data.length} beads`);
      setBeads(data);
      
      const filtered = data.filter(b => {
        const search = filterText.toLowerCase();
        if (!search) return true;
        return (
          b.title.toLowerCase().includes(search) ||
          b.id.toLowerCase().includes(search) ||
          b.owner?.toLowerCase().includes(search) ||
          b.labels?.some(l => l.toLowerCase().includes(search))
        );
      });

      console.log(`Filtered to ${filtered.length} beads`);
      const wbsTree = buildWBSTree(filtered);
      setTree(wbsTree);
      setGanttLayout(calculateGanttLayout(data, wbsTree, zoom));
    } catch (error) {
      console.error("Error in loadData:", error);
    } finally {
      setLoading(false);
    }
  }, [filterText, zoom]);

  useEffect(() => {
    const init = async () => {
      const path = await invoke<string>("get_current_dir");
      setCurrentProjectPath(path);
    };
    init();
    loadData();
    loadProjects();

    const unlistenBeads = listen("beads-updated", () => loadData());
    const unlistenProjs = listen("projects-updated", () => loadProjects());

    return () => {
      unlistenBeads.then(f => f());
      unlistenProjs.then(f => f());
    };
  }, [loadData, loadProjects]);

  const toggleNode = useCallback((id: string) => {
    setTree(prevTree => {
      const newTree = JSON.parse(JSON.stringify(prevTree));
      const findAndToggle = (nodes: WBSNode[]) => {
        for (const node of nodes) {
          if (node.id === id) {
            node.isExpanded = !node.isExpanded;
            return true;
          }
          if (node.children && findAndToggle(node.children)) return true;
        }
        return false;
      };
      findAndToggle(newTree);
      setGanttLayout(calculateGanttLayout(beads, newTree, zoom));
      return newTree;
    });
  }, [beads, zoom]);

  const handleBeadClick = (bead: Bead) => {
    setSelectedBead(bead);
    setIsEditing(false);
    setIsCreating(false);
  };

  const handleStartEdit = () => {
    if (selectedBead) {
      const parent = beads.find(b => b.dependencies?.some(d => d.issue_id === selectedBead.id && d.type === 'parent-child'))?.id;
      setEditForm({ 
        ...selectedBead,
        parent // Virtual field for editing
      });
      setIsEditing(true);
      setIsCreating(false);
    }
  };

  const handleSaveEdit = async () => {
    if (selectedBead && editForm) {
      const updated = { ...editForm } as Bead;
      
      // Handle parent change
      const currentParent = beads.find(b => b.dependencies?.some(d => d.issue_id === selectedBead.id && d.type === 'parent-child'));
      if (updated.parent !== currentParent?.id) {
        // Remove old parent-child dependency
        updated.dependencies = (updated.dependencies || []).filter(d => d.type !== 'parent-child');
        // Add new one if specified
        if (updated.parent) {
          updated.dependencies.push({
            issue_id: updated.id,
            depends_on_id: updated.parent,
            type: 'parent-child'
          });
        }
      }
      
      delete (updated as any).parent; // Don't save virtual field
      
      await updateBead(updated);
      setSelectedBead(updated);
      setIsEditing(false);
      loadData();
    }
  };

  const handleStartCreate = () => {
    setSelectedBead(null);
    setEditForm({
      id: `bp6-${Math.random().toString(36).substr(2, 3)}`,
      title: "",
      status: "open",
      priority: 2,
      issue_type: "task",
      dependencies: [],
      created_at: new Date().toISOString(),
      acceptance_criteria: []
    });
    setIsEditing(false);
    setIsCreating(true);
  };

  const handleSaveCreate = async () => {
    if (editForm.title) {
      const newBead = { ...editForm } as Bead;
      if (newBead.parent) {
        newBead.dependencies = [...(newBead.dependencies || []), {
          issue_id: newBead.id,
          depends_on_id: newBead.parent,
          type: 'parent-child'
        }];
      }
      delete (newBead as any).parent;
      await createBead(newBead);
      setIsCreating(false);
      loadData();
    }
  };

  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const target = e.currentTarget;
    if (activeScrollSource.current && activeScrollSource.current !== target) return;
    const other = target === scrollRefWBS.current ? scrollRefBERT.current : scrollRefWBS.current;
    if (other) other.scrollTop = target.scrollTop;
  };

  const handleMouseEnter = (e: React.MouseEvent<HTMLDivElement>) => {
    activeScrollSource.current = e.currentTarget;
  };

  const toggleFavorite = async (bead: Bead) => {
    const updated = { ...bead, is_favorite: !bead.is_favorite };
    await updateBead(updated);
    loadData();
  };

  const favoriteBeads = useMemo(() => beads.filter(b => b.is_favorite), [beads]);
  
  const favoriteProjects = useMemo(() => projects.filter(p => p.is_favorite), [projects]);
  const recentProjects = useMemo(() => 
    projects.filter(p => !p.is_favorite)
    .sort((a, b) => (b.last_opened || "").localeCompare(a.last_opened || ""))
  , [projects]);

  const stats = useMemo(() => {
    const total = beads.length;
    const open = beads.filter(b => b.status === 'open').length;
    const inProgress = beads.filter(b => b.status === 'in_progress').length;
    const closed = beads.filter(b => b.status === 'closed').length;
    
    // Simple blocked calculation for stats: has a 'blocks' dependency that is NOT closed
    const blocked = beads.filter(b => {
      if (b.status === 'closed') return false;
      const deps = b.dependencies || [];
      return deps.some(d => {
        if (d.type !== 'blocks') return false;
        const pred = beads.find(p => p.id === d.depends_on_id);
        return pred && pred.status !== 'closed';
      });
    }).length;

    return { total, open, inProgress, closed, blocked };
  }, [beads]);

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[var(--background-primary)] text-[var(--text-primary)] font-sans selection:bg-indigo-100 dark:selection:bg-indigo-900/30">
      <nav className="w-16 flex flex-col items-center py-6 border-r border-[var(--border-primary)] bg-[var(--background-secondary)] z-30 shadow-xl">
        <div className="flex flex-col gap-4 flex-1">
          <button className="p-3 rounded-xl bg-[var(--background-primary)] text-indigo-700 dark:text-indigo-400 transition-all border-2 border-[var(--border-primary)] shadow-md hover:shadow-lg active:scale-95"><ListTree size={24} strokeWidth={2.5} /></button>
          <div className="h-px w-8 bg-[var(--border-primary)] mx-auto" />
        </div>
        <div className="mt-auto border-t border-[var(--border-primary)] pt-4">
          <button className="p-3 text-[var(--text-primary)] hover:text-indigo-600 transition-colors">
            <Settings size={22} strokeWidth={2.5} />
          </button>
        </div>
      </nav>

      <main className="flex-1 flex flex-col min-w-0 bg-[var(--background-primary)] relative">
        <header className="h-16 border-b-2 border-[var(--border-primary)] flex items-center px-6 justify-between bg-[var(--background-primary)]/90 backdrop-blur-xl z-20">
          <div className="flex items-center gap-6">
            <div className="flex items-center gap-3">
               <div className="w-8 h-8 bg-indigo-700 rounded-lg flex items-center justify-center font-black text-xs shadow-xl shadow-indigo-700/30 text-white">B</div>
               <h1 className="text-base font-black tracking-tighter uppercase text-[var(--text-primary)]">BERT <span className="text-indigo-700 dark:text-indigo-400 font-mono">BP6</span></h1>
            </div>
            <div className="h-6 w-px bg-[var(--border-primary)]" />
            <div className="flex items-center gap-4">
               <div className="flex items-center gap-2">
                  <div className="w-2 h-2 rounded-full bg-emerald-600 shadow-[0_0_10px_rgba(5,150,105,0.5)] animate-pulse" />
                  <span className="text-[11px] font-black text-[var(--text-primary)] uppercase tracking-widest">Live</span>
               </div>
               <span className="text-[11px] text-[var(--text-secondary)] font-bold tracking-tight">/ Workspace / <span className="text-indigo-700 dark:text-indigo-400 font-black">Default</span></span>
            </div>
          </div>
          <div className="flex items-center gap-3">
            <button 
              onClick={() => setIsDark(!isDark)} 
              className="h-10 w-10 hover:bg-[var(--background-tertiary)] text-[var(--text-primary)] rounded-xl flex items-center justify-center transition-all border-2 border-[var(--border-primary)] shadow-sm active:scale-90"
            >
              {isDark ? <Sun size={18} strokeWidth={2.5} /> : <Moon size={18} strokeWidth={2.5} />}
            </button>
            <button 
              onClick={handleStartCreate} 
              className="h-10 bg-indigo-700 hover:bg-indigo-600 text-white px-5 rounded-xl text-xs font-black flex items-center gap-2 transition-all shadow-xl shadow-indigo-700/20 active:scale-95 border-2 border-indigo-800/20 uppercase tracking-widest"
            >
              <Plus size={16} strokeWidth={3} /> New Bead
            </button>
            <button 
              onClick={loadData} 
              className="h-10 bg-[var(--background-secondary)] hover:bg-[var(--background-tertiary)] text-[var(--text-primary)] px-5 rounded-xl text-xs font-black border-2 border-[var(--border-primary)] flex items-center gap-2 transition-all active:scale-95 shadow-sm uppercase tracking-widest"
            >
              <Package size={16} className="text-indigo-700 dark:text-indigo-400" strokeWidth={2.5} /> Sync
            </button>

            <div className="relative">
              <button 
                onClick={() => setProjectMenuOpen(!projectMenuOpen)}
                className="h-10 bg-[var(--background-secondary)] hover:bg-[var(--background-tertiary)] text-[var(--text-primary)] px-5 rounded-xl text-xs font-black border-2 border-[var(--border-primary)] flex items-center gap-2 transition-all active:scale-95 shadow-sm uppercase tracking-widest"
              >
                <FolderOpen size={16} className="text-indigo-700 dark:text-indigo-400" strokeWidth={2.5} />
                <span>Select Project</span>
                <ChevronDown size={16} className={cn("transition-transform opacity-100", projectMenuOpen && "rotate-180")} strokeWidth={3} />
              </button>
              
              {projectMenuOpen && (
                <div className="absolute right-0 mt-3 w-80 bg-[var(--background-primary)] border border-[var(--border-primary)] rounded-2xl shadow-2xl z-[60] py-3 animate-in fade-in zoom-in-95 duration-150 backdrop-blur-xl">
                  {favoriteProjects.length > 0 && (
                    <div className="px-2 pb-3 mb-2">
                      <div className="px-4 py-1.5 text-[9px] font-black text-[var(--text-muted)] uppercase tracking-[0.2em]">Favorites</div>
                      {favoriteProjects.map(p => (
                        <div 
                          key={p.path}
                          className={cn(
                            "group flex items-center gap-3 px-4 py-2.5 rounded-xl cursor-pointer transition-all",
                            currentProjectPath === p.path 
                              ? "bg-indigo-500/10 text-indigo-600 dark:text-indigo-400" 
                              : "hover:bg-[var(--background-secondary)] text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                          )}
                          onClick={() => { setProjectMenuOpen(false); handleOpenProject(p.path); }}
                        >
                          <div className="flex-1 min-w-0">
                            <div className="text-[11px] font-bold truncate tracking-tight">{p.name}</div>
                            <div className="text-[9px] opacity-50 truncate font-mono">{p.path}</div>
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
                      <div className="px-4 py-1.5 text-[9px] font-black text-[var(--text-muted)] uppercase tracking-[0.2em]">Recent</div>
                      {recentProjects.map(p => (
                        <div 
                          key={p.path}
                          className={cn(
                            "group flex items-center gap-3 px-4 py-2.5 rounded-xl cursor-pointer transition-all",
                            currentProjectPath === p.path 
                              ? "bg-indigo-500/10 text-indigo-600 dark:text-indigo-400" 
                              : "hover:bg-[var(--background-secondary)] text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
                          )}
                          onClick={() => { setProjectMenuOpen(false); handleOpenProject(p.path); }}
                        >
                          <div className="flex-1 min-w-0">
                            <div className="text-[11px] font-bold truncate tracking-tight">{p.name}</div>
                            <div className="text-[9px] opacity-50 truncate font-mono">{p.path}</div>
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
                      className="w-full flex items-center gap-2 px-4 py-3 text-[11px] font-bold text-indigo-500 hover:bg-indigo-500/10 rounded-xl transition-all text-left"
                    >
                      <Plus size={16} /> Add New Project
                    </button>
                  </div>
                </div>
              )}
            </div>
          </div>
        </header>

        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Shared Header Row for WBS and Gantt */}
          <div className="flex shrink-0 border-b-2 border-[var(--border-primary)] bg-[var(--background-secondary)] z-20">
            {/* WBS Header Area */}
            <div className="w-1/3 min-w-[420px] border-r-2 border-[var(--border-primary)] flex flex-col">
              <div className="px-6 py-4 border-b-2 border-[var(--border-primary)]/50">
                <h2 className="text-[11px] font-black text-[var(--text-primary)] uppercase tracking-[0.25em] flex items-center gap-2">Task Breakdown</h2>
              </div>
              {favoriteBeads.length > 0 && (
                <div className="px-6 py-4 border-b-2 border-[var(--border-primary)]/50 bg-indigo-500/10">
                  <h2 className="text-[10px] font-black text-indigo-800 dark:text-indigo-300 uppercase tracking-[0.2em] flex items-center gap-2 mb-3"><Star size={12} className="fill-current" /> Favorites</h2>
                  <div className="flex flex-wrap gap-2">
                    {favoriteBeads.map(f => (
                      <div key={f.id} onClick={() => handleBeadClick(f)} className="flex items-center gap-2 px-2.5 py-1.5 rounded-xl bg-[var(--background-primary)] border-2 border-[var(--border-primary)] hover:border-indigo-500 shadow-sm cursor-pointer transition-all active:scale-95">
                        <span className="font-mono text-[9px] font-black text-indigo-700 dark:text-indigo-400">{f.id}</span>
                        <span className="text-[11px] font-black text-[var(--text-primary)] truncate max-w-[140px]">{f.title}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
              <div className="px-4 py-3 border-b-2 border-[var(--border-primary)]/50 bg-[var(--background-primary)]">
                <div className="relative group">
                  <input 
                    type="text"
                    placeholder="Search by title, ID, or label..."
                    className="w-full bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-xl px-4 py-2.5 text-[12px] font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none transition-all placeholder:text-[var(--text-muted)] shadow-inner"
                    value={filterText}
                    onChange={e => setFilterText(e.target.value)}
                  />
                </div>
              </div>
              <div className="flex items-center px-4 py-2 bg-[var(--background-tertiary)] text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.3em] mt-auto border-t border-[var(--border-primary)]/50">
                <div className="w-10 shrink-0" />
                <div className="w-28 shrink-0 px-2 border-r-2 border-[var(--border-primary)]/50">ID</div>
                <div className="flex-1 px-4">Name</div>
              </div>
            </div>

            {/* Gantt Header Area (Metrics + Controls) */}
            <div className="flex-1 flex items-end justify-between px-8 py-3 bg-[var(--background-primary)]">
               <div className="flex items-center gap-10 mb-1">
                  <div className="flex flex-col">
                     <span className="text-[9px] font-black text-[var(--text-muted)] uppercase tracking-[0.25em] mb-1">Total</span>
                     <span className="text-base font-black text-[var(--text-primary)]">{stats.total}</span>
                  </div>
                  <div className="flex flex-col border-l-2 border-[var(--border-primary)] pl-6">
                     <span className="text-[9px] font-black text-emerald-600 dark:text-emerald-400 uppercase tracking-[0.25em] mb-1">Open</span>
                     <span className="text-base font-black text-emerald-700 dark:text-emerald-400">{stats.open}</span>
                  </div>
                  <div className="flex flex-col border-l-2 border-[var(--border-primary)] pl-6">
                     <span className="text-[9px] font-black text-amber-600 dark:text-amber-400 uppercase tracking-[0.25em] mb-1">Active</span>
                     <span className="text-base font-black text-amber-700 dark:text-amber-400">{stats.inProgress}</span>
                  </div>
                  <div className="flex flex-col border-l-2 border-[var(--border-primary)] pl-6">
                     <span className="text-[9px] font-black text-rose-600 dark:text-rose-400 uppercase tracking-[0.25em] mb-1">Blocked</span>
                     <span className="text-base font-black text-rose-700 dark:text-rose-400">{stats.blocked}</span>
                  </div>
                  <div className="flex flex-col border-l-2 border-[var(--border-primary)] pl-6">
                     <span className="text-[9px] font-black text-[var(--text-primary)] uppercase tracking-[0.25em] mb-1">Done</span>
                     <span className="text-base font-black text-[var(--text-secondary)]">{stats.closed}</span>
                  </div>
               </div>

               <div className="flex items-center gap-1 bg-[var(--background-secondary)] p-1.5 rounded-2xl border-2 border-[var(--border-primary)] shadow-md mb-1">
                  <button onClick={() => setZoom(Math.max(0.25, zoom - 0.25))} className="p-2 hover:bg-[var(--background-tertiary)] rounded-xl text-[var(--text-primary)] transition-all text-xs font-black px-4 active:scale-90">-</button>
                  <span className="text-[11px] font-mono font-black text-[var(--text-primary)] min-w-[50px] text-center">{Math.round(zoom * 100)}%</span>
                  <button onClick={() => setZoom(Math.min(3, zoom + 0.25))} className="p-2 hover:bg-[var(--background-tertiary)] rounded-xl text-[var(--text-primary)] transition-all text-xs font-black px-4 active:scale-90">+</button>
                  <div className="w-px h-5 bg-[var(--border-primary)] mx-2" />
                  <button onClick={() => setZoom(1)} className="px-4 py-2 hover:bg-[var(--background-tertiary)] rounded-xl text-[var(--text-primary)] transition-all text-[10px] font-black uppercase tracking-[0.2em] active:scale-95">Reset</button>
               </div>
            </div>
          </div>

          <div className="flex-1 flex overflow-hidden">
                      {/* WBS Side */}
                      <div 
                        ref={scrollRefWBS}
                        onScroll={handleScroll}
                        onMouseEnter={handleMouseEnter}
                        className="w-1/3 border-r border-[var(--border-primary)] flex flex-col bg-[var(--background-secondary)] min-w-[420px] overflow-y-auto custom-scrollbar"
                      >
                        <div className="p-0">                {loading ? <div className="p-12 animate-pulse text-[var(--text-muted)] text-xs font-medium tracking-widest uppercase">Syncing Schedule...</div> : (
                  <div className="flex flex-col">
                    <WBSTreeList nodes={tree} onToggle={toggleNode} onClick={handleBeadClick} />
                  </div>
                )}
              </div>
            </div>

            {/* Gantt Side */}
            <div 
              ref={scrollRefBERT}
              onScroll={handleScroll}
              onMouseEnter={handleMouseEnter}
              className="flex-1 relative bg-[var(--background-primary)] overflow-auto custom-scrollbar"
            >
              <div className="relative" style={{ height: ganttLayout.rowCount * 48, width: 5000 * zoom }}>
                 {/* Grid */}
                 <div className="absolute inset-0 pointer-events-none">
                    {Array.from({ length: ganttLayout.rowCount }).map((_, i) => (
                      <div key={i} className="w-full border-b-2 border-[var(--border-primary)]/40" style={{ height: '48px' }} />
                    ))}
                    <div className="absolute inset-0 flex">
                      {Array.from({ length: Math.ceil(50 * zoom) }).map((_, i) => (
                        <div key={i} className="h-full border-r-2 border-[var(--border-primary)]/40" style={{ width: 100 * zoom }} />
                      ))}
                    </div>
                 </div>

               {/* Connectors (SVG) */}
               <svg className="absolute inset-0 pointer-events-none overflow-visible" style={{ width: '100%', height: '100%' }}>
                  {ganttLayout.connectors.map((c: any, i: number) => (
                    <path
                      key={i}
                      d={`M ${c.from.x} ${c.from.y} L ${c.from.x + 20} ${c.from.y} L ${c.from.x + 20} ${c.to.y} L ${c.to.x} ${c.to.y}`}
                      fill="none"
                      stroke={c.isCritical ? "#f43f5e" : "var(--text-muted)"}
                      strokeWidth={c.isCritical ? 2.5 : 1.5}
                      strokeDasharray={c.isCritical ? "0" : "4 2"}
                      opacity={0.8}
                    />
                  ))}
               </svg>

               {/* Items */}
               {ganttLayout.items.map((item: any, i: number) => (
                 <div key={i} className="absolute w-full" style={{ top: item.row * 48, height: 48 }}>
                    <GanttBar item={item} onClick={handleBeadClick} />
                 </div>
               ))}
            </div>
          </div>
        </div>
      </div>

        {/* Sidebar */}
        {(selectedBead || isCreating) && (
          <div className="absolute right-0 top-0 bottom-0 w-[480px] bg-[var(--background-primary)] border-l-2 border-[var(--border-primary)] shadow-[0_0_60px_rgba(0,0,0,0.2)] z-50 flex flex-col animate-in slide-in-from-right duration-300 backdrop-blur-2xl">
            <div className="p-6 border-b-2 border-[var(--border-primary)] flex items-center justify-between bg-[var(--background-secondary)]">
              <div className="flex items-center gap-4">
                <span className="font-mono text-[11px] px-3 py-2 rounded-xl bg-indigo-700 dark:bg-indigo-500 text-white font-black border-2 border-indigo-800 shadow-md tracking-widest uppercase">
                  {isCreating ? "NEW BEAD" : selectedBead?.id}
                </span>
                {selectedBead && !isEditing && !isCreating && (
                  <button 
                    onClick={() => toggleFavorite(selectedBead)}
                    className={cn(
                      "p-2.5 rounded-xl hover:bg-[var(--background-tertiary)] border-2 border-transparent hover:border-[var(--border-primary)] transition-all active:scale-90",
                      selectedBead.is_favorite ? "text-amber-500 bg-amber-500/5 border-amber-500/20" : "text-[var(--text-secondary)]"
                    )}
                  >
                    <Star size={20} className={cn(selectedBead.is_favorite && "fill-current")} />
                  </button>
                )}
                {(isEditing || isCreating) && (
                  <select 
                    value={editForm.status} 
                    onChange={e => {
                      const newStatus = e.target.value;
                      let extra = {};
                      if (newStatus === 'closed' && editForm.status !== 'closed') {
                        const reason = prompt("Enter closure reason:");
                        extra = {
                          closed_at: new Date().toISOString(),
                          close_reason: reason || "Completed"
                        };
                      } else if (newStatus !== 'closed') {
                        extra = {
                          closed_at: undefined,
                          close_reason: undefined
                        };
                      }
                      setEditForm({...editForm, status: newStatus, ...extra});
                    }}
                    className="bg-[var(--background-primary)] text-[11px] font-black text-[var(--text-primary)] border-2 border-[var(--border-primary)] rounded-xl px-4 py-2.5 focus:ring-4 focus:ring-indigo-500/20 focus:border-indigo-500 outline-none uppercase tracking-[0.2em] shadow-sm"
                  >
                    <option value="open">Open</option>
                    <option value="in_progress">In Progress</option>
                    <option value="closed">Closed</option>
                  </select>
                )}
              </div>
              <button onClick={() => { setSelectedBead(null); setIsCreating(false); }} className="p-3 hover:bg-rose-500/10 rounded-2xl text-[var(--text-secondary)] hover:text-rose-600 transition-all active:scale-90 border-2 border-transparent hover:border-rose-500/20">
                <X size={24} strokeWidth={3} />
              </button>
            </div>
            
            <div className="flex-1 overflow-y-auto p-10 flex flex-col gap-12 custom-scrollbar">
              <section className="flex flex-col gap-6">
                {(isEditing || isCreating) ? (
                  <div className="flex flex-col gap-6">
                    <div className="flex gap-4">
                       <div className="flex-1 flex flex-col gap-2.5">
                          <span className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.25em] pl-1">Type</span>
                          <select 
                            value={editForm.issue_type} 
                            onChange={e => setEditForm({...editForm, issue_type: e.target.value})}
                            className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-[12px] font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm"
                          >
                            <option value="task">Task</option>
                            <option value="feature">Feature</option>
                            <option value="bug">Bug</option>
                            <option value="epic">Epic</option>
                          </select>
                       </div>
                       <div className="flex-1 flex flex-col gap-2.5">
                          <span className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.25em] pl-1">Parent</span>
                          <select 
                            value={editForm.parent || ""} 
                            onChange={e => setEditForm({...editForm, parent: e.target.value})}
                            className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-[12px] font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm"
                          >
                            <option value="">None</option>
                            {beads.filter(b => b.issue_type === 'epic' || b.issue_type === 'feature').map(b => (
                              <option key={b.id} value={b.id}>{b.id}: {b.title}</option>
                            ))}
                          </select>
                       </div>
                    </div>
                    
                    <div className="flex flex-col gap-2.5">
                      <span className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.25em] pl-1">Title</span>
                      <input 
                        className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-4 text-lg font-black w-full focus:border-indigo-500 outline-none text-[var(--text-primary)] shadow-sm placeholder:text-[var(--text-muted)]"
                        value={editForm.title}
                        onChange={e => setEditForm({...editForm, title: e.target.value})}
                        placeholder="Enter title..."
                      />
                    </div>

                    <div className="flex flex-col gap-2.5">
                      <span className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.25em] pl-1">Description</span>
                      <textarea 
                        className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-4 text-[13px] font-bold min-h-[160px] w-full focus:border-indigo-500 outline-none resize-none text-[var(--text-primary)] shadow-sm leading-relaxed placeholder:text-[var(--text-muted)]"
                        value={editForm.description || ""}
                        onChange={e => setEditForm({...editForm, description: e.target.value})}
                        placeholder="Enter description..."
                      />
                    </div>
                  </div>
                ) : selectedBead && (
                  <>
                    <h2 className="text-2xl font-black text-[var(--text-primary)] leading-tight tracking-tight drop-shadow-sm">{selectedBead.title}</h2>
                    <p className="text-[13px] font-bold text-[var(--text-secondary)] leading-relaxed">{selectedBead.description || "No description provided."}</p>
                  </>
                )}
              </section>

              <div className="grid grid-cols-2 gap-10">
                <div className="flex flex-col gap-3">
                  <span className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.25em] flex items-center gap-2"><User size={14} className="text-indigo-600 dark:text-indigo-400 stroke-[3]" /> Owner</span>
                  {(isEditing || isCreating) ? (
                    <input 
                      className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-[12px] font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm"
                      value={editForm.owner || ""}
                      onChange={e => setEditForm({...editForm, owner: e.target.value})}
                      placeholder="Assignee"
                    />
                  ) : (
                    <span className="text-[13px] text-[var(--text-primary)] font-black tracking-tight bg-[var(--background-tertiary)] px-4 py-2.5 rounded-xl border-2 border-[var(--border-primary)]/50">{selectedBead?.owner || "Unassigned"}</span>
                  )}
                </div>
                <div className="flex flex-col gap-3">
                  <span className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.25em] flex items-center gap-2"><Tag size={14} className="text-indigo-600 dark:text-indigo-400 stroke-[3]" /> Priority</span>
                  {(isEditing || isCreating) ? (
                    <select 
                      value={editForm.priority} 
                      onChange={e => setEditForm({...editForm, priority: parseInt(e.target.value)})}
                      className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-[12px] font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm"
                    >
                      {[0,1,2,3,4].map(p => <option key={p} value={p}>P{p} - {['Critical', 'High', 'Medium', 'Low', 'Trivial'][p]}</option>)}
                    </select>
                  ) : (
                    <span className="text-[13px] text-[var(--text-primary)] font-black tracking-tight bg-[var(--background-tertiary)] px-4 py-2.5 rounded-xl border-2 border-[var(--border-primary)]/50 text-center">P{selectedBead?.priority}</span>
                  )}
                </div>
              </div>

              <section className="flex flex-col gap-5">
                <h3 className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.3em] pl-1">Labels</h3>
                <div className="flex flex-wrap gap-3">
                  {(isEditing || isCreating) ? (
                    <input 
                      className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-[12px] font-bold text-[var(--text-primary)] w-full focus:border-indigo-500 outline-none shadow-sm"
                      value={(editForm.labels || []).join(", ")}
                      onChange={e => setEditForm({...editForm, labels: e.target.value.split(",").map(l => l.trim()).filter(l => l)})}
                      placeholder="Add labels (comma separated)..."
                    />
                  ) : (
                    <>
                      {selectedBead && <Chip label={selectedBead.issue_type} />}
                      {selectedBead?.labels?.map(l => (
                        <Chip key={l} label={l} />
                      )) || (!selectedBead?.issue_type && <span className="text-[12px] text-[var(--text-muted)] font-black italic">No labels</span>)}
                    </>
                  )}
                </div>
              </section>

              <section className="flex flex-col gap-5">
                <h3 className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.3em] pl-1">Notes & References</h3>
                <div className="flex flex-col gap-8">
                  {(isEditing || isCreating) ? (
                    <>
                      <div className="flex flex-col gap-2.5">
                        <span className="text-[9px] font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">Design Notes</span>
                        <textarea 
                          className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-4 text-[12px] font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none min-h-[100px] resize-none shadow-sm leading-relaxed"
                          value={editForm.design_notes || ""}
                          onChange={e => setEditForm({...editForm, design_notes: e.target.value})}
                          placeholder="Architectural decisions..."
                        />
                      </div>
                      <div className="flex flex-col gap-2.5">
                        <span className="text-[9px] font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">Working Notes</span>
                        <textarea 
                          className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-4 text-[12px] font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none min-h-[100px] resize-none shadow-sm leading-relaxed"
                          value={editForm.working_notes || ""}
                          onChange={e => setEditForm({...editForm, working_notes: e.target.value})}
                          placeholder="Progress observations..."
                        />
                      </div>
                      <div className="flex flex-col gap-2.5">
                        <span className="text-[9px] font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">External Reference</span>
                        <input 
                          className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-[12px] font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm"
                          value={editForm.external_reference || ""}
                          onChange={e => setEditForm({...editForm, external_reference: e.target.value})}
                          placeholder="URLs, IDs, or paths..."
                        />
                      </div>
                    </>
                  ) : (
                    <>
                      {selectedBead?.design_notes && (
                        <div className="flex flex-col gap-3">
                           <span className="text-[9px] font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">Design Notes</span>
                           <p className="text-[13px] font-bold text-[var(--text-secondary)] leading-relaxed whitespace-pre-wrap bg-[var(--background-secondary)] p-4 rounded-2xl border-2 border-[var(--border-primary)] shadow-sm">{selectedBead.design_notes}</p>
                        </div>
                      )}
                      {selectedBead?.working_notes && (
                        <div className="flex flex-col gap-3">
                           <span className="text-[9px] font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">Working Notes</span>
                           <p className="text-[13px] font-bold text-[var(--text-secondary)] leading-relaxed whitespace-pre-wrap bg-[var(--background-secondary)] p-4 rounded-2xl border-2 border-[var(--border-primary)] shadow-sm">{selectedBead.working_notes}</p>
                        </div>
                      )}
                      {selectedBead?.external_reference && (
                        <div className="flex flex-col gap-3">
                           <span className="text-[9px] font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">External Reference</span>
                           <span className="text-[12px] text-indigo-700 dark:text-indigo-400 font-black font-mono bg-indigo-500/10 p-3 rounded-xl border-2 border-indigo-500/20 shadow-sm">{selectedBead.external_reference}</span>
                        </div>
                      )}
                    </>
                  )}
                </div>
              </section>

              <section className="flex flex-col gap-6">
                <h3 className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.3em] pl-1">Acceptance Criteria</h3>
                <div className="flex flex-col gap-4">
                  {(isEditing || isCreating) ? (
                    <>
                      {(editForm.acceptance_criteria || []).map((ac, i) => (
                        <div key={i} className="flex gap-3">
                          <input 
                            className="flex-1 bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-[12px] font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm"
                            value={ac}
                            onChange={e => {
                              const newAC = [...(editForm.acceptance_criteria || [])];
                              newAC[i] = e.target.value;
                              setEditForm({...editForm, acceptance_criteria: newAC});
                            }}
                          />
                          <button 
                            onClick={() => {
                              const newAC = (editForm.acceptance_criteria || []).filter((_, idx) => idx !== i);
                              setEditForm({...editForm, acceptance_criteria: newAC});
                            }}
                            className="text-rose-600 dark:text-rose-400 hover:bg-rose-500/10 p-3 rounded-xl transition-colors border-2 border-transparent hover:border-rose-500/20 active:scale-90"
                          >
                            <Trash2 size={20} strokeWidth={3} />
                          </button>
                        </div>
                      ))}
                      <button 
                        onClick={() => setEditForm({...editForm, acceptance_criteria: [...(editForm.acceptance_criteria || []), ""]})}
                        className="text-[11px] font-black text-indigo-700 dark:text-indigo-400 hover:text-indigo-800 self-start flex items-center gap-3 bg-indigo-500/10 px-5 py-3 rounded-2xl border-2 border-indigo-500/20 transition-all hover:bg-indigo-500/10 active:scale-95 uppercase tracking-widest"
                      >
                        <Plus size={16} strokeWidth={3} /> ADD CRITERION
                      </button>
                    </>
                  ) : (
                    (selectedBead?.acceptance_criteria || []).map((ac, i) => (
                      <div key={i} className="flex gap-4 items-start bg-[var(--background-secondary)] p-4 rounded-2xl border-2 border-[var(--border-primary)] shadow-sm">
                        <CheckCircle2 size={18} className="text-emerald-600 dark:text-emerald-400 shrink-0 mt-0.5 stroke-[3]" />
                        <span className="text-[13px] font-bold text-[var(--text-secondary)] leading-relaxed">{ac}</span>
                      </div>
                    )) || <span className="text-[12px] text-[var(--text-muted)] font-black italic pl-1">None specified</span>
                  )}
                </div>
              </section>

              <div className="grid grid-cols-2 gap-10">
                <div className="flex flex-col gap-3">
                  <span className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.25em] flex items-center gap-2"><Clock size={14} className="text-indigo-600 dark:text-indigo-400 stroke-[3]" /> Estimate</span>
                  {(isEditing || isCreating) ? (
                    <input 
                      type="number"
                      className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-[12px] font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm"
                      value={editForm.estimate || ""}
                      onChange={e => setEditForm({...editForm, estimate: parseInt(e.target.value) || 0})}
                      placeholder="Minutes"
                    />
                  ) : (
                    <span className="text-[13px] text-[var(--text-primary)] font-black tracking-tight bg-[var(--background-tertiary)] px-4 py-2.5 rounded-xl border-2 border-[var(--border-primary)]/50 text-center">{selectedBead?.estimate || 0}m</span>
                  )}
                </div>
                <div className="flex flex-col gap-3">
                  <span className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.25em] flex items-center gap-2">Status</span>
                  {(isEditing || isCreating) ? (
                     <select 
                      value={editForm.status} 
                      onChange={e => {
                        const newStatus = e.target.value;
                        let extra = {};
                        if (newStatus === 'closed' && editForm.status !== 'closed') {
                          const reason = prompt("Enter closure reason:");
                          extra = {
                            closed_at: new Date().toISOString(),
                            close_reason: reason || "Completed"
                          };
                        } else if (newStatus !== 'closed') {
                          extra = {
                            closed_at: undefined,
                            close_reason: undefined
                          };
                        }
                        setEditForm({...editForm, status: newStatus, ...extra});
                      }}
                      className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-[12px] font-black text-[var(--text-primary)] focus:border-indigo-500 outline-none uppercase tracking-[0.2em] shadow-sm"
                    >
                      <option value="open">Open</option>
                      <option value="in_progress">In Progress</option>
                      <option value="closed">Closed</option>
                    </select>
                  ) : (
                    <div className="flex items-center gap-3 bg-indigo-700 dark:bg-indigo-500 px-4 py-2.5 rounded-xl border-2 border-indigo-800 dark:border-indigo-400 self-start shadow-md">
                      <StatusIcon status={selectedBead?.status || ""} size={16} className="text-white" />
                      <span className="text-[11px] text-white font-black uppercase tracking-widest">{selectedBead?.status.replace('_', ' ')}</span>
                    </div>
                  )}
                </div>
              </div>

              <section className="flex flex-col gap-6 pb-6">
                <div className="flex items-center justify-between">
                  <h3 className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.3em] pl-1">Dependencies</h3>
                  {(isEditing || isCreating) && (
                    <button 
                      onClick={() => {
                        const target = prompt("Enter Target Bead ID:");
                        if (target) {
                          const newDeps = [...(editForm.dependencies || []), { issue_id: isCreating ? editForm.id! : selectedBead!.id, depends_on_id: target, type: "blocks" }];
                          setEditForm({...editForm, dependencies: newDeps as any});
                        }
                      }}
                      className="text-[11px] font-black text-indigo-700 dark:text-indigo-400 hover:text-indigo-800 flex items-center gap-3 bg-indigo-500/10 px-5 py-3 rounded-2xl border-2 border-indigo-500/20 transition-all hover:bg-indigo-500/10 active:scale-95 uppercase tracking-widest"
                    >
                      <Plus size={16} strokeWidth={3} /> ADD
                    </button>
                  )}
                </div>
                <div className="flex flex-col gap-4">
                  {(isCreating ? editForm.dependencies : selectedBead?.dependencies)?.map((d, i) => (
                    <div key={i} className="flex items-center justify-between p-5 rounded-2xl bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] group hover:border-indigo-500 shadow-sm transition-all">
                      <div className="flex flex-col gap-2">
                        <div className="flex items-center gap-4">
                           <span className="text-[11px] font-black font-mono text-indigo-700 dark:text-indigo-400 bg-indigo-500/10 px-3 py-1 rounded-lg border-2 border-indigo-500/20 shadow-sm">{d.depends_on_id}</span>
                           <span className="text-[10px] uppercase font-black text-[var(--text-primary)] tracking-[0.2em]">{d.type}</span>
                        </div>
                        {d.metadata && Object.keys(d.metadata).length > 0 && (
                          <div className="flex flex-wrap gap-2 mt-1 pl-1">
                            {Object.entries(d.metadata).map(([k, v]) => (
                              <span key={k} className="text-[9px] font-black bg-[var(--background-tertiary)] px-2.5 py-1 rounded-lg text-[var(--text-muted)] border-2 border-[var(--border-primary)]/50 font-mono tracking-tight">
                                {k}: {String(v)}
                              </span>
                            ))}
                          </div>
                        )}
                      </div>
                      {(isEditing || isCreating) && (
                        <button 
                          onClick={() => {
                            const currentDeps = editForm.dependencies || [];
                            const newDeps = currentDeps.filter((_, index) => index !== i);
                            setEditForm({...editForm, dependencies: newDeps});
                          }}
                          className="text-rose-600 dark:text-rose-400 hover:bg-rose-500/10 p-3 rounded-xl transition-colors border-2 border-transparent hover:border-rose-500/20 active:scale-90"
                        >
                          <Trash2 size={20} strokeWidth={3} />
                        </button>
                      )}
                    </div>
                  )) || <div className="text-[13px] text-[var(--text-muted)] font-black italic pl-1">None</div>}
                </div>
              </section>
            </div>
            
            <div className="p-8 border-t-2 border-[var(--border-primary)] bg-[var(--background-secondary)] flex gap-5 shadow-2xl backdrop-blur-md">
              {(isEditing || isCreating) ? (
                <>
                  <button onClick={() => { setIsEditing(false); setIsCreating(false); }} className="flex-1 py-4 rounded-2xl bg-[var(--background-tertiary)] hover:bg-[var(--border-primary)] text-[var(--text-primary)] text-xs font-black transition-all border-2 border-[var(--border-primary)] active:scale-95 uppercase tracking-widest">Cancel</button>
                  <button onClick={isCreating ? handleSaveCreate : handleSaveEdit} className="flex-1 py-4 rounded-2xl bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-black transition-all shadow-xl shadow-emerald-600/30 flex items-center justify-center gap-3 active:scale-95 border-2 border-emerald-700 uppercase tracking-widest"><Save size={20} strokeWidth={3} /> {isCreating ? "Create Bead" : "Save Changes"}</button>
                </>
              ) : (
                <button onClick={handleStartEdit} className="w-full py-4 rounded-2xl bg-indigo-700 hover:bg-indigo-600 text-white text-xs font-black transition-all shadow-xl shadow-indigo-700/30 flex items-center justify-center gap-3 active:scale-95 border-2 border-indigo-800 uppercase tracking-widest"><Edit3 size={20} strokeWidth={3} /> Edit Bead</button>
              )}
            </div>
          </div>
        )}
                    </button>
                  )}
                </div>
                <div className="flex flex-col gap-3">
                  {(isCreating ? editForm.dependencies : selectedBead?.dependencies)?.map((d, i) => (
                    <div key={i} className="flex items-center justify-between p-4 rounded-xl bg-[var(--background-secondary)] border border-[var(--border-primary)] group hover:border-indigo-500/30 transition-all">
                      <div className="flex flex-col gap-1.5">
                        <div className="flex items-center gap-3">
                           <span className="text-[10px] font-mono text-indigo-500 font-bold bg-indigo-500/5 px-2 py-0.5 rounded border border-indigo-500/10">{d.depends_on_id}</span>
                           <span className="text-[9px] uppercase font-black text-[var(--text-muted)] tracking-widest">{d.type}</span>
                        </div>
                        {d.metadata && Object.keys(d.metadata).length > 0 && (
                          <div className="flex flex-wrap gap-1.5 mt-1">
                            {Object.entries(d.metadata).map(([k, v]) => (
                              <span key={k} className="text-[8px] bg-[var(--background-tertiary)] px-2 py-0.5 rounded text-[var(--text-muted)] border border-[var(--border-primary)]/50 font-mono">
                                {k}: {String(v)}
                              </span>
                            ))}
                          </div>
                        )}
                      </div>
                      {(isEditing || isCreating) && (
                        <button 
                          onClick={() => {
                            const currentDeps = editForm.dependencies || [];
                            const newDeps = currentDeps.filter((_, index) => index !== i);
                            setEditForm({...editForm, dependencies: newDeps});
                          }}
                          className="text-[var(--text-muted)] hover:text-rose-500 p-2 opacity-0 group-hover:opacity-100 transition-all"
                        >
                          <Trash2 size={16} />
                        </button>
                      )}
                    </div>
                  )) || <div className="text-xs text-[var(--text-muted)] italic">None</div>}
                </div>
              </section>
            </div>
            
            <div className="p-6 border-t border-[var(--border-primary)] bg-[var(--background-secondary)]/50 flex gap-4 backdrop-blur-md">
              {(isEditing || isCreating) ? (
                <>
                  <button onClick={() => { setIsEditing(false); setIsCreating(false); }} className="flex-1 py-3 rounded-xl bg-[var(--background-tertiary)] hover:bg-[var(--border-primary)] text-[var(--text-secondary)] text-xs font-bold transition-all border border-[var(--border-primary)] active:scale-95">Cancel</button>
                  <button onClick={isCreating ? handleSaveCreate : handleSaveEdit} className="flex-1 py-3 rounded-xl bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-bold transition-all shadow-lg shadow-emerald-600/20 flex items-center justify-center gap-2 active:scale-95 border border-emerald-500/20"><Save size={16} /> {isCreating ? "Create Bead" : "Save Changes"}</button>
                </>
              ) : (
                <button onClick={handleStartEdit} className="w-full py-3 rounded-xl bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-bold transition-all shadow-lg shadow-indigo-600/20 flex items-center justify-center gap-2 active:scale-95 border border-indigo-500/20"><Edit3 size={16} /> Edit Bead</button>
              )}
            </div>
          </div>
        )}
      </main>

      <style dangerouslySetInnerHTML={{ __html: `
        .custom-scrollbar::-webkit-scrollbar { width: 8px; height: 8px; }
        .custom-scrollbar::-webkit-scrollbar-thumb { background: var(--text-muted); border-radius: 10px; border: 2px solid var(--background-primary); }
        .custom-scrollbar::-webkit-scrollbar-thumb:hover { background: var(--text-secondary); }
      `}} />
    </div>
  );
}

export default App;
