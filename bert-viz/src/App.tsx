import { useState, useEffect, useCallback, useRef, useMemo, startTransition } from "react";
import { listen } from "@tauri-apps/api/event";
import { Star, ChevronsDown, ChevronsUp } from "lucide-react";
import { fetchBeads, fetchProcessedData, fetchProjectViewModel, updateBead, createBead, closeBead, reopenBead, claimBead, type WBSNode, type Bead, type BeadNode, type Project, type ProcessedData, type ProjectViewModel, fetchProjects, removeProject, openProject, toggleFavoriteProject } from "./api";

// Components
import { Navigation } from "./components/layout/Navigation";
import { Header } from "./components/layout/Header";
import { Sidebar } from "./components/layout/Sidebar";
import { WBSTreeList } from "./components/wbs/WBSTreeItem";
import { GanttBar } from "./components/gantt/GanttBar";
import { GanttStateHeader } from "./components/gantt/GanttStateHeader";
import { WBSSkeleton, GanttSkeleton } from "./components/shared/Skeleton";

// Time-based filter options for closed tasks
type ClosedTimeFilter =
  | 'all'           // Show all closed tasks
  | '1h'            // Closed within last hour
  | '6h'            // Closed within last 6 hours
  | '24h'           // Closed within last 24 hours
  | '7d'            // Closed within last 7 days
  | '30d'           // Closed within last 30 days
  | 'older_than_6h'; // Closed more than 6 hours ago

function App() {
  // Unified View Model State (replaces beads + processedData)
  const [viewModel, setViewModel] = useState<ProjectViewModel | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [currentProjectPath, setCurrentProjectPath] = useState<string>("");
  const [hasProject, setHasProject] = useState(false);
  const [collapsedIds, setCollapsedIds] = useState<Set<string>>(() => {
    const saved = localStorage.getItem("collapsedIds");
    return saved ? new Set(JSON.parse(saved)) : new Set();
  });
  const [loading, setLoading] = useState(true);
  const [selectedBead, setSelectedBead] = useState<BeadNode | null>(null);
  const [isEditing, setIsEditing] = useState(false);
  const [isCreating, setIsCreating] = useState(false);
  const [editForm, setEditForm] = useState<Partial<Bead>>({});
  const [filterText, setFilterText] = useState("");
  const [hideClosed, setHideClosed] = useState(false);
  const [closedTimeFilter, setClosedTimeFilter] = useState<ClosedTimeFilter>(() => {
    const saved = localStorage.getItem("closedTimeFilter");
    return (saved as ClosedTimeFilter) || 'all';
  });
  const [includeHierarchy, setIncludeHierarchy] = useState(true);
  const [zoom, setZoom] = useState(1);
  const [isDark, setIsDark] = useState(true);
  const [projectMenuOpen, setProjectMenuOpen] = useState(false);
  const [refetchTrigger, setRefetchTrigger] = useState(0);
  const searchInputRef = useRef<HTMLInputElement>(null);

  const scrollRefWBS = useRef<HTMLDivElement>(null);
  const scrollRefBERT = useRef<HTMLDivElement>(null);
  const scrollRefGanttHeader = useRef<HTMLDivElement>(null);
  const activeScrollSource = useRef<HTMLDivElement | null>(null);
  const hasInitialized = useRef(false);
  const lastToggledNode = useRef<{ id: string; offsetTop: number } | null>(null);

  const loadProjects = useCallback(async () => {
    const data = await fetchProjects();
    setProjects(data);
  }, []);

  const loadData = useCallback(async (showLoading = false) => {
    if (showLoading) setLoading(true);
    try {
      // Trigger a refetch of the view model
      setRefetchTrigger(prev => prev + 1);
    } catch (error) {
      console.error("Error in loadData:", error);
    } finally {
      setLoading(false);
    }
  }, []);

  const handleOpenProject = useCallback(async (path: string) => {
    try {
      setLoading(true);
      await openProject(path);
      setCurrentProjectPath(path);
      setHasProject(true);

      // Use startTransition to keep UI responsive during heavy computation
      startTransition(() => {
        loadData(false).then(() => {
          loadProjects();
          setLoading(false);
        });
      });
    } catch (error) {
      alert(`Failed to open project: ${error}`);
      setLoading(false);
    }
  }, [loadData, loadProjects]);

  const [processingData, setProcessingData] = useState(false);

  // The view model tree already includes isExpanded and isVisible state from backend
  // No need for complex frontend processing - just access viewModel.tree directly

  // Fetch view model from Rust backend when filters change
  useEffect(() => {
    // Debounce filter text to avoid excessive backend calls while typing
    const debounceTimeout = setTimeout(() => {
      const fetchData = async () => {
        const startTime = performance.now();
        setProcessingData(true);
        try {
          // Use startTransition to maintain UI responsiveness
          startTransition(() => {
            fetchProjectViewModel({
              filter_text: filterText,
              hide_closed: hideClosed,
              closed_time_filter: closedTimeFilter,
              include_hierarchy: includeHierarchy,
              zoom: zoom,
              collapsed_ids: Array.from(collapsedIds) // Backend now handles collapsed state
            }).then(data => {
              const endTime = performance.now();
              console.log(`⏱️  Frontend: IPC call took ${(endTime - startTime).toFixed(2)}ms`);
              setViewModel(data);
              setProcessingData(false);
            }).catch(error => {
              console.error("Failed to fetch view model:", error);
              setProcessingData(false);
            });
          });
        } catch (error) {
          console.error("Failed to fetch view model:", error);
          setProcessingData(false);
        }
      };

      fetchData();
    }, 150); // 150ms debounce for filter text changes

    return () => clearTimeout(debounceTimeout);
  }, [filterText, zoom, hideClosed, includeHierarchy, closedTimeFilter, collapsedIds, currentProjectPath, refetchTrigger]);

  useEffect(() => {
    // Prevent double initialization (React 19 Strict Mode runs effects twice)
    if (hasInitialized.current) return;
    hasInitialized.current = true;

    const init = async () => {
      try {
        // Load projects list from ~/.bert-viz/projects.json
        const projs = await fetchProjects();
        setProjects(projs);

        // Auto-open most recent project if it exists
        const mostRecent = [...projs].sort((a, b) => (b.last_opened || "").localeCompare(a.last_opened || ""))[0];
        if (mostRecent) {
          await handleOpenProject(mostRecent.path);
        } else {
          // No projects - show welcome screen
          setHasProject(false);
        }
      } catch (error) {
        console.error("Initialization failed:", error);
        setHasProject(false);
      } finally {
        setLoading(false);
      }
    };

    console.log('🔧 Setting up event listeners...');
    init();

    listen("beads-updated", (event) => {
      console.log('🎉 beads-updated event received!', event);
      loadData();
      setRefetchTrigger(prev => prev + 1);
    }).then((unlisten) => {
      console.log('✅ beads-updated listener registered');
      return unlisten;
    }).catch((err) => {
      console.error('❌ Failed to register beads-updated listener:', err);
    });

    listen("projects-updated", (event) => {
      console.log('🎉 projects-updated event received!', event);
      loadProjects();
    }).then((unlisten) => {
      console.log('✅ projects-updated listener registered');
      return unlisten;
    }).catch((err) => {
      console.error('❌ Failed to register projects-updated listener:', err);
    });

    return () => {
      console.log('🧹 Cleaning up event listeners');
    };
  }, [handleOpenProject, loadData, loadProjects]);

  // Helper: Flatten tree to array of beads for backward compatibility
  const flattenTree = useCallback((nodes: BeadNode[]): BeadNode[] => {
    const result: BeadNode[] = [];
    const traverse = (nodeList: BeadNode[]) => {
      nodeList.forEach(node => {
        result.push(node);
        if (node.children.length > 0) {
          traverse(node.children);
        }
      });
    };
    traverse(nodes);
    return result;
  }, []);

  const beads = useMemo(() => viewModel ? flattenTree(viewModel.tree) : [], [viewModel, flattenTree]);

  const toggleNode = useCallback((id: string) => {
    // Find the DOM element for this node to track its position
    const element = document.querySelector(`[data-bead-id="${id}"]`);
    if (element && scrollRefWBS.current) {
      const elementRect = element.getBoundingClientRect();
      const containerRect = scrollRefWBS.current.getBoundingClientRect();
      const offsetTop = elementRect.top - containerRect.top;
      lastToggledNode.current = { id, offsetTop };
    }

    setCollapsedIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  const expandAll = useCallback(() => setCollapsedIds(new Set()), []);
  const collapseAll = useCallback(() => {
    const allParentIds = new Set<string>();
    const findParents = (nodes: BeadNode[]) => {
      nodes.forEach(node => {
        if (node.children.length > 0) {
          allParentIds.add(node.id);
          findParents(node.children);
        }
      });
    };
    if (viewModel) {
      findParents(viewModel.tree);
    }
    setCollapsedIds(allParentIds);
  }, [viewModel]);

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
        case '/': e.preventDefault(); searchInputRef.current?.focus(); break;
        case 'Escape': setSelectedBead(null); setIsCreating(false); setIsEditing(false); break;
        case '+': case '=': e.preventDefault(); expandAll(); break;
        case '-': case '_': e.preventDefault(); collapseAll(); break;
        case 'n': e.preventDefault(); handleStartCreate(); break;
        case 'r': e.preventDefault(); loadData(); break;
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [expandAll, collapseAll, loadData]);

  useEffect(() => {
    localStorage.setItem("collapsedIds", JSON.stringify(Array.from(collapsedIds)));
  }, [collapsedIds]);

  useEffect(() => {
    localStorage.setItem("closedTimeFilter", closedTimeFilter);
  }, [closedTimeFilter]);

  useEffect(() => {
    if (isDark) document.documentElement.classList.add('dark');
    else document.documentElement.classList.remove('dark');
  }, [isDark]);

  // Keep selectedBead synchronized with the latest bead data
  useEffect(() => {
    if (selectedBead && beads.length > 0) {
      const updatedBead = beads.find(b => b.id === selectedBead.id);
      if (updatedBead) {
        setSelectedBead(updatedBead);
      }
    }
  }, [beads]);

  const handleToggleFavoriteProject = async (path: string) => {
    try {
      await toggleFavoriteProject(path);
      await loadProjects(); // Explicitly refresh projects list
    } catch (error) {
      alert(`Failed to toggle favorite: ${error}`);
    }
  };

  const handleRemoveProject = async (path: string) => {
    try {
      await removeProject(path);
      await loadProjects(); // Explicitly refresh projects list
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
      setEditForm({ ...selectedBead, parent });
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
            updated.dependencies.push({ issue_id: updated.id, depends_on_id: updated.parent, type: 'parent-child' });
          }
        }
        await updateBead(updated);
        setSelectedBead(updated);
        setIsEditing(false);
        await loadData();
      } catch (error) { alert(`Failed to save bead: ${error}`); }
    }
  };

  const handleStartCreate = useCallback(() => {
    setSelectedBead(null);
    setEditForm({
      id: `bp6-${Math.random().toString(36).slice(2, 5)}`,
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
    console.log('🆕 handleSaveCreate called', { editForm });
    if (editForm.title) {
      console.log('🆕 Title exists, proceeding with create');
      try {
        const newBead = { ...editForm } as Bead;
        console.log('🆕 newBead prepared:', newBead);
        if (newBead.parent) {
          newBead.dependencies = [...(newBead.dependencies || []), { issue_id: newBead.id, depends_on_id: newBead.parent, type: 'parent-child' }];
        }
        console.log('🆕 Calling createBead...');
        const createdId = await createBead(newBead);
        console.log('🆕 Created bead ID:', createdId);
        setIsCreating(false);
        const freshBeads = await loadData();
        const createdBead = freshBeads.find(b => b.id === createdId);
        if (createdBead) {
          setSelectedBead(createdBead);
        }
      } catch (error) {
        console.error('🆕 Error creating bead:', error);
        alert(`Failed to create bead: ${error}`);
      }
    } else {
      console.log('🆕 No title, skipping create');
    }
  };

  const handleCloseBead = async (beadId: string) => {
    try {
      await closeBead(beadId);
      setSelectedBead(null);
      await loadData();
    } catch (error) {
      console.error('Failed to close bead:', error);
    }
  };

  const handleReopenBead = async (beadId: string) => {
    try {
      await reopenBead(beadId);
      await loadData();
    } catch (error) {
      console.error('Failed to reopen bead:', error);
    }
  };

  const handleClaimBead = async (beadId: string) => {
    try {
      await claimBead(beadId);
      await loadData();
    } catch (error) {
      console.error('Failed to claim bead:', error);
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

  // Restore scroll position after processedData updates
  useEffect(() => {
    if (loading || processingData) return;

    // If we just toggled a node, adjust scroll to keep it in the same visual position
    if (lastToggledNode.current) {
      setTimeout(() => {
        const { id, offsetTop } = lastToggledNode.current!;
        const element = document.querySelector(`[data-bead-id="${id}"]`);
        if (element && scrollRefWBS.current) {
          const elementRect = element.getBoundingClientRect();
          const containerRect = scrollRefWBS.current.getBoundingClientRect();
          const currentOffsetTop = elementRect.top - containerRect.top;
          const scrollAdjustment = currentOffsetTop - offsetTop;

          const newScrollTop = scrollRefWBS.current.scrollTop + scrollAdjustment;
          scrollRefWBS.current.scrollTop = newScrollTop;
          if (scrollRefBERT.current) scrollRefBERT.current.scrollTop = newScrollTop;

          localStorage.setItem("scrollTop", newScrollTop.toString());
        }
        lastToggledNode.current = null;
      }, 0);
    } else {
      // Normal scroll restoration
      const savedScroll = localStorage.getItem("scrollTop");
      if (savedScroll) {
        const top = parseInt(savedScroll);
        setTimeout(() => {
          if (scrollRefWBS.current) scrollRefWBS.current.scrollTop = top;
          if (scrollRefBERT.current) scrollRefBERT.current.scrollTop = top;
        }, 0);
      }
    }
  }, [viewModel, loading, processingData]);

  const handleScroll = (e: React.UIEvent<HTMLDivElement>) => {
    const target = e.currentTarget;
    if (activeScrollSource.current && activeScrollSource.current !== target) return;
    if (target === scrollRefWBS.current) {
      if (scrollRefBERT.current) scrollRefBERT.current.scrollTop = target.scrollTop;
    } else if (target === scrollRefBERT.current) {
      if (scrollRefWBS.current) scrollRefWBS.current.scrollTop = target.scrollTop;
      if (scrollRefGanttHeader.current) scrollRefGanttHeader.current.scrollLeft = target.scrollLeft;
    } else if (target === scrollRefGanttHeader.current) {
      if (scrollRefBERT.current) scrollRefBERT.current.scrollLeft = target.scrollLeft;
    }
    if (target === scrollRefWBS.current || target === scrollRefBERT.current) {
      localStorage.setItem("scrollTop", target.scrollTop.toString());
    }
  };

  const handleMouseEnter = (e: React.MouseEvent<HTMLDivElement>) => {
    activeScrollSource.current = e.currentTarget;
  };

  const toggleFavorite = async (bead: Bead) => {
    try {
      const updated = { ...bead, is_favorite: !bead.is_favorite };
      await updateBead(updated);
      await loadData();
    } catch (error) { alert(`Failed to toggle favorite: ${error}`); }
  };

  const handleSelectProject = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({ directory: true, multiple: false, title: "Select BERT Project Directory" });
      if (selected && typeof selected === 'string') await handleOpenProject(selected);
    } catch (error) { alert(`Failed to select project: ${error}`); }
  };

  const favoriteBeads = useMemo(() => beads.filter(b => b.is_favorite), [beads]);
  const favoriteProjects = useMemo(() => projects.filter(p => p.is_favorite), [projects]);
  const recentProjects = useMemo(() => projects.filter(p => !p.is_favorite).sort((a, b) => (b.last_opened || "").localeCompare(a.last_opened || "")), [projects]);

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
        <Header isDark={isDark} setIsDark={setIsDark} handleStartCreate={handleStartCreate} loadData={loadData} projectMenuOpen={projectMenuOpen} setProjectMenuOpen={setProjectMenuOpen} favoriteProjects={favoriteProjects} recentProjects={recentProjects} currentProjectPath={currentProjectPath} handleOpenProject={handleOpenProject} toggleFavoriteProject={handleToggleFavoriteProject} removeProject={handleRemoveProject} handleSelectProject={handleSelectProject} />
        {!hasProject ? (
          <div className="flex-1 flex items-center justify-center">
            {loading ? (
              <div className="text-center">
                <div className="inline-block h-12 w-12 animate-spin rounded-full border-4 border-solid border-indigo-600 border-r-transparent mb-4"></div>
                <p className="text-lg text-[var(--text-muted)] font-medium">Loading project...</p>
              </div>
            ) : (
              <div className="text-center max-w-md px-8">
                <h1 className="text-4xl font-black text-[var(--text-primary)] mb-4 tracking-tight">Welcome to BERT</h1>
                <p className="text-lg text-[var(--text-muted)] mb-8 font-medium">Get started by loading a project directory with a .beads folder</p>
                <button
                  onClick={handleSelectProject}
                  className="px-8 py-4 bg-indigo-600 hover:bg-indigo-700 text-white font-black rounded-xl shadow-lg hover:shadow-xl transition-all active:scale-95 text-lg uppercase tracking-wider"
                >
                  Load Project
                </button>
              </div>
            )}
          </div>
        ) : (
          <div className="flex-1 flex flex-col overflow-hidden">
          <div className="flex shrink-0 border-b-2 border-[var(--border-primary)] bg-[var(--background-secondary)] z-20">
            <div className="w-1/3 min-w-[420px] border-r-2 border-[var(--border-primary)] flex flex-col">
              <div className="px-6 py-4 border-b-2 border-[var(--border-primary)]/50 flex items-center justify-between">
                <h2 className="text-sm font-black text-[var(--text-primary)] uppercase tracking-[0.25em] flex items-center gap-2">Task Breakdown</h2>
                <div className="flex items-center gap-1">
                  <button onClick={expandAll} title="Expand All" className="p-1.5 hover:bg-[var(--background-tertiary)] rounded-md text-[var(--text-muted)] hover:text-indigo-500 transition-colors"><ChevronsDown size={16} strokeWidth={2.5} /></button>
                  <button onClick={collapseAll} title="Collapse All" className="p-1.5 hover:bg-[var(--background-tertiary)] rounded-md text-[var(--text-muted)] hover:text-indigo-500 transition-colors"><ChevronsUp size={16} strokeWidth={2.5} /></button>
                </div>
              </div>
              {favoriteBeads.length > 0 && (
                <div className="px-6 py-4 border-b-2 border-[var(--border-primary)]/50 bg-indigo-500/10">
                  <h2 className="text-xs font-black text-indigo-800 dark:text-indigo-300 uppercase tracking-[0.2em] flex items-center gap-2 mb-3"><Star size={12} className="fill-current" /> Favorites</h2>
                  <div className="flex flex-wrap gap-2">
                    {favoriteBeads.map(f => (
                      <div key={f.id} onClick={() => handleBeadClick(f)} className="flex items-center gap-2 px-2.5 py-1.5 rounded-xl bg-[var(--background-primary)] border-[var(--border-thick)] border-[var(--border-primary)] hover:border-indigo-500 shadow-[var(--shadow-sm)] cursor-pointer transition-all active:scale-95 hover-lift">
                        <span className="font-mono text-xs font-black text-indigo-700 dark:text-indigo-400">{f.id}</span>
                        <span className="text-sm font-black text-[var(--text-primary)] truncate max-w-[140px]">{f.title}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
              <div className="px-4 py-3 bg-[var(--background-primary)]">
                <div className="relative group mb-3">
                  <input ref={searchInputRef} type="text" placeholder="Search by title, ID, or label..." className="w-full bg-[var(--background-secondary)] border-[var(--border-thick)] border-[var(--border-primary)] rounded-xl px-4 py-2.5 text-sm font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none transition-all placeholder:text-[var(--text-muted)] shadow-[var(--shadow-inset)]" value={filterText} onChange={e => setFilterText(e.target.value)} />
                </div>
                <div className="flex items-center gap-4 px-1 flex-wrap">
                  <label className="flex items-center gap-2 cursor-pointer group">
                    <input type="checkbox" checked={hideClosed} onChange={e => setHideClosed(e.target.checked)} className="rounded border-2 border-[var(--border-primary)] text-indigo-600 focus:ring-indigo-500 h-3.5 w-3.5 bg-[var(--background-secondary)]" />
                    <span className="text-xs font-black text-[var(--text-muted)] group-hover:text-[var(--text-primary)] uppercase tracking-wider">Hide Closed</span>
                  </label>
                  <label className="flex items-center gap-2 cursor-pointer group">
                    <input type="checkbox" checked={includeHierarchy} onChange={e => setIncludeHierarchy(e.target.checked)} className="rounded border-2 border-[var(--border-primary)] text-indigo-600 focus:ring-indigo-500 h-3.5 w-3.5 bg-[var(--background-secondary)]" />
                    <span className="text-xs font-black text-[var(--text-muted)] group-hover:text-[var(--text-primary)] uppercase tracking-wider">Hierarchy</span>
                  </label>
                  <label className="flex items-center gap-2 group">
                    <span className="text-xs font-black text-[var(--text-muted)] uppercase tracking-wider">Closed:</span>
                    <select
                      value={closedTimeFilter}
                      onChange={e => setClosedTimeFilter(e.target.value as ClosedTimeFilter)}
                      className="text-xs font-black bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-lg px-2 py-1 text-[var(--text-primary)] focus:border-indigo-500 outline-none uppercase tracking-wider"
                    >
                      <option value="all">All Time</option>
                      <option value="1h">Last Hour</option>
                      <option value="6h">Last 6 Hours</option>
                      <option value="24h">Last 24 Hours</option>
                      <option value="7d">Last 7 Days</option>
                      <option value="30d">Last 30 Days</option>
                      <option value="older_than_6h">Older Than 6h</option>
                    </select>
                  </label>
                </div>
              </div>
            </div>
            <div className="flex-1 flex items-end justify-between px-8 py-3 bg-[var(--background-primary)]">
               <div className="flex items-center gap-10 mb-1">
                  <div className="flex flex-col"><span className="text-xs font-black text-[var(--text-muted)] uppercase tracking-[0.25em] mb-1">Total</span><span className="text-base font-black text-[var(--text-primary)]">{stats.total}</span></div>
                  <div className="flex flex-col border-l-2 border-[var(--border-primary)] pl-6"><span className="text-xs font-black text-[var(--status-open)] uppercase tracking-[0.25em] mb-1">Open</span><span className="text-base font-black text-[var(--status-open)]">{stats.open}</span></div>
                  <div className="flex flex-col border-l-2 border-[var(--border-primary)] pl-6"><span className="text-xs font-black text-[var(--status-active)] uppercase tracking-[0.25em] mb-1">In Progress</span><span className="text-base font-black text-[var(--status-active)]">{stats.inProgress}</span></div>
                  <div className="flex flex-col border-l-2 border-[var(--border-primary)] pl-6"><span className="text-xs font-black text-[var(--status-blocked)] uppercase tracking-[0.25em] mb-1">Blocked</span><span className="text-base font-black text-[var(--status-blocked)]">{stats.blocked}</span></div>
                  <div className="flex flex-col border-l-2 border-[var(--border-primary)] pl-6"><span className="text-xs font-black text-[var(--status-done)] uppercase tracking-[0.25em] mb-1">Closed</span><span className="text-base font-black text-[var(--status-done)]">{stats.closed}</span></div>
               </div>
               <div className="flex items-center gap-1 bg-[var(--background-secondary)] p-1.5 rounded-2xl border-[var(--border-thick)] border-[var(--border-primary)] shadow-[var(--shadow-md)] mb-1">
                  <button onClick={() => setZoom(Math.max(0.25, zoom - 0.25))} className="p-2 hover:bg-[var(--background-tertiary)] rounded-xl text-[var(--text-primary)] transition-all text-xs font-black px-4 active:scale-90">-</button>
                  <span className="text-sm font-mono font-black text-[var(--text-primary)] min-w-[50px] text-center">{Math.round(zoom * 100)}%</span>
                  <button onClick={() => setZoom(Math.min(3, zoom + 0.25))} className="p-2 hover:bg-[var(--background-tertiary)] rounded-xl text-[var(--text-primary)] transition-all text-xs font-black px-4 active:scale-90">+</button>
                  <div className="w-px h-5 bg-[var(--border-primary)] mx-2" />
                  <button onClick={() => setZoom(1)} className="px-4 py-2 hover:bg-[var(--background-tertiary)] rounded-xl text-[var(--text-primary)] transition-all text-xs font-black uppercase tracking-[0.2em] active:scale-95">Reset</button>
               </div>
            </div>
          </div>
          <div className="flex shrink-0 border-b-2 border-[var(--border-primary)] bg-[var(--background-secondary)] z-10">
            <div className="w-1/3 min-w-[420px] border-r-2 border-[var(--border-primary)] flex items-center px-4 py-2 bg-[var(--background-tertiary)] text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.3em]">
              <div className="w-10 shrink-0" />
              <div className="w-16 shrink-0 px-2 border-r-2 border-[var(--border-primary)]/50">P</div>
              <div className="flex-1 px-4 border-r-2 border-[var(--border-primary)]/50">Name</div>
              <div className="w-20 shrink-0 px-2 border-r-2 border-[var(--border-primary)]/50">Type</div>
              <div className="w-24 shrink-0 px-2">ID</div>
            </div>
            <div className="flex-1 overflow-hidden bg-[var(--background-tertiary)]">
              <div ref={scrollRefGanttHeader} onScroll={handleScroll} onMouseEnter={handleMouseEnter} className="overflow-x-auto overflow-y-hidden no-scrollbar">
                <div style={{ width: Math.max(5000 * zoom, ((viewModel?.metadata.distributions.length || 0) * 100 * zoom)) }}>
                  <GanttStateHeader distributions={viewModel?.metadata.distributions || []} zoom={zoom} />
                </div>
              </div>
            </div>
          </div>
          <div className="flex-1 flex overflow-hidden">
            <div ref={scrollRefWBS} onScroll={handleScroll} onMouseEnter={handleMouseEnter} className="w-1/3 border-r-2 border-[var(--border-primary)] flex flex-col bg-[var(--background-secondary)] min-w-[420px] overflow-y-auto custom-scrollbar">
              <div className="p-0">
                {(loading || processingData) ? <WBSSkeleton /> : (
                  <div className="flex flex-col">
                    <WBSTreeList nodes={viewModel?.tree || []} onToggle={toggleNode} onClick={handleBeadClick} selectedId={selectedBead?.id} />
                  </div>
                )}
              </div>
            </div>
            <div ref={scrollRefBERT} onScroll={handleScroll} onMouseEnter={handleMouseEnter} className="flex-1 relative bg-[var(--background-primary)] overflow-auto custom-scrollbar">
              {/* TODO: Implement Gantt rendering with BeadNode logical positioning */}
              {/* For now, show a placeholder. Full Gantt rendering needs: */}
              {/* 1. Flatten tree to get visible nodes with row numbers */}
              {/* 2. Convert logical positions (earliestStart, duration) to pixels */}
              {/* 3. Render connectors based on blockingIds */}
              <div className="p-8 text-[var(--text-secondary)]">
                <div className="text-xl font-semibold mb-4">Gantt Chart (In Progress)</div>
                <div className="space-y-2">
                  <p>The Gantt chart is being refactored to use the unified view model.</p>
                  <p className="text-sm">The tree view on the left is now working with the new architecture.</p>
                  <p className="text-sm">Total beads: {viewModel?.metadata.totalBeads || 0}</p>
                  <p className="text-sm">Open: {viewModel?.metadata.openCount || 0} | In Progress: {viewModel?.metadata.inProgressCount || 0} | Blocked: {viewModel?.metadata.blockedCount || 0} | Closed: {viewModel?.metadata.closedCount || 0}</p>
                </div>
              </div>
            </div>
          </div>
        </div>
        )}
      {hasProject && <Sidebar selectedBead={selectedBead} isCreating={isCreating} isEditing={isEditing} editForm={editForm} beads={beads} setIsEditing={setIsEditing} setIsCreating={setIsCreating} setSelectedBead={setSelectedBead} setEditForm={setEditForm} handleSaveEdit={handleSaveEdit} handleSaveCreate={handleSaveCreate} handleStartEdit={handleStartEdit} handleCloseBead={handleCloseBead} handleReopenBead={handleReopenBead} handleClaimBead={handleClaimBead} toggleFavorite={toggleFavorite} />}
      </main>
    </div>
  );
}

export default App;
