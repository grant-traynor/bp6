import type {
  Project,
  Node,
  Phase,
  Artifact,
  Edge,
  QueueItem,
  FeedItem,
  AdvisorTurn,
  KnowledgeItem,
  KnowledgeEntry,
  EventRecord,
  AgentRecord,
} from '../types';

// ── Project ───────────────────────────────────────────────────────────────────

export function makeProject(overrides: Partial<Project> = {}): Project {
  return {
    id: 'project-1',
    name: 'Test Project',
    path: '/tmp/test-project',
    conopsRef: null,
    activePhaseId: null,
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

// ── Phase ─────────────────────────────────────────────────────────────────────

export function makePhase(overrides: Partial<Phase> = {}): Phase {
  return {
    id: 'phase-1',
    projectId: 'project-1',
    number: 1,
    title: 'Test Phase',
    status: 'planning',
    gateHeld: false,
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

// ── Node ──────────────────────────────────────────────────────────────────────

export function makeNode(overrides: Partial<Node> = {}): Node {
  return {
    id: 'node-1',
    projectId: 'project-1',
    phaseId: null,
    parentId: null,
    nodeType: 'task',
    title: 'Test Task',
    description: null,
    status: 'pending',
    skillId: null,
    assignee: null,
    skillModes: null,
    sortOrder: null,
    requiresManualVerification: false,
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

// ── Artifact ──────────────────────────────────────────────────────────────────

export function makeArtifact(overrides: Partial<Artifact> = {}): Artifact {
  return {
    id: 'artifact-1',
    projectId: 'project-1',
    phaseId: null,
    artifactType: 'document',
    filename: 'test-artifact.md',
    producedByStage: null,
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

// ── Edge ──────────────────────────────────────────────────────────────────────

export function makeEdge(overrides: Partial<Edge> = {}): Edge {
  return {
    fromId: 'node-1',
    toId: 'node-2',
    ...overrides,
  };
}

// ── QueueItem ─────────────────────────────────────────────────────────────────

export function makeQueueItem(overrides: Partial<QueueItem> = {}): QueueItem {
  return {
    id: 'queue-1',
    projectId: 'project-1',
    agentId: null,
    taskId: null,
    question: 'What should we do?',
    options: null,
    resolution: null,
    createdAt: '2026-01-01T00:00:00Z',
    resolvedAt: null,
    ...overrides,
  };
}

// ── FeedItem ──────────────────────────────────────────────────────────────────

export function makeFeedItem(overrides: Partial<FeedItem> = {}): FeedItem {
  return {
    id: 'feed-1',
    type: 'event',
    eventType: 'poe-event',
    taskId: null,
    skillId: null,
    model: null,
    message: 'Something happened',
    ts: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

// ── AdvisorTurn ───────────────────────────────────────────────────────────────

export function makeAdvisorTurn(overrides: Partial<AdvisorTurn> = {}): AdvisorTurn {
  return {
    id: 'advisor-turn-1',
    taskId: 'node-1',
    content: 'Please review this.',
    response: null,
    createdAt: '2026-01-01T00:00:00Z',
    respondedAt: null,
    ...overrides,
  };
}

// ── KnowledgeItem ─────────────────────────────────────────────────────────────

export function makeKnowledgeItem(overrides: Partial<KnowledgeItem> = {}): KnowledgeItem {
  return {
    id: 'knowledge-1',
    projectId: 'project-1',
    key: 'test-key',
    content: 'Test knowledge content',
    sourceTaskId: null,
    supersededId: null,
    promoted: 0,
    createdAt: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

// ── KnowledgeEntry ────────────────────────────────────────────────────────────

export function makeKnowledgeEntry(overrides: Partial<KnowledgeEntry> = {}): KnowledgeEntry {
  return {
    id: 'knowledge-entry-1',
    projectId: 'project-1',
    key: 'test-key',
    value: 'Test knowledge value',
    source: null,
    supersededId: null,
    createdAt: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

// ── EventRecord ───────────────────────────────────────────────────────────────

export function makeEventRecord(overrides: Partial<EventRecord> = {}): EventRecord {
  return {
    id: 'event-1',
    projectId: 'project-1',
    agentId: null,
    taskId: null,
    eventType: 'poe-event',
    payload: '{}',
    createdAt: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

// ── AgentRecord ───────────────────────────────────────────────────────────────

export function makeAgentRecord(overrides: Partial<AgentRecord> = {}): AgentRecord {
  return {
    id: 'agent-1',
    projectId: 'project-1',
    skillId: 'default',
    taskId: 'node-1',
    status: 'running',
    sessionId: null,
    startedAt: '2026-01-01T00:00:00Z',
    endedAt: null,
    ...overrides,
  };
}
