import { useState, useEffect, useCallback, useRef, useMemo, startTransition } from "react";
import { Star, ChevronsDown, ChevronsUp, ArrowUp, ArrowDown, PanelRight } from "lucide-react";
import { cn } from "./utils";
import { 
  fetchProjectViewModel, 
  updateBead, 
  createBead, 
  closeBead, 
  reopenBead, 
  claimBead, 
  beadNodeToBead, 
  type BeadNode, 
  type Project, 
  type ProjectViewModel, 
  fetchProjects, 
  removeProject, 
  openProject, 
  toggleFavoriteProject, 
  getCliPreference, 
  type CliBackend, 
  type SessionInfo,
  onBeadsUpdated,
  onProjectsUpdated,
  onSessionListChanged
} from "./api";

// Components
import { Navigation } from "./components/layout/Navigation";
import { Header } from "./components/layout/Header";
import { Sidebar } from "./components/layout/Sidebar";
import { WBSTreeList } from "./components/wbs/WBSTreeItem";
import { GanttBar } from "./components/gantt/GanttBar";
import { GanttStateHeader } from "./components/gantt/GanttStateHeader";
import { WBSSkeleton, GanttSkeleton } from "./components/shared/Skeleton";
import ChatDialog from "./components/chat/ChatDialog";

// Time-based filter options for closed tasks
type ClosedTimeFilter =
  | 'all'           // Show all closed tasks
  | '1h'            // Closed within last hour
  | '6h'            // Closed within last 6 hours
  | '24h'           // Closed within last 24 hours
  | '7d'            // Closed within last 7 days
  | '30d'           // Closed within last 30 days
  | 'older_than_6h'; // Closed more than 6 hours ago

interface AppProps {
  isSessionWindow?: boolean;
  sessionId?: string | null;
  windowLabel?: string;
}

function App({ isSessionWindow = false, sessionId = null, windowLabel = "main" }: AppProps = {}) {
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
  const [editForm, setEditForm] = useState<Partial<BeadNode>>({});
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

  // Agent Session State
  const [sessionsByBead, setSessionsByBead] = useState<Record<string, SessionInfo[]>>({});

  // Chat state
  const [isChatOpen, setIsChatOpen] = useState(false);
  const [chatPersona, setChatPersona] = useState("product-manager");
  const [chatTask, setChatTask] = useState<string | null>(null);
  const [chatBeadId, setChatBeadId] = useState<string | null>(null);

  // CLI preference state
  const [currentCli, setCurrentCli] = useState<CliBackend>("gemini");

  const scrollRefWBS = useRef<HTMLDivElement>(null);
  const scrollRefBERT = useRef<HTMLDivElement>(null);
  const scrollRefGanttHeader = useRef<HTMLDivElement>(null);
  const activeScrollSource = useRef<HTMLDivElement | null>(null);
  const hasInitialized = useRef(false);
  const lastToggledNode = useRef<{ id: string; offsetTop: number } | null>(null);

  // Early return for session windows - render only ChatDialog
  // Note: Full session connection implementation is in bp6-643.005.4
  if (isSessionWindow && sessionId) {
    console.log('📱 Session window mode:', { sessionId, windowLabel });

    return (
      <div className="flex h-screen w-screen overflow-hidden bg-[var(--background-primary)] text-[var(--text-primary)] font-sans">
        {/* Session window: fullscreen ChatDialog */}
        {/* TODO (bp6-643.005.4): Connect to existing session instead of creating new */}
        <ChatDialog
          isOpen={true}
          onClose={() => {
            // Session windows can't be closed from within - only via window close
            console.log('Session window ChatDialog close requested (no-op)');
          }}
          persona="product-manager"
          task={`Session window for session ${sessionId}`}
          beadId={null}
          cliBackend={currentCli}
        />
      </div>
    );
  }

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
  const [sortBy, setSortBy] = useState<'priority' | 'title' | 'type' | 'id' | 'none'>('none');
  const [sortOrder, setSortOrder] = useState<'asc' | 'desc' | 'none'>('none');
  const [sidebarOpen, setSidebarOpen] = useState(true);

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
              collapsed_ids: Array.from(collapsedIds), // Backend now handles collapsed state
              sort_by: sortBy,
              sort_order: sortOrder
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
  }, [filterText, zoom, hideClosed, includeHierarchy, closedTimeFilter, collapsedIds, currentProjectPath, refetchTrigger, sortBy, sortOrder]);

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

    const unlistenPromises = [
      onBeadsUpdated(() => {
        console.log('🎉 beads-updated event received!');
        loadData();
        setRefetchTrigger(prev => prev + 1);
      }),
      onProjectsUpdated(() => {
        console.log('🎉 projects-updated event received!');
        loadProjects();
      }),
      onSessionListChanged((sessions) => {
        console.log('🎉 session-list-changed event received!', sessions);
        const grouped: Record<string, SessionInfo[]> = {};
        sessions.forEach(s => {
          if (!grouped[s.bead_id]) grouped[s.bead_id] = [];
          grouped[s.bead_id].push(s);
        });
        setSessionsByBead(grouped);
      })
    ];

    return () => {
      console.log('🧹 Cleaning up event listeners');
      unlistenPromises.forEach(async (p) => {
        try {
          const unlisten = await p;
          unlisten();
        } catch (err) {
          console.error('❌ Failed to unlisten:', err);
        }
      });
    };
  }, [handleOpenProject, loadData, loadProjects]);

  // Load CLI preference on mount
  useEffect(() => {
    const loadCliPreference = async () => {
      try {
        const preference = await getCliPreference();
        setCurrentCli(preference as CliBackend);
      } catch (error) {
        console.error("Failed to load CLI preference:", error);
        // Keep default value (gemini) on error
      }
    };
    loadCliPreference();
  }, []);

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

  // Gantt layout: flatten tree to visible nodes with row numbers and pixel positions
  const ganttLayout = useMemo(() => {
    if (!viewModel) {
      return { items: [], rowCount: 0, rowDepths: [], connectors: [] };
    }

    const items: Array<{
      bead: BeadNode;
      x: number;
      width: number;
      row: number;
      depth: number;
      isCritical: boolean;
    }> = [];
    const rowDepths: number[] = [];
    const idToItem = new Map<string, { x: number; width: number; row: number }>();
    let rowIndex = 0;

    // Traverse tree and build visible items with row numbers
    const traverse = (nodes: BeadNode[], depth: number = 0) => {
      nodes.forEach(node => {
        // Convert cell offset/count to pixels
        // Cell size: 100px at zoom=1
        const cellSize = 100 * zoom;
        const x = node.cellOffset * cellSize;
        const width = node.cellCount * cellSize;

        // Debug: log first few items to see values
        if (rowIndex < 3) {
          console.log(`Bead ${node.id}: cellOffset=${node.cellOffset}, cellCount=${node.cellCount}, x=${x}px, width=${width}px`);
        }

        const item = {
          bead: node,
          x,
          width,
          row: rowIndex,
          depth,
          isCritical: node.isCritical,
        };
        items.push(item);
        idToItem.set(node.id, { x, width, row: rowIndex });
        rowDepths.push(depth);
        rowIndex++;

        // Recurse to children if node is expanded
        if (node.isExpanded && node.children.length > 0) {
          traverse(node.children, depth + 1);
        }
      });
    };

    traverse(viewModel.tree);

    // Build connectors from "blocks" dependencies
    const connectors: Array<{
      fromId: string;
      toId: string;
      fromX: number;
      fromY: number;
      toX: number;
      toY: number;
      isCritical: boolean;
    }> = [];

    items.forEach(item => {
      // Find "blocks" dependencies (other tasks block this task)
      // If item has {type: "blocks", depends_on_id: "B"}, it means B blocks item
      // So we draw: B → item (from blocker to blocked)
      const blocksDeps = item.bead.dependencies?.filter((d: any) => d.type === 'blocks') || [];

      // Debug: log dependencies for first few items
      if (item.row < 3 && item.bead.dependencies && item.bead.dependencies.length > 0) {
        console.log(`Dependencies for ${item.bead.id}:`, item.bead.dependencies);
      }

      blocksDeps.forEach((dep: any) => {
        const blocker = idToItem.get(dep.depends_on_id);
        if (blocker) {
          // Account for the 20px left padding (8px + 12px) on bars
          const CONNECTOR_PADDING = 20;

          // Connector from right edge of BLOCKER to left edge of BLOCKED task
          const connector = {
            fromId: dep.depends_on_id,
            toId: item.bead.id,
            fromX: blocker.x + blocker.width,      // Right edge of blocker cell
            fromY: blocker.row * 48 + 24,
            toX: item.x + CONNECTOR_PADDING,       // Left edge of blocked bar (after padding)
            toY: item.row * 48 + 24,
            isCritical: items.find(i => i.bead.id === dep.depends_on_id)?.isCritical && item.isCritical || false
          };
          connectors.push(connector);

          // Enhanced debug logging
          if (connectors.length <= 3) {
            console.log(`Connector ${connectors.length}:`, {
              from: dep.depends_on_id,
              to: item.bead.id,
              blockerCell: items.find(i => i.bead.id === dep.depends_on_id)?.bead.cellOffset,
              blockerPixels: blocker.x,
              blockedCell: item.bead.cellOffset,
              blockedPixels: item.x,
              connector
            });
          }
        }
      });
    });

    console.log(`Total connectors: ${connectors.length}`);
    return { items, rowCount: rowIndex, rowDepths, connectors };
  }, [viewModel, zoom]);

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

  const handleOpenChat = useCallback((persona: string, task?: string, beadId?: string) => {
    setChatPersona(persona);
    setChatTask(task ?? null);
    setChatBeadId(beadId ?? null);
    setIsChatOpen(true);
  }, []);

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
        case 'Escape': setSelectedBead(null); setIsCreating(false); setIsEditing(false); setIsChatOpen(false); break;
        case '+': case '=': e.preventDefault(); expandAll(); break;
        case '-': case '_': e.preventDefault(); collapseAll(); break;
        case 'n': e.preventDefault(); handleStartCreate(); break;
        case 'r': e.preventDefault(); loadData(); break;
        case 'c': e.preventDefault(); handleOpenChat('product-manager'); break;
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [expandAll, collapseAll, loadData, handleOpenChat]);

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

  const handleBeadClick = (bead: BeadNode) => {
    setSelectedBead(bead);
    setIsEditing(false);
    setIsCreating(false);
  };

  const handleHeaderClick = (column: 'priority' | 'title' | 'type' | 'id') => {
    if (sortBy === column) {
      if (sortOrder === 'asc') setSortOrder('desc');
      else if (sortOrder === 'desc') {
        setSortOrder('none');
        setSortBy('none');
      } else {
        setSortOrder('asc');
      }
    } else {
      setSortBy(column);
      setSortOrder('asc');
    }
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
        const beads = flattenTree(viewModel?.tree || []);
        const currentParent = beads.find(b => b.dependencies?.some((d: any) => d.issue_id === selectedBead.id && d.type === 'parent-child'));
        const updatedNode = { ...editForm };
        if (updatedNode.parent !== currentParent?.id) {
          updatedNode.dependencies = (updatedNode.dependencies || []).filter((d: any) => d.type !== 'parent-child');
          if (updatedNode.parent) {
            updatedNode.dependencies.push({ issue_id: updatedNode.id!, depends_on_id: updatedNode.parent, type: 'parent-child' });
          }
        }
        const updated = beadNodeToBead(updatedNode);
        await updateBead(updated as any);
        setSelectedBead(updatedNode as BeadNode);
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
      issueType: "task",
      dependencies: [],
      createdAt: new Date().toISOString(),
      acceptanceCriteria: []
    } as Partial<BeadNode>);
    setIsEditing(false);
    setIsCreating(true);
  }, []);

  const handleSaveCreate = async () => {
    console.log('🆕 handleSaveCreate called', { editForm });
    if (editForm.title) {
      console.log('🆕 Title exists, proceeding with create');
      try {
        const newNode = { ...editForm };
        console.log('🆕 newNode prepared:', newNode);
        if (newNode.parent) {
          newNode.dependencies = [...(newNode.dependencies || []), { issue_id: newNode.id!, depends_on_id: newNode.parent, type: 'parent-child' }];
        }
        const newBead = beadNodeToBead(newNode);
        console.log('🆕 Calling createBead...');
        const createdId = await createBead(newBead as any);
        console.log('🆕 Created bead ID:', createdId);
        setIsCreating(false);
        await loadData();
        const allBeads = flattenTree(viewModel?.tree || []);
        const createdBead = allBeads.find((b: BeadNode) => b.id === createdId);
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

  const toggleFavorite = async (bead: BeadNode) => {
    try {
      const updated = beadNodeToBead({
        ...bead,
        isFavorite: !bead.isFavorite
      });
      await updateBead(updated as any);
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
        <Header isDark={isDark} setIsDark={setIsDark} handleStartCreate={handleStartCreate} loadData={loadData} onOpenChat={handleOpenChat} projectMenuOpen={projectMenuOpen} setProjectMenuOpen={setProjectMenuOpen} favoriteProjects={favoriteProjects} recentProjects={recentProjects} currentProjectPath={currentProjectPath} handleOpenProject={handleOpenProject} toggleFavoriteProject={handleToggleFavoriteProject} removeProject={handleRemoveProject} handleSelectProject={handleSelectProject} currentCli={currentCli} setCurrentCli={setCurrentCli} />
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
          <div className="flex-1 flex overflow-hidden">
            <div className="flex-1 flex flex-col overflow-hidden">
              <div className="flex shrink-0 border-b-2 border-[var(--border-primary)] bg-[var(--background-secondary)] z-20">
                <div className="w-[40%] min-w-[525px] border-r-2 border-[var(--border-primary)] flex flex-col">
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
                      <div className="w-px h-5 bg-[var(--border-primary)] mx-2" />
                      <button 
                        onClick={() => setSidebarOpen(!sidebarOpen)} 
                        className={cn(
                          "p-2 hover:bg-[var(--background-tertiary)] rounded-xl transition-all active:scale-90",
                          sidebarOpen ? "text-indigo-500 bg-indigo-500/10" : "text-[var(--text-primary)]"
                        )}
                        title={sidebarOpen ? "Close Sidebar" : "Open Sidebar"}
                      >
                        <PanelRight size={18} strokeWidth={2.5} />
                      </button>
                  </div>
                </div>
              </div>
              <div className="flex shrink-0 border-b-2 border-[var(--border-primary)] bg-[var(--background-secondary)] z-10">
                <div className="w-[40%] min-w-[525px] border-r-2 border-[var(--border-primary)] flex items-center px-4 py-2 bg-[var(--background-tertiary)] text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.3em] select-none">
                  <div className="w-10 shrink-0" />
                  <div 
                    className={cn(
                      "w-16 shrink-0 px-2 border-r-2 border-[var(--border-primary)]/50 cursor-pointer hover:text-indigo-500 transition-colors flex items-center justify-between group",
                      sortBy === 'priority' && sortOrder !== 'none' && "text-indigo-500"
                    )}
                    onClick={() => handleHeaderClick('priority')}
                  >
                    <span>P</span>
                    {sortBy === 'priority' && sortOrder !== 'none' && (
                      sortOrder === 'asc' ? <ArrowUp size={12} strokeWidth={3} /> : <ArrowDown size={12} strokeWidth={3} />
                    )}
                  </div>
                  <div 
                    className={cn(
                      "flex-1 px-4 border-r-2 border-[var(--border-primary)]/50 cursor-pointer hover:text-indigo-500 transition-colors flex items-center justify-between group",
                      sortBy === 'title' && sortOrder !== 'none' && "text-indigo-500"
                    )}
                    onClick={() => handleHeaderClick('title')}
                  >
                    <span>Name</span>
                    {sortBy === 'title' && sortOrder !== 'none' && (
                      sortOrder === 'asc' ? <ArrowUp size={12} strokeWidth={3} /> : <ArrowDown size={12} strokeWidth={3} />
                    )}
                  </div>
                  <div 
                    className={cn(
                      "w-20 shrink-0 px-2 border-r-2 border-[var(--border-primary)]/50 cursor-pointer hover:text-indigo-500 transition-colors flex items-center justify-between group",
                      sortBy === 'type' && sortOrder !== 'none' && "text-indigo-500"
                    )}
                    onClick={() => handleHeaderClick('type')}
                  >
                    <span>Type</span>
                    {sortBy === 'type' && sortOrder !== 'none' && (
                      sortOrder === 'asc' ? <ArrowUp size={12} strokeWidth={3} /> : <ArrowDown size={12} strokeWidth={3} />
                    )}
                  </div>
                  <div 
                    className={cn(
                      "w-24 shrink-0 px-2 cursor-pointer hover:text-indigo-500 transition-colors flex items-center justify-between group",
                      sortBy === 'id' && sortOrder !== 'none' && "text-indigo-500"
                    )}
                    onClick={() => handleHeaderClick('id')}
                  >
                    <span>ID</span>
                    {sortBy === 'id' && sortOrder !== 'none' && (
                      sortOrder === 'asc' ? <ArrowUp size={12} strokeWidth={3} /> : <ArrowDown size={12} strokeWidth={3} />
                    )}
                  </div>
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
                <div ref={scrollRefWBS} onScroll={handleScroll} onMouseEnter={handleMouseEnter} className="w-[40%] border-r-2 border-[var(--border-primary)] flex flex-col bg-[var(--background-secondary)] min-w-[525px] overflow-y-auto custom-scrollbar">
                  <div className="p-0">
                    {(loading || processingData) ? <WBSSkeleton /> : (
                      <div className="flex flex-col">
                        <WBSTreeList 
                          nodes={viewModel?.tree || []} 
                          onToggle={toggleNode} 
                          onClick={handleBeadClick} 
                          selectedId={selectedBead?.id} 
                          sessionsByBead={sessionsByBead}
                        />
                      </div>
                    )}
                  </div>
                </div>
                <div ref={scrollRefBERT} onScroll={handleScroll} onMouseEnter={handleMouseEnter} className="flex-1 relative bg-[var(--background-primary)] overflow-auto custom-scrollbar">
                  <div className="relative" style={{ height: Math.max(800, ganttLayout.rowCount * 48), width: 5000 * zoom }}>
                    {loading && <GanttSkeleton />}
                    {/* Background grid */}
                    <div className="absolute inset-0 pointer-events-none">
                      {ganttLayout.rowDepths.map((depth, i) => (
                        <div key={i} className="w-full border-b-2 border-[var(--border-primary)]/40" style={{ height: '48px', backgroundColor: `var(--level-${Math.min(depth, 4)})` }} />
                      ))}
                      <div className="absolute inset-0 flex">
                        {Array.from({ length: 50 }).map((_, i) => (
                          <div key={i} className="h-full border-r-2 border-[var(--border-primary)]/40" style={{ width: 100 * zoom }} />
                        ))}
                      </div>
                    </div>
                    {/* Dependency connectors */}
                    <svg
                      className="absolute inset-0 pointer-events-none"
                      style={{ zIndex: 30 }}
                      width={5000 * zoom}
                      height={Math.max(800, ganttLayout.rowCount * 48)}
                    >
                      {ganttLayout.connectors.map((conn, idx) => {
                        // Keep vertical segment in connector channel (first 20px of each cell)
                        // Place it 10px into the channel immediately after the blocker
                        const channelOffset = 10 * zoom;
                        const verticalX = conn.fromX + channelOffset;

                        const path = `M ${conn.fromX} ${conn.fromY} L ${verticalX} ${conn.fromY} L ${verticalX} ${conn.toY} L ${conn.toX} ${conn.toY}`;
                        return (
                          <path
                            key={`${conn.fromId}-${conn.toId}-${idx}`}
                            d={path}
                            stroke={conn.isCritical ? "#ef4444" : "#94a3b8"}
                            strokeWidth="3"
                            fill="none"
                            opacity="0.9"
                            strokeLinecap="round"
                            strokeLinejoin="round"
                          />
                        );
                      })}
                    </svg>
                    {/* Gantt bars */}
                    {ganttLayout.items.map((item) => (
                      <div key={item.bead.id} style={{ position: 'absolute', top: item.row * 48, height: 48, left: 0, right: 0 }}>
                        <GanttBar
                          item={{
                            bead: item.bead,
                            x: item.x,
                            width: item.width,
                            row: item.row,
                            depth: item.depth,
                            isCritical: item.isCritical,
                            isBlocked: item.bead.isBlocked,
                          }}
                          onClick={handleBeadClick}
                          isSelected={selectedBead?.id === item.bead.id}
                        />
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            </div>
            {sidebarOpen && <Sidebar selectedBead={selectedBead} isCreating={isCreating} isEditing={isEditing} editForm={editForm} beads={beads} setIsEditing={setIsEditing} setIsCreating={setIsCreating} setSelectedBead={setSelectedBead} setEditForm={setEditForm} handleSaveEdit={handleSaveEdit} handleSaveCreate={handleSaveCreate} handleStartEdit={handleStartEdit} handleCloseBead={handleCloseBead} handleReopenBead={handleReopenBead} handleClaimBead={handleClaimBead} toggleFavorite={toggleFavorite} onOpenChat={handleOpenChat} />}
          </div>
        )}
        <ChatDialog isOpen={isChatOpen} onClose={() => setIsChatOpen(false)} persona={chatPersona} task={chatTask} beadId={chatBeadId} cliBackend={currentCli} />
      </main>
    </div>
  );
}

export default App;
