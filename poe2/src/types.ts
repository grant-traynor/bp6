// Mirror of Rust types (camelCase per serde rename_all)

export interface Project {
  id: string;
  name: string;
  path: string;
  conopsRef: string | null;
  activePhaseId: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface Node {
  id: string;
  projectId: string;
  phaseId: string | null;
  parentId: string | null;
  nodeType: 'project' | 'phase' | 'epic' | 'feature' | 'task' | 'bug' | 'chore' | 'subtask' | 'plan_review' | 'advisor';
  title: string;
  description: string | null;
  status: 'pending' | 'running' | 'waiting' | 'blocked' | 'complete' | 'cancelled';
  skillId: string | null;
  assignee: string | null;
  skillModes: string | null;  // JSON array string, e.g. '["autonomous","interactive"]'
  sortOrder: number | null;
  requiresManualVerification: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface DecisionOption {
  id: string;
  label: string;
  description?: string;
}

export interface QueueItem {
  id: string;
  projectId: string;
  agentId: string | null;
  taskId: string | null;
  question: string;
  options: string | null;   // JSON array string or null
  resolution: string | null;
  createdAt: string;
  resolvedAt: string | null;
}

export interface EventRecord {
  id: string;
  projectId: string;
  agentId: string | null;
  taskId: string | null;
  eventType: string;
  payload: string;          // JSON string
  createdAt: string;
}

export interface AgentRecord {
  id: string;
  projectId: string;
  skillId: string;
  taskId: string;
  status: string;
  sessionId: string | null;
  startedAt: string;
  endedAt: string | null;
}

export interface Phase {
  id: string;
  projectId: string;
  number: number;
  title: string;
  status: string;
  gateHeld: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface Artifact {
  id: string;
  projectId: string;
  phaseId: string | null;
  artifactType: string;
  filename: string;
  producedByStage: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface KnowledgeEntry {
  id: string;
  projectId: string;
  key: string;
  value: string;
  source: string | null;
  supersededId: string | null;
  createdAt: string;
}

// Phase 3 types

export interface AdvisorTurn {
  id: string;
  taskId: string;
  content: string;
  response: string | null;
  createdAt: string;
  respondedAt: string | null;
}

export interface PhaseGateEvent {
  phaseId: string;
  phaseTitle: string;
  completedCount: number;
  totalCount: number;
}

export interface PoePhaseUpdateEvent {
  phaseId: string;
  status: string;
  projectId: string;
}

export interface Edge {
  fromId: string;
  toId: string;
}

export interface KnowledgeItem {
  id: string;
  projectId: string;
  key: string;
  content: string;
  sourceTaskId: string | null;
  supersededId: string | null;
  promoted: number;
  createdAt: string;
}

export interface FeedItem {
  id: string;
  type: 'event' | 'agent-start' | 'agent-exit' | 'node-created';
  eventType?: string;
  taskId?: string | null;
  skillId?: string | null;
  model?: string | null;
  message: string;
  ts: string;   // ISO timestamp or createdAt
  /** bp6-7r2.8: human-readable task title carried by poe:step / poe:brief entries */
  taskTitle?: string | null;
}

export interface PoeAgentActivityEvent {
  taskId: string;
  agentId: string;
  type: 'step' | 'brief';
  content: string;
  /** bp6-7r2.8: skill name for the agent (e.g. "implementor", "reviewer") */
  skillId: string;
  /** bp6-7r2.8: human-readable title of the task the agent is working on */
  taskTitle: string;
}
