# POE — Protocol Specification

**Status**: Draft
**Last updated**: 2026-03-07

**Artifact classification**: This document serves as both:
- `interface-control.md` — the authoritative Interface Control Document for POE v2. Defines all external interface contracts: the poe: event wire format (§2), the agent stdin bundle format (§3), and the frontend update mechanism (§4).
- `data-model.md` — the authoritative Database Design Document. Defines the SQLite schema for all internal data structures (§1).

These are standard artifact types in the POE corpus (see Architecture.md §Artifact Corpus). They are injected into every implementation task's input bundle. When in doubt about a wire format, field name, or schema question — this document is the answer.

This document specifies the four I/O contracts that Phase 2 is built around. Everything else in the system is implementation detail; divergence here causes cross-component rework.

---

## 1. SQLite Schema

All durable state lives in `{project}/.poe/poe.db`. Phase 2 creates this schema on first run. Phase 1's `dag/mod.rs` NodeType/EdgeType enums are **not extended** — they are superseded by this schema and the v1 lifecycle module is retired.

```sql
CREATE TABLE projects (
  id          TEXT    PRIMARY KEY,
  name        TEXT    NOT NULL,
  path        TEXT    NOT NULL,
  created_at  INTEGER NOT NULL
);

CREATE TABLE phases (
  id          TEXT    PRIMARY KEY,
  project_id  TEXT    NOT NULL REFERENCES projects(id),
  name        TEXT    NOT NULL,
  stage_type  TEXT    NOT NULL,  -- 'conops' | 'guardrails' | 'increment_planning' | 'execution' | 'rework' | 'retrospective' | 'onboarding'
  status      TEXT    NOT NULL DEFAULT 'pending',  -- 'pending' | 'running' | 'gate' | 'complete'
  pdca_state  TEXT    NOT NULL DEFAULT 'plan',     -- 'plan' | 'do' | 'check' | 'act'
  position    INTEGER NOT NULL,
  created_at  INTEGER NOT NULL
);

CREATE TABLE tasks (
  id          TEXT    PRIMARY KEY,
  project_id  TEXT    NOT NULL REFERENCES projects(id),
  phase_id    TEXT    REFERENCES phases(id),
  parent_id   TEXT    REFERENCES tasks(id),
  title       TEXT    NOT NULL,
  description TEXT,
  type        TEXT    NOT NULL DEFAULT 'task',    -- 'task' | 'bug' | 'chore' | 'subtask'
  skill       TEXT,
  status      TEXT    NOT NULL DEFAULT 'pending', -- 'pending' | 'running' | 'waiting' | 'done' | 'cancelled'
  session_id  TEXT,   -- Claude --resume handle, stored on spawn, used on restart
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);

CREATE TABLE edges (
  from_id  TEXT NOT NULL REFERENCES tasks(id),
  to_id    TEXT NOT NULL REFERENCES tasks(id),
  PRIMARY KEY (from_id, to_id)
);

-- Append-only. Never update or delete rows. Every poe: event lands here.
CREATE TABLE event_log (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id     TEXT    NOT NULL REFERENCES tasks(id),
  event_type  TEXT    NOT NULL,  -- 'poe:brief' | 'poe:step' | 'poe:decision' | 'poe:artifact' | 'poe:knowledge' | 'poe:done' | ...
  payload     TEXT    NOT NULL,  -- full original JSON line
  created_at  INTEGER NOT NULL
);

CREATE TABLE decisions (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id     TEXT    NOT NULL REFERENCES tasks(id),
  question    TEXT    NOT NULL,
  options     TEXT,              -- JSON array of strings, null if agent provided no options
  resolution  TEXT,              -- null until human resolves
  created_at  INTEGER NOT NULL,
  resolved_at INTEGER
);

CREATE TABLE artifacts (
  id                TEXT    PRIMARY KEY,
  project_id        TEXT    NOT NULL REFERENCES projects(id),
  name              TEXT    NOT NULL,  -- e.g. 'conops.md'
  artifact_type     TEXT    NOT NULL,  -- e.g. 'conops' | 'architecture-constraints' | 'phase-plan'
  path              TEXT    NOT NULL,  -- relative to project dir, e.g. 'docs/conops.md'
  producing_task_id TEXT    REFERENCES tasks(id),
  created_at        INTEGER NOT NULL,
  updated_at        INTEGER NOT NULL
);

CREATE TABLE knowledge (
  id             TEXT    PRIMARY KEY,
  project_id     TEXT    NOT NULL REFERENCES projects(id),
  key            TEXT    NOT NULL,   -- human-readable slug, unique per project
  content        TEXT    NOT NULL,
  source_task_id TEXT    REFERENCES tasks(id),
  supersedes_id  TEXT    REFERENCES knowledge(id),
  created_at     INTEGER NOT NULL
);
```

---

## 2. poe: Event Wire Format

Agents write structured events to **stdout**. The event ingester scans each stdout line. A line is a poe: event if and only if it parses as valid JSON and contains a `"poe"` key. All other lines are PTY output — captured to a per-task log file, not processed.

### Format

```
{"poe": "<event-type>", ...fields}
```

One event per line. No multi-line JSON.

### Event catalogue

#### DAG mutations

```jsonc
// Create a task node
{"poe": "task", "id": "<uuid>", "title": "...", "description": "...", "skill": "<skill-id>",
 "type": "task",  // "task" | "bug" | "chore" | "subtask"
 "parent_id": "<task-id>",  // optional — for subtask hierarchy
 "depends_on": ["<task-id>", "..."]}  // optional — creates edges

// Update an existing task
{"poe": "task:update", "id": "<task-id>",
 "title": "...",        // optional
 "description": "...",  // optional
 "skill": "..."}        // optional

// Cancel a task (preserved in history, status → cancelled)
{"poe": "task:cancel", "id": "<task-id>", "reason": "..."}  // reason optional

// Add a dependency edge (from depends on to — to must complete before from)
{"poe": "edge", "from": "<task-id>", "to": "<task-id>"}

// Remove a dependency edge
{"poe": "edge:remove", "from": "<task-id>", "to": "<task-id>"}
```

#### Artifacts and knowledge

```jsonc
// Produce or revise an artifact. Orchestrator writes content to docs/<name>,
// upserts artifacts table, emits Tauri poe://task-update.
{"poe": "artifact", "name": "conops.md", "artifact_type": "conops", "content": "..."}

// Write a knowledge register entry. key must be unique per project;
// supersedes retires the prior entry (not deleted, linked).
{"poe": "knowledge", "key": "target-platform", "content": "...", "supersedes": "<prior-id>"}
// supersedes is optional
```

#### Execution and oversight

```jsonc
// Agent's interpretation of its task — written before execution begins.
// Non-blocking. Appears in activity feed immediately.
{"poe": "brief", "content": "..."}

// Named progress milestone during execution.
{"poe": "step", "name": "Analysing existing artifacts", "detail": "..."}
// detail is optional

// Raise a question for the human queue. Task status → waiting.
// Agent then blocks (reads stdin). Orchestrator delivers resolution via stdin write.
{"poe": "decision", "question": "...", "options": ["Option A", "Option B"]}
// options is optional

// Signal task completion.
{"poe": "done", "summary": "..."}  // summary optional

// Request a peer review. Orchestrator assigns reviewer_skill agent,
// injects result via stdin, unblocks this agent.
{"poe": "review", "reviewer_skill": "senior-engineer", "content": "..."}
```

### Ingester responsibilities per event type

| Event | DAG write | event_log | Tauri emit |
|---|---|---|---|
| `poe:task` | INSERT tasks + edges | yes | `poe://task-update` |
| `poe:task:update` | UPDATE tasks | yes | `poe://task-update` |
| `poe:task:cancel` | UPDATE tasks.status | yes | `poe://task-update` |
| `poe:edge` | INSERT edges | yes | — |
| `poe:edge:remove` | DELETE edges | yes | — |
| `poe:artifact` | UPSERT artifacts, write file | yes | `poe://event` |
| `poe:knowledge` | INSERT knowledge | yes | `poe://event` |
| `poe:brief` | — | yes | `poe://event` |
| `poe:step` | — | yes | `poe://event` |
| `poe:decision` | UPDATE tasks.status=waiting, INSERT decisions | yes | `poe://decision` |
| `poe:review` | spawn reviewer agent | yes | `poe://event` |
| `poe:done` | UPDATE tasks.status=done | yes | `poe://task-update` |

---

## 3. Agent Stdin Bundle (T + S + K)

The input bundle is a **structured markdown document** written to the agent's stdin pipe before execution begins. The agent reads it as its opening context. Sections are separated by `---` with level-1 headings. The format is chosen for readability by an LLM — no binary encoding, no custom delimiters.

### Template

```markdown
# Task

**ID**: {task.id}
**Title**: {task.title}
**Type**: {task.type}
**Skill**: {task.skill}

## Ancestry

- Project: {project.name}
- Phase: {phase.name} ({phase.stage_type})
{if task.parent_id}- Parent: {parent.title} ({parent.id}){end}

## Description

{task.description}

{if task.depends_on}
## Completed Dependencies

{for dep in completed_deps}
- {dep.title} [{dep.status}] — {dep.id}
{end}
{end}

---

# Skill

{contents of skill file, resolved from priority chain}

---

# Artifacts

{for artifact in declared_inputs}
## {artifact.name} ({artifact.artifact_type})

{artifact content, read from docs/{artifact.name}}

---
{end}

# Knowledge Register

{for entry in knowledge where entry.project_id = task.project_id}
## {entry.key}

{entry.content}

---
{end}
```

### Notes

- If no artifacts are declared for this stage type, the Artifacts section is omitted entirely.
- Knowledge register is always injected in full (entries are expected to stay small).
- If a task has no declared skill, the `# Skill` section is omitted and the agent runs with no persona constraint.
- **Decision resolution delivery**: when the human resolves a `poe:decision`, the orchestrator writes the following to the agent's open stdin pipe:

```
---
Human: {resolution text}
```

The agent reads this as a continuation of its context and resumes. The pipe stays open until `poe:done` is received.

### Skill priority chain

The skill file for `{skill-id}` is resolved as follows (first match wins):

1. `{project.path}/.poe/skills/{skill-id}.md`
2. `~/.poe/skills/{skill-id}.md`
3. App bundle `skills/{skill-id}.md`

If no file is found, abort the task with an error — do not spawn the agent with no skill.

---

## 4. Frontend Live Update Mechanism

The frontend does **not poll**. All live state is event-driven via the Tauri event system.

### Startup sequence

1. Frontend calls `invoke("get_project_state", {project_id})` to hydrate initial state from SQLite (projects, phases, tasks, edges, recent event_log, open decisions).
2. Frontend registers three listeners (see below).
3. Rust side begins emitting events as agent stdout arrives.

### Tauri events (Rust → Frontend)

All events use `app_handle.emit_all(event_name, payload)`.

| Event | Payload | Frontend action |
|---|---|---|
| `poe://event` | `{task_id, event_type, payload, created_at}` | Append to activity feed |
| `poe://decision` | `{decision_id, task_id, question, options}` | Add item to queue panel, increment badge |
| `poe://task-update` | `{task_id, status, title?, updated_at}` | Update task status in Phase × Scope matrix and project card |

### Decision resolution (Frontend → Rust)

Human submits a resolution via the queue panel:

```
invoke("resolve_decision", {decision_id, resolution: "..."})
```

Rust handler:
1. Updates `decisions.resolution` and `resolved_at` in SQLite.
2. Writes `\n---\nHuman: {resolution}\n` to the waiting agent's stdin.
3. Updates `tasks.status = 'running'` for the unblocked task.
4. Emits `poe://task-update` and `poe://decision-resolved {decision_id}` (frontend removes from queue).

### Why not polling

Polling introduces latency that makes the activity feed feel dead. poe:brief and poe:step events can arrive seconds apart. Tauri's event system adds no meaningful overhead and is already used in the codebase (XtermPane.tsx uses `listen`).

---

## 5. Phase 3 Tauri Command Surface

New Tauri commands required for Phase 3. All work in `poe2/src-tauri/src/`. Commands shared across beads are noted.

### Phase / Plan Composer (7ct.1, 7ct.3)

```
list_phases(project_id: String) → Vec<Phase>
create_phase(project_id: String, name: String, stage_type: String, position: i32) → Phase
```

**Stage type catalogue** — static constant in Rust + TypeScript (not a SQLite table):
```
conops | guardrails | increment_planning | execution | pm_review
rework | validity_analysis | retrospective | onboarding
```
Each stage type has a static list of consumed and produced artifact types. The plan composer validates connections against this catalogue at the UI layer.

**Schema change**: add `stage_type TEXT` column to `phases` table.

### Matrix / DAG (7ct.1)

```
list_edges(project_id: String) → Vec<Edge>
update_node_sort_order(node_id: String, sort_order: i32) → ()
```

**Schema change**: add `sort_order INTEGER` column to `nodes` table.

### WBS Ancestry + Artifact Content (7ct.2, 7ct.4, 7ct.6)

```
get_node_ancestry(node_id: String) → Vec<Node>      // walks parent_id chain to root
read_artifact_content(artifact_id: String) → String  // reads file from {project}/docs/{path}
```

### Knowledge (7ct.6)

```
update_knowledge(id: String, content: String) → ()
```

### Queue Advisor / Node Chat (7ct.4)

Claude API calls go through a **Rust-side proxy** — API key in Tauri config, never exposed to frontend. Streaming via Tauri `Channel`.

```
advisor_chat_start(project_id: String, context: AdvisorContext) → Channel<AdvisorChunk>
advisor_chat_send(session_id: String, message: String) → ()
advisor_chat_stop(session_id: String) → ()
```

`AdvisorContext` carries: `mode` (queue | node), `decision_id?`, `node_id?`. Rust assembles artifact corpus + knowledge register + relevant task context from SQLite before opening the Claude stream.

Add to `Cargo.toml`: `reqwest` with `features = ["json", "stream"]`.

### Terminal (7ct.5)

```
tmux_create(project_id: String) → ()          // creates session poe-{project_id}
tmux_attach(project_id: String) → PtyHandle   // returns pty fd for xterm
tmux_resize(project_id: String, cols: u16, rows: u16) → ()
tmux_send_keys(project_id: String, keys: String) → ()
tmux_kill(project_id: String) → ()
```

Shell out to system `tmux`. Detect live session before creating.

Add to `package.json`: `@xterm/xterm`, `@xterm/addon-fit`, `@xyflow/react`.

---

## Minor gaps addressed

**Stage runner trigger (.3)**: The orchestrator is event-driven via a Tokio `mpsc` channel (`DagChanged` signal). The event ingester sends a signal on every event that mutates DAG structure or task status (`poe:task`, `poe:task:update`, `poe:task:cancel`, `poe:edge`, `poe:edge:remove`, `poe:done`, plus human gate advances and decision resolutions). On each signal, the orchestrator queries SQLite for tasks where `status = 'pending'` and all `depends_on` tasks have `status = 'done'`, and the running count is below the concurrency limit. Eligible tasks are spawned. No explicit Tauri command triggers execution — the loop fires on DAG changes automatically.

**v1 skill locations (.5)**: Existing skill files are at `poe/src-tauri/skills/`. Port targets:
- `poe/src-tauri/skills/operational-analyst.md` → `skills/operational-analyst.md` in app bundle
- `poe/src-tauri/skills/product-manager.md` → `skills/product-manager.md` in app bundle

**UX-Brief §Pane 3**: Exists and is finalized. Always-visible right panel: decision queue (top) + Queue Advisor chatbot (bottom). Never in a tab.
