# POE — Protocol Specification

**Status**: Draft
**Last updated**: 2026-03-15 (rev 2026-03-15: DAG Service (MCP) added as §6; poe:task/poe:edge removed from poe: protocol; reviewers read DAG directly)

**Artifact classification**: This document serves as both:
- `interface-control.md` — the authoritative Interface Control Document for POE v2. Defines all external interface contracts: the poe: event wire format (§2), the agent stdin bundle format (§3), the frontend update mechanism (§4), the DAG Service MCP tool surface (§6).
- `data-model.md` — the authoritative Database Design Document. Defines the SQLite schema for all internal data structures (§1).

These are standard artifact types in the POE corpus (see Architecture.md §Artifact Corpus). They are injected into every implementation task's input bundle. When in doubt about a wire format, field name, schema question, or MCP tool signature — this document is the answer.

This document specifies the five I/O contracts that Phase 2 is built around. Everything else in the system is implementation detail; divergence here causes cross-component rework.

---

## 1. SQLite Schema

All durable state lives in `{project}/.poe/dag.db`. All timestamps are ISO 8601 text (`TEXT NOT NULL`). WAL mode and foreign keys are always enabled.

```sql
PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;

CREATE TABLE IF NOT EXISTS projects (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    path            TEXT NOT NULL UNIQUE,
    conops_ref      TEXT,           -- path to conops artifact, set after guardrails phase
    active_phase_id TEXT,           -- currently active phase; NULL between gates
    status          TEXT NOT NULL DEFAULT 'active',  -- 'active' | 'paused'. Set to 'paused' by abort_project; back to 'active' on resume. Orchestrator excludes paused projects from dispatch.
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS phases (
    id              TEXT PRIMARY KEY,
    project_id      TEXT NOT NULL REFERENCES projects(id),
    number          INTEGER NOT NULL,   -- 1-based ordering; advance_phase uses number+1
    title           TEXT NOT NULL,
    stage_type      TEXT NOT NULL DEFAULT 'execution', -- 'conops' | 'guardrails' | 'increment_planning' | 'execution' | 'plan_review' | 'rework' | 'validity_analysis' | 'retrospective' | 'onboarding'
    status          TEXT NOT NULL DEFAULT 'pending',   -- 'pending' | 'running' | 'gate' | 'complete'
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    UNIQUE(project_id, number)
);

-- WBS nodes: tasks, reviewers, skill-author nodes, and container nodes (epic/feature).
-- Reviewer nodes: phase_id=NULL, requesting_task_id set, review_id set.
-- Skill-author nodes: phase_id=NULL, skill_id='skill-author'.
CREATE TABLE IF NOT EXISTS nodes (
    id                  TEXT PRIMARY KEY,
    project_id          TEXT NOT NULL REFERENCES projects(id),
    phase_id            TEXT REFERENCES phases(id),
    parent_id           TEXT REFERENCES nodes(id),
    node_type           TEXT NOT NULL,  -- 'task' | 'bug' | 'chore' | 'subtask' | 'plan_review' | 'advisor' | 'epic' | 'feature'
    title               TEXT NOT NULL,
    description         TEXT,
    status              TEXT NOT NULL DEFAULT 'pending',  -- 'pending' | 'running' | 'waiting' | 'resuming' | 'complete' | 'cancelled'
    skill_id            TEXT,
    assignee            TEXT,
    yield_reason        TEXT,   -- 'review' | 'decision' | 'chat' | 'advisor' | NULL. Set when status='waiting'. Used by SF-4 routing without events join.
    session_id          TEXT,   -- Claude --resume handle. Written on init event. Overwritten on each SF-4 continuation.
    requesting_task_id  TEXT REFERENCES nodes(id),  -- reviewer nodes only: back-reference to the requesting task
    review_id           TEXT,   -- reviewer nodes only: the 'id' field from the originating poe:review event. Enables batch-scoped completion tracking.
    retry_count         INTEGER NOT NULL DEFAULT 0, -- reviewer nodes only: watchdog retry counter. Default max: 2.
    sort_order          INTEGER,
    skill_modes         TEXT,   -- JSON array from skill frontmatter, e.g. '["autonomous","interactive"]'. Set at SF-1 dispatch. Read by frontend to enforce the node-scoped conversation mode guard: if skill_modes = '["autonomous"]' only, the "Open conversation" button is disabled (UX-Brief §Node-Scoped Conversation).
    verdict             TEXT,   -- reviewer nodes only: 'APPROVED' | 'APPROVED_WITH_CONDITIONS' | 'BLOCKED' | 'FAILED'. Set by poe:review-outcome or watchdog.
    last_dispatch_at    TEXT,   -- ISO 8601 timestamp set at SF-1 step 0 (the atomic pending→running claim). Used by SF-3 to scope poe:review event collection to the current review round: only events with created_at > last_dispatch_at are included, excluding reviewer nodes from prior rework rounds.
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_nodes_project   ON nodes(project_id);
CREATE INDEX IF NOT EXISTS idx_nodes_phase     ON nodes(phase_id);
CREATE INDEX IF NOT EXISTS idx_nodes_parent    ON nodes(parent_id);
CREATE INDEX IF NOT EXISTS idx_nodes_status    ON nodes(status);
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
CREATE INDEX IF NOT EXISTS idx_edges_to   ON edges(to_id);

CREATE TABLE IF NOT EXISTS artifacts (
    id                  TEXT PRIMARY KEY,
    project_id          TEXT NOT NULL REFERENCES projects(id),
    phase_id            TEXT REFERENCES phases(id),
    artifact_type       TEXT NOT NULL,  -- 'conops' | 'must-nots' | 'phase-plan' | 'plan-review' | ...
    filename            TEXT NOT NULL,  -- e.g. 'must-nots.md'. Relative to project docs/ dir.
    produced_by_stage   TEXT,           -- stage_type that produced this artifact
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL,
    UNIQUE(project_id, filename)
);

CREATE TABLE IF NOT EXISTS knowledge (
    id              TEXT PRIMARY KEY,
    project_id      TEXT NOT NULL REFERENCES projects(id),
    key             TEXT NOT NULL,
    value           TEXT NOT NULL,
    source          TEXT,           -- free-text provenance (task id, agent, or 'human')
    supersedes_id   TEXT REFERENCES knowledge(id),
    created_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_knowledge_project ON knowledge(project_id);
CREATE INDEX IF NOT EXISTS idx_knowledge_key     ON knowledge(project_id, key);

-- Append-only. Never update or delete rows. Every poe: event lands here in full.
CREATE TABLE IF NOT EXISTS events (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id),
    agent_id    TEXT,       -- agents.id, if the event came from a managed agent
    task_id     TEXT,       -- nodes.id the event belongs to
    event_type  TEXT NOT NULL,  -- 'poe:brief' | 'poe:step' | 'poe:decision' | 'poe:artifact' | 'poe:done' | ...
    payload     TEXT NOT NULL,  -- full original JSON line
    created_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_events_project ON events(project_id);
CREATE INDEX IF NOT EXISTS idx_events_task    ON events(task_id);

-- Live agent processes. One row per spawn. status='running' until the process exits.
-- db_count_running_agents queries this table. Ghost-agent recovery sweeps it against AgentMap.
CREATE TABLE IF NOT EXISTS agents (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id),
    skill_id    TEXT NOT NULL,
    task_id     TEXT NOT NULL REFERENCES nodes(id),
    status      TEXT NOT NULL DEFAULT 'running',  -- 'running' | 'done' | 'failed'
    session_id  TEXT,
    started_at  TEXT NOT NULL,
    ended_at    TEXT
);

CREATE INDEX IF NOT EXISTS idx_agents_project ON agents(project_id);
CREATE INDEX IF NOT EXISTS idx_agents_task    ON agents(task_id);
CREATE INDEX IF NOT EXISTS idx_agents_status  ON agents(status);

-- Human decision queue. One row per poe:decision event.
CREATE TABLE IF NOT EXISTS queue_items (
    id          TEXT PRIMARY KEY,
    project_id  TEXT NOT NULL REFERENCES projects(id),
    agent_id    TEXT,
    task_id     TEXT,       -- nodes.id that raised this decision
    question    TEXT NOT NULL,
    options     TEXT,       -- JSON array of candidate options, null if open-ended
    resolution  TEXT,       -- null until human resolves via invoke("resolve_decision")
    created_at  TEXT NOT NULL,
    resolved_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_queue_project    ON queue_items(project_id);
CREATE INDEX IF NOT EXISTS idx_queue_unresolved ON queue_items(project_id, resolved_at);

CREATE TABLE IF NOT EXISTS chat_turns (
    id           TEXT PRIMARY KEY,
    task_id      TEXT NOT NULL REFERENCES nodes(id),
    content      TEXT NOT NULL,   -- agent's message during collaborative session
    response     TEXT,            -- null until human responds via respond_to_chat
    created_at   TEXT NOT NULL,
    responded_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_chat_turns_task ON chat_turns(task_id);

-- Structurally identical to chat_turns but routes to Pane 3 (advisor panel), not Artifact Viewer.
CREATE TABLE IF NOT EXISTS advisor_turns (
    id           TEXT PRIMARY KEY,
    task_id      TEXT NOT NULL REFERENCES nodes(id),
    content      TEXT NOT NULL,
    response     TEXT,
    created_at   TEXT NOT NULL,
    responded_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_advisor_turns_task ON advisor_turns(task_id);
```

---

## 2. poe: Event Wire Format

Agents write structured events to **stdout embedded within the stream-json transport**. In autonomous mode, Claude emits newline-delimited JSON objects; poe: events appear as text within `assistant` message content. The ingester accumulates text from assistant events into a buffer, splits on newlines, and passes complete lines to the poe: parser.

A line extracted from the assistant text is a poe: event if and only if it parses as valid JSON and contains a `"poe"` key. All other extracted lines are agent commentary and are discarded (not stored, not processed).

The event wire format is transport-independent. The JSON payloads below are identical whether the agent runs autonomously (stream-json) or interactively. The transport envelope changes; the event schema does not.

### Format

```
{"poe": "<event-type>", ...fields}
```

One event per line. No multi-line JSON.

### Event catalogue

> **DAG mutations are not poe: events.** Creating tasks, adding edges, updating or cancelling nodes, and removing edges are all performed via DAG Service MCP tool calls (see §6). The `poe:` protocol is control flow and observability only.

#### Artifacts and knowledge

```jsonc
// Declare an artifact the agent has already written to docs/<name> using its own tools
// (e.g. Claude's Write / Edit / Bash). Orchestrator indexes the path in the artifacts table
// and emits poe-artifact-created. No content field — the file is already on disk.
// Downstream agents read the file directly; the orchestrator injects the path, not the content.
{"poe": "artifact", "name": "conops.md", "artifact_type": "conops"}

// Write a knowledge register entry. key must be unique per project;
// supersedes retires the prior entry (not deleted, linked).
{"poe": "knowledge", "key": "target-platform", "content": "...", "supersedes": "<prior-id>"}
// supersedes is optional.
// Field mapping: "content" on the wire → "value" column in the knowledge table. The ingester
// performs this translation on INSERT. Callers emit "content"; the DB stores "value".

// Write a reusable skill pattern to {project}/.poe/skills/<name>.md.
// Closes the self-improvement loop: agents that discover a useful pattern
// can persist it as a project-local skill without manual prompt authoring.
// The agent is responsible for producing well-formed SKILL.md content.
// NOT emitted automatically — agents emit this explicitly when a pattern is worth capturing.
{"poe": "skill", "name": "<skill-id>", "content": "<full SKILL.md markdown>"}
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
// For autonomous agents only — routes to the Decision Queue (Pane 3).
// Agent emits poe:yield immediately after. Orchestrator resumes via --resume once resolved.
{"poe": "decision", "question": "...", "options": [{"id": "opt_a", "label": "Option A", "description": "..."}, {"id": "opt_b", "label": "Option B"}]}
// options is optional. Each option is {id, label, description?}.
// id is the machine-readable resolution value delivered to the agent on --resume.
// label is the human-readable button text shown in the Decision Queue.
// description is optional hover/tooltip text.

// Collaborative turn — agent sends a message or question to the human during a co-authoring session.
// For interactive agents only — routes to the Artifact Viewer chat panel, NOT the Decision Queue.
// Agent emits poe:yield immediately after. Orchestrator resumes via --resume once human responds.
{"poe": "chat", "content": "...", "id": "c1"}
// id is optional for single-turn sessions

// Advisor turn — advisor agent sends a message to the human in the Pane 3 advisor panel.
// Structurally identical to poe:chat but routes to a different surface.
// Use poe:advisor in the advisor skill; use poe:chat in artifact-building skills.
// Agent emits poe:yield immediately after. Orchestrator resumes via --resume once human responds.
{"poe": "advisor", "content": "...", "id": "a1"}
// id is optional for single-turn sessions

// Yield control while awaiting an asynchronous response.
// Emitted after poe:chat, poe:advisor, poe:decision, or poe:review events for this checkpoint.
// task status → waiting. yield_reason is derived by the ingester from the last
// substantive poe: event before this yield:
//   poe:chat     → 'chat'
//   poe:advisor  → 'advisor'
//   poe:decision → 'decision'
//   poe:review   → 'review'
//   none         → NULL
// The reason field on the wire was removed in Phase 2.3.
// Agents MUST emit exactly {"poe": "yield"} with NO reason field.
// Including a reason field (e.g. {"poe":"yield","reason":"decision"}) is a no-op
// — the ingester ignores it — but is misleading and must not appear in agent output.
{"poe": "yield"}

// Signal task completion (all work done — not a yield checkpoint).
{"poe": "done", "summary": "..."}  // summary optional

// Request a peer review. Orchestrator spawns reviewer_skill agent,
// injects result via stdin when complete, unblocks this agent.
// id is required when emitting multiple poe:review events — omit only
// when emitting a single review (single-reviewer path).
// content is a REVIEW DIRECTIVE — task IDs to review and focus area.
// It is NOT a plan transcription. The reviewer reads actual tasks from
// the DAG via DAG Service tools (get_phase_wbs, get_task). See §6.
{"poe": "review", "reviewer_skill": "senior-engineer", "content": "Review tasks t-01..t-27 in the current phase. Focus: skill assignments and task sizing.", "id": "r-eng"}

// Multi-specialist plan review — product-manager emits one per domain:
// {"poe": "review", "reviewer_skill": "senior-engineer",      "id": "r-eng",  "content": "Review all tasks in phase. Focus: completeness, sizing, Must-Not coverage."}
// {"poe": "review", "reviewer_skill": "architecture-analyst", "id": "r-arch", "content": "Review tasks t-05, t-13 in phase. Focus: IIFE encapsulation pattern and animation guard lifecycle. Use get_phase_wbs to read tasks."}
// {"poe": "review", "reviewer_skill": "interface-analyst",    "id": "r-icd",  "content": "Review tasks introducing new API contracts. Use query_tasks to find tasks with artifactType=api."}
// Orchestrator spawns all reviewers in parallel. Each result delivered via stdin:
// ---
// ReviewResult id=r-eng skill=senior-engineer verdict=APPROVED|APPROVED_WITH_CONDITIONS|BLOCKED|FAILED
// {findings text}
// ---
// Verdict values: APPROVED | APPROVED_WITH_CONDITIONS | BLOCKED | FAILED (underscore, no spaces)
// FAILED indicates the reviewer exceeded max retries and was cancelled by the watchdog.
// Agent checks that all expected review IDs are present in the bundle before proceeding.
// Any FAILED verdict should be treated as a signal to escalate via poe:decision.

// Reviewer emits this BEFORE poe:done to record their explicit verdict.
// The orchestrator reads nodes.verdict when building the ReviewResult bundle.
// review_id must match the Review Request **Review ID** field.
// Missing poe:review-outcome defaults to BLOCKED with a poe-ingester-warning emitted.
{"poe": "review-outcome", "verdict": "APPROVED_WITH_CONDITIONS", "review_id": "r-eng"}
```

### Ingester responsibilities per event type

DAG mutations (nodes, edges) are handled by the DAG Service, not the ingester. The ingester processes control-flow and observability events only.

| Event | DB write | events | Tauri emit |
|---|---|---|---|
| `poe:artifact` | UPSERT artifacts (file already on disk — agent wrote it) | yes | `poe-artifact-created` |
| `poe:knowledge` | INSERT knowledge | yes | `poe-knowledge-created` |
| `poe:skill` | write `{project}/.poe/skills/{name}.md` | yes | `poe-event` |
| `poe:brief` | — | yes | `poe-event` |
| `poe:step` | — | yes | `poe-event` |
| `poe:decision` | INSERT queue_items, UPDATE nodes.status=waiting | yes | `poe-decision-queued` |
| `poe:chat` | INSERT chat_turns | yes | `poe-chat-turn` |
| `poe:advisor` | INSERT advisor_turns | yes | `poe-advisor-turn` |
| `poe:review` | log only (orchestrator handles reviewer dispatch post-poe:yield) | yes | `poe-event` |
| `poe:review-outcome` | UPDATE nodes SET verdict=? WHERE id=task_id | yes | `poe-event` |
| `poe:yield` | UPDATE nodes.status=waiting, SET yield_reason, signal orchestrator | yes | `poe-node-updated` + `poe-event` |
| `poe:done` | UPDATE nodes.status=complete, signal orchestrator | yes | `poe-task-done` |

---

## 3. Agent Stdin Bundle (T + S + K)

The input bundle is a **structured markdown document** written to the agent's stdin pipe before execution begins. The agent reads it as its opening context. Sections are separated by `---` with level-1 headings. The format is chosen for readability by an LLM — no binary encoding, no custom delimiters.

### Mode Protocol Injection

Before assembling the bundle, the orchestrator determines the **execution mode** and prepends a standard mode protocol block at the very top. Skills are written to describe domain expertise only — mode protocol (how to behave, what to emit, whether to wait for input) is injected by the orchestrator at bundle assembly time.

**Mode is never selected explicitly by the human in the UI.** It is implicit in the action:

| Who initiates | Mode | Transport |
|---|---|---|
| Orchestrator schedules a ready task | `autonomous` | stream-json + -p |
| Human opens a node-scoped conversation | `interactive` | stream-json, new session |
| Human clicks agent handover in activity feed | PTY resume | PTY + xterm.js |
| Queue Advisor chatbot | `interactive` | Rust-side API proxy |

The skill's `modes:` frontmatter field declares which modes it supports. If a human tries to open a node-scoped conversation with a skill that only supports `autonomous`, the UI blocks it with an explanation rather than starting a broken session.

> **Mode invariant**: The orchestrator ALWAYS injects the autonomous mode block for tasks it schedules. Interactive mode is exclusively for human-initiated sessions (node-scoped conversations or advisor). An agent dispatched by the orchestrator cannot yield with `poe:chat` — it must use `poe:decision` for blockers.
>
> **`respond_to_chat` signal**: The `respond_to_chat` Tauri command writes the response to `chat_turns` and signals the orchestrator via `DagChanged::QueueItemResolved` (the same variant as decision resolution — the orchestrator routes by `node.yield_reason`, not by signal variant). This means the orchestrator's wake-up path is identical for both `poe:decision` and `poe:chat` continuations; only the yield_reason field differentiates them.
>
> **`DagChanged::QueueItemResolved` — `turn_type` field**: The signal carries a `turn_type: String` field with one of three values: `'decision'` | `'chat'` | `'advisor'`. This field allows `resume_waiting_agent()` to route directly to the correct continuation path without performing a DB existence probe on `queue_items`, `chat_turns`, or `advisor_turns` — the type is embedded in the signal itself. Callers must set this field: `resolve_decision` passes `'decision'`, `respond_to_chat` passes `'chat'`, `respond_to_advisor` passes `'advisor'`.

**Autonomous mode block** (prepended for orchestrator-initiated tasks):

```markdown
## Execution Protocol

You are running in autonomous mode — there is no human at the keyboard.

- Begin by emitting `poe:brief` describing your understanding of the task.
- Work from the context in this bundle. Do not ask questions.
- Raise genuine blockers via `poe:decision`, then continue with your best judgement.
- Emit `poe:done` as your final output. The process exits after this.
```

**Interactive mode block** (prepended for human-initiated conversations):

```markdown
## Interactive Mode

You are running in interactive mode — a human is present at the keyboard.

- Use `poe:chat` + `poe:yield` to ask questions and wait for the human's answer.
- The orchestrator will resume your run with the answer as `Human: {response}`.
- Follow the Interactive Mode instructions in your skill prompt exactly.
```

### Skill frontmatter — mode declaration and model selection

Skills declare which modes they support and, optionally, which Claude model to use in YAML frontmatter:

```yaml
---
id: operational-analyst
name: Operational Analyst
description: Elicits and writes the project CONOPS.
modes: [autonomous, interactive]   # supports both
model: claude-opus-4-6             # optional — overrides the default model
---
```

```yaml
---
id: product-manager
name: Product Manager
description: Plans phase work and creates the task DAG.
modes: [autonomous]                # autonomous only — not conversational
---
```

If `modes` is omitted, the orchestrator assumes `[autonomous]` only.

If `model` is omitted, the orchestrator uses the claude binary's configured default.
When `model` is present, the orchestrator passes `--model <value>` to the claude spawn.
Use this to direct high-stakes analysis skills (e.g. `operational-analyst`) at a more capable model.

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
- **{artifact.artifact_type}**: `{project.path}/docs/{artifact.name}` — read this file for context.
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
- **Artifact injection is path-only.** The orchestrator does NOT embed file content in the bundle. It injects the absolute path to each artifact and instructs the agent to read it. The agent reads files directly using its own tools. This keeps bundles small, avoids double-encoding large files, and lets agents like Claude Code use their native file-reading capabilities.
- **Artifact writing is agent-owned.** When an agent produces a document (conops.md, a phase plan, implementation code), it writes the file directly using its tools, then emits `poe:artifact` to declare the path. The orchestrator indexes the path; it does not write the file.
- If a task has no declared skill, the `# Skill` section is omitted and the agent runs with no persona constraint.
- **Decision resolution delivery**: when the human resolves a `poe:decision`, the orchestrator spawns a new stream-json session with `--resume <session_id>` and writes the resolution as the bundle content:

```
---
Human: {resolution text}
```

The agent reads full session history plus this new message and continues. See §5 "Decision resolution via --resume" for the full spawn sequence. The original session's stdin pipe is closed (EOF) immediately after the initial bundle is written — the pipe does not stay open.

### Reviewer stdin bundle (ReviewRequest)

When the orchestrator spawns a reviewer agent in response to a `poe:review` event, it builds a modified stdin bundle. The T section identifies this as a review task so the skill can activate its plan-review mode:

```markdown
# Task

**ID**: {generated-reviewer-task-id}
**Title**: Plan Review — {requesting-task-title}
**Type**: plan_review
**Skill**: {reviewer-skill}

## Review Request

**Requested by**: {requesting-task-id} ({requesting-task-title})
**Review ID**: {id from poe:review event}
**Project ID**: {project.id}

{content from poe:review event — the review directive: which task IDs to review and what to focus on}

> **Read the DAG directly.** Do not rely on the directive for the task content — it is a pointer, not a transcription. Use DAG Service tools (`get_phase_wbs`, `get_task`, `query_tasks`) to read the actual tasks and edges before forming your verdict. See §6 for the full tool surface.

> **Naming convention**: The reviewer MUST write its findings to `docs/review-{review_id}.md` (using its own file-writing tools) and then emit `{"poe":"artifact","name":"review-{review_id}.md","artifact_type":"plan-review"}` where `{review_id}` is the Review ID above. This makes the artifact path deterministic — the orchestrator derives `docs/review-{review_id}.md` directly without an artifacts table query.

---

# Skill

{reviewer skill file}

---

# Artifacts

{same artifact corpus as the requesting agent's bundle}

---

# Knowledge Register

{same knowledge register}
```

Skills detect plan-review mode by the presence of `**Type**: plan_review` in the T section. All other bundle sections are identical to a standard task bundle.

> **Autonomous mode block in reviewer bundles**: The orchestrator prepends the same autonomous mode protocol block (the `## Execution Protocol` block described above) before the `# Task` section in the reviewer stdin bundle — identical to regular task dispatch. Without this block the reviewer agent receives no execution protocol instruction and may behave incorrectly (prompting for input, failing to emit `poe:done`). The reviewer skill file is read-only and must not contain the execution protocol itself; it is always injected by the orchestrator at bundle assembly time.

### Skill-Author Bundle (skill-author tasks only)

When the orchestrator dispatches a task with `skill_id = "skill-author"`, it does **not** assemble a standard T+S+K bundle. Instead it calls `assemble_skill_author_bundle()`, which produces a non-standard bundle described here.

#### When it applies

Any task whose `skill` column equals `"skill-author"` receives this bundle. These tasks are created automatically when the scheduling loop attempts to dispatch a task and cannot resolve the required skill through the priority chain — the orchestrator creates a `skill-author` task as a prerequisite and re-queues the original.

#### Bundle layout

The mode protocol block (autonomous) is prepended identically to a standard bundle. The `# Skill` section contains the `skill-author` skill prompt. The `# Task` / Task Context sections are **replaced** by a single `## Skill Authoring Context` section:

```markdown
## Execution Protocol

You are running in autonomous mode — there is no human at the keyboard.

- Begin by emitting `poe:brief` describing your understanding of the task.
- Work from the context in this bundle. Do not ask questions.
- Raise genuine blockers via `poe:decision`, then continue with your best judgement.
- Emit `poe:done` as your final output. The process exits after this.

---

# Skill

{contents of skill-author skill file}

---

## Skill Authoring Context

**skill_name**: {missing_skill_id}

### Failing tasks that need this skill

{for task in failing_tasks}
- **{task.title}**: {task.description}
{end}

### Existing skills

{comma-separated list of skill names already present in the priority chain}

### Knowledge Register

{for entry in knowledge where entry.project_id = project_id}
## {entry.key}

{entry.content}

---
{end}

### Relevant Artifacts

{for artifact in declared_inputs}
- **{artifact.artifact_type}**: `{project.path}/docs/{artifact.name}` — read this file for context.
{end}
```

Field semantics:

- **`skill_name`**: the exact skill ID that is missing. The agent must use this value as the `name` field in its `poe:skill` output event. Divergence here causes a write-path mismatch.
- **Failing tasks**: one entry per task that was blocked waiting for this skill. The list is the agent's primary signal for what the skill must be capable of — it defines the use-case.
- **Existing skills**: a comma-separated list of skill IDs already registered in the priority chain (project-local + user-level + app bundle). The agent uses this for naming-convention consistency; it should not produce a skill whose ID conflicts with an existing one.
- **Knowledge Register** and **Relevant Artifacts**: identical to the standard bundle — same format, same injection rules.

> **No `# Task` section**: the skill-author bundle omits the standard `# Task` block entirely. The agent's "task" is implicitly defined by the `## Skill Authoring Context`. The `skill-author` skill prompt must not assume a `# Task` section is present.

#### poe:skill output contract

The skill-author agent must emit **exactly two events**, in order:

1. **`poe:skill`** — writes the authored skill to disk:

   ```json
   {"poe": "skill", "name": "<skill_name>", "content": "<complete skill markdown>"}
   ```

   - `name` must exactly match the `skill_name` value from the bundle. Any deviation causes the orchestrator to write the file to the wrong path and leaves the original failing tasks unblocked.
   - `content` must be a complete, runnable skill file including valid YAML frontmatter (with at minimum `id`, `name`, `description`, and `modes` fields). Partial or frontmatter-less content will cause skill-load failure on next dispatch.

2. **`poe:done`**:

   ```json
   {"poe": "done"}
   ```

The orchestrator ingests the `poe:skill` event and writes the content to `{project.path}/.poe/skills/{name}.md` (project-local tier, highest priority in the skill priority chain). After `poe:done` is processed, the orchestrator scheduling loop re-evaluates pending tasks; the originally failing tasks are now unblocked and can be dispatched normally.

> **No intermediate events required**: the skill-author agent should not emit `poe:task`, `poe:artifact`, or `poe:knowledge` events. Its only output contract is `poe:skill` + `poe:done`.

---

### Skill priority chain

The skill file for `{skill-id}` is resolved as follows (first match wins):

1. `{project.path}/.poe/skills/{skill-id}.md`
2. `~/.poe/skills/{skill-id}.md`
3. App bundle `skills/{skill-id}.md`

If no file is found, abort the task with an error — do not spawn the agent with no skill.

> **Skill-load failure**: When the skill file cannot be resolved through the priority chain, the orchestrator sets the node status to `'cancelled'` and logs an error to `events`. The task does **not** retry. Recovery requires human intervention: either add the missing skill file (which makes the task eligible for re-dispatch on the next scheduling loop pass) or cancel and replace the task with a corrected one.

The **mode protocol block** is prepended to the bundle before the `# Skill` section — not inserted into the skill file itself. The skill file is read-only from the orchestrator's perspective; it is never modified at runtime.

---

## 4. Frontend Live Update Mechanism

The frontend does **not poll**. All live state is event-driven via the Tauri event system.

### Startup sequence

1. Frontend calls `invoke("get_project_state", {project_id})` to hydrate initial state from SQLite (projects, phases, nodes, edges, recent events, open queue_items).
2. Frontend registers three listeners (see below).
3. Rust side begins emitting events as agent stdout arrives.

### Tauri events (Rust → Frontend)

All events use `app_handle.emit(event_name, payload)`. Event names use hyphen notation — there is no `poe://` prefix in the implementation.

| Event | Payload | Frontend action |
|---|---|---|
| `poe-event` | `{eventType, projectId, agentId, taskId, payload, summary?}` | Append to activity feed |
| `poe-agent-started` | `{agentId, taskId, projectId, skillId, model}` | Mark task as running in task tree; emitted before process spawns |
| `poe-task-created` | `{id, title, skill, status, ...node fields}` | Add task node to DAG view |
| `poe-node-updated` | `{id, status, title?, skill?, ...node fields}` | Update task status in Phase × Scope matrix |
| `poe-task-done` | `{id, status: "complete", ...node fields}` | Mark task complete in task tree; triggers orchestrator scheduling loop |
| `poe-edge-created` | `{fromId, toId, edgeType}` | Add dependency edge to DAG view |
| `poe-edge-removed` | `{fromId, toId}` | Remove dependency edge from DAG view |
| `poe-artifact-created` | `{id, filename, artifactType, projectId, ...}` | Add artifact to Artifact Viewer; frontend reads content from `{project.path}/docs/{filename}` |
| `poe-knowledge-created` | `{id, key, value, projectId, ...}` | Add entry to Knowledge Register panel |
| `poe-decision-queued` | `{id, taskId, question, options, resolvedAt}` | Add item to queue panel, increment badge |
| `poe-decision-resolved` | `{itemId, projectId, taskId}` | Remove item from queue panel, decrement badge |
| `poe-chat-turn` | `{turnId, taskId, content}` | Display agent message in Artifact Viewer chat panel |
| `poe-chat-responded` | `{turnId, projectId, taskId}` | Frontend scrolls, awaits next agent turn |
| `poe-advisor-turn` | `{turnId, taskId, content}` | Display advisor message in Pane 3 advisor panel |
| `poe-advisor-responded` | `{turnId, projectId, taskId}` | Frontend scrolls, awaits next advisor turn |
| `poe-agent-stream` | `{agentId, taskId, projectId, event}` | Raw stream-json event from agent stdout — used by DebugPanel for live output display |
| `poe-agent-exited` | `{agentId, taskId, projectId, success}` | Agent process exited; `success=false` if poe:done was never received |
| `poe-phase-update` | `{phaseId, status, projectId}` | Update phase lifecycle state in Phase × Scope Matrix header |
| `poe-ingester-warning` | `{taskId, agentId, eventType, error}` | A structured poe: event (`poe:task`, `poe:edge`, `poe:decision`, `poe:skill`) failed to process; the originating agent task was unaffected. Frontend may surface in activity feed. |
| `poe-project-opened` | `{projectId, isNew: bool}` | Emitted after `open_project` completes DB initialisation and recovery. `isNew=true` for first-time opens; `false` for existing projects. Frontend calls `invoke("get_project_state", {project_id})` in response to hydrate nodes, phases, edges, artifacts, and open queue items. |

### Decision resolution (Frontend → Rust)

Human submits a resolution via the queue panel:

```
invoke("resolve_decision", {decision_id, resolution: "..."})
```

Rust handler:
1. Updates `queue_items.resolution` and `resolved_at` in SQLite.
2. Signals the orchestrator (`DagChanged`).
3. Emits `poe-decision-resolved {itemId, projectId, taskId}` (frontend removes from queue).

The orchestrator wakes on `DagChanged`, identifies the waiting task with `yield_reason='decision'`, confirms all decisions are resolved, and triggers SF-4: Agent Continuation. The continuation bundle is `Human: {resolution text}` — see Flows.md §3.2 for the full sequence.

### Chat response (Frontend → Rust)

Human submits a response in the Artifact Viewer chat panel:

```
invoke("respond_to_chat", {project_id, turn_id, response: "..."})
```

Rust handler:
1. Updates `chat_turns.response` and `responded_at` in SQLite.
2. Signals the orchestrator (`DagChanged::QueueItemResolved`).
3. Emits `poe-chat-responded {turnId, projectId, taskId}` (frontend scrolls, awaits next agent turn).

The orchestrator wakes on `DagChanged`, identifies the waiting task with `yield_reason='chat'`, confirms the turn has a response, and triggers SF-4. The continuation bundle is `Human: {response text}` — identical format to decision resolution. See Flows.md §3.8.

### Advisor response (Frontend → Rust)

Human submits a response in the Pane 3 advisor panel:

```
invoke("respond_to_advisor", {project_id, turn_id, response: "..."})
```

Rust handler:
1. Updates `advisor_turns.response` and `responded_at` in SQLite.
2. Signals the orchestrator (`DagChanged::QueueItemResolved`).
3. Emits `poe-advisor-responded {turnId, projectId, taskId}` (frontend scrolls, awaits next advisor turn).

The orchestrator wakes on `DagChanged`, identifies the waiting task with `yield_reason='advisor'`, confirms the turn has a response, and triggers SF-4. The continuation bundle is `Human: {response text}` — identical format to chat and decision resolution. See Flows.md §3.9.

### Plan Composer — Phase activation (Frontend → Rust)

After composing a plan in the Plan Composer and clicking "Run":

```
// Step 1: create all phase records (called once per stage node, in topological order)
invoke("create_phase", {project_id, name, stage_type, number})
// → Returns Phase. All phases created with status='pending'.

// Step 2: activate the first phase (triggers orchestrator to begin execution)
invoke("activate_phase", {project_id, phase_id: first_phase_id})
```

> **Note**: `project_id` is required — the handler uses it to look up the phase's project for bootstrap and event emission.

`activate_phase` Rust handler:
1. `UPDATE phases SET status='running' WHERE id = phase_id` (also runs `maybe_bootstrap_phase`).
2. Signals the orchestrator (`DagChanged::DagStructureChanged`).
3. Emits `poe-phase-update {phaseId, status: 'running', projectId}`.

The orchestrator wakes, finds pending tasks in the now-active phase with no unmet dependencies, and dispatches via SF-1. See Flows.md SF-5.

### Phase Gate Commands (Frontend → Rust)

These three commands handle the three gate outcomes described in Flows.md §3.7. All require `project_id` so the handler can emit `poe-phase-update` events.

```
invoke("advance_phase", {project_id: String, phase_id: String})
```
Rust handler:
1. `UPDATE phases SET status='complete' WHERE id = phase_id`
2. Finds the next phase (lowest `number` > current with `status='pending'`). Updates it: `status='running'`.
3. Runs `maybe_bootstrap_phase` on the next phase (creates default task if none exist — see SF-5 §2b).
4. Signals orchestrator (`DagChanged::DagStructureChanged`).
5. Emits `poe-phase-update {phaseId: phase_id, status: 'complete', projectId}` and `poe-phase-update {phaseId: next_phase_id, status: 'running', projectId}`.

```
invoke("revise_phase", {project_id: String, phase_id: String, task_ids: Vec<String>})
```
Rust handler:
1. `UPDATE nodes SET status='pending' WHERE id IN (task_ids)` — resets only the specified tasks.
2. `UPDATE phases SET status='running' WHERE id = phase_id`
3. Signals orchestrator (`DagChanged`).
4. Emits `poe-phase-update {phaseId, status: 'running', projectId}` + `poe-node-updated` for each reset task.

```
invoke("rerun_phase", {project_id: String, phase_id: String})
```
Rust handler:
1. `UPDATE nodes SET status='pending' WHERE phase_id = phase_id AND status = 'done'` — resets all done tasks.
2. `UPDATE phases SET status='running' WHERE id = phase_id`
3. Signals orchestrator (`DagChanged`).
4. Emits `poe-phase-update {phaseId, status: 'running', projectId}` + `poe-node-updated` for each reset task.

**Note**: `revise_phase` and `rerun_phase` do not delete artifacts. When the reset tasks re-run they produce new artifact versions (UPSERT). Prior versions remain accessible via artifact history.

### Phase Pause and Abort (Frontend → Rust)

```
invoke("pause_stage", {project_id: String, phase_id: String})
invoke("resume_stage", {project_id: String, phase_id: String})
invoke("abort_project", {project_id: String})
invoke("resume_project", {project_id: String})
```

See Flows.md §3.5 for the full sequences. `pause_stage` and `abort_project` send SIGTERM to running agents and reset their nodes to `pending`. The orchestrator does NOT emit `DagChanged` after a pause — no further dispatch until the human resumes. `pause_stage` sets `phases.status='paused'`; `abort_project` sets `projects.status='paused'`.

**Schema note**: `phases.status` valid values: `'pending' | 'running' | 'gate' | 'complete' | 'paused'`. `projects.status` valid values: `'active' | 'paused'`.

### User-level knowledge promotion

`promote_knowledge(id: String)` Rust handler:
1. Reads the knowledge entry from SQLite by id.
2. Writes `~/.poe/knowledge/{key}.md` — one file per entry, content is the raw `content` field.
3. Logs the promotion action to `events` with `event_type = 'knowledge:promoted'`.
4. Sets a `promoted = 1` flag on the knowledge row (already stubbed).

**Bundle assembly merge**: at T+S+K assembly time, the orchestrator merges user-level knowledge from `~/.poe/knowledge/*.md` with the project's SQLite knowledge entries. Project-level entries take precedence on key collision (same `key` filename stem). User-level entries are injected as additional `## {key}` sections in the `# Knowledge Register` block, after project entries.

---

### Why not polling

Polling introduces latency that makes the activity feed feel dead. poe:brief and poe:step events can arrive seconds apart. Tauri's event system adds no meaningful overhead and is already used in the codebase (XtermPane.tsx uses `listen`).

---

## 5. Agent Spawn Model

This section is the authoritative reference for how Claude agents are spawned. Every implementer touching `agent_lifecycle` must read this. Inconsistency here breaks session resume, decision continuation, and human handover.

---

### Primary transport — stream-json (autonomous, programmatic)

Every orchestrated agent runs via:

```
claude --output-format stream-json --verbose -p --dangerously-skip-permissions \
       --mcp-config {project.path}/.poe/mcp-config.json
```

The `--mcp-config` flag injects the DAG Service MCP server into the agent's tool set. The config file is written by the orchestrator to `{project.path}/.poe/mcp-config.json` before the first agent is spawned. See §6 for the config file format and DAG Service tool surface.

`--verbose` causes Claude to emit additional stream-json metadata objects (token usage, stop reasons, timing). The ingester ignores these extra fields — they do not affect event processing. The flag is included because it enables monitoring and cost tracking without any code change; the overhead is negligible.

Stdin receives the T+S+K input bundle; stdin is closed (EOF) immediately after writing. Claude processes the bundle, emits a stream of JSON objects to stdout, and exits cleanly.

```
spawn: claude --output-format stream-json --verbose -p --dangerously-skip-permissions \
              --mcp-config {project.path}/.poe/mcp-config.json
  → write T+S+K bundle to stdin, then close stdin (EOF)
  → read stdout: newline-delimited JSON objects
  → extract session_id from {"type":"system","subtype":"init","session_id":"..."}
  → accumulate text from assistant events into text_buf
  → split text_buf on '\n' → parse_poe_event on each complete line
  → flush remaining text_buf after {"type":"result",...} (process exit)
```

**Why `-p` now — reversal from the prior model:**

The prior model used interactive PTY mode (no `-p`) so that stdin could stay open for mid-task decision delivery. This produced three intractable problems in practice:

1. ANSI escape codes were inserted between characters by the Claude TUI, making output impossible to parse reliably.
2. Long poe: JSON lines were fragmented by PTY line-discipline — 300-character artifact payloads arrived split across multiple lines.
3. The TUI-ready heuristic (detecting `⏵` U+23F5 in PTY output) was fragile and caused deadlocks on slow machines.

The stream-json transport eliminates all three. `poe:decision` and `poe:review` continuation uses `--resume` (see below) — not an open stdin pipe.

---

### Session ID capture

Session ID is extracted from the **first** JSON event on stdout:

```json
{"type":"system","subtype":"init","session_id":"<uuid>",...}
```

Store it in `nodes.session_id` immediately. This ID is used for:

- App restart recovery (`--resume <session_id>` to continue interrupted tasks)
- Human xterm.js handover (see below)
- Decision resolution continuations
- Review result injection

**No banner text scanning.** There is no PTY startup banner in stream-json mode.

---

### poe: event extraction

poe: events are embedded in assistant text content. The ingester uses a text buffer to accumulate text across potentially split `content_block_delta` chunks:

```
{"type":"assistant","message":{"content":[{"type":"text","text":"..."}]}}
  → extract text → push to text_buf
  → while '\n' in text_buf: extract line → parse_poe_event
{"type":"content_block_delta","delta":{"type":"text_delta","text":"..."}}
  → same extraction path
{"type":"result",...}
  → flush text_buf tail (last event may have no trailing newline)
```

`parse_poe_event` requires no ANSI stripping — stream-json output contains no escape codes.

---

### Completion signal

`{"type":"result","subtype":"success",...}` on stdout signals process exit. The process exits cleanly — no SIGTERM or SIGKILL required for normal completion.

`poe:done` in the assistant text is the **task completion signal** — the same semantic authority as before, now unambiguous because there are no ANSI codes or line fragmentation to corrupt it.

If the process exits and `poe:done` was never received:

```
process exits (result event received)
  → check nodes.status
  → Complete  → success (poe:done was received and processed), notify orchestrator
  → Running   → poe:done was never emitted; re-queue task to Pending
```

---

### Decision resolution via --resume

When an agent emits `poe:decision` followed by `poe:yield`, the orchestrator marks the task `waiting` and holds until the human resolves via `invoke("resolve_decision")`. On resolution, the orchestrator triggers SF-4 with the continuation bundle `Human: {resolution text}`. See Flows.md §3.2 for the full sequence.

The continuation bundle format:

```
---
Human: {resolution text}
```

`--resume` with `--output-format stream-json` is a proven combination (validated by `stream_json_integration.rs::resume_continuation_captures_new_session_and_emits_done`).

---

### Review injection via --resume

The ReviewResult stdin bundle format:

```
---
ReviewResult id={id} skill={skill} verdict={APPROVED|APPROVED_WITH_CONDITIONS|BLOCKED|FAILED}
{findings text}
---
```

Verdict values are specified in §2. For the complete orchestrator-level sequence — when the resume is triggered, how multiple reviewer results are batched, and session_id handling — see `doc-POE/Flows.md §3.1`.

---

### Human handover — PTY + xterm.js

When a human wants to directly interact with an agent's session (check-in, unblock, or explore), the handover flow is:

```
1. Look up session_id from nodes.session_id in SQLite
2. Spawn: claude --resume <session_id> --dangerously-skip-permissions (via PTY)
   — cwd MUST match the project directory used for the original stream-json session (see constraint below)
3. Bridge PTY output (raw bytes) → WebSocket → xterm.js in frontend
4. Bridge xterm.js keyboard input → WebSocket → PTY stdin
5. Handle resize: WS message {type:"resize", cols, rows} → PTY master.resize(PtySize)
6. On browser tab close: Ctrl-C → drop PTY master → SIGTERM → poll 5s → SIGKILL
```

xterm.js renders ANSI codes natively — **no stripping required on this path**. Raw bytes flow directly from PTY to browser. This is the `session_handoff_harness.rs` / `decision_handoff_harness.rs` pattern.

**Critical cwd constraint**: Claude scopes sessions to the working directory used at spawn time (by project directory hash). The PTY spawn and all `--resume` continuations **must use the same cwd** as the original stream-json session that created the session_id. A mismatch produces `"No conversation found with session ID: ..."`. In production this is natural — the project cwd is always `project.path` for every spawn. In tests, always pass the same path to every step in a multi-session flow.

The PTY handover is for human use only. The orchestrator does not parse PTY output on this path.

---

### Skill design constraint

Because orchestrated agents run with no human at the keyboard, **skills must be designed for autonomous single-pass execution**:

- Do not prompt the user with questions and wait for typed answers — there is no keyboard on the programmatic path.
- Raise genuine blockers via `poe:decision`, then emit `poe:done`. The orchestrator handles continuation via `--resume`.
- Emit `poe:done` as the final event. The process exits after the result event.
- A skill designed for iterative conversation will produce skeleton output when run autonomously. Rewrite it to reason through the task from the injected context alone.

See §3 for the **mode protocol injection** pattern, which allows skills to be invoked in both autonomous and interactive modes without requiring separate skill files.

---

## 6. DAG Service (MCP)

The DAG Service is an MCP server embedded in POE and injected into every agent's tool set via `--mcp-config` at spawn time. It is the primary interface for all reads and writes to the project's task graph, knowledge register, and artifact index. The `poe:` protocol handles control flow and observability only — it carries no data.

### Architecture

```
Agent process (claude --mcp-config .poe/mcp-config.json)
  └── MCP tool call  (e.g. create_task)
        └── DAG Service process  (poe-dag-mcp, spawned by POE)
              ├── Commits to SQLite (dag.db, WAL mode)
              ├── Notifies Orchestrator  (Unix socket → DagChanged signal)
              └── Emits Tauri event to Frontend  (via main process relay)
```

**Deployment**: The DAG Service runs as a child process (`poe-dag-mcp`) spawned by the POE orchestrator before the first agent is dispatched on each project. It communicates with the orchestrator over a project-scoped Unix socket at `{project.path}/.poe/dag.sock`. The agent-facing transport is MCP stdio (the `poe-dag-mcp` binary is the server process; Claude connects to it via the `command` entry in `mcp-config.json`).

**Why a separate process**: The DAG Service must be reachable by Claude agents, which use MCP's stdio transport (subprocess model). Running it as a separate process is the natural fit — POE spawns it, wires its stdin/stdout for MCP, and communicates with it over the local socket for orchestrator notifications and Tauri event relay.

### mcp-config.json

The orchestrator writes this file to `{project.path}/.poe/mcp-config.json` before the first agent spawn on a project. Every agent on the project uses the same config file.

```json
{
  "mcpServers": {
    "poe": {
      "command": "{app.resource_dir}/poe-dag-mcp",
      "args": [
        "--project-id", "{project.id}",
        "--db",         "{project.path}/.poe/dag.db",
        "--socket",     "{project.path}/.poe/dag.sock"
      ],
      "env": {}
    }
  }
}
```

Field notes:
- `{app.resource_dir}` — the Tauri resource directory where `poe-dag-mcp` is bundled. The orchestrator resolves this at config-write time using `tauri::AppHandle::path().resource_dir()`.
- `--project-id` — scopes all DB writes to the correct project row. The DAG Service rejects tool calls that reference nodes belonging to a different project.
- `--socket` — the Unix socket path for back-channel notification to the orchestrator.
- The `env` block is empty by default; reserved for future use (e.g. injecting API tokens for test-runner integrations).

Claude creates a new `poe-dag-mcp` subprocess per agent spawn (MCP stdio model). Each subprocess connects to the shared `dag.sock` on startup. The DB is WAL-mode SQLite — concurrent reads and serialised writes are safe across multiple short-lived subprocess clients.

### Tool Surface

Agents call tools using the server name prefix `poe__<tool_name>` (double-underscore, MCP naming convention). Tool names below are shown without the prefix for readability.

---

#### Task CRUD

**`create_task`** — Create a new node in the WBS.

```jsonc
// Request
{
  "title":       "string",         // required
  "type":        "epic|feature|task|bug|chore|subtask",  // required
  "skill":       "string",         // skill_id; optional (omit for container nodes)
  "parent_id":   "string",         // nodes.id of parent epic or feature; optional
  "description": "string",         // optional
  "sort_order":  42                // optional; controls ordering within parent
}

// Response
{
  "id":          "string",         // generated node ID (e.g. "t-abc123")
  "title":       "string",
  "type":        "string",
  "skill":       "string|null",
  "parent_id":   "string|null",
  "status":      "pending",
  "phase_id":    "string",         // current active phase, set automatically
  "project_id":  "string",
  "created_at":  "ISO 8601"
}
```

Notes:
- `phase_id` is set by the DAG Service to the currently active phase for the project. Agents do not specify it.
- `project_id` is derived from the `--project-id` arg at startup. Agents do not specify it.
- `type = "epic"` or `"feature"` creates a container node with no `skill`. Tasks, bugs, chores, and subtasks have a `skill`.

---

**`get_task`** — Read a single node with full WBS ancestry.

```jsonc
// Request
{ "id": "string" }

// Response
{
  "id":           "string",
  "title":        "string",
  "type":         "string",
  "skill":        "string|null",
  "status":       "string",
  "description":  "string|null",
  "parent_id":    "string|null",
  "phase_id":     "string",
  "project_id":   "string",
  "sort_order":   "integer|null",
  "ancestry": [   // parent chain from root to direct parent, inclusive
    { "id": "string", "title": "string", "type": "string" }
  ],
  "created_at":   "ISO 8601",
  "updated_at":   "ISO 8601"
}
```

---

**`get_phase_wbs`** — Read the complete task graph for a phase.

```jsonc
// Request
{ "phase_id": "string" }  // pass the phase ID from the Task bundle

// Response
{
  "phase_id":  "string",
  "nodes": [
    {
      "id":          "string",
      "title":       "string",
      "type":        "string",
      "skill":       "string|null",
      "status":      "string",
      "parent_id":   "string|null",
      "sort_order":  "integer|null",
      "description": "string|null"
    }
  ],
  "edges": [
    { "from_id": "string", "to_id": "string", "edge_type": "depends_on" }
  ]
}
```

Notes:
- Returns all non-cancelled nodes for the phase, in topological order (epics → features → tasks).
- `edges` contains all finish-to-start dependency edges between nodes in this phase.
- Reviewers call this tool first to read the full WBS before forming a verdict.

---

**`query_tasks`** — Filtered query over nodes.

```jsonc
// Request (all fields optional — omit to return all phase nodes)
{
  "phase_id":  "string",
  "parent_id": "string",
  "skill":     "string",
  "status":    "pending|running|waiting|complete|cancelled",
  "type":      "epic|feature|task|bug|chore|subtask"
}

// Response
{
  "nodes": [ /* same shape as get_phase_wbs nodes */ ]
}
```

---

**`update_task`** — Update mutable fields on an existing node.

```jsonc
// Request
{
  "id":          "string",         // required
  "title":       "string",         // optional
  "skill":       "string",         // optional
  "description": "string",         // optional
  "sort_order":  42                // optional
}

// Response
{ /* updated node — same shape as get_task response */ }
```

Notes:
- `status` is NOT writable via this tool. Status transitions are managed by the orchestrator.
- `type` and `phase_id` are immutable after creation.
- Emits `poe-node-updated` Tauri event after commit.

---

**`cancel_task`** — Mark a node as cancelled. Preserved in history; never hard-deleted.

```jsonc
// Request
{
  "id":     "string",   // required
  "reason": "string"    // optional; stored in description field
}

// Response
{ "id": "string", "status": "cancelled" }
```

Notes:
- Cascades to dependent nodes: any node with `depends_on` edges pointing to this node is also cancelled, recursively.
- Emits `poe-node-updated` Tauri event for each cancelled node.

---

#### Dependency Edges

**`add_edge`** — Add a finish-to-start dependency. `to_id` cannot start until `from_id` is complete.

```jsonc
// Request
{
  "from_id":   "string",        // the prerequisite node
  "to_id":     "string",        // the node that depends on from_id
  "edge_type": "depends_on"     // currently the only supported type; optional, defaults to "depends_on"
}

// Response
{ "id": "string", "from_id": "string", "to_id": "string", "edge_type": "depends_on" }
```

Notes:
- Rejects cycles: if adding this edge would create a cycle, returns an error.
- Emits `poe-edge-created` Tauri event after commit.

---

**`remove_edge`** — Remove a dependency edge.

```jsonc
// Request
{ "from_id": "string", "to_id": "string" }

// Response
{ "removed": true }
```

Emits `poe-edge-removed` Tauri event after commit.

---

#### Knowledge and Artifacts

**`write_knowledge`** — Write an entry to the project knowledge register.

```jsonc
// Request
{
  "key":          "string",      // unique per project; used as lookup key
  "value":        "string",      // the knowledge content
  "source":       "string",      // free-text provenance (task ID, agent name, or 'human'); optional
  "supersedes_id": "string"      // ID of a prior entry this one replaces; optional
}

// Response
{ "id": "string", "key": "string", "created_at": "ISO 8601" }
```

Notes:
- Equivalent to emitting `poe:knowledge` — both write to the same `knowledge` table. Agents should use whichever fits their pattern; the DAG Service tool is preferred for programmatic writes within a task, while `poe:knowledge` is acceptable for end-of-task summaries.
- Emits `poe-knowledge-created` Tauri event after commit.

---

**`query_knowledge`** — Read knowledge entries.

```jsonc
// Request
{ "key": "string" }    // optional; omit to return all project entries

// Response
{
  "entries": [
    { "id": "string", "key": "string", "value": "string", "source": "string|null", "created_at": "ISO 8601" }
  ]
}
```

---

**`register_artifact`** — Index an artifact that the agent has already written to disk.

```jsonc
// Request
{
  "filename":      "string",   // relative to {project.path}/docs/  e.g. "conops.md"
  "artifact_type": "string"    // e.g. "conops", "must-nots", "phase-plan", "plan-review"
}

// Response
{ "id": "string", "filename": "string", "artifact_type": "string", "created_at": "ISO 8601" }
```

Notes:
- Equivalent to emitting `poe:artifact`. Both write to the same `artifacts` table. Agents may use either; `poe:artifact` is idiomatic for file-writing tasks.
- The file must already exist at `{project.path}/docs/{filename}` before this call. The DAG Service does NOT write the file.
- Emits `poe-artifact-created` Tauri event after indexing.

---

**`get_artifact`** — Look up an artifact's path and metadata.

```jsonc
// Request
{
  "artifact_type": "string",   // optional; returns most-recent artifact of this type
  "filename":      "string"    // optional; exact filename match. One of artifact_type or filename required.
}

// Response
{
  "id":            "string",
  "filename":      "string",
  "artifact_type": "string",
  "path":          "string",   // absolute path: {project.path}/docs/{filename}
  "created_at":    "ISO 8601",
  "updated_at":    "ISO 8601"
}
```

---

#### Execution Support

**`run_tests`** — Run the project's test suite and return structured results.

```jsonc
// Request
{}   // no parameters

// Response
{
  "passed":  42,
  "failed":  3,
  "skipped": 1,
  "output":  "string"   // raw test output, truncated to 8 KB
}
```

Notes:
- Runs the test command configured for the project (default: `cargo test`; overrideable via project knowledge entry `test-command`).
- Returns immediately with results; does not stream.

---

**`git_status`** — Read current git state.

```jsonc
// Request
{}   // no parameters

// Response
{
  "branch":           "string",
  "staged":           ["filename", ...],
  "unstaged":         ["filename", ...],
  "untracked":        ["filename", ...],
  "recent_commits":   [
    { "hash": "string", "message": "string", "author": "string", "date": "ISO 8601" }
  ]   // most recent 5 commits
}
```

---

### Orchestrator Notification

After every successful mutation (create, update, cancel, add_edge, remove_edge, write_knowledge, register_artifact), the DAG Service sends a notification to the orchestrator via the Unix socket at `{project.path}/.poe/dag.sock`. The message is a single JSON line:

```json
{ "type": "DagChanged", "project_id": "string", "node_id": "string|null" }
```

The orchestrator reads from `dag.sock` on a background Tokio task. On receipt of `DagChanged`, it runs the scheduling loop for the project: queries for pending tasks with all dependencies complete, and dispatches up to the concurrency limit. This is the same loop triggered by `poe:done` and human gate advances.

`node_id` is populated for node mutations (create, update, cancel) and null for edge and knowledge writes. The orchestrator uses it to emit the appropriate Tauri event (see below) before running the scheduling loop.

### Tauri Events Emitted by DAG Service

These events are emitted by the main POE process on behalf of the DAG Service (relayed from the socket notification). They are listed here separately from §4 because their source is agent tool calls, not the poe: ingester.

| Tool call | Tauri event | Payload |
|---|---|---|
| `create_task` | `poe-task-created` | `{id, title, type, skill, status, phase_id, project_id, parent_id, sort_order}` |
| `update_task` | `poe-node-updated` | `{id, title?, skill?, description?, sort_order?, updated_at}` |
| `cancel_task` | `poe-node-updated` | `{id, status: "cancelled"}` (one event per affected node) |
| `add_edge` | `poe-edge-created` | `{fromId, toId, edgeType}` |
| `remove_edge` | `poe-edge-removed` | `{fromId, toId}` |
| `write_knowledge` | `poe-knowledge-created` | `{id, key, value, projectId}` |
| `register_artifact` | `poe-artifact-created` | `{id, filename, artifactType, projectId}` |

Read-only tool calls (`get_task`, `get_phase_wbs`, `query_tasks`, `query_knowledge`, `get_artifact`, `run_tests`, `git_status`) do not emit Tauri events.

### Error Handling

All tool calls return a structured error on failure. The MCP error response body:

```json
{
  "error": {
    "code":    "string",   // machine-readable error code (see below)
    "message": "string"    // human-readable description
  }
}
```

Standard error codes:

| Code | Meaning |
|---|---|
| `NOT_FOUND` | The requested node, edge, or artifact does not exist |
| `WRONG_PROJECT` | The referenced ID belongs to a different project |
| `CYCLE_DETECTED` | `add_edge` would create a dependency cycle |
| `IMMUTABLE_FIELD` | Attempt to modify `type`, `phase_id`, or `project_id` via `update_task` |
| `DB_ERROR` | SQLite write failed (disk full, locked, etc.) |
| `NOT_READY` | The DAG Service subprocess has not yet connected to `dag.sock` |

On `DB_ERROR` or `NOT_READY`, the agent should emit `poe:decision` to surface the failure rather than silently continuing.

---

## 7. Tauri Command Surface (Full Catalogue)

All Tauri commands exposed by the backend. Authoritative interface spec — not phase-specific planning notes. All implementations live in `poe2/src-tauri/src/`.

### Phase / Plan Composer

```
list_phases(project_id: String) → Vec<Phase>
create_phase(project_id: String, name: String, stage_type: String, number: i64) → Phase
```

`number` maps to the existing `phases.number INTEGER` column (UNIQUE per project — serves as ordered position). Do **not** add a `position` column.

**Stage type catalogue** — static constant in Rust + TypeScript (not a SQLite table):
```
conops | guardrails | increment_planning | execution | plan_review
rework | validity_analysis | retrospective | onboarding
```
Each stage type has a static list of consumed and produced artifact types. The plan composer validates connections against this catalogue at the UI layer.

**Schema change**: add `stage_type TEXT` column to `phases` table.

### Matrix / DAG

```
list_edges(project_id: String) → Vec<Edge>
update_node_sort_order(node_id: String, sort_order: i32) → ()
```

**Schema change**: add `sort_order INTEGER` column to `nodes` table.

### WBS Ancestry + Artifact Content

```
get_node_ancestry(node_id: String) → Vec<Node>                          // walks parent_id chain to root
read_artifact_content(artifact_id: String, project_id: String) → String // reads file from disk
```

`read_artifact_content` path formula: look up `artifact.filename` from the artifacts table, look up `project.path` from the project registry by `project_id`. Path resolution uses a two-step fallback:

1. Try `{project.path}/docs/{artifact.filename}` — the canonical location for declared artifacts.
2. If not found, fall back to `{project.path}/{artifact.filename}` — covers artifacts written directly to the project root.
3. If neither path exists, return an error.

The `Artifact` struct has `filename: String`; there is no stored `path` field.

### Knowledge

```
update_knowledge(id: String, content: String) → ()
```

### Queue Advisor / Node Chat

The Queue Advisor uses an agent-spawn approach. Conversations are persisted to SQLite and streamed to the frontend via `poe:advisor` Tauri events.

**Tauri commands:**
```
start_advisor_session(task_id: String) → ()
respond_to_advisor(task_id: String, content: String) → ()
get_advisor_turns(task_id: String) → Vec<AdvisorTurn>
```

**Wire events**: The backend emits `poe:advisor` Tauri events to the frontend as the agent streams its response. The frontend listens for these events to update the conversation panel in real time.

**Storage**: All conversation turns (both human and advisor) are persisted in the `advisor_turns` SQLite table, keyed by `task_id`. `get_advisor_turns` returns the full conversation history for a task.

### Terminal

Two distinct terminal surfaces — they are not the same feature:

**1. Project terminal (tmux)** — persistent background shell for the project directory. Used for code review, manual investigation, and running commands. Not for launching agents.

```
tmux_create(project_id: String) → ()          // creates session poe-{project_id}
tmux_attach(project_id: String) → PtyHandle   // returns pty fd for xterm
tmux_resize(project_id: String, cols: u16, rows: u16) → ()
tmux_send_keys(project_id: String, keys: String) → ()
tmux_kill(project_id: String) → ()
```

Shell out to system `tmux`. Detect live session before creating.

**2. Agent session handover (xterm.js + WebSocket bridge)** — direct PTY connection to a specific agent's Claude session via `--resume`. Used when a human wants to check in on or interact with a running or completed agent session. See §5 for the full handover protocol.

```
agent_handover_open(node_id: String) → ()     // look up session_id, spawn PTY bridge, open xterm panel
agent_handover_close(node_id: String) → ()    // Ctrl-C → drop master → SIGTERM → SIGKILL
```

The two surfaces coexist. The project terminal is project-scoped (one per project); agent handover is node-scoped (one per agent session).

Add to `package.json`: `@xterm/xterm`, `@xterm/addon-fit`, `@xyflow/react`.

---

## Implementation Notes

**Stage runner trigger**: The orchestrator is event-driven via a Tokio `mpsc` channel (`DagChanged` signal). Signals arrive from three sources: the DAG Service (after every node/edge mutation via MCP tool call — see §6 Orchestrator Notification); the poe: ingester (on `poe:done`, `poe:yield`); and human actions (gate advances, decision resolutions). On each signal, the orchestrator queries SQLite for tasks where `status = 'pending'` and all `depends_on` tasks have `status = 'done'`, and the running count is below the concurrency limit. Eligible tasks are spawned. No explicit Tauri command triggers execution — the loop fires on DAG changes automatically.

**Skill file locations**: Skill files live in the app bundle at `resources/skills/<skill-id>.md`. User-level overrides at `~/.poe/skills/<skill-id>.md`. Project-level overrides at `{project.path}/.poe/skills/<skill-id>.md`.

**UX-Brief §Pane 3**: Always-visible right panel: decision queue (top) + Queue Advisor chatbot (bottom). Never in a tab.
