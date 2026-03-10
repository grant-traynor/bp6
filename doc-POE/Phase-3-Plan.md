---
id: phase-3-plan
title: POE Phase 3 — Oversight, Skills & Plan Creation
status: Draft
date: 2026-03-10
---

# POE Phase 3 — Oversight, Skills & Plan Creation

## 1. Objective

Deliver the oversight surface, plan composition interface, advisor chatbot, and the two skills that unlock autonomous planning. By the end of this phase:

- A human opens POE, sees the **Phase × Scope Matrix** — a live orientation grid showing phases across the top, WBS scope down the side, and task status in each cell.
- **Stage Gate UI** holds the human at each phase transition; they review artifacts before advancing.
- **Conversational decision thread mode** in the queue panel shows the full Q+A history when an agent is running a multi-round elicitation.
- The **Plan Composer** (React Flow DAG editor) lets a human compose a new project plan from stage type blocks, connect them, and press Start.
- The **product-manager skill** (ported to Protocol v2 + multi-reviewer consensus) and **senior-engineer skill** (authored from scratch) are available as orchestrated specialists.
- The **Knowledge Register panel** is browsable and searchable; the **Artifact Viewer** shows a side-by-side diff for versioned artifacts.
- The **Queue Advisor chatbot** (Rust-side Claude API proxy) is live in Pane 3, pre-loaded with project context, ready to research before the human decides.
- The **Task detail panel** slides in on any matrix cell click: WBS ancestry, event log, node-scoped advisor conversation.

**Self-validation milestone** (after Wave 2): use POE to orchestrate the planning of POE's next feature. The product-manager skill decomposes the work; the senior-engineer reviews the plan; the resulting task DAG is visible in the Phase × Scope Matrix. If the lifecycle produces output of equal or better quality than the manual process used to design bp6-0xf, the concept is proven.

---

## 2. Scope

### In scope

| Area | Item | Bead |
|---|---|---|
| UI | Phase × Scope Matrix — custom CSS grid with collapsible WBS rows and live task status | bp6-0xf.1 |
| UI | Conversational decision thread mode in QueuePanel | bp6-0xf.2 |
| UI | Stage Gate UI — artifact review panel, gate actions (Advance / Revise / Re-run) | bp6-0xf.3 |
| UI | Plan Composer — React Flow DAG editor with stage type catalogue | bp6-0xf.6 |
| UI | Knowledge Register panel — browsable, filterable, with promotion | bp6-0xf.7 |
| UI | Artifact Viewer diff view — side-by-side comparison for versioned artifacts | bp6-0xf.7 |
| UI | Queue Advisor chatbot — Rust-side Claude API proxy, streaming via Tauri Channel | bp6-0xf.8 |
| UI | Task detail panel — WBS ancestry, event log, node-scoped conversation | bp6-0xf.9 |
| Skills | product-manager — port to Protocol v2 + multi-reviewer plan consensus | bp6-0xf.4 |
| Skills | senior-engineer — authored from scratch using EIAMOE framework | bp6-0xf.5 |
| Schema | Add `stage_type TEXT` to `phases` table | 0xf.1 + 0xf.6 |
| Schema | Add `sort_order INTEGER` to `nodes` table | bp6-0xf.1 |
| Backend | New Tauri commands: `list_edges`, `update_node_sort_order`, `create_phase` | 0xf.1, 0xf.6 |
| Backend | New Tauri commands: `update_knowledge`, `promote_knowledge`, `get_node_ancestry` | 0xf.7, 0xf.9 |
| Backend | New Tauri commands: `advance_phase`, `revise_phase`, `rerun_phase` | bp6-0xf.3 |
| Backend | New Tauri commands: `start_advisor_session`, `respond_to_advisor` | bp6-0xf.8 |
| Deps | Install `@xyflow/react` (React Flow v12) | bp6-0xf.6 |
| Deps | Install `react-diff-viewer-continued` | bp6-0xf.7 |
| Deps | Add `reqwest` with `features = ["json", "stream"]` to `Cargo.toml` | bp6-0xf.8 |

### Out of scope

- PTY / `agent_handover_open` improvements — separate
- MCP tooling — deferred
- Validity Analysis and Retrospective skills (`validity-analyst`, `rca-analyst`) — deferred
- Agent interrupt (Pause Stage / Abort Project) — deferred
- Cross-project global concurrency indicator — deferred
- `interface-analyst`, `data-model-analyst`, `architecture-analyst` skill authoring — deferred
- Skill promotion from Retrospective gate (skill diff + approval UI) — deferred beyond Wave 3

---

## 3. Current State

### What exists (after Phase 2.3)

| Component | State |
|---|---|
| `phases` table | Exists: `id, project_id, number, title, lifecycle_stage, gate_held, created_at, updated_at`. Missing: `stage_type` |
| `nodes` table | Exists. Missing: `sort_order` |
| `list_phases(project_id)` | Implemented |
| `read_artifact_content(artifact_id, project_id)` | Implemented — reads `{project.path}/docs/{artifact.filename}` |
| `flag_knowledge_for_promotion(id, project_id)` | Stub — sets `promoted = 1` on knowledge row; does not write to `~/.poe/knowledge/` |
| `respond_to_chat` | Implemented (Phase 2.3) |
| `ArtifactViewer.tsx` | Chat panel implemented (Phase 2.3). No diff view. |
| `QueuePanel.tsx` | Handles `poe:decision` items (standard mode only). No conversational thread mode. |
| `KnowledgePanel.tsx` | Component file exists — shell only, no implementation. |
| `StageGate.tsx` | Component file exists — shell only, no implementation. |
| `ActivityFeed.tsx` | Handles `poe:brief`, `poe:step`, `poe:artifact`, `poe:yield`, `poe:done`, `poe:chat` (Phase 2.3). |
| `operational-analyst.md` | Implemented (Phase 2.3) — interactive `poe:chat` elicitation mode. |
| `product-manager.md` | Exists at `poe2/src-tauri/skills/product-manager.md` — v1 port not yet done for Protocol v2; no multi-reviewer extension. |
| `senior-engineer.md` | Exists at `poe2/src-tauri/skills/senior-engineer.md` — stub; not authored per EIAMOE framework. |

### What is missing

| Gap | Description |
|---|---|
| `phases.stage_type` | Column not in schema |
| `nodes.sort_order` | Column not in schema |
| `list_edges(project_id)` | No command — Matrix cannot render dependency indicators |
| `update_node_sort_order(node_id, sort_order)` | No command — Matrix drag-to-reorder has no write path |
| `create_phase(project_id, name, stage_type, number)` | No command — Plan Composer cannot write phase records |
| `update_knowledge(id, content)` | No command — human cannot author knowledge entries |
| `promote_knowledge(id)` | No full implementation — stub `flag_knowledge_for_promotion` only sets a flag; must write to `~/.poe/knowledge/` and log the promotion |
| `get_node_ancestry(node_id)` | No command — task detail panel cannot show WBS ancestry |
| `advance_phase`, `revise_phase`, `rerun_phase` | No commands — Stage Gate UI has no write path |
| `start_advisor_session`, `respond_to_advisor` | No commands — Advisor chatbot not implemented |
| Phase × Scope Matrix | Not built |
| Conversational decision thread mode | Not built |
| Stage Gate UI | Shell only |
| Plan Composer | Not built |
| Knowledge Register panel | Shell only |
| Artifact Viewer diff view | Not built |
| Queue Advisor chatbot | Not built |
| Task detail panel | Not built |
| `@xyflow/react` | Not in `package.json` |
| `react-diff-viewer-continued` | Not in `package.json` |
| `reqwest` | Not in `Cargo.toml` |

---

## 4. UX Components

### 4a. Phase × Scope Matrix (new — replaces placeholder in Pane 2)

**Current**: no matrix view; Pane 2 shows a stub.

**Phase 3 implementation**:

Custom CSS grid. X-axis: phases (columns). Y-axis: WBS scope (Epic → Feature → Task), collapsible rows. Each cell is a task status badge. Phase headers show PDCA stage and gate status.

**Task status visual states:**

| Status | Indicator | Meaning |
|---|---|---|
| `pending` | `░░░░` | Not yet started |
| `running` | `████ ●` | Agent actively executing |
| `waiting` | `⏸ skill-name` | Agent yielded — awaiting review or decision |
| `done` | `████` muted | Complete |
| `cancelled` | `────` strikethrough | Cancelled, preserved in history |

**Live updates**: `poe://task-update` Tauri events drive status badge changes without a full re-render.

**Dependency indicators**: `list_edges(project_id)` returns all edges; cells with upstream dependencies show a connector symbol. Clicking a connector shows the upstream/downstream chain.

**Collapsible rows**: row open/close state is local component state (not persisted). Epics collapse to a single summary row; expanded by default.

**Drag-to-reorder**: task rows can be reordered within a phase. On drop, `invoke("update_node_sort_order", {node_id, sort_order})` persists. `sort_order` is a sparse integer — no re-numbering of sibling rows required on every drag.

**Phase header gate status**: reads `phases.gate_held`. When `gate_held = 1`, the header shows a lock icon and the next-phase column is dimmed. The gate action panel (§4c) appears.

**Clicking a cell**: opens the Task detail panel (§4g).

**Layout:**
```
┌─ Selected Project ──────────────────────────────────────────────────────┐
│                    Phase 1 (Do)     Phase 2 (Plan)    Phase 3           │
│ ─────────────────────────────────────────────────────────────────────── │
│ Epic A             ████ ████ ░░░░   [ planned ]                         │
│   Feature A1       ████ done                                            │
│   Feature A2       ████ running ●                                       │
│   Feature A3       ░░░░ pending                                         │
│                                                                         │
│ Epic B             ████ ████        [ planned ]                         │
│   Feature B1       ████ done                                            │
│   Feature B2       ⏸ senior-eng    ░░░░ pending                        │
│ ─────────────────────────────────────────────────────────────────────── │
└─────────────────────────────────────────────────────────────────────────┘
```

**New Tauri commands**: `list_edges(project_id)`, `update_node_sort_order(node_id, sort_order)`.
**Schema changes**: `phases.stage_type TEXT`, `nodes.sort_order INTEGER`.
**Work in**: `poe2/src/` (new component), `poe2/src-tauri/src/dag_store/`.

---

### 4b. Conversational Decision Thread Mode (modification to QueuePanel.tsx)

**Current**: all queue items render in standard mode — question + options + text input.

**Phase 3 change**: when a task has > 0 resolved `decisions` rows AND exactly 1 pending row for the same `task_id`, the queue item renders in conversation thread mode. Prior Q+A pairs shown read-only above the current question. Current pending question highlighted. Free-text input only (no option buttons).

**Detection logic** (frontend): on receiving any `poe://decision` event, query existing decisions for `task_id`. If `resolved_count > 0`, switch to thread mode render.

```
┌─ CONOPS Elicitation — operational-analyst ──────────────┐
│                                                          │
│  ● Q1: What problem does this system solve?              │
│    A: It's a tool for managing agentic workflows...      │
│                                                          │
│  ● Q2: Who are the primary users?                        │
│    A: Software engineers coordinating multi-agent...     │
│                                                          │
│  ● Q3 (pending): What are the 3 most important           │
│    workflows the system must support?                    │
│                                                          │
│  [ type your response...                              ]  │
│                                              [ Send ↵ ]  │
└──────────────────────────────────────────────────────────┘
```

**No new Tauri commands** — existing `list_queue_items` returns all rows; frontend groups by `task_id` and counts resolved vs. pending.
**Work in**: `poe2/src/components/QueuePanel.tsx`.

---

### 4c. Stage Gate UI (implementation of StageGate.tsx shell)

**Current**: `StageGate.tsx` is a shell component with no logic.

**Phase 3 implementation**: when `phases.gate_held = 1`, the Phase × Scope Matrix header shows a lock icon and the project card in Pane 1 highlights the gate. Clicking the lock (or the project card indicator) opens the gate panel.

**Gate panel contents**:
- All artifacts produced by the completed stage, with links to Artifact Viewer
- Inner loop state summary: how many plan review iterations ran, whether any review was BLOCKED
- Resolved decisions from this stage (count and summaries)
- Agent activity summary (brief + step count per task)
- Gate actions: **Advance / Revise / Re-run** — each confirmed with a modal showing the count of tasks that will be affected

**Retrospective gate** — additional affordance: shows diff of updated skill files produced by the Retrospective stage. One-click approval writes skill changes to `{project}/.poe/skills/`. Promotion to `~/.poe/skills/` available from the same panel.

**Orchestrator response to `advance_phase`**: transitions `phases.gate_held = 0`, sets next phase `lifecycle_stage = 'do'` (or equivalent), triggers the orchestrator loop which picks up all pending tasks in the newly activated phase.

**New Tauri commands**: `advance_phase(phase_id)`, `revise_phase(phase_id)`, `rerun_phase(phase_id)`.
**Work in**: `poe2/src/components/StageGate.tsx`, `poe2/src-tauri/src/dag_store/commands.rs`, `poe2/src-tauri/src/orchestrator/`.

---

### 4d. Plan Composer — React Flow DAG Editor (new component)

**Current**: no plan composition UI.

**Phase 3 implementation**: when a project is first created (or when the human initiates a new phase plan), the Plan Composer replaces the Phase × Scope Matrix in Pane 2.

Built with `@xyflow/react` (React Flow v12). Left side: stage library panel listing all stage types. Canvas: drag-and-drop composition with connector validation.

**Stage type catalogue** — static TypeScript constant (not a SQLite table):

```typescript
export const STAGE_TYPES = [
  'conops', 'guardrails', 'increment_planning', 'execution',
  'pm_review', 'rework', 'validity_analysis', 'retrospective', 'onboarding'
] as const;
```

Each stage type declares consumed and produced artifact types in the same constant. Connector compatibility validation: the downstream stage's declared input artifact types must be satisfied by the upstream stage's declared output artifact types. Incompatible connections are blocked at the UI (the edge snaps back).

**"Start Project" flow**:
1. Human validates plan (UI shows connector warnings).
2. Human clicks "Start Project".
3. Frontend calls `invoke("create_phase", ...)` for each stage node in topological order.
4. Phase records created in SQLite.
5. Orchestrator begins with the first phase.
6. Plan Composer is replaced by the Phase × Scope Matrix.

**New Tauri commands**: `create_phase(project_id, name, stage_type, number)` — `number` maps to `phases.number INTEGER (UNIQUE per project)`. No `position` column.
**Schema change**: `phases.stage_type TEXT` (shared with Matrix bead).
**Dependencies**: `@xyflow/react` added to `package.json`.
**Work in**: `poe2/src/` (new `PlanComposer.tsx`), `poe2/src-tauri/src/dag_store/`.

---

### 4e. Knowledge Register Panel (implementation of KnowledgePanel.tsx shell)

**Current**: `KnowledgePanel.tsx` is a shell with no logic.

**Phase 3 implementation**: browsable, filterable panel for all knowledge entries in the current project. Accessible from the project toolbar (persistent icon).

- Key/value table with columns: key, content preview, phase, author (agent | human), created_at
- Filter by phase (dropdown), author (toggle), keyword search (text input)
- Click an entry: expands to show full value, supersedes chain (linked entries), and `Update` button for human-authored edits
- Entries are read-only by default; agents write via `poe:knowledge`
- Human edit: click `Update` → inline textarea → `invoke("update_knowledge", {id, content})`
- Promote button: flags entry for promotion to `~/.poe/knowledge/` via `invoke("promote_knowledge", {id})`. Promotion is logged in `event_log`. Promoted entries show a badge.

**New Tauri commands**:
- `update_knowledge(id: String, content: String)` — updates `knowledge.content` in SQLite (does not supersede; this is human correction, not a new entry)
- `promote_knowledge(id: String)` — reads knowledge entry from SQLite by id; writes to `~/.poe/knowledge/{key}.md` (or appends to a JSONL file — implementor chooses format, but it must match how the knowledge register is read at bundle assembly time); logs to `event_log`

**Work in**: `poe2/src/components/KnowledgePanel.tsx`, `poe2/src-tauri/src/dag_store/`.

---

### 4f. Artifact Viewer Diff View (modification to ArtifactViewer.tsx)

**Current**: `ArtifactViewer.tsx` renders artifact markdown. No diff view.

**Phase 3 change**: when an artifact has more than one revision in the `artifacts` table (same `name`, different `created_at`), a **"Compare with previous"** button appears in the viewer toolbar.

Clicking it opens a split-pane diff (left = old, right = new) using `react-diff-viewer-continued`. Line-level highlighting. Both artifact content strings are fetched via `read_artifact_content` (already implemented) — the viewer fetches old and new artifact IDs separately.

**Dependencies**: `react-diff-viewer-continued` added to `package.json`.
**Work in**: `poe2/src/components/ArtifactViewer.tsx`.

---

### 4g. Queue Advisor Chatbot (implementation — advisor skill + poe:advisor protocol)

**Current**: no advisor chatbot.

**Phase 3 implementation**: persistent chat interface in the bottom half of Pane 3 (below the decision queue). Never in a tab.

**Architecture**: the advisor is a standard interactive agent dispatched via SF-1 — the same `claude` CLI + `--resume` infrastructure used by all other agents. No `reqwest`, no bespoke module. The advisor uses `poe:advisor` events (not `poe:chat`) so the ingester routes turns to the advisor panel rather than the Artifact Viewer. See Flows.md §3.9 for the full sequence.

```
Human clicks advisor panel (or "Ask advisor" from a queue item)
  → invoke("start_advisor_session", {project_id, decision_id?})
  → Orchestrator: creates advisor task node, assembles T+S+K bundle:
      - Skill: advisor.md (interactive mode)
      - Task: "Research for decision: {question}" (or "General project advisor")
      - K: full artifact corpus + knowledge register + blocked task ancestry
  → SF-1: spawn agent
  → Agent emits poe:advisor + poe:yield per turn
  → Ingester: INSERT advisor_turns, emit poe://advisor-turn
  → Frontend: append to advisor panel in Pane 3
  → Human responds: invoke("respond_to_advisor", {turn_id, response})
  → SF-4: --resume with "Human: {response}"
  → Cycle repeats until poe:done
```

**New skill required**: `advisor.md` — interactive mode skill. Uses `poe:advisor` for turns. Context includes artifact corpus and knowledge register (assembled at dispatch). Responds to human questions about the project, decision options, architecture constraints, knowledge register entries. Does NOT resolve queue items — it informs human decisions.

**New Tauri commands**: `start_advisor_session(project_id, decision_id?)`, `respond_to_advisor(project_id, turn_id, response)`.
**No new Cargo dependencies** — uses existing `claude` CLI subprocess infrastructure.
**Work in**: `poe2/src-tauri/src/` (advisor task creation in `dag_store/commands.rs`), `poe2/src/components/` (new `AdvisorChatbot.tsx`, update `QueuePanel.tsx`), `poe2/src-tauri/skills/advisor.md` (new skill).

---

### 4h. Task Detail Panel (new component)

**Current**: clicking a matrix cell has no action.

**Phase 3 implementation**: slide-in panel triggered by clicking any task cell in the Phase × Scope Matrix.

**Panel contents**:
- WBS ancestry: walks from the task up through parent feature, epic, phase, project via `get_node_ancestry(node_id)`. Rendered as a breadcrumb trail.
- Task description, type, skill assignment, status
- Agent brief: most recent `poe:brief` event for this `task_id` from `event_log`
- Full event log for this task: all `poe:` events in chronological order (type, payload preview, timestamp)
- Dependency chain: upstream tasks (must be done first) and downstream tasks (blocked by this one) — fetched from edges

**Node-scoped conversation**: inline chat UI (Queue Advisor component, `mode: 'node'`, `node_id: task.id`). Pre-loaded with task context.

**Mode guard**: if the task's assigned skill declares `modes: [autonomous]` only (read from skill frontmatter), the "Open conversation" button is disabled with tooltip: *"This skill runs autonomously. View its activity feed to see what it did."*

**New Tauri commands**: `get_node_ancestry(node_id: String) → Vec<Node>` — walks `parent_id` chain to root.
**Shared commands**: `read_artifact_content`, `start_advisor_session`, `respond_to_advisor`.
**Work in**: `poe2/src/` (new `TaskDetailPanel.tsx`).

---

## 5. Schema Changes

Four `ALTER TABLE` migrations plus one new table. All fail gracefully if the column already exists (SQLite does not support `IF NOT EXISTS` on `ALTER TABLE` — wrap in a `PRAGMA` check or use a migration version flag).

```sql
-- phases: add stage_type column (used by Matrix header and Plan Composer)
ALTER TABLE phases ADD COLUMN stage_type TEXT NOT NULL DEFAULT 'execution';

-- nodes: add sort_order column (used by Matrix drag-to-reorder)
ALTER TABLE nodes ADD COLUMN sort_order INTEGER;

-- nodes: add skill_modes column (used by task detail panel mode guard)
-- Populated at SF-1 dispatch time from skill frontmatter. JSON array, e.g. '["autonomous","interactive"]'.
ALTER TABLE nodes ADD COLUMN skill_modes TEXT;

-- New table: advisor_turns (parallel to chat_turns, routes to Pane 3 advisor panel)
CREATE TABLE IF NOT EXISTS advisor_turns (
  id           TEXT    PRIMARY KEY,
  task_id      TEXT    NOT NULL REFERENCES nodes(id),
  content      TEXT    NOT NULL,
  response     TEXT,
  created_at   TEXT    NOT NULL,
  responded_at TEXT
);
```

Migrations land in `dag_store/schema.rs`. Existing projects get `stage_type = 'execution'` (safe default), `sort_order = NULL` (Matrix renders in creation order when NULL), and `skill_modes = NULL` (mode guard treats NULL as `['autonomous']` only — safe default).

---

## 6. New Tauri Commands Summary

All commands implemented in `poe2/src-tauri/src/dag_store/commands.rs` unless noted.

| Command | Signature | Notes |
|---|---|---|
| `list_edges` | `(project_id: String) → Vec<Edge>` | Returns all edges for project; Matrix uses for dependency indicators |
| `update_node_sort_order` | `(node_id: String, sort_order: i32) → ()` | Persists Matrix drag-to-reorder |
| `create_phase` | `(project_id: String, name: String, stage_type: String, number: i64) → Phase` | Plan Composer write path; validates `stage_type` against catalogue; all phases created as `pending` |
| `activate_phase` | `(phase_id: String) → ()` | Sets phase `status='running'`, signals orchestrator; called once by Plan Composer "Run" button for Phase 1 only |
| `advance_phase` | `(phase_id: String) → ()` | Clears gate, sets current phase `complete`, activates next phase, triggers orchestrator |
| `revise_phase` | `(phase_id: String, task_ids: Vec<String>) → ()` | Re-queues selected completed tasks to `pending`; other done tasks stay done |
| `rerun_phase` | `(phase_id: String) → ()` | Full re-run: resets all done tasks in phase to `pending` |
| `update_knowledge` | `(id: String, content: String) → ()` | Human-authored correction to a knowledge entry (updates in place; does not supersede) |
| `promote_knowledge` | `(id: String) → ()` | Writes `~/.poe/knowledge/{key}.md` (raw content field); logs to `event_log` with `event_type='knowledge:promoted'` |
| `get_node_ancestry` | `(node_id: String) → Vec<Node>` | Walks `parent_id` chain to root |
| `start_advisor_session` | `(project_id: String, decision_id: Option<String>) → ()` | Creates advisor task node, dispatches via SF-1 with advisor skill and full artifact+knowledge context |
| `respond_to_advisor` | `(project_id: String, turn_id: String, response: String) → ()` | Writes response to `advisor_turns`, signals DagChanged, triggers SF-4 with `yield_reason='advisor'` |

**Already implemented** (no change needed):
- `read_artifact_content(artifact_id, project_id)` — used by Artifact Viewer diff and advisor context assembly
- `list_phases(project_id)` — used by Matrix and Plan Composer
- `respond_to_chat` — Phase 2.3

---

## 7. Architecture Components

| Component | Role in Phase 3 | Change |
|---|---|---|
| `dag_store/schema.rs` | Schema migrations | **Modify**: add `stage_type` to `phases`, `sort_order` + `skill_modes` to `nodes`, create `advisor_turns` table |
| `dag_store/commands.rs` | New command surface | **Add**: `list_edges`, `update_node_sort_order`, `create_phase`, `activate_phase`, `advance_phase`, `revise_phase`, `rerun_phase`, `update_knowledge`, `promote_knowledge`, `get_node_ancestry`, `start_advisor_session`, `respond_to_advisor` |
| `orchestrator/mod.rs` | Phase gate + activation | **Add**: respond to `activate_phase` (SF-5); respond to `advance_phase` — set next phase active, trigger scheduling loop |
| `agent_lifecycle/mod.rs` | skill_modes population | **Modify**: at SF-1 dispatch, write parsed skill frontmatter `modes` field as JSON to `nodes.skill_modes` |
| `event_ingester/mod.rs` | poe:advisor handling | **Add**: `poe:advisor` handler — INSERT `advisor_turns`, emit `poe://advisor-turn`; add `'advisor'` to yield_reason derivation |
| `lib.rs` | Command registration | **Add**: register all new commands |
| New: `skills/advisor.md` | Advisor skill | **Create**: interactive mode skill using `poe:advisor`; responds to human queries about project context, decision options, artifacts, knowledge register |
| `skills/product-manager.md` | v1 → v2 port + multi-reviewer | **Rewrite**: `poe:node` → `poe:task`; add `poe:review` to `senior-engineer` (always) and `architecture-analyst` (conditional per trigger rules); handle `ReviewResult`; escalate via `poe:decision` on reviewer conflict |
| `skills/senior-engineer.md` | Authored from scratch | **Rewrite**: EIAMOE framework; handles `plan_review` task type; emits `review-{review_id}.md` artifact with APPROVED \| APPROVED_WITH_CONDITIONS \| BLOCKED verdict |
| New: `src/PlanComposer.tsx` | React Flow DAG editor | **Create**: stage library + canvas; connector validation; writes phases via `create_phase`; "Run" button calls `activate_phase` |
| `src/ArtifactViewer.tsx` | Add diff view | **Extend**: "Compare with previous" button; split-pane diff via `react-diff-viewer-continued` |
| `src/KnowledgePanel.tsx` | Knowledge register UI | **Implement**: filterable table; inline edit; `update_knowledge`; `promote_knowledge` |
| `src/QueuePanel.tsx` | Add thread mode + advisor panel | **Extend**: detect resolved-count > 0 → render in conversation thread mode; add `AdvisorChatbot` in bottom half; listen for `poe://advisor-turn` |
| `src/StageGate.tsx` | Gate UI | **Implement**: artifact list; inner loop summary; gate action buttons; `advance/revise/rerun_phase` |
| New: `src/TaskDetailPanel.tsx` | Task detail slide-in | **Create**: ancestry breadcrumb, event log, dependency chain; mode guard reads `task.skill_modes`; node-scoped advisor via `start_advisor_session` |
| New: `src/AdvisorChatbot.tsx` | Advisor chat UI | **Create**: renders `advisor_turns`; submits via `respond_to_advisor`; listens for `poe://advisor-turn`; used in QueuePanel (Pane 3) and TaskDetailPanel (node-scoped) |
| `package.json` | New frontend deps | **Add**: `@xyflow/react`, `react-diff-viewer-continued` |

---

## 8. Implementation Tasks (ordered)

### Wave 1 — Orientation & Human Gate

**Milestone A — Schema & Commands (backend)**

1. **Schema**: four migrations in `dag_store/schema.rs`:
   - `ALTER TABLE phases ADD COLUMN stage_type TEXT NOT NULL DEFAULT 'execution'`
   - `ALTER TABLE nodes ADD COLUMN sort_order INTEGER`
   - `ALTER TABLE nodes ADD COLUMN skill_modes TEXT`
   - `CREATE TABLE IF NOT EXISTS advisor_turns (...)` — see §5 for full DDL
2. **`list_edges` command**: returns `Vec<Edge>` for project.
3. **`update_node_sort_order` command**: persists Matrix drag-to-reorder.
4. **`create_phase` command**: creates a phase with `stage_type` (status='pending'); validates type against catalogue constant.
5. **`activate_phase` command**: sets phase status='running', signals DagChanged — called by Plan Composer "Run" button for Phase 1 only.
6. **`advance_phase`, `revise_phase(phase_id, task_ids)`, `rerun_phase` commands**: gate transition write paths.
7. **`get_node_ancestry` command**: walks `parent_id` chain to root.
8. **`start_advisor_session`, `respond_to_advisor` commands**: advisor session write paths.
9. **`poe:advisor` ingester handler**: INSERT advisor_turns, emit poe://advisor-turn, add 'advisor' to yield_reason derivation.
10. **skill_modes write in `agent_lifecycle`**: at SF-1 dispatch, parse skill frontmatter `modes` field, write as JSON to `nodes.skill_modes`.
11. **Register all new commands** in `lib.rs`.

**Milestone B — Phase × Scope Matrix**

8. **Build `PhaseMatrix` component** (`poe2/src/`): CSS grid, collapsible rows, five task status visual states, phase headers.
9. **Wire Tauri events**: `poe://task-update` drives status badge changes; `poe://phase-update` drives header state.
10. **Dependency indicators**: `list_edges` → connector symbols on cells with upstream deps.
11. **Drag-to-reorder**: drag handler calls `update_node_sort_order`.
12. **Cell click**: placeholder open for Task detail panel (§Wave 3).

**Milestone C — Conversational Decision Thread Mode**

13. **Extend `QueuePanel.tsx`**: detect `resolved_count > 0` for `task_id` → thread mode render with Q+A history and highlighted pending question.

**Milestone D — Stage Gate UI**

14. **Implement `StageGate.tsx`**: artifact list, inner loop summary, resolved decisions, agent activity, gate actions with confirmation modal.
15. **Orchestrator**: wire `advance_phase` to phase activation logic.

---

### Wave 2 — Skills & Plan Composer

**Milestone E — Skills**

16. **Port `product-manager` skill** to Protocol v2:
    - Rename `poe:node` → `poe:task` throughout.
    - Verify all event payloads match Protocol.md §2.
    - Add multi-reviewer extension: emit `poe:review` to `senior-engineer` (always); emit `poe:review` to `architecture-analyst` when plan includes schema migrations, new Tauri commands, new event types, or new subsystems.
    - On `ReviewResult` resume: address BLOCKED findings, revise DAG.
    - Escalate via `poe:decision` if reviewers disagree on a structural question.
17. **Author `senior-engineer` skill** from scratch (EIAMOE framework):
    - Handles `plan_review` task type (triggered by `**Type**: plan_review` in stdin bundle).
    - Reads review request, evaluates plan against CONOPS + guardrails artifacts.
    - Emits `poe:artifact` named `review-{review_id}.md` with structured findings and a verdict: `APPROVED | APPROVED_WITH_CONDITIONS | BLOCKED`.
    - Then `poe:done`.

**Milestone F — Plan Composer**

18. **Install `@xyflow/react`** in `package.json`.
19. **Build `PlanComposer` component**: stage library panel + React Flow canvas; stage type catalogue as TS constant; connector compatibility validation.
20. **Wire "Start Project"**: sequential `create_phase` calls for each canvas node in topological order; navigate to Phase × Scope Matrix on success.

---

### Self-Validation Milestone (Wave 2 exit criterion)

> Use POE to orchestrate the planning of the next POE feature. Submit a task to the product-manager skill; verify the resulting task DAG appears in the Phase × Scope Matrix; verify the senior-engineer review completes with a verdict. If the lifecycle produces output of equal or better quality than the manual planning process used to design bp6-0xf, Phase 3 Wave 2 is accepted.

---

### Wave 3 — Advisor, Knowledge, Task Detail

**Milestone G — Knowledge Register & Artifact Diff**

21. **Install `react-diff-viewer-continued`** in `package.json`.
22. **`update_knowledge` command**: updates knowledge entry content.
23. **`promote_knowledge` command**: writes to `~/.poe/knowledge/`; logs to `event_log`.
24. **Implement `KnowledgePanel.tsx`**: filterable table, full-value expand, supersedes chain, inline edit, promote button.
25. **Extend `ArtifactViewer.tsx`**: detect multi-revision artifacts; add "Compare with previous" button; split-pane diff render.

**Milestone H — Queue Advisor Chatbot**

26. **Add `poe:advisor` ingester handler**: INSERT into `advisor_turns`, emit `poe://advisor-turn`, add `'advisor'` to yield_reason derivation logic (parallel to `poe:chat` → `'chat'`).
27. **Add `advisor_turns` migration** in `dag_store/schema.rs`.
28. **Implement `start_advisor_session` command**: creates advisor task node (`type='advisor'`), assembles T+S+K bundle with advisor skill + full artifact corpus + knowledge register, dispatches via SF-1.
29. **Implement `respond_to_advisor` command**: writes response to `advisor_turns`, signals DagChanged; orchestrator SF-4 handles `yield_reason='advisor'` — identical path to `yield_reason='chat'`.
30. **Author `advisor.md` skill**: interactive mode, uses `poe:advisor` + `poe:yield`. Reads context bundle (decision question, artifacts, knowledge register), responds to human queries, emits `poe:done` when complete.
31. **Build `AdvisorChatbot.tsx`**: renders `advisor_turns` for the active advisor task; listens for `poe://advisor-turn`; submits via `respond_to_advisor`. Used in both QueuePanel (Pane 3 bottom) and TaskDetailPanel (node-scoped, with `decision_id=null`).
32. **Wire into `QueuePanel.tsx`**: `AdvisorChatbot` appears in bottom half of Pane 3; `start_advisor_session` called with `decision_id` when a queue item is selected.

**Milestone I — Task Detail Panel**

33. **Build `TaskDetailPanel.tsx`**: WBS ancestry breadcrumb (via `get_node_ancestry`), task metadata, agent brief, full event log, dependency chain.
34. **Wire node-scoped advisor**: embed `AdvisorChatbot` with `decision_id=null`; `start_advisor_session` called with the node's task context.
35. **Mode guard**: read `task.skill_modes` from task state (already populated at dispatch — no extra call); disable "Open conversation" with tooltip for tasks where `skill_modes` does not include `'interactive'`.
36. **Wire Matrix cell click**: open `TaskDetailPanel` slide-in.

---

## 9. Success Criteria

- [ ] Phase × Scope Matrix renders all phases and WBS scope for a live project. Task status badges update live via `poe://task-update`. Collapsible rows work.
- [ ] Five distinct task status states are visually differentiated in the matrix cell.
- [ ] Dependency indicators appear on cells with upstream deps; clicking shows the chain.
- [ ] Drag-to-reorder in the Matrix persists via `update_node_sort_order`.
- [ ] When a task has > 0 resolved decisions and 1 pending, the queue item renders in conversation thread mode with full Q+A history.
- [ ] Stage Gate: when `gate_held = 1`, the project card shows a gate indicator and the matrix next-phase column dims. Gate panel shows all artifacts produced, inner loop summary, resolved decisions, and agent activity. Gate actions (Advance / Revise / Re-run) work.
- [ ] Plan Composer: human can drag stage blocks onto canvas, connect them (incompatible connections blocked), and press "Start Project" to create phase records and begin orchestration.
- [ ] `product-manager` skill: emits `poe:review` to `senior-engineer` on every plan; conditionally emits `poe:review` to `architecture-analyst` per trigger rules; handles `ReviewResult` correctly; escalates via `poe:decision` on reviewer conflict.
- [ ] `senior-engineer` skill: correctly handles `plan_review` task type; emits `review-{review_id}.md` with a valid verdict; completes with `poe:done`.
- [ ] **Self-validation**: a product-manager task run via POE produces a task DAG visible in the Phase × Scope Matrix, with a senior-engineer review artifact in the Artifact Viewer.
- [ ] Knowledge Register panel: browsable, filterable by phase and author, keyword-searchable. Human can update an entry; promoted entries are written to `~/.poe/knowledge/`.
- [ ] Artifact Viewer: "Compare with previous" button appears for multi-revision artifacts; split-pane diff renders correctly with line-level highlighting.
- [ ] Queue Advisor chatbot: present in Pane 3 bottom half; `start_advisor_session` dispatches advisor skill via SF-1; `poe:advisor` turns render in advisor panel via `poe://advisor-turn`; `respond_to_advisor` resumes via SF-4; advisor does not resolve queue items autonomously.
- [ ] Task detail panel: opens on matrix cell click; shows WBS ancestry, event log, dependency chain; node-scoped advisor conversation available; mode guard reads `task.skill_modes` and disables conversation for autonomous-only skills.
- [ ] All new Tauri commands registered in `lib.rs` and callable without error.
- [ ] `cargo check` passes. TypeScript build passes. No `any` types introduced.

---

## 10. Protocol Elements

New protocol elements introduced in Phase 3. Authoritative definitions are in `doc-POE/Protocol.md` and `doc-POE/Flows.md` — this section is a summary for implementors.

### New event: `poe:advisor`

```jsonc
{"poe": "advisor", "content": "...", "id": "a1"}
// id is optional for single-turn sessions
```

Emitted by the advisor skill instead of `poe:chat`. Structurally identical but routes to a different surface. Ingester writes to `advisor_turns`; emits `poe://advisor-turn`. Sets `yield_reason='advisor'` when followed by `poe:yield`. See Protocol.md §2.

### New Tauri event: `poe://advisor-turn`

```
payload: {turn_id: string, task_id: string, content: string}
```

Emitted by the ingester when a `poe:advisor` event is processed. Frontend listens to append turns to the advisor panel in Pane 3 (and in the task detail panel when a node-scoped advisor session is active).

### New yield_reason: `'advisor'`

`nodes.yield_reason` gains a new value: `'advisor'`. SF-4 handles it identically to `'chat'` — `--resume` with `Human: {response}`. The distinction is which table is queried for the responded turn (`advisor_turns` vs `chat_turns`).

### New column: `nodes.skill_modes TEXT`

JSON array of modes from the skill's frontmatter `modes:` field, written at SF-1 dispatch time. Example: `'["autonomous","interactive"]'`. Frontend reads this from the task record — no Tauri call required. NULL is treated as `'["autonomous"]'` (safe default for tasks dispatched before Phase 3).

### Plan Composer flow

Two commands, called in sequence by the Plan Composer "Run" button:
1. `create_phase(project_id, name, stage_type, number)` — called N times, all phases created as `status='pending'`
2. `activate_phase(first_phase_id)` — called once; sets Phase 1 to `status='running'`; triggers orchestrator

See Flows.md SF-5 for the full sequence.

### Knowledge promotion format

`~/.poe/knowledge/{key}.md` — one file per promoted entry. Content is the raw `content` field from the knowledge row. The `key` is the knowledge entry's `key` field (human-readable slug, unique per project). Bundle assembly merges user-level entries from this directory with project SQLite entries; project-level takes precedence on key collision.
