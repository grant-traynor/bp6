import { invoke } from "@tauri-apps/api/core";

export interface Dependency {
  issue_id: string;
  depends_on_id: string;
  type: string;
  metadata?: Record<string, any>;
  [key: string]: any;
}

export interface Bead {
  id: string;
  title: string;
  description?: string;
  status: string;
  priority: number;
  issue_type: string;
  estimate?: number;
  dependencies: Dependency[];
  owner?: string;
  created_at?: string;
  created_by?: string;
  updated_at?: string;
  labels?: string[];
  acceptance_criteria?: string[];
  closed_at?: string;
  close_reason?: string;
  is_favorite?: boolean;
  parent?: string;
  external_reference?: string;
  design_notes?: string;
  working_notes?: string;
  [key: string]: any;
}

export interface WBSNode extends Bead {
  children: WBSNode[];
  parent?: string;
  isExpanded?: boolean;
  isCritical?: boolean;
  isBlocked?: boolean;
}

export interface Project {
  name: string;
  path: string;
  is_favorite?: boolean;
  last_opened?: string;
}

export async function fetchProjects(): Promise<Project[]> {
  try {
    return await invoke<Project[]>("get_projects");
  } catch (error) {
    console.error("Failed to fetch projects:", error);
    return [];
  }
}

export async function addProject(project: Project): Promise<void> {
  await invoke("add_project", { project });
}

export async function removeProject(path: string): Promise<void> {
  await invoke("remove_project", { path });
}

export async function toggleFavoriteProject(path: string): Promise<void> {
  await invoke("toggle_favorite", { path });
}

export async function openProject(path: string): Promise<void> {
  await invoke("open_project", { path });
}

export async function fetchBeads(): Promise<Bead[]> {
  try {
    return await invoke<Bead[]>("get_beads");
  } catch (error) {
    console.error("Failed to fetch beads:", error);
    throw error;
  }
}

export async function updateBead(bead: Bead): Promise<void> {
  try {
    await invoke("update_bead", { updatedBead: bead });
  } catch (error) {
    console.error("Failed to update bead:", error);
    throw error;
  }
}

export async function createBead(bead: Bead): Promise<string> {
  try {
    console.log('🔧 api.createBead: Invoking Tauri command with bead:', bead);
    const result = await invoke<string>("create_bead", { newBead: bead });
    console.log('🔧 api.createBead: Tauri returned ID:', result);
    return result;
  } catch (error) {
    console.error("🔧 api.createBead: Failed to create bead:", error);
    throw error;
  }
}

export async function closeBead(beadId: string, reason?: string): Promise<void> {
  try {
    await invoke("close_bead", { beadId, reason });
  } catch (error) {
    console.error("Failed to close bead:", error);
    throw error;
  }
}

export async function reopenBead(beadId: string): Promise<void> {
  try {
    await invoke("reopen_bead", { beadId });
  } catch (error) {
    console.error("Failed to reopen bead:", error);
    throw error;
  }
}

export async function claimBead(beadId: string): Promise<void> {
  try {
    await invoke("claim_bead", { beadId });
  } catch (error) {
    console.error("Failed to claim bead:", error);
    throw error;
  }
}

export function buildWBSTree(beads: Bead[]): WBSNode[] {
  const nodeMap = new Map<string, WBSNode>();
  const roots: WBSNode[] = [];

  const checkBlocked = (bead: Bead, allBeads: Bead[]): boolean => {
    const deps = (bead.dependencies || []).filter(d => d.type === "blocks");
    return deps.some(d => {
      const pred = allBeads.find(b => b.id === d.depends_on_id);
      return pred && pred.status !== 'closed';
    });
  };

  beads.forEach(bead => {
    nodeMap.set(bead.id, { 
      ...bead, 
      children: [], 
      isExpanded: true,
      isBlocked: checkBlocked(bead, beads)
    });
  });

  beads.forEach(bead => {
    const node = nodeMap.get(bead.id)!;
    const deps = bead.dependencies || [];
    const parentDep = deps.find(d => d.type === "parent-child");
    
    if (parentDep) {
      const parent = nodeMap.get(parentDep.depends_on_id);
      if (parent) {
        parent.children.push(node);
        node.parent = parent.id;
      } else {
        roots.push(node);
      }
    } else {
      roots.push(node);
    }
  });

  // Helper to topologically sort sibling nodes based on "blocks" dependencies
  const sortSiblings = (nodes: WBSNode[]): WBSNode[] => {
    if (nodes.length <= 1) return nodes;

    const nodeParams = new Map(nodes.map(n => [n.id, n]));
    const inDegree = new Map<string, number>();
    const graph = new Map<string, string[]>();

    // Initialize graph
    nodes.forEach(node => {
      inDegree.set(node.id, 0);
      graph.set(node.id, []);
    });

    // Build dependency graph (only considering dependencies between siblings)
    nodes.forEach(node => {
      // Find dependencies where this node is BLOCKED BY another sibling
      // If node A is blocked by node B, B must come FIRST.
      // Edge: B -> A
      const blockers = (node.dependencies || []).filter(d => d.type === "blocks");
      
      blockers.forEach(d => {
        const blockerId = d.depends_on_id;
        // Only consider if the blocker is essentially a sibling (in the list of nodes passed)
        if (nodeParams.has(blockerId)) {
          graph.get(blockerId)?.push(node.id);
          inDegree.set(node.id, (inDegree.get(node.id) || 0) + 1);
        }
      });
    });

    // Kahn's Algorithm
    const queue: string[] = [];
    
    // Sort initial queue by priority (higher priority first) to have some deterministic secondary sort
    const initialNodes = nodes.filter(n => (inDegree.get(n.id) || 0) === 0);
    initialNodes.sort((a, b) => a.priority - b.priority); // Ascending priority (1 is high)
    
    initialNodes.forEach(n => queue.push(n.id));

    const result: WBSNode[] = [];
    
    while (queue.length > 0) {
      const u = queue.shift()!;
      const node = nodeParams.get(u);
      if (node) result.push(node);

      const neighbors = graph.get(u) || [];
      // Sort neighbors to ensure deterministic order if multiple become available
      // neighbors.sort(); 

      neighbors.forEach(v => {
        inDegree.set(v, (inDegree.get(v) || 0) - 1);
        if (inDegree.get(v) === 0) {
          queue.push(v);
        }
      });
    }

    // Handle circular dependencies or disconnected components (append any remaining nodes)
    if (result.length !== nodes.length) {
      const addedIds = new Set(result.map(n => n.id));
      const remaining = nodes.filter(n => !addedIds.has(n.id));
      // Append remaining nodes, sorted by priority
      remaining.sort((a, b) => a.priority - b.priority);
      result.push(...remaining);
    }

    return result;
  };

  // Recursively sort the tree
  const processedRoots = sortSiblings(roots);
  
  const sortRecursive = (nodes: WBSNode[]) => {
    nodes.forEach(node => {
      if (node.children.length > 0) {
        node.children = sortSiblings(node.children);
        sortRecursive(node.children);
      }
    });
  };

  sortRecursive(processedRoots);

  return processedRoots;
}

export interface GanttItem {
  bead: Bead;
  x: number;
  width: number;
  row: number;
  depth: number;
  isCritical: boolean;
  isBlocked: boolean;
}

export interface GanttConnector {
  from: { x: number; y: number };
  to: { x: number; y: number };
  isCritical: boolean;
}

export interface GanttLayout {
  items: GanttItem[];
  connectors: GanttConnector[];
  rowCount: number;
  rowDepths: number[];
}

export interface BucketDistribution {
  open: number;
  inProgress: number;
  blocked: number;
  closed: number;
}

export function calculateStateDistribution(items: GanttItem[], zoom: number): BucketDistribution[] {
  if (items.length === 0) return [];

  // Find the total width in pre-zoom coordinates
  let maxRight = 0;
  items.forEach(item => {
    const right = (item.x / zoom); // item.x is already zoomed
    const width = (item.width / zoom);
    maxRight = Math.max(maxRight, right + width);
  });

  // Buckets are 100 units wide (pre-zoom)
  const numBuckets = Math.ceil(maxRight / 100);
  const distributions: BucketDistribution[] = Array.from({ length: numBuckets }, () => ({
    open: 0,
    inProgress: 0,
    blocked: 0,
    closed: 0
  }));

  items.forEach(item => {
    // Skip epics and features for the count? 
    // Usually we only count tasks.
    if (item.bead.issue_type === 'epic' || item.bead.issue_type === 'feature') return;

    const left = (item.x / zoom);
    const right = (item.x + item.width) / zoom;
    const status = item.bead.status;
    const isBlocked = item.isBlocked;

    // A bead is active in bucket i if it overlaps with [i*100, (i+1)*100]
    for (let i = 0; i < numBuckets; i++) {
        const bLeft = i * 100;
        const bRight = (i + 1) * 100;

        if (Math.max(left, bLeft) < Math.min(right, bRight)) {
            if (status === 'closed') distributions[i].closed++;
            else if (isBlocked) distributions[i].blocked++;
            else if (status === 'in_progress') distributions[i].inProgress++;
            else distributions[i].open++;
        }
    }
  });

  return distributions;
}

export function calculateGanttLayout(beads: Bead[], tree: WBSNode[], zoom: number = 1): GanttLayout {
  const items: GanttItem[] = [];
  const connectors: GanttConnector[] = [];
  
  const visibleRows: string[] = [];
  const rowDepthMap = new Map<string, number>();
  const rowDepths: number[] = [];
  
  const flatten = (nodes: WBSNode[], depth: number = 0) => {
    nodes.forEach(node => {
      visibleRows.push(node.id);
      rowDepthMap.set(node.id, depth);
      rowDepths.push(depth);
      if (node.isExpanded) flatten(node.children, depth + 1);
    });
  };
  flatten(tree);

  const rowCount = visibleRows.length;
  const rowMap = new Map<string, number>();
  visibleRows.forEach((id, index) => rowMap.set(id, index));

  const xMap = new Map<string, number>();
  const blocksMap = new Map<string, string[]>(); // successor -> [predecessors]
  const successorsMap = new Map<string, string[]>(); // predecessor -> [successors]
  
  beads.forEach(bead => {
    const deps = (bead.dependencies || []).filter(d => d.type === "blocks");
    deps.forEach(d => {
      const preds = blocksMap.get(bead.id) || [];
      preds.push(d.depends_on_id);
      blocksMap.set(bead.id, preds);

      const succs = successorsMap.get(d.depends_on_id) || [];
      succs.push(bead.id);
      successorsMap.set(d.depends_on_id, succs);
    });
  });

  // Calculate X (Earliest Start)
  const getX = (id: string, visited = new Set<string>()): number => {
    if (xMap.has(id)) return xMap.get(id)!;
    if (visited.has(id)) return 0;
    visited.add(id);

    const preds = blocksMap.get(id) || [];
    if (preds.length === 0) {
      xMap.set(id, 0);
      return 0;
    }
    const maxPredX = Math.max(...preds.map(p => getX(p, new Set(visited))));
    const x = maxPredX + 1;
    xMap.set(id, x);
    return x;
  };

  beads.forEach(bead => getX(bead.id));

  const rangeMap = new Map<string, { x: number, width: number }>();
  
  const calculateNodeRange = (node: WBSNode): { x: number, width: number } => {
    if (rangeMap.has(node.id)) return rangeMap.get(node.id)!;

    let x: number;
    let width: number;

    if (node.children.length === 0) {
      x = ((xMap.get(node.id) || 0) * 100 + 40);
      width = Math.max((node.estimate || 600) / 10, 40);
    } else {
      const childrenRanges = node.children.map(c => calculateNodeRange(c));
      const minX = Math.min(...childrenRanges.map(r => r.x));
      const maxX = Math.max(...childrenRanges.map(r => r.x + r.width));
      x = minX;
      width = maxX - minX;
    }

    const res = { x, width };
    rangeMap.set(node.id, res);
    return res;
  };

  tree.forEach(root => calculateNodeRange(root));

  // Find Critical Path (Longest Path)
  const criticalPathNodes = new Set<string>();
  if (beads.length > 0) {
    const maxDistMap = new Map<string, number>();
    const nextInPath = new Map<string, string>();

    const findMaxDist = (id: string): number => {
      if (maxDistMap.has(id)) return maxDistMap.get(id)!;
      const succs = successorsMap.get(id) || [];
      if (succs.length === 0) {
        maxDistMap.set(id, 0);
        return 0;
      }
      
      let maxVal = -1;
      let bestSucc = "";
      
      succs.forEach(s => {
        const d = findMaxDist(s);
        if (d > maxVal) {
          maxVal = d;
          bestSucc = s;
        }
      });

      maxDistMap.set(id, maxVal + 1);
      nextInPath.set(id, bestSucc);
      return maxVal + 1;
    };

    // Find the global maximum distance
    let globalMax = -1;
    let startNode = "";
    beads.forEach(b => {
      const d = findMaxDist(b.id);
      if (d > globalMax) {
        globalMax = d;
        startNode = b.id;
      }
    });

    // Reconstruct path
    let curr: string | undefined = startNode;
    while (curr) {
      criticalPathNodes.add(curr);
      curr = nextInPath.get(curr);
    }
  }

  const isBlocked = (bead: Bead, allBeads: Bead[]): boolean => {
    const deps = (bead.dependencies || []).filter(d => d.type === "blocks");
    return deps.some(d => {
      const pred = allBeads.find(b => b.id === d.depends_on_id);
      return pred && pred.status !== 'closed';
    });
  };

  beads.forEach(bead => {
    const row = rowMap.get(bead.id);
    if (row === undefined) return; 

    const range = rangeMap.get(bead.id) || { 
      x: ((xMap.get(bead.id) || 0) * 100 + 40),
      width: Math.max((bead.estimate || 600) / 10, 40)
    };
    
    const x = range.x * zoom;
    const width = range.width * zoom;

    items.push({
      bead,
      row,
      depth: rowDepthMap.get(bead.id) || 0,
      x,
      width,
      isCritical: criticalPathNodes.has(bead.id),
      isBlocked: isBlocked(bead, beads)
    });

    const deps = (bead.dependencies || []).filter(d => d.type === "blocks");
    deps.forEach(d => {
      const predRow = rowMap.get(d.depends_on_id);
      if (predRow === undefined) return;

      const predRange = rangeMap.get(d.depends_on_id) || {
        x: ((xMap.get(d.depends_on_id) || 0) * 100 + 40),
        width: Math.max((beads.find(b => b.id === d.depends_on_id)?.estimate || 600) / 10, 40)
      };

      const predX = predRange.x * zoom;
      const predWidth = predRange.width * zoom;

      connectors.push({
        from: { x: predX + predWidth, y: predRow * 48 + 24 },
        to: { x: x, y: row * 48 + 24 },
        isCritical: criticalPathNodes.has(bead.id) && criticalPathNodes.has(d.depends_on_id)
      });
    });
  });

  return { items, connectors, rowCount, rowDepths };
}
