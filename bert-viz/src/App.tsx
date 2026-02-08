import { useState, useEffect, useCallback, useRef, useMemo } from "react";
import { listen } from "@tauri-apps/api/event";
import { Star, ChevronsDown, ChevronsUp } from "lucide-react";
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
  const [collapsedIds, setCollapsedIds] = useState<Set<string>>(new Set());
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

  const loadProjects = useCallback(async () => {
    const data = await fetchProjects();
    setProjects(data);
  }, []);

  const handleOpenProject = async (path: string) => {
    await openProject(path);
    setCurrentProjectPath(path);
    loadData();
  };

  const loadData = useCallback(async () => {
    try {
      const data = await fetchBeads();
      setBeads(data);
    } catch (error) {
      console.error("Error in loadData:", error);
    } finally {
      setLoading(false);
    }
  }, []);

  const processedData = useMemo(() => {
    const filtered = beads.filter(b => {
      const search = filterText.toLowerCase();
      if (!search) return true;
      return (
        b.title.toLowerCase().includes(search) ||
        b.id.toLowerCase().includes(search) ||
        b.owner?.toLowerCase().includes(search) ||
        b.labels?.some(l => l.toLowerCase().includes(search))
      );
    });

    const wbsTree = buildWBSTree(filtered);
    
    // Apply expansion state
    const applyExpansion = (nodes: WBSNode[]) => {
      nodes.forEach(node => {
        node.isExpanded = !collapsedIds.has(node.id);
        if (node.children) applyExpansion(node.children);
      });
    };
    applyExpansion(wbsTree);

    const layout = calculateGanttLayout(beads, wbsTree, zoom);
    return { tree: wbsTree, layout };
  }, [beads, filterText, zoom, collapsedIds]);

  useEffect(() => {
    const init = async () => {
      const currentDir = await invoke_get_current_dir();
      
      // Load projects to find the most recent one
      const projs = await fetchProjects();
      setProjects(projs);
      
      const mostRecent = [...projs].sort((a, b) => 
        (b.last_opened || "").localeCompare(a.last_opened || "")
      )[0];
      
      if (mostRecent && mostRecent.path !== currentDir) {
        // Only auto-load if we're not already in a specific project dir
        // (Wait, if we're in a dir with .beads, we should probably stay there)
        // For now, let's just follow the "Auto-load most recent" requirement.
        await handleOpenProject(mostRecent.path);
      } else {
        setCurrentProjectPath(currentDir);
        loadData();
      }
    };
    init();
    loadProjects();

    const unlistenBeads = listen("beads-updated", () => loadData());
    const unlistenProjs = listen("projects-updated", () => loadProjects());

    return () => {
      unlistenBeads.then(f => f());
      unlistenProjs.then(f => f());
    };
  }, [loadData, loadProjects]);

  // Helper because we can't import invoke directly into App if we want to stay clean
  // but for now let's just keep it simple.
  const invoke_get_current_dir = async () => {
    const { invoke } = await import("@tauri-apps/api/core");
    return await invoke<string>("get_current_dir");
  };

  const handleToggleFavoriteProject = async (path: string) => {
    await toggleFavoriteProject(path);
    loadProjects();
  };

  const handleRemoveProject = async (path: string) => {
    await removeProject(path);
    loadProjects();
  };

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

  const handleSelectProject = async () => {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Select BERT Project Directory"
    });
    if (selected && typeof selected === 'string') {
      await handleOpenProject(selected);
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
                  <button 
                    onClick={expandAll}
                    title="Expand All"
                    className="p-1.5 hover:bg-[var(--background-tertiary)] rounded-md text-[var(--text-muted)] hover:text-indigo-500 transition-colors"
                  >
                    <ChevronsDown size={16} strokeWidth={2.5} />
                  </button>
                  <button 
                    onClick={collapseAll}
                    title="Collapse All"
                    className="p-1.5 hover:bg-[var(--background-tertiary)] rounded-md text-[var(--text-muted)] hover:text-indigo-500 transition-colors"
                  >
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
            <div 
              ref={scrollRefWBS}
              onScroll={handleScroll}
              onMouseEnter={handleMouseEnter}
              className="w-1/3 border-r-2 border-[var(--border-primary)] flex flex-col bg-[var(--background-secondary)] min-w-[420px] overflow-y-auto custom-scrollbar"
            >
              <div className="p-0">
                {loading ? <div className="p-12 animate-pulse text-[var(--text-muted)] text-xs font-medium tracking-widest uppercase">Syncing Schedule...</div> : (
                  <div className="flex flex-col">
                    <WBSTreeList nodes={processedData.tree} onToggle={toggleNode} onClick={handleBeadClick} />
                  </div>
                )}
              </div>
            </div>

            <div 
              ref={scrollRefBERT}
              onScroll={handleScroll}
              onMouseEnter={handleMouseEnter}
              className="flex-1 relative bg-[var(--background-primary)] overflow-auto custom-scrollbar"
            >
              <div className="relative" style={{ height: processedData.layout.rowCount * 48, width: 5000 * zoom }}>
                 <div className="absolute inset-0 pointer-events-none">
                    {Array.from({ length: processedData.layout.rowCount }).map((_, i) => (
                      <div key={i} className="w-full border-b-2 border-[var(--border-primary)]/40" style={{ height: '48px' }} />
                    ))}
                    <div className="absolute inset-0 flex">
                      {Array.from({ length: 50 }).map((_, i) => (
                        <div key={i} className="h-full border-r-2 border-[var(--border-primary)]/40" style={{ width: 100 * zoom }} />
                      ))}
                    </div>
                 </div>

               <svg className="absolute inset-0 pointer-events-none overflow-visible" style={{ width: '100%', height: '100%' }}>
                  {processedData.layout.connectors.map((c: any, i: number) => (
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
