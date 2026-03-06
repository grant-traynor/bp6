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

// ── Mission control types (bp6-80q) ────────────────────────────────────────────

export interface ProbeData {
  node: DagNode;
  incomingEdges: DagEdge[];
  outgoingEdges: DagEdge[];
  decisions: DagNode[];
  artifacts: DagNode[];
  connectedNodes: DagNode[];
}

export type WorkflowStatus = "running" | "paused" | "blocked" | "completed" | "cancelled" | "failed";

export interface WorkflowInfo {
  workflowId: string;
  nodeId: string;
  agentId: string;
  status: WorkflowStatus;
  currentStep?: string;
  startedAt: string;
}

/// Phase 3: SQLite workflow record (matches WorkflowRecord in dag/mod.rs)
export interface WorkflowRecord {
  id: string;
  projectId: string;
  nodeId: string;
  agentId?: string;
  workflowType: string;
  status: WorkflowStatus;
  currentStep?: string;
  config: Record<string, unknown>;
  startedAt: string;
  updatedAt: string;
  completedAt?: string;
  error?: string;
}

export interface AgentStdoutLine {
  workflowId: string;
  line: string;
  ts: string;
}

// ── Queue types ─────────────────────────────────────────────────────────────────

export interface QueueItemOption {
  id: string;
  label: string;
  description?: string;
}

export type QueueItemStatus = "pending" | "resolved" | "dismissed";

// ── Skill types ─────────────────────────────────────────────────────────────────

export interface SkillMeta {
  id: string;
  name: string;
  description: string;
  tags: string[];
  appliesTo: string[];
  source: "global" | "user" | "project";
  path: string;
}

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

// ── Lifecycle types (bp6-ims.9) ─────────────────────────────────────────────

export interface LifecycleStatus {
  step: number;
  substep: string | null;
  status: "idle" | "running" | "awaiting_approval" | "complete";
  pendingApprovalId: string | null;
  projectId: string;
  activeAgentId: string | null;
  stageTaskCount: number | null;
}

export interface ArtifactRecord {
  id: string;
  projectId: string;
  step: number;
  substep: string | null;
  title: string;
  content: string;
  createdAt: string;
}
