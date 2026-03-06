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
