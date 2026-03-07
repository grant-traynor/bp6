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
  nodeType: 'project' | 'phase' | 'epic' | 'feature' | 'task' | 'bug' | 'chore' | 'subtask';
  title: string;
  description: string | null;
  status: 'pending' | 'running' | 'blocked' | 'complete' | 'cancelled';
  skillId: string | null;
  assignee: string | null;
  createdAt: string;
  updatedAt: string;
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

export interface FeedItem {
  id: string;
  type: 'event' | 'agent-start' | 'agent-exit' | 'node-created';
  eventType?: string;
  taskId?: string | null;
  skillId?: string | null;
  message: string;
  ts: string;   // ISO timestamp or createdAt
}
