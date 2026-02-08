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
  [key: string]: any;
}

export interface WBSNode extends Bead {
  children: WBSNode[];
  parent?: string;
  isExpanded?: boolean;
  isCritical?: boolean;
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
    return [];
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

export async function createBead(bead: Bead): Promise<void> {
  try {
    await invoke("create_bead", { newBead: bead });
  } catch (error) {
    console.error("Failed to create bead:", error);
    throw error;
  }
}

export function buildWBSTree(beads: Bead[]): WBSNode[] {
  const nodeMap = new Map<string, WBSNode>();
  const roots: WBSNode[] = [];

  beads.forEach(bead => {
    nodeMap.set(bead.id, { ...bead, children: [], isExpanded: true });
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

  return roots;
}

export interface GanttItem {
  bead: Bead;
  x: number;
  width: number;
  row: number;
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
}

export function calculateGanttLayout(beads: Bead[], tree: WBSNode[], zoom: number = 1): GanttLayout {
  const items: GanttItem[] = [];
  const connectors: GanttConnector[] = [];
  
  const visibleRows: string[] = [];
  const flatten = (nodes: WBSNode[]) => {
    nodes.forEach(node => {
      visibleRows.push(node.id);
      if (node.isExpanded) flatten(node.children);
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

    const width = Math.max((bead.estimate || 600) / 10, 40) * zoom;
    const x = (xMap.get(bead.id) || 0) * (100 * zoom) + (40 * zoom);

    items.push({
      bead,
      row,
      x,
      width,
      isCritical: criticalPathNodes.has(bead.id),
      isBlocked: isBlocked(bead, beads)
    });

    const deps = (bead.dependencies || []).filter(d => d.type === "blocks");
    deps.forEach(d => {
      const predRow = rowMap.get(d.depends_on_id);
      if (predRow === undefined) return;

      const predX = (xMap.get(d.depends_on_id) || 0) * (100 * zoom) + (40 * zoom);
      const predWidth = Math.max((beads.find(b => b.id === d.depends_on_id)?.estimate || 600) / 10, 40) * zoom;

      connectors.push({
        from: { x: predX + predWidth, y: predRow * 48 + 24 },
        to: { x: x, y: row * 48 + 24 },
        isCritical: criticalPathNodes.has(bead.id) && criticalPathNodes.has(d.depends_on_id)
      });
    });
  });

  return { items, connectors, rowCount };
}
