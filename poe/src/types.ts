// ── Node & Edge types ──────────────────────────────────────────────────────────

export type NodeType =
  | "Project"
  | "Epic"
  | "Feature"
  | "Task"
  | "Decision"
  | "KnowledgeArtifact"
  | "AgentOutput"
  | "Review";

export type EdgeType =
  | "blocks"
  | "depends-on"
  | "generated-by"
  | "approved-by"
  | "discovered-from"
  | "implements"
  | "tests"
  | "contradicts";

export interface DagNode {
  id: string;
  nodeType: NodeType;
  projectId: string;
  data: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
}

export interface DagEdge {
  id: string;
  fromId: string;
  toId: string;
  edgeType: EdgeType;
  data: Record<string, unknown>;
  createdAt: string;
}

export interface DagSnapshot {
  nodes: DagNode[];
  edges: DagEdge[];
}

export interface ProjectInfo {
  projectId: string;
  projectDir: string;
  name: string;
}

// ── Queue types ─────────────────────────────────────────────────────────────────

export interface QueueItemOption {
  id: string;
  label: string;
  description?: string;
}

export type QueueItemStatus = "pending" | "resolved" | "dismissed";

export interface QueueItem {
  id: string;
  projectId: string;
  agentId: string;
  workflowId?: string;
  awakeableId?: string;
  question: string;
  options: QueueItemOption[];
  contextSnapshot: Record<string, unknown>;
  priority: number;
  status: QueueItemStatus;
  createdAt: string;
  resolvedAt?: string;
  resolution?: Record<string, unknown>;
}
