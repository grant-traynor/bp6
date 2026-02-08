import { useState, useEffect, useCallback, useRef, useMemo } from "react";
import { listen } from "@tauri-apps/api/event";
import { ListTree, Settings, ChevronRight, ChevronDown, Package, CheckCircle2, Circle, Clock, X, User, Tag, Save, Edit3, Trash2, Plus, Flame, Star, Sun, Moon } from "lucide-react";
import { twMerge } from "tailwind-merge";
import { clsx, type ClassValue } from "clsx";
import { fetchBeads, buildWBSTree, calculateGanttLayout, updateBead, createBead, type WBSNode, type Bead, type GanttItem, type GanttLayout } from "./api";

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

// --- Custom Components ---

const getChipStyles = (label: string) => {
  const l = label.toLowerCase();
  if (l === 'epic') return "bg-purple-500/10 text-purple-400 border-purple-500/20";
  if (l === 'bug') return "bg-red-500/10 text-red-400 border-red-500/20";
  if (l === 'feature') return "bg-emerald-500/10 text-emerald-400 border-emerald-500/20";
  if (l === 'task') return "bg-blue-500/10 text-blue-400 border-blue-500/20";
  if (l.includes('infra')) return "bg-orange-500/10 text-orange-400 border-orange-500/20";
  if (l.includes('doc')) return "bg-cyan-500/10 text-cyan-400 border-cyan-500/20";
  return "bg-zinc-500/10 text-zinc-400 border-zinc-500/20";
};

const Chip = ({ label }: { label: string }) => (
  <span className={cn("px-2 py-0.5 rounded-md text-xs font-bold border transition-colors", getChipStyles(label))}>
    {label}
  </span>
);

const StatusIcon = ({ status, size = 14 }: { status: string; size?: number }) => {
  switch (status) {
    case 'closed': return <CheckCircle2 size={size} className="text-emerald-500" />;
    case 'in_progress': return <Clock size={size} className="text-amber-500" />;
    default: return <Circle size={size} className="text-zinc-500" />;
  }
};

const GanttBar = ({ item, onClick }: { item: GanttItem; onClick: (bead: Bead) => void }) => {
  const { bead, isCritical, isBlocked, x, width } = item;
  const isEpic = bead.issue_type === 'epic';
  const isMilestone = bead.estimate === 0;

  const getStatusColor = () => {
    if (isEpic) return "bg-zinc-800 border-zinc-700";
    if (isMilestone) return "bg-zinc-100 border-zinc-400 rotate-45";
    if (bead.status === 'closed') return "bg-blue-500 border-blue-400";
    if (isCritical) return "bg-red-500 border-red-400";
    if (isBlocked) return "bg-amber-500 border-amber-400";
    if (bead.status === 'in_progress') return "bg-emerald-500 border-emerald-400";
    return "bg-zinc-600 border-zinc-500";
  };

  return (
    <div 
      className="absolute h-full flex items-center group cursor-pointer"
      style={{ left: x, width: isMilestone ? 12 : width }}
      onClick={() => onClick(bead)}
    >
      <div className="flex-1 flex items-center h-full relative">
        {isEpic ? (
          <div className="w-full h-2 bg-zinc-400 relative">
             <div className="absolute left-0 -top-1 bottom-[-4px] w-1.5 bg-zinc-400" />
             <div className="absolute right-0 -top-1 bottom-[-4px] w-1.5 bg-zinc-400" />
          </div>
        ) : isMilestone ? (
          <div className={cn("w-3 h-3 border shadow-sm", getStatusColor())} />
        ) : (
          <div className={cn(
            "h-4 rounded-sm border shadow-sm transition-all w-full",
            getStatusColor(),
            bead.status === 'closed' ? "opacity-40" : "opacity-100"
          )} />
        )}

        <div className="absolute left-full ml-4 flex items-center gap-2 whitespace-nowrap pointer-events-none z-10">
          <span className={cn(
            "text-sm font-bold tracking-tight",
            bead.status === 'closed' ? "text-zinc-600" : "text-zinc-300"
          )}>{bead.title}</span>
          {isCritical && !isMilestone && <Flame size={10} className="text-red-500 fill-current opacity-60" />}
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
    <div className="select-none h-[40px] flex flex-col justify-center border-b border-zinc-200 dark:border-zinc-900/30">
      <div 
        className={cn(
          "flex items-center hover:bg-zinc-100 dark:bg-zinc-900/50 cursor-pointer group transition-all h-full",
          node.isCritical && "bg-red-500/5"
        )}
        onClick={() => onClick(node as Bead)}
      >
        <div 
          className="w-8 shrink-0 flex items-center justify-center h-full hover:text-zinc-900 dark:text-zinc-100 text-zinc-600 transition-colors" 
          onClick={(e) => { e.stopPropagation(); onToggle(node.id); }}
        >
          {hasChildren ? (
            node.isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />
          ) : null}
        </div>

        <div className="w-24 shrink-0 px-2 flex items-center h-full border-r border-zinc-200 dark:border-zinc-900/50">
           <span className={cn(
             "font-mono text-xs font-bold px-1.5 py-0.5 rounded tracking-tighter",
             node.isCritical ? "bg-red-500/20 text-red-400" : "bg-zinc-800 text-zinc-500"
           )}>
             {node.id}
           </span>
        </div>

        <div 
          className="flex-1 px-3 flex items-center gap-2 truncate h-full"
          style={{ paddingLeft: `${depth * 0.75 + 0.75}rem` }}
        >
          <StatusIcon status={node.status} size={10} />
          <span className={cn(
            "text-sm truncate font-semibold tracking-tight",
            node.status === 'closed' ? "text-zinc-600 line-through" : "text-zinc-300"
          )}>
            {node.title}
          </span>
          {node.issue_type !== 'task' && (
            <span className={cn(
              "text-[7px] font-black px-1 rounded-sm uppercase tracking-tighter border",
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

  useEffect(() => {
    if (isDark) {
      document.documentElement.classList.remove('light');
    } else {
      document.documentElement.classList.add('light');
    }
  }, [isDark]);

  const scrollRefWBS = useRef<HTMLDivElement>(null);
  const scrollRefBERT = useRef<HTMLDivElement>(null);
  const activeScrollSource = useRef<HTMLDivElement | null>(null);

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
    loadData();

    // Listen for backend file changes
    const unlisten = listen("beads-updated", () => {
      loadData();
    });

    // Periodic refresh every 10s
    const interval = setInterval(loadData, 10000);

    return () => {
      unlisten.then(f => f());
      clearInterval(interval);
    };
  }, [loadData]);

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
      setEditForm({ ...selectedBead });
      setIsEditing(true);
      setIsCreating(false);
    }
  };

  const handleSaveEdit = async () => {
    if (selectedBead && editForm) {
      const updated = { ...selectedBead, ...editForm };
      await updateBead(updated as Bead);
      setSelectedBead(updated as Bead);
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
      created_at: new Date().toISOString()
    });
    setIsEditing(false);
    setIsCreating(true);
  };

  const handleSaveCreate = async () => {
    if (editForm.title) {
      await createBead(editForm as Bead);
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

  const favorites = useMemo(() => beads.filter(b => b.is_favorite), [beads]);

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
    <div className="flex h-screen w-screen overflow-hidden bg-white dark:bg-zinc-950 text-zinc-900 dark:text-zinc-100 font-sans">
      <nav className="w-16 flex flex-col items-center py-6 border-r border-zinc-200 dark:border-zinc-900 bg-white dark:bg-zinc-950 z-30">
        <div className="flex flex-col gap-2 flex-1">
          <button className="p-3 rounded-xl bg-zinc-100 dark:bg-zinc-900 text-indigo-400 transition-all"><ListTree size={22} /></button>
        </div>
        <div className="mt-auto border-t border-zinc-200 dark:border-zinc-900 pt-4"><Settings size={20} className="text-zinc-600 p-3" /></div>
      </nav>

      <main className="flex-1 flex flex-col min-w-0 bg-white dark:bg-zinc-950 relative">
        <header className="h-12 border-b border-zinc-200 dark:border-zinc-900 flex items-center px-6 justify-between bg-white dark:bg-zinc-950/50 backdrop-blur-md z-20">
          <div className="flex items-center gap-6">
            <div className="flex items-center gap-2">
               <div className="w-6 h-6 bg-indigo-600 rounded-lg flex items-center justify-center font-black text-xs shadow-lg text-white">B</div>
               <h1 className="text-sm font-black tracking-tighter uppercase">BERT <span className="text-indigo-400 font-mono">bp6</span></h1>
            </div>
            <div className="h-4 w-px bg-zinc-800" />
            <div className="flex items-center gap-3">
               <div className="flex items-center gap-1.5">
                  <div className="w-1.5 h-1.5 rounded-full bg-emerald-500 shadow-[0_0_8px_rgba(16,185,129,0.5)] animate-pulse" />
                  <span className="text-xs font-bold text-zinc-400 uppercase tracking-widest">Live</span>
               </div>
               <span className="text-xs text-zinc-600 font-medium">/ Workspace / <span className="text-zinc-300">Default</span></span>
            </div>
          </div>
          <div className="flex items-center gap-1.5">
            <button onClick={() => setIsDark(!isDark)} className="h-8 w-8 bg-zinc-100 dark:bg-zinc-900 hover:bg-zinc-800 text-zinc-400 hover:text-zinc-900 dark:text-zinc-100 rounded-md border border-zinc-800 flex items-center justify-center transition-all">
              {isDark ? <Sun size={14} /> : <Moon size={14} />}
            </button>
            <button onClick={handleStartCreate} className="h-8 bg-indigo-600 hover:bg-indigo-500 text-white px-3 rounded-md text-xs font-bold flex items-center gap-2 transition-all shadow-lg shadow-indigo-600/10">
              <Plus size={12} /> New Bead
            </button>
            <button onClick={loadData} className="h-8 bg-zinc-100 dark:bg-zinc-900 hover:bg-zinc-800 text-zinc-300 px-3 rounded-md text-xs font-bold border border-zinc-800 flex items-center gap-2 transition-all">
              <Package size={12} className="text-indigo-400" /> Sync
            </button>
          </div>
        </header>

        <div className="flex-1 flex flex-col overflow-hidden">
          {/* Shared Header Row for WBS and Gantt */}
          <div className="flex shrink-0 border-b border-zinc-200 dark:border-zinc-900 bg-zinc-100/90 dark:bg-zinc-900/90 backdrop-blur-md z-20">
            {/* WBS Header Area */}
            <div className="w-1/3 min-w-[420px] border-r border-zinc-200 dark:border-zinc-900 flex flex-col">
              <div className="px-6 py-3 border-b border-zinc-200 dark:border-zinc-900">
                <h2 className="text-xs font-black text-zinc-500 uppercase tracking-[0.2em] flex items-center gap-2">Task Breakdown</h2>
              </div>
              {favorites.length > 0 && (
                <div className="px-6 py-3 border-b border-zinc-200 dark:border-zinc-900 bg-indigo-500/5">
                  <h2 className="text-xs font-black text-indigo-400 uppercase tracking-[0.2em] flex items-center gap-2 mb-2"><Star size={10} className="fill-current" /> Favorites</h2>
                  <div className="flex flex-wrap gap-1.5">
                    {favorites.map(f => (
                      <div key={f.id} onClick={() => handleBeadClick(f)} className="flex items-center gap-2 px-2 py-1 rounded bg-zinc-100 dark:bg-zinc-900/50 border border-zinc-800 hover:border-indigo-500/50 cursor-pointer transition-all">
                        <span className="font-mono text-[8px] font-bold text-zinc-500">{f.id}</span>
                        <span className="text-[10px] font-medium text-zinc-300 truncate max-w-[120px]">{f.title}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
              <div className="px-4 py-2 border-b border-zinc-200 dark:border-zinc-900 bg-white dark:bg-zinc-950/20">
                <input 
                  type="text"
                  placeholder="Filter by title, ID, owner, or label..."
                  className="w-full bg-zinc-100 dark:bg-zinc-900 border border-zinc-800 rounded-lg px-3 py-1 text-sm text-zinc-300 focus:border-indigo-500 outline-none transition-all"
                  value={filterText}
                  onChange={e => setFilterText(e.target.value)}
                />
              </div>
              <div className="flex items-center px-4 py-2 bg-white dark:bg-zinc-950/50 text-xs font-black text-zinc-600 uppercase tracking-widest mt-auto">
                <div className="w-8 shrink-0" />
                <div className="w-24 shrink-0 px-2 border-r border-zinc-200 dark:border-zinc-900/50">ID</div>
                <div className="flex-1 px-3">Name</div>
              </div>
            </div>

            {/* Gantt Header Area (Metrics + Controls) */}
            <div className="flex-1 flex items-end justify-between px-6 py-2 bg-[#09090b]">
               <div className="flex items-center gap-6 mb-1">
                  <div className="flex flex-col">
                     <span className="text-[9px] font-black text-zinc-500 uppercase tracking-widest">Total</span>
                     <span className="text-sm font-bold text-zinc-100">{stats.total}</span>
                  </div>
                  <div className="flex flex-col">
                     <span className="text-[9px] font-black text-emerald-500 uppercase tracking-widest">Open</span>
                     <span className="text-sm font-bold text-emerald-400">{stats.open}</span>
                  </div>
                  <div className="flex flex-col">
                     <span className="text-[9px] font-black text-amber-500 uppercase tracking-widest">Active</span>
                     <span className="text-sm font-bold text-amber-400">{stats.inProgress}</span>
                  </div>
                  <div className="flex flex-col">
                     <span className="text-[9px] font-black text-rose-500 uppercase tracking-widest">Blocked</span>
                     <span className="text-sm font-bold text-rose-400">{stats.blocked}</span>
                  </div>
                  <div className="flex flex-col">
                     <span className="text-[9px] font-black text-zinc-500 uppercase tracking-widest">Done</span>
                     <span className="text-sm font-bold text-zinc-400">{stats.closed}</span>
                  </div>
               </div>

               <div className="flex items-center gap-1 bg-zinc-100 dark:bg-zinc-900/80 backdrop-blur-md p-1 rounded-lg border border-zinc-800 shadow-xl mb-1">
                  <button onClick={() => setZoom(Math.max(0.25, zoom - 0.25))} className="p-1 hover:bg-zinc-800 rounded text-zinc-400 hover:text-zinc-900 dark:text-zinc-100 transition-all text-xs font-bold px-2">-</button>
                  <span className="text-xs font-mono font-bold text-zinc-500 min-w-[40px] text-center">{Math.round(zoom * 100)}%</span>
                  <button onClick={() => setZoom(Math.min(3, zoom + 0.25))} className="p-1 hover:bg-zinc-800 rounded text-zinc-400 hover:text-zinc-900 dark:text-zinc-100 transition-all text-xs font-bold px-2">+</button>
                  <div className="w-px h-4 bg-zinc-800 mx-1" />
                  <button onClick={() => setZoom(1)} className="px-2 py-1 hover:bg-zinc-800 rounded text-zinc-400 hover:text-zinc-900 dark:text-zinc-100 transition-all text-xs font-bold uppercase tracking-tighter">Reset</button>
               </div>
            </div>
          </div>

          <div className="flex-1 flex overflow-hidden">
            {/* WBS Side */}
            <div 
              ref={scrollRefWBS}
              onScroll={handleScroll}
              onMouseEnter={handleMouseEnter}
              className="w-1/3 border-r border-zinc-200 dark:border-zinc-900 flex flex-col bg-white dark:bg-zinc-950/30 min-w-[420px] overflow-y-auto custom-scrollbar"
            >
              <div className="p-0">
                {loading ? <div className="p-8 animate-pulse text-zinc-700 text-sm">Syncing Schedule...</div> : (
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
              className="flex-1 relative bg-[#09090b] overflow-auto custom-scrollbar"
            >
              <div className="relative" style={{ height: ganttLayout.rowCount * 40, width: 5000 * zoom }}>
                 {/* Grid */}
                 <div className="absolute inset-0 pointer-events-none">
                    {Array.from({ length: ganttLayout.rowCount }).map((_, i) => (
                      <div key={i} className="w-full border-b border-zinc-200 dark:border-zinc-900/50" style={{ height: '40px' }} />
                    ))}
                    <div className="absolute inset-0 flex">
                      {Array.from({ length: Math.ceil(50 * zoom) }).map((_, i) => (
                        <div key={i} className="h-full border-r border-zinc-200 dark:border-zinc-900/30" style={{ width: 100 * zoom }} />
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
                      stroke={c.isCritical ? "#ef4444" : "#3f3f46"}
                      strokeWidth={c.isCritical ? 2 : 1}
                      strokeDasharray={c.isCritical ? "0" : "4 2"}
                      opacity={0.6}
                    />
                  ))}
               </svg>

               {/* Items */}
               {ganttLayout.items.map((item: any, i: number) => (
                 <div key={i} className="absolute w-full" style={{ top: item.row * 40, height: 40 }}>
                    <GanttBar item={item} onClick={handleBeadClick} />
                 </div>
               ))}
            </div>
          </div>
        </div>
      </div>

        {/* Sidebar */}
        {(selectedBead || isCreating) && (
          <div className="absolute right-0 top-0 bottom-0 w-[400px] bg-zinc-100 dark:bg-zinc-900 border-l border-zinc-800 shadow-2xl z-50 flex flex-col animate-in slide-in-from-right duration-300">
            <div className="p-6 border-b border-zinc-800 flex items-center justify-between">
              <div className="flex items-center gap-3">
                <span className="font-mono text-xs px-2 py-1 rounded bg-zinc-800 text-zinc-400 font-bold">
                  {isCreating ? "New Bead" : selectedBead?.id}
                </span>
                {selectedBead && !isEditing && !isCreating && (
                  <button 
                    onClick={() => toggleFavorite(selectedBead)}
                    className={cn(
                      "p-1 rounded hover:bg-zinc-800 transition-all",
                      selectedBead.is_favorite ? "text-amber-500" : "text-zinc-600"
                    )}
                  >
                    <Star size={14} className={cn(selectedBead.is_favorite && "fill-current")} />
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
                    className="bg-zinc-800 text-sm border-none rounded p-1 focus:ring-0"
                  >
                    <option value="open">Open</option>
                    <option value="in_progress">In Progress</option>
                    <option value="closed">Closed</option>
                  </select>
                )}
              </div>
              <button onClick={() => { setSelectedBead(null); setIsCreating(false); }} className="p-2 hover:bg-zinc-800 rounded-lg text-zinc-500 hover:text-zinc-200 transition-colors">
                <X size={18} />
              </button>
            </div>
            
            <div className="flex-1 overflow-y-auto p-8 flex flex-col gap-8 custom-scrollbar">
              <section className="flex flex-col gap-4">
                {(isEditing || isCreating) ? (
                  <>
                    <input 
                      className="bg-white dark:bg-zinc-950 border border-zinc-800 rounded-lg p-3 text-lg font-bold w-full focus:border-indigo-500 outline-none"
                      value={editForm.title}
                      onChange={e => setEditForm({...editForm, title: e.target.value})}
                      placeholder="Bead Title"
                      autoFocus
                    />
                    <textarea 
                      className="bg-white dark:bg-zinc-950 border border-zinc-800 rounded-lg p-3 text-sm min-h-[120px] w-full focus:border-indigo-500 outline-none resize-none"
                      value={editForm.description || ""}
                      onChange={e => setEditForm({...editForm, description: e.target.value})}
                      placeholder="Bead Description"
                    />
                    {isCreating && (
                      <div className="flex flex-col gap-1">
                        <span className="text-xs font-black text-zinc-600 uppercase tracking-wider">Type</span>
                        <select 
                          value={editForm.issue_type} 
                          onChange={e => setEditForm({...editForm, issue_type: e.target.value})}
                          className="bg-white dark:bg-zinc-950 border border-zinc-800 rounded-lg p-2 text-sm text-zinc-300"
                        >
                          <option value="task">Task</option>
                          <option value="feature">Feature</option>
                          <option value="bug">Bug</option>
                          <option value="epic">Epic</option>
                        </select>
                      </div>
                    )}
                  </>
                ) : selectedBead && (
                  <>
                    <h2 className="text-xl font-bold text-zinc-900 dark:text-zinc-100 leading-tight">{selectedBead.title}</h2>
                    <p className="text-sm text-zinc-400 leading-relaxed">{selectedBead.description || "No description provided."}</p>
                  </>
                )}
              </section>

              {(isEditing || isCreating || selectedBead) && (
                <div className="grid grid-cols-2 gap-6">
                  <div className="flex flex-col gap-1">
                    <span className="text-xs font-black text-zinc-600 uppercase tracking-wider flex items-center gap-2"><User size={12} /> Owner</span>
                    <span className="text-sm text-zinc-300 font-medium">{selectedBead?.owner || "Unassigned"}</span>
                  </div>
                  <div className="flex flex-col gap-1">
                    <span className="text-xs font-black text-zinc-600 uppercase tracking-wider flex items-center gap-2"><Tag size={12} /> Priority</span>
                    {(isEditing || isCreating) ? (
                      <select 
                        value={editForm.priority} 
                        onChange={e => setEditForm({...editForm, priority: parseInt(e.target.value)})}
                        className="bg-zinc-800 text-sm border-none rounded p-1"
                      >
                        {[0,1,2,3,4].map(p => <option key={p} value={p}>P{p}</option>)}
                      </select>
                    ) : (
                      <span className="text-sm text-zinc-300 font-medium">P{selectedBead?.priority}</span>
                    )}
                  </div>
                </div>
              )}

              <section className="flex flex-col gap-3">
                <h3 className="text-xs font-black text-zinc-600 uppercase tracking-[0.2em]">Labels & Type</h3>
                <div className="flex flex-wrap gap-2">
                  {selectedBead && <Chip label={selectedBead.issue_type} />}
                  {(isCreating ? editForm.labels : selectedBead?.labels)?.map(l => (
                    <Chip key={l} label={l} />
                  )) || (!selectedBead?.issue_type && <span className="text-sm text-zinc-700 italic">No labels</span>)}
                </div>
              </section>

              <section className="flex flex-col gap-3">
                <h3 className="text-xs font-black text-zinc-600 uppercase tracking-[0.2em]">Acceptance Criteria</h3>
                <div className="flex flex-col gap-2">
                  {(isEditing || isCreating) ? (
                    <>
                      {(editForm.acceptance_criteria || []).map((ac, i) => (
                        <div key={i} className="flex gap-2">
                          <input 
                            className="flex-1 bg-white dark:bg-zinc-950 border border-zinc-800 rounded-lg p-2 text-sm text-zinc-300 focus:border-indigo-500 outline-none"
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
                            className="text-zinc-600 hover:text-red-400"
                          >
                            <Trash2 size={12} />
                          </button>
                        </div>
                      ))}
                      <button 
                        onClick={() => setEditForm({...editForm, acceptance_criteria: [...(editForm.acceptance_criteria || []), ""]})}
                        className="text-xs font-bold text-indigo-400 hover:text-indigo-300 self-start flex items-center gap-1"
                      >
                        <Plus size={10} /> Add Criterion
                      </button>
                    </>
                  ) : (
                    (selectedBead?.acceptance_criteria || []).map((ac, i) => (
                      <div key={i} className="flex gap-2 items-start">
                        <CheckCircle2 size={12} className="text-zinc-700 shrink-0 mt-0.5" />
                        <span className="text-sm text-zinc-400">{ac}</span>
                      </div>
                    )) || <span className="text-sm text-zinc-700 italic">None specified</span>
                  )}
                </div>
              </section>

              {selectedBead?.status === 'closed' && !isEditing && (
                <section className="flex flex-col gap-4 p-4 rounded-lg bg-emerald-500/5 border border-emerald-500/10">
                  <h3 className="text-xs font-black text-emerald-500 uppercase tracking-[0.2em]">Closure Info</h3>
                  <div className="flex flex-col gap-2">
                    <div className="flex justify-between text-xs">
                      <span className="text-zinc-500 font-bold uppercase">Closed At</span>
                      <span className="text-emerald-400 font-mono">{selectedBead.closed_at ? new Date(selectedBead.closed_at).toLocaleString() : "Unknown"}</span>
                    </div>
                    {selectedBead.close_reason && (
                      <div className="flex flex-col gap-1">
                        <span className="text-zinc-500 font-bold uppercase text-xs">Reason</span>
                        <p className="text-sm text-zinc-300 italic">"{selectedBead.close_reason}"</p>
                      </div>
                    )}
                  </div>
                </section>
              )}

              <section className="flex flex-col gap-3">
                <div className="flex items-center justify-between">
                  <h3 className="text-xs font-black text-zinc-600 uppercase tracking-[0.2em]">Dependencies</h3>
                  {(isEditing || isCreating) && (
                    <button 
                      onClick={() => {
                        const target = prompt("Enter Target Bead ID:");
                        if (target) {
                          const newDeps = [...(editForm.dependencies || []), { issue_id: isCreating ? editForm.id! : selectedBead!.id, depends_on_id: target, type: "blocks" }];
                          setEditForm({...editForm, dependencies: newDeps as any});
                        }
                      }}
                      className="text-xs font-bold text-indigo-400 hover:text-indigo-300 flex items-center gap-1"
                    >
                      <Plus size={10} /> Add
                    </button>
                  )}
                </div>
                <div className="flex flex-col gap-2">
                  {(isCreating ? editForm.dependencies : selectedBead?.dependencies)?.map((d, i) => (
                    <div key={i} className="flex items-center justify-between p-3 rounded-lg bg-white dark:bg-zinc-950 border border-zinc-800 group hover:border-zinc-700 transition-all">
                      <div className="flex flex-col gap-0.5">
                        <div className="flex items-center gap-3">
                           <span className="text-xs font-mono text-zinc-500">{d.depends_on_id}</span>
                           <span className="text-xs uppercase font-black text-zinc-600 tracking-tighter">{d.type}</span>
                        </div>
                        {d.metadata && Object.keys(d.metadata).length > 0 && (
                          <div className="flex flex-wrap gap-1 mt-1">
                            {Object.entries(d.metadata).map(([k, v]) => (
                              <span key={k} className="text-[8px] bg-zinc-100 dark:bg-zinc-900 px-1 rounded text-zinc-500 border border-zinc-800">
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
                          className="text-zinc-600 hover:text-red-400 p-1 opacity-0 group-hover:opacity-100 transition-all"
                        >
                          <Trash2 size={12} />
                        </button>
                      )}
                    </div>
                  )) || <div className="text-sm text-zinc-700 italic">None</div>}
                </div>
              </section>
            </div>
            
            <div className="p-6 border-t border-zinc-800 bg-zinc-100 dark:bg-zinc-900/50 flex gap-3">
              {(isEditing || isCreating) ? (
                <>
                  <button onClick={() => { setIsEditing(false); setIsCreating(false); }} className="flex-1 py-2.5 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-zinc-300 text-sm font-bold transition-all">Cancel</button>
                  <button onClick={isCreating ? handleSaveCreate : handleSaveEdit} className="flex-1 py-2.5 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white text-sm font-bold transition-all shadow-lg shadow-emerald-500/10 flex items-center justify-center gap-2"><Save size={14} /> {isCreating ? "Create Bead" : "Save Changes"}</button>
                </>
              ) : (
                <button onClick={handleStartEdit} className="w-full py-2.5 rounded-lg bg-indigo-600 hover:bg-indigo-500 text-white text-sm font-bold transition-all shadow-lg shadow-indigo-500/10 flex items-center justify-center gap-2"><Edit3 size={14} /> Edit Bead</button>
              )}
            </div>
          </div>
        )}
      </main>

      <style dangerouslySetInnerHTML={{ __html: `
        .custom-scrollbar::-webkit-scrollbar { width: 4px; height: 4px; }
        .custom-scrollbar::-webkit-scrollbar-thumb { background: #27272a; border-radius: 10px; }
      `}} />
    </div>
  );
}

export default App;
