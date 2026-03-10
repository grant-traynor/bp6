pub const CREATE_TABLES: &str = "
PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;

CREATE TABLE IF NOT EXISTS projects (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    path        TEXT NOT NULL UNIQUE,
    conops_ref  TEXT,
    active_phase_id TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS phases (
    id              TEXT PRIMARY KEY,
    project_id      TEXT NOT NULL REFERENCES projects(id),
    number          INTEGER NOT NULL,
    title           TEXT NOT NULL,
    lifecycle_stage TEXT NOT NULL DEFAULT 'planning',
    gate_held       INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    UNIQUE(project_id, number)
);

CREATE TABLE IF NOT EXISTS nodes (
    id                  TEXT PRIMARY KEY,
    project_id          TEXT NOT NULL REFERENCES projects(id),
    phase_id            TEXT REFERENCES phases(id),
    parent_id           TEXT REFERENCES nodes(id),
    node_type           TEXT NOT NULL,
    title               TEXT NOT NULL,
    description         TEXT,
    status              TEXT NOT NULL DEFAULT 'pending',
    skill_id            TEXT,
    assignee            TEXT,
    yield_reason        TEXT,
    session_id          TEXT,
    requesting_task_id  TEXT REFERENCES nodes(id),
    review_id           TEXT,
    retry_count         INTEGER NOT NULL DEFAULT 0,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_nodes_project ON nodes(project_id);
CREATE INDEX IF NOT EXISTS idx_nodes_phase ON nodes(phase_id);
CREATE INDEX IF NOT EXISTS idx_nodes_parent ON nodes(parent_id);
CREATE INDEX IF NOT EXISTS idx_nodes_status ON nodes(status);
CREATE INDEX IF NOT EXISTS idx_nodes_requesting_task_id ON nodes(requesting_task_id);

CREATE TABLE IF NOT EXISTS edges (
    id          TEXT PRIMARY KEY,
    from_id     TEXT NOT NULL REFERENCES nodes(id),
    to_id       TEXT NOT NULL REFERENCES nodes(id),
    edge_type   TEXT NOT NULL DEFAULT 'depends_on',
    created_at  TEXT NOT NULL,
    UNIQUE(from_id, to_id, edge_type)
);

CREATE INDEX IF NOT EXISTS idx_edges_from ON edges(from_id);
CREATE INDEX IF NOT EXISTS idx_edges_to ON edges(to_id);

CREATE TABLE IF NOT EXISTS artifacts (
    id                  TEXT PRIMARY KEY,
    project_id          TEXT NOT NULL REFERENCES projects(id),
    phase_id            TEXT REFERENCES phases(id),
    artifact_type       TEXT NOT NULL,
    filename            TEXT NOT NULL,
    produced_by_stage   TEXT,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL,
    UNIQUE(project_id, filename)
);

CREATE TABLE IF NOT EXISTS knowledge (
    id              TEXT PRIMARY KEY,
    project_id      TEXT NOT NULL REFERENCES projects(id),
    key             TEXT NOT NULL,
    value           TEXT NOT NULL,
    source          TEXT,
    supersedes_id   TEXT REFERENCES knowledge(id),
    created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_knowledge_project ON knowledge(project_id);
CREATE INDEX IF NOT EXISTS idx_knowledge_key ON knowledge(project_id, key);

CREATE TABLE IF NOT EXISTS events (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id),
    agent_id    TEXT,
    task_id     TEXT,
    event_type  TEXT NOT NULL,
    payload     TEXT NOT NULL,
    created_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_events_project ON events(project_id);
CREATE INDEX IF NOT EXISTS idx_events_task ON events(task_id);

CREATE TABLE IF NOT EXISTS agents (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id),
    skill_id    TEXT NOT NULL,
    task_id     TEXT NOT NULL REFERENCES nodes(id),
    status      TEXT NOT NULL DEFAULT 'running',
    session_id  TEXT,
    started_at  TEXT NOT NULL,
    ended_at    TEXT
);

CREATE INDEX IF NOT EXISTS idx_agents_project ON agents(project_id);
CREATE INDEX IF NOT EXISTS idx_agents_task ON agents(task_id);
CREATE INDEX IF NOT EXISTS idx_agents_status ON agents(status);

CREATE TABLE IF NOT EXISTS queue_items (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id),
    agent_id    TEXT,
    task_id     TEXT,
    question    TEXT NOT NULL,
    options     TEXT,
    resolution  TEXT,
    created_at  TEXT NOT NULL,
    resolved_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_queue_project ON queue_items(project_id);
CREATE INDEX IF NOT EXISTS idx_queue_unresolved ON queue_items(project_id, resolved_at);

CREATE TABLE IF NOT EXISTS chat_turns (
    id          TEXT PRIMARY KEY,
    task_id     TEXT NOT NULL,
    content     TEXT NOT NULL,
    response    TEXT,
    created_at  TEXT NOT NULL,
    responded_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_chat_turns_task ON chat_turns(task_id);

CREATE TABLE IF NOT EXISTS advisor_turns (
    id           TEXT PRIMARY KEY,
    task_id      TEXT NOT NULL REFERENCES nodes(id),
    content      TEXT NOT NULL,
    response     TEXT,
    created_at   TEXT NOT NULL,
    responded_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_advisor_turns_task ON advisor_turns(task_id);
";
