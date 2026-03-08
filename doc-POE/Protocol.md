# POE — Protocol Specification

**Status**: Draft
**Last updated**: 2026-03-08

**Artifact classification**: This document serves as both:
- `interface-control.md` — the authoritative Interface Control Document for POE v2. Defines all external interface contracts: the poe: event wire format (§2), the agent stdin bundle format (§3), and the frontend update mechanism (§4).
- `data-model.md` — the authoritative Database Design Document. Defines the SQLite schema for all internal data structures (§1).

These are standard artifact types in the POE corpus (see Architecture.md §Artifact Corpus). They are injected into every implementation task's input bundle. When in doubt about a wire format, field name, or schema question — this document is the answer.

This document specifies the four I/O contracts that Phase 2 is built around. Everything else in the system is implementation detail; divergence here causes cross-component rework.

---

## 1. SQLite Schema

> **Note**: This section reflects the intended design schema. The actual poe2 implementation diverges in table and column names: `nodes` (not `tasks`), `events` (not `event_log`), `queue_items` (not `decisions`), and ISO text timestamps (not INTEGER). §5 is correct for the actual codebase. Do not use §1 as a migration guide against the live schema — treat it as the canonical design intent and refer to `poe2/src-tauri/src/dag_store/schema.rs` for the actual DDL.

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
  type        TEXT    NOT NULL DEFAULT 'task',    -- 'task' | 'bug' | 'chore' | 'subtask' | 'plan_review'
  skill       TEXT,
  status      TEXT    NOT NULL DEFAULT 'pending', -- 'pending' | 'running' | 'waiting' | 'done' | 'cancelled'
  session_id   TEXT,        -- Claude --resume handle, stored on spawn, used on restart
  yield_reason TEXT,        -- 'review' | 'decision' | NULL. Set when status=waiting. Used by recovery to determine SF-4 path without event_log join.
  review_id    TEXT,        -- populated for reviewer tasks only: the 'id' from the originating poe:review event. Enables ID-based completion tracking.
  retry_count  INTEGER NOT NULL DEFAULT 0,  -- reviewer tasks only: watchdog retry counter. Max configurable, default 2.
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

Agents write structured events to **stdout embedded within the stream-json transport**. In autonomous mode, Claude emits newline-delimited JSON objects; poe: events appear as text within `assistant` message content. The ingester accumulates text from assistant events into a buffer, splits on newlines, and passes complete lines to the poe: parser.

A line extracted from the assistant text is a poe: event if and only if it parses as valid JSON and contains a `"poe"` key. All other extracted lines are agent commentary and are discarded (not stored, not processed).

The event wire format is transport-independent. The JSON payloads below are identical whether the agent runs autonomously (stream-json) or interactively. The transport envelope changes; the event schema does not.

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

// Add a finish-to-start dependency edge: "from" must finish before "to" can start.
// Example: {"poe":"edge","from":"A","to":"B"} — A must complete before B is dispatched.
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
// Agent emits poe:yield immediately after. Orchestrator resumes via --resume once resolved.
{"poe": "decision", "question": "...", "options": ["Option A", "Option B"]}
// options is optional

// Yield control while awaiting an asynchronous response.
// Emitted after all poe:review or poe:decision events for this checkpoint.
// task status → waiting. reason: "review" | "decision"
{"poe": "yield", "reason": "review"}

// Signal task completion (all work done — not a yield checkpoint).
{"poe": "done", "summary": "..."}  // summary optional

// Request a peer review. Orchestrator spawns reviewer_skill agent,
// injects result via stdin when complete, unblocks this agent.
// id is required when emitting multiple poe:review events — omit only
// when emitting a single review (single-reviewer path).
{"poe": "review", "reviewer_skill": "senior-engineer", "content": "...", "id": "r-eng"}

// Multi-specialist plan review — product-manager emits one per domain:
// {"poe": "review", "reviewer_skill": "senior-engineer",      "id": "r-eng",  "content": "..."}
// {"poe": "review", "reviewer_skill": "architecture-analyst", "id": "r-arch", "content": "..."}
// {"poe": "review", "reviewer_skill": "interface-analyst",    "id": "r-icd",  "content": "..."}
// Orchestrator spawns all reviewers in parallel. Each result delivered via stdin:
// ---
// ReviewResult id=r-eng skill=senior-engineer verdict=APPROVED|APPROVED_WITH_CONDITIONS|BLOCKED|FAILED
// {findings text}
// ---
// Verdict values: APPROVED | APPROVED_WITH_CONDITIONS | BLOCKED | FAILED (underscore, no spaces)
// FAILED indicates the reviewer exceeded max retries and was cancelled by the watchdog.
// Agent checks that all expected review IDs are present in the bundle before proceeding.
// Any FAILED verdict should be treated as a signal to escalate via poe:decision.
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
| `poe:skill` | write `{project}/.poe/skills/{name}.md` | yes | `poe://event` |
| `poe:brief` | — | yes | `poe://event` |
| `poe:step` | — | yes | `poe://event` |
| `poe:decision` | INSERT decisions | yes | `poe://decision` |
| `poe:review` | INSERT event_log only (yield handles status) | yes | `poe://event` |
| `poe:yield` | UPDATE tasks.status=waiting, SET yield_reason, signal orchestrator (SF-3) | yes | `poe://task-update` + `poe://event` |
| `poe:done` | UPDATE tasks.status=done, signal orchestrator | yes | `poe://task-update` |

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
## Execution Protocol

You are in a conversation with a human developer.

- This is a collaborative, multi-round session — ask questions and wait for answers.
- Do not emit poe: events unless you have produced a concrete output
  (poe:artifact when you write a document, poe:knowledge when you record a decision).
- End the conversation naturally. Do not emit `poe:done` unless explicitly asked.
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

{content from poe:review event — the plan summary or artifact under review}

> **Naming convention**: The reviewer MUST emit its artifact as `{"poe":"artifact","name":"review-{review_id}.md",...}` where `{review_id}` is the Review ID above. This makes the artifact path deterministic — the orchestrator derives `docs/review-{review_id}.md` directly without an artifacts table query.

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

### Skill priority chain

The skill file for `{skill-id}` is resolved as follows (first match wins):

1. `{project.path}/.poe/skills/{skill-id}.md`
2. `~/.poe/skills/{skill-id}.md`
3. App bundle `skills/{skill-id}.md`

If no file is found, abort the task with an error — do not spawn the agent with no skill.

The **mode protocol block** is prepended to the bundle before the `# Skill` section — not inserted into the skill file itself. The skill file is read-only from the orchestrator's perspective; it is never modified at runtime.

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

## 5. Agent Spawn Model

This section is the authoritative reference for how Claude agents are spawned. Every implementer touching `agent_lifecycle` must read this. Inconsistency here breaks session resume, decision continuation, and human handover.

---

### Primary transport — stream-json (autonomous, programmatic)

Every orchestrated agent runs via:

```
claude --output-format stream-json --verbose -p --dangerously-skip-permissions
```

Stdin receives the T+S+K input bundle; stdin is closed (EOF) immediately after writing. Claude processes the bundle, emits a stream of JSON objects to stdout, and exits cleanly.

```
spawn: claude --output-format stream-json --verbose -p --dangerously-skip-permissions
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

> **TODO**: Once the Decision Escalation flow is written in `Flows.md`, trim the sequence below to a wire-format reference + link, matching the pattern used for §"Review injection via --resume". The numbered steps are a flow description and belong in Flows.md.

When an agent emits `poe:decision` and then `poe:yield` (indicating it is awaiting a human decision before it can continue):

```
1. Orchestrator records decision in SQLite, marks task status = waiting
2. Human resolves via queue panel (or engages Advisor first)
3. Orchestrator spawns a new stream-json session:
     claude --output-format stream-json --verbose -p --dangerously-skip-permissions --resume <session_id>
4. Bundle written to stdin:
     ---
     Human: {resolution text}
5. Agent reads full session history + new message, continues work
6. Agent emits further poe: events + final poe:done
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

## 6. Phase 3 Tauri Command Surface

New Tauri commands required for Phase 3. All work in `poe2/src-tauri/src/`. Commands shared across beads are noted.

### Phase / Plan Composer (7ct.1, 7ct.3)

```
list_phases(project_id: String) → Vec<Phase>
create_phase(project_id: String, name: String, stage_type: String, number: i64) → Phase
```

`number` maps to the existing `phases.number INTEGER` column (UNIQUE per project — serves as ordered position). Do **not** add a `position` column.

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
get_node_ancestry(node_id: String) → Vec<Node>                          // walks parent_id chain to root
read_artifact_content(artifact_id: String, project_id: String) → String // reads file from disk
```

`read_artifact_content` path formula: look up `artifact.filename` from the artifacts table, look up `project.path` from the project registry by `project_id`, construct `{project.path}/docs/{artifact.filename}`. The `Artifact` struct has `filename: String`; there is no stored `path` field.

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

## Minor gaps addressed

**Stage runner trigger (.3)**: The orchestrator is event-driven via a Tokio `mpsc` channel (`DagChanged` signal). The event ingester sends a signal on every event that mutates DAG structure or task status (`poe:task`, `poe:task:update`, `poe:task:cancel`, `poe:edge`, `poe:edge:remove`, `poe:done`, plus human gate advances and decision resolutions). On each signal, the orchestrator queries SQLite for tasks where `status = 'pending'` and all `depends_on` tasks have `status = 'done'`, and the running count is below the concurrency limit. Eligible tasks are spawned. No explicit Tauri command triggers execution — the loop fires on DAG changes automatically.

**v1 skill locations (.5)**: Existing skill files are at `poe/src-tauri/skills/`. Port targets:
- `poe/src-tauri/skills/operational-analyst.md` → `skills/operational-analyst.md` in app bundle
- `poe/src-tauri/skills/product-manager.md` → `skills/product-manager.md` in app bundle

**UX-Brief §Pane 3**: Exists and is finalized. Always-visible right panel: decision queue (top) + Queue Advisor chatbot (bottom). Never in a tab.
