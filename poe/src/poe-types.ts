// ── POE Protocol types (from Protocol.md SQLite schema rows) ───────────────────

export interface TaskRow {
  id: string;
  projectId: string;
  phaseId: string | null;
  parentId: string | null;
  title: string;
  description: string | null;
  type: string;
  skill: string | null;
  status: 'pending' | 'running' | 'waiting' | 'done' | 'cancelled';
  sessionId: string | null;
  createdAt: number;
  updatedAt: number;
}

export interface PhaseRow {
  id: string;
  projectId: string;
  name: string;
  stageType: string;
  status: string;
  pdcaState: string;
  position: number;
  createdAt: number;
}

export interface EventLogRow {
  id: number;
  taskId: string;
  eventType: string;
  payload: string;
  createdAt: number;
}

export interface DecisionRow {
  id: number;
  taskId: string;
  question: string;
  options: string | null;  // JSON array string or null
  resolution: string | null;
  createdAt: number;
  resolvedAt: number | null;
}

export interface ProjectRow {
  id: string;
  name: string;
  path: string;
  createdAt: number;
}

export interface EdgeRow {
  fromId: string;
  toId: string;
}

export interface ProjectStateSnapshot {
  project: ProjectRow | null;
  phases: PhaseRow[];
  tasks: TaskRow[];
  edges: EdgeRow[];
  recentEvents: EventLogRow[];
  openDecisions: DecisionRow[];
}

// ── Tauri event payloads ────────────────────────────────────────────────────────

export interface PoeEventPayload {
  taskId: string;
  eventType: string;
  payload: unknown;
  createdAt: number;
}

export interface PoeTaskUpdatePayload {
  taskId: string;
  status: string;
  title?: string;
  updatedAt: number;
}

export interface PoeDecisionPayload {
  decisionId: number;
  taskId: string;
  question: string;
  options: string[] | null;
}

// ── Activity feed ───────────────────────────────────────────────────────────────

export interface ActivityItem {
  id: string;
  taskId: string;
  eventType: string;
  payload: unknown;
  createdAt: number;
}
