import { useState, useEffect, useCallback, useRef, useMemo } from "react";
import { listen } from "@tauri-apps/api/event";
import { Star, ChevronsDown, ChevronsUp } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { fetchBeads, buildWBSTree, calculateGanttLayout, updateBead, createBead, type WBSNode, type Bead, type Project, fetchProjects, removeProject, openProject, toggleFavoriteProject } from "./api";

// Components
import { Navigation } from "./components/layout/Navigation";
import { Header } from "./components/layout/Header";
import { Sidebar } from "./components/layout/Sidebar";
import { WBSTreeList } from "./components/wbs/WBSTreeItem";
import { GanttBar } from "./components/gantt/GanttBar";

function App() {
  const [beads, setBeads] = useState<Bead[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [currentProjectPath, setCurrentProjectPath] = useState<string>("");
  const [collapsedIds, setCollapsedIds] = useState<Set<string>>(() => {
    const saved = localStorage.getItem("collapsedIds");
    return saved ? new Set(JSON.parse(saved)) : new Set();
  });
  const [loading, setLoading] = useState(true);
  const [selectedBead, setSelectedBead] = useState<Bead | null>(null);
  const [isEditing, setIsEditing] = useState(false);
  const [isCreating, setIsCreating] = useState(false);
  const [editForm, setEditForm] = useState<Partial<Bead>>({});
  const [filterText, setFilterText] = useState("");
  const [hideClosed, setHideClosed] = useState(false);
  const [includeHierarchy, setIncludeHierarchy] = useState(true);
  const [zoom, setZoom] = useState(1);
  const [isDark, setIsDark] = useState(true);
  const [projectMenuOpen, setProjectMenuOpen] = useState(false);
  const searchInputRef = useRef<HTMLInputElement>(null);

  const scrollRefWBS = useRef<HTMLDivElement>(null);
  const scrollRefBERT = useRef<HTMLDivElement>(null);
  const activeScrollSource = useRef<HTMLDivElement | null>(null);

  const loadProjects = useCallback(async () => {
    const data = await fetchProjects();
    setProjects(data);
  }, []);

  const loadData = useCallback(async () => {
    setLoading(true);
    try {
      const data = await fetchBeads();
      setBeads(data);
    } catch (error) {
      console.error("Error in loadData:", error);
      setBeads([]);
    } finally {
      setLoading(false);
    }
  }, []);

  const handleOpenProject = async (path: string) => {
    try {
      await openProject(path);
      setCurrentProjectPath(path);
      await loadData();
      await loadProjects();
    } catch (error) {
      alert(`Failed to open project: ${error}`);
    }
  };

  const processedData = useMemo(() => {
    let filtered: Bead[] = [];
    
    if (!filterText && !hideClosed) {
      filtered = beads;
    } else {
      const matches = beads.filter(b => {
        if (hideClosed && b.status === 'closed') return false;
        if (!filterText) return true;
        
        const search = filterText.toLowerCase();
        return (
          b.title.toLowerCase().includes(search) ||
          b.id.toLowerCase().includes(search) ||
          b.owner?.toLowerCase().includes(search) ||
          b.labels?.some(l => l.toLowerCase().includes(search))
        );
      });

      if (includeHierarchy && filterText) {
        const includedIds = new Set<string>();
        const addWithAncestors = (bead: Bead) => {
          if (includedIds.has(bead.id)) return;
          includedIds.add(bead.id);
          const parentDep = bead.dependencies?.find(d => d.type === 'parent-child');
          if (parentDep) {
            const parent = beads.find(b => b.id === parentDep.depends_on_id);
            if (parent) addWithAncestors(parent);
          }
        };
        matches.forEach(addWithAncestors);
        filtered = beads.filter(b => includedIds.has(b.id));
      } else {
        filtered = matches;
      }
    }

    const wbsTree = buildWBSTree(filtered);
    
    const applyExpansion = (nodes: WBSNode[]) => {
      nodes.forEach(node => {
        node.isExpanded = !collapsedIds.has(node.id);
        if (node.children) applyExpansion(node.children);
      });
    };
    applyExpansion(wbsTree);

    const layout = calculateGanttLayout(beads, wbsTree, zoom);
    return { tree: wbsTree, layout };
  }, [beads, filterText, zoom, collapsedIds, hideClosed, includeHierarchy]);

  useEffect(() => {
    const init = async () => {
      try {
        const currentDir = await invoke<string>("get_current_dir");
        const projs = await fetchProjects();
        setProjects(projs);
        
        const mostRecent = [...projs].sort((a, b) => 
          (b.last_opened || "").localeCompare(a.last_opened || "")
        )[0];
        
        if (mostRecent && mostRecent.path !== currentDir) {
          await handleOpenProject(mostRecent.path);
        } else {
          await handleOpenProject(currentDir);
        }
      } catch (error) {
        console.error("Initialization failed:", error);
      }
    };
    init();

    const unlistenBeads = listen("beads-updated", () => loadData());
    const unlistenProjs = listen("projects-updated", () => loadProjects());

    return () => {
      unlistenBeads.then(f => f());
      unlistenProjs.then(f => f());
    };
  }, []);

  const toggleNode = useCallback((id: string) => {
    setCollapsedIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  }, []);

  const expandAll = useCallback(() => {
    setCollapsedIds(new Set());
  }, []);

  const collapseAll = useCallback(() => {
    const allParentIds = new Set<string>();
    const findParents = (nodes: WBSNode[]) => {
      nodes.forEach(node => {
        if (node.children.length > 0) {
          allParentIds.add(node.id);
          findParents(node.children);
        }
      });
    };
    findParents(processedData.tree);
    setCollapsedIds(allParentIds);
  }, [processedData.tree]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) {
        if (e.key === 'Escape') {
          e.preventDefault();
          (e.target as HTMLElement).blur();
        }
        return;
      }

      switch (e.key) {
        case '/':
          e.preventDefault();
          searchInputRef.current?.focus();
          break;
        case 'Escape':
          setSelectedBead(null);
          setIsCreating(false);
          setIsEditing(false);
          break;
        case '+':
        case '=':
          e.preventDefault();
          expandAll();
          break;
        case '-':
        case '_':
          e.preventDefault();
          collapseAll();
          break;
        case 'n':
          e.preventDefault();
          handleStartCreate();
          break;
        case 'r':
          e.preventDefault();
          loadData();
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [expandAll, collapseAll, loadData]); // handleStartCreate omitted from deps if it's not stable or just let it be

  useEffect(() => {
    localStorage.setItem("collapsedIds", JSON.stringify(Array.from(collapsedIds)));
  }, [collapsedIds]);

  useEffect(() => {
    if (isDark) {
      document.documentElement.classList.add('dark');
    } else {
      document.documentElement.classList.remove('dark');
    }
  }, [isDark]);

  const handleToggleFavoriteProject = async (path: string) => {
    try {
      await toggleFavoriteProject(path);
    } catch (error) {
      alert(`Failed to toggle favorite: ${error}`);
    }
  };

  const handleRemoveProject = async (path: string) => {
    try {
      await removeProject(path);
    } catch (error) {
      alert(`Failed to remove project: ${error}`);
    }
  };

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
        parent
      });
      setIsEditing(true);
      setIsCreating(false);
    }
  };

  const handleSaveEdit = async () => {
    if (selectedBead && editForm) {
      try {
        const updated = { ...editForm } as Bead;
        const currentParent = beads.find(b => b.dependencies?.some(d => d.issue_id === selectedBead.id && d.type === 'parent-child'));
        if (updated.parent !== currentParent?.id) {
          updated.dependencies = (updated.dependencies || []).filter(d => d.type !== 'parent-child');
          if (updated.parent) {
            updated.dependencies.push({
              issue_id: updated.id,
              depends_on_id: updated.parent,
              type: 'parent-child'
            });
          }
        }
        delete (updated as any).parent;
        await updateBead(updated);
        setSelectedBead(updated);
        setIsEditing(false);
        await loadData();
      } catch (error) {
        alert(`Failed to save bead: ${error}`);
      }
    }
  };

  const handleStartCreate = useCallback(() => {
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
  }, []);

  const handleSaveCreate = async () => {
    if (editForm.title) {
      try {
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
        await loadData();
      } catch (error) {
        alert(`Failed to create bead: ${error}`);
      }
    }
  };

  useEffect(() => {
    if (!loading) {
      const savedScroll = localStorage.getItem("scrollTop");
      if (savedScroll) {
        const top = parseInt(savedScroll);
        if (scrollRefWBS.current) scrollRefWBS.current.scrollTop = top;
        if (scrollRefBERT.current) scrollRefBERT.current.scrollTop = top;
      }
    }
  }, [loading]);

  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const target = e.currentTarget;
    if (activeScrollSource.current && activeScrollSource.current !== target) return;
    const other = target === scrollRefWBS.current ? scrollRefBERT.current : scrollRefWBS.current;
    if (other) other.scrollTop = target.scrollTop;
    localStorage.setItem("scrollTop", target.scrollTop.toString());
  };

  const handleMouseEnter = (e: React.MouseEvent<HTMLDivElement>) => {
    activeScrollSource.current = e.currentTarget;
  };

  const toggleFavorite = async (bead: Bead) => {
    try {
      const updated = { ...bead, is_favorite: !bead.is_favorite };
      await updateBead(updated);
      await loadData();
    } catch (error) {
      alert(`Failed to toggle favorite: ${error}`);
    }
  };

  const handleSelectProject = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select BERT Project Directory"
      });
      if (selected && typeof selected === 'string') {
        await handleOpenProject(selected);
      }
    } catch (error) {
      alert(`Failed to select project: ${error}`);
    }
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
      <Navigation />

      <main className="flex-1 flex flex-col min-w-0 bg-[var(--background-primary)] relative">
        <Header 
          isDark={isDark}
          setIsDark={setIsDark}
          handleStartCreate={handleStartCreate}
          loadData={loadData}
          projectMenuOpen={projectMenuOpen}
          setProjectMenuOpen={setProjectMenuOpen}
          favoriteProjects={favoriteProjects}
          recentProjects={recentProjects}
          currentProjectPath={currentProjectPath}
          handleOpenProject={handleOpenProject}
          toggleFavoriteProject={handleToggleFavoriteProject}
          removeProject={handleRemoveProject}
          handleSelectProject={handleSelectProject}
        />

        <div className="flex-1 flex flex-col overflow-hidden">
          <div className="flex shrink-0 border-b-2 border-[var(--border-primary)] bg-[var(--background-secondary)] z-20">
            <div className="w-1/3 min-w-[420px] border-r-2 border-[var(--border-primary)] flex flex-col">
              <div className="px-6 py-4 border-b-2 border-[var(--border-primary)]/50 flex items-center justify-between">
                <h2 className="text-[11px] font-black text-[var(--text-primary)] uppercase tracking-[0.25em] flex items-center gap-2">Task Breakdown</h2>
                <div className="flex items-center gap-1">
                  <button onClick={expandAll} title="Expand All" className="p-1.5 hover:bg-[var(--background-tertiary)] rounded-md text-[var(--text-muted)] hover:text-indigo-500 transition-colors">
                    <ChevronsDown size={16} strokeWidth={2.5} />
                  </button>
                  <button onClick={collapseAll} title="Collapse All" className="p-1.5 hover:bg-[var(--background-tertiary)] rounded-md text-[var(--text-muted)] hover:text-indigo-500 transition-colors">
                    <ChevronsUp size={16} strokeWidth={2.5} />
                  </button>
                </div>
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
                <div className="relative group mb-3">
                  <input 
                    ref={searchInputRef}
                    type="text" 
                    placeholder="Search by title, ID, or label..." 
                    className="w-full bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-xl px-4 py-2.5 text-[12px] font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none transition-all placeholder:text-[var(--text-muted)] shadow-inner" 
                    value={filterText} 
                    onChange={e => setFilterText(e.target.value)} 
                  />
                </div>
                <div className="flex items-center gap-4 px-1">
                  <label className="flex items-center gap-2 cursor-pointer group">
                    <input type="checkbox" checked={hideClosed} onChange={e => setHideClosed(e.target.checked)} className="rounded border-2 border-[var(--border-primary)] text-indigo-600 focus:ring-indigo-500 h-3.5 w-3.5 bg-[var(--background-secondary)]" />
                    <span className="text-[10px] font-black text-[var(--text-muted)] group-hover:text-[var(--text-primary)] uppercase tracking-wider">Hide Closed</span>
                  </label>
                  <label className="flex items-center gap-2 cursor-pointer group">
                    <input type="checkbox" checked={includeHierarchy} onChange={e => setIncludeHierarchy(e.target.checked)} className="rounded border-2 border-[var(--border-primary)] text-indigo-600 focus:ring-indigo-500 h-3.5 w-3.5 bg-[var(--background-secondary)]" />
                    <span className="text-[10px] font-black text-[var(--text-muted)] group-hover:text-[var(--text-primary)] uppercase tracking-wider">Hierarchy</span>
                  </label>
                </div>
              </div>
              <div className="flex items-center px-4 py-2 bg-[var(--background-tertiary)] text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.3em] mt-auto border-t border-[var(--border-primary)]/50">
                <div className="w-10 shrink-0" />
                <div className="w-28 shrink-0 px-2 border-r-2 border-[var(--border-primary)]/50">ID</div>
                <div className="flex-1 px-4">Name</div>
              </div>
            </div>

            <div className="flex-1 flex items-end justify-between px-8 py-3 bg-[var(--background-primary)]">
               <div className="flex items-center gap-10 mb-1">
                  <div className="flex flex-col">
                     <span className="text-[9px] font-black text-[var(--text-muted)] uppercase tracking-[0.25em] mb-1">Total</span>
                     <span className="text-base font-black text-[var(--text-primary)]">{stats.total}</span>
                  </div>
                  <div className="flex flex-col border-l-2 border-[var(--border-primary)] pl-6">
                     <span className="text-[9px] font-black text-[var(--status-open)] uppercase tracking-[0.25em] mb-1">Open</span>
                     <span className="text-base font-black text-[var(--status-open)]">{stats.open}</span>
                  </div>
                  <div className="flex flex-col border-l-2 border-[var(--border-primary)] pl-6">
                     <span className="text-[9px] font-black text-[var(--status-active)] uppercase tracking-[0.25em] mb-1">Active</span>
                     <span className="text-base font-black text-[var(--status-active)]">{stats.inProgress}</span>
                  </div>
                  <div className="flex flex-col border-l-2 border-[var(--border-primary)] pl-6">
                     <span className="text-[9px] font-black text-[var(--status-blocked)] uppercase tracking-[0.25em] mb-1">Blocked</span>
                     <span className="text-base font-black text-[var(--status-blocked)]">{stats.blocked}</span>
                  </div>
                  <div className="flex flex-col border-l-2 border-[var(--border-primary)] pl-6">
                     <span className="text-[9px] font-black text-[var(--status-done)] uppercase tracking-[0.25em] mb-1">Done</span>
                     <span className="text-base font-black text-[var(--status-done)]">{stats.closed}</span>
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
            <div ref={scrollRefWBS} onScroll={handleScroll} onMouseEnter={handleMouseEnter} className="w-1/3 border-r-2 border-[var(--border-primary)] flex flex-col bg-[var(--background-secondary)] min-w-[420px] overflow-y-auto custom-scrollbar">
              <div className="p-0">
                {loading ? <div className="p-12 animate-pulse text-[var(--text-muted)] text-xs font-medium tracking-widest uppercase">Syncing Schedule...</div> : (
                  <div className="flex flex-col">
                    <WBSTreeList nodes={processedData.tree} onToggle={toggleNode} onClick={handleBeadClick} />
                  </div>
                )}
              </div>
            </div>

            <div ref={scrollRefBERT} onScroll={handleScroll} onMouseEnter={handleMouseEnter} className="flex-1 relative bg-[var(--background-primary)] overflow-auto custom-scrollbar">
              <div className="relative" style={{ height: processedData.layout.rowCount * 48, width: 5000 * zoom }}>
                 <div className="absolute inset-0 pointer-events-none">
                    {processedData.layout.rowDepths.map((depth, i) => (
                      <div key={i} className="w-full border-b-2 border-[var(--border-primary)]/40" style={{ height: '48px', backgroundColor: `var(--level-${Math.min(depth, 4)})` }} />
                    ))}
                    <div className="absolute inset-0 flex">
                      {Array.from({ length: 50 }).map((_, i) => (
                        <div key={i} className="h-full border-r-2 border-[var(--border-primary)]/40" style={{ width: 100 * zoom }} />
                      ))}
                    </div>
                 </div>

               <svg className="absolute inset-0 pointer-events-none overflow-visible" style={{ width: '100%', height: '100%' }}>
                  {processedData.layout.connectors.map((c: any, i: number) => (
                    <path key={i} d={`M ${c.from.x} ${c.from.y} L ${c.from.x + 20} ${c.from.y} L ${c.from.x + 20} ${c.to.y} L ${c.to.x} ${c.to.y}`} fill="none" stroke={c.isCritical ? "#f43f5e" : "var(--text-muted)"} strokeWidth={c.isCritical ? 2.5 : 1.5} strokeDasharray={c.isCritical ? "0" : "4 2"} opacity={0.8} />
                  ))}
               </svg>

               {processedData.layout.items.map((item: any, i: number) => (
                 <div key={i} className="absolute w-full" style={{ top: item.row * 48, height: 48 }}>
                    <GanttBar item={item} onClick={handleBeadClick} />
                 </div>
               ))}
            </div>
          </div>
        </div>
      </div>

      <Sidebar 
        selectedBead={selectedBead}
        isCreating={isCreating}
        isEditing={isEditing}
        editForm={editForm}
        beads={beads}
        setIsEditing={setIsEditing}
        setIsCreating={setIsCreating}
        setSelectedBead={setSelectedBead}
        setEditForm={setEditForm}
        handleSaveEdit={handleSaveEdit}
        handleSaveCreate={handleSaveCreate}
        handleStartEdit={handleStartEdit}
        toggleFavorite={toggleFavorite}
      />
      </main>
    </div>
  );
}

export default App;