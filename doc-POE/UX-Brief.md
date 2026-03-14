# POE — UX Design Brief

**Status**: Draft
**Last updated**: 2026-03-13 (rev 2026-03-13: spec corrections from architecture review)

---

## Design Intent

POE is not a project management tool. It is a human oversight interface for autonomous agents executing work. The UX is built around one core truth: **the human's job is to supervise, unblock, and steer — not to do.**

Five things the interface must always make clear:

1. **Scope** — what are we building (WBS hierarchy)
2. **Phasing** — where we are in the PDCA cycle
3. **Logic** — what depends on what (dependency chain)
4. **Assignment** — which specialist is on it, is it running
5. **Visibility** — what's happening right now, what needs the human

---

## Primary Layout

Two panes. Always visible. No hidden tabs for critical information.

```
┌─────────────────────────────────────────────────────────────────┐
│  Global Bar: [agents running: 7/15]  [queue: 3 items]           │
├──────────────┬──────────────────────────────────────────────────┤
│              │                                                   │
│   Projects   │              Selected Project                     │
│   Overview   │                                                   │
│              │  ┌─────────────────────────┐  ┌───────────────┐  │
│  ┌─────────┐ │  │                         │  │  Queue        │  │
│  │ Proj A  │ │  │   Phase × Scope Matrix  │  │               │  │
│  │ 3 queue │ │  │                         │  │  [Question 1] │  │
│  │ 5 agents│ │  │                         │  │  [Question 2] │  │
│  │ 12 tasks│ │  │                         │  │               │  │
│  └─────────┘ │  │                         │  │  ┌──────────┐ │  │
│              │  │                         │  │  │ Advisor  │ │  │
│  ┌─────────┐ │  └─────────────────────────┘  │  │ Chatbot  │ │  │
│  │ Proj B  │ │                               │  └──────────┘ │  │
│  │ 0 queue │ │  [ Activity Feed ]            └───────────────┘  │
│  │ 3 agents│ │                                                   │
│  │ 8 tasks │ │                                                   │
│  └─────────┘ │                                                   │
│              │                                                   │
└──────────────┴──────────────────────────────────────────────────┘
```

---

## Pane 1 — Projects Overview (left)

A scrollable list of all open projects. Each project is a summary card showing:

- Project name and current phase
- PDCA status indicator (Plan / Do / Check / Act)
- Queued questions count — highlighted if > 0
- Active agents count / per-project limit (e.g. `5 / 5`)
- Tasks remaining in current phase

Clicking a project selects it and populates Pane 2. Visual urgency cues: a project with queued questions draws the eye. A project running at full concurrency with no queue is healthy — muted treatment.

---

## Pane 2 — Selected Project (main)

Three sections within the selected project view.

### 2a. Phase × Scope Matrix

The primary orientation view. Not a Gantt chart. Not a Kanban board.

```
              Phase 1 (Do)     Phase 2 (Plan)    Phase 3
──────────────────────────────────────────────────────────
Epic A        ████ ████ ░░░░   [ planned ]
  Feature A1  ████ done
  Feature A2  ████ running ●
  Feature A3  ░░░░ pending

Epic B        ████ ████        [ planned ]       [ scoped ]
  Feature B1  ████ done
  Feature B2  ████ running ●
──────────────────────────────────────────────────────────
```

- **X axis** — phases (time / PDCA progression)
- **Y axis** — scope (Epic → Feature → Task, collapsible)
- **Within each cell** — task status, dependency indicators, assigned skill badge, running agent indicator (●)
- **Phase header** — shows PDCA stage and human gate status (awaiting human / running / complete)

**Task status visual states** (all must be visually distinct in the matrix cell):

| Status | Indicator | Meaning |
|---|---|---|
| `pending` | `░░░░` | Not yet started |
| `running` | `████ ●` | Agent actively executing |
| `waiting` | `⏸ skill-name` | Agent yielded — awaiting review or decision result |
| `done` | `████` muted | Complete |
| `cancelled` | `────` strikethrough | Cancelled, preserved in history |

`waiting` must be visually distinct from both `running` and `pending`. A task that shows `waiting` tells the human: the agent has done its part and handed off — the orchestrator is managing the continuation. No human action required unless the reason is `decision` (in which case the queue panel shows the pending item).

**Skill-author tasks — meta-task presentation**:

Skill-author tasks are infrastructure tasks automatically dispatched by the orchestrator when a required skill is missing. They are not work tasks — they are system self-healing. They must be visually distinct from both implementation and review tasks:

- **Icon**: wrench or gear icon (e.g. `⚙`) in the task badge instead of the assigned-skill badge used by work tasks
- **Colour**: muted / secondary colour (e.g. desaturated blue-grey) — not the accent colour used for work task progress
- **Collapsible / hidden by default**: skill-author nodes are collapsed in the matrix by default; a toggle ("Show meta-tasks") reveals them. When hidden, the rows they would occupy are not shown, preserving visual density for work tasks
- **Phase completion metrics**: skill-author tasks do **not** count toward phase completion. The phase progress bar and task counters in the project summary card exclude them entirely. Only work tasks (implementation, review) count.

Example matrix cell appearance when visible:

```
  Feature A2  ████ running ●
  ⚙ skill:    ░░░░ pending   [author-skill: deploy-validator]
  Feature A3  ░░░░ pending
```

The `⚙` prefix and muted colour signal to the human: this is a housekeeping task the system is managing — not a deliverable. No action required.

Clicking a task opens a detail panel: full description, WBS ancestry, skill assignment, dependency chain, agent brief (`poe:brief`), event log, and the option to open a node-scoped agent conversation.

### 2b. Activity Feed Panel

A live, structured stream of agent events across the selected project — not raw PTY output. Built from the `poe:` event log.

**Panel behaviour:**
- **Default state**: open (visible). The operator should see activity without any manual action.
- **Collapsible**: a toggle button (chevron) collapses or expands the panel. State is per-session.
- **Position**: bottom of the main content area, below the Phase × Scope Matrix.
- **Rolling feed**: capped at 50 entries — oldest entries are dropped when the cap is reached.
- **Auto-scroll**: the feed scrolls to the latest entry on every new event.
- **Purpose**: operator reassurance that the system is alive and making progress. The primary liveness signal during long agent runs (e.g. a task running for 10+ minutes should show poe:step entries throughout, not silence).

Each entry shows the following fields. All fields are **mandatory** — missing any one of them makes the feed opaque to the human:

| Field | Content | Notes |
|---|---|---|
| **Timestamp** | HH:MM:SS | Local time |
| **Skill name** | The `skill_id` of the agent that emitted this event | e.g. `must-not-analyst`, `senior-engineer`. Required — without it, the human cannot tell which kind of agent is acting |
| **Task title** | Short title of the WBS node this agent is working on | e.g. `Develop Guardrails`. Required — without it, the human cannot tell *what* the agent is doing |
| **Event type badge** | Colour-coded category (brief = blue, step = neutral, agent-started = purple, agent-exited = slate/red, task-done = emerald) | |
| **Content** | Step name or brief text, where applicable | Omitted for agent-start / agent-exit entries |
| **WBS ancestry** | Parent feature → epic → phase (secondary line or hover) | Provides the "why" chain |
| **Model badge** | Model override if skill declares one | Omitted when skill uses default |

Clicking an entry opens the **agent session handover** — an xterm.js panel that resumes the agent's Claude session (`claude --resume <session_id>`) in a PTY, bridged to the browser via WebSocket. The human can read the raw conversation, ask follow-up questions, or assist an agent that raised a decision. Closing the panel does not terminate the agent's session — the session_id persists in SQLite.

**Event sources and feed entry types:**

| Tauri event | Feed entry type | Content |
|---|---|---|
| `poe-agent-activity` (type: `brief`) | `poe:brief` | `{skill_name} · {task_title}` — agent's interpretation of its task |
| `poe-agent-activity` (type: `step`) | `poe:step` | `{skill_name} · {task_title} — {step_name}` — primary liveness signal |
| `poe-agent-started` | `agent-start` | `{skill_name} started · {task_title}` with model badge |
| `poe-agent-exited` | `agent-exit` | `{skill_name} exited (success/failed) · {task_title}` |
| `poe-task-done` | `poe-task-done` | `Task done: {task_title} ({skill_name})` |
| `poe-artifact-created` | `poe:artifact` | `Artifact: {filename} — {skill_name}` |
| `poe-knowledge-created` | `poe:knowledge` | `Knowledge: {key}` |

The `poe-agent-activity` event is emitted directly by the `EventIngester` for each `poe:step` and `poe:brief` line the agent writes to stdout. This ensures the feed updates in real time without polling — the frontend receives entries as soon as the agent emits them.

**Yield entries**: `poe:yield` must produce an activity feed entry. Without it, the human sees a task go from `running` to `waiting` with no explanation.

| yield_reason | Feed entry text |
|---|---|
| `review` | `{skill_name} · {task_title} — Yielded, awaiting review from {reviewer_skill}` |
| `decision` | `{skill_name} · {task_title} — Yielded, awaiting human decision` |
| `chat` | `{skill_name} · {task_title} — Awaiting your response` |
| `advisor` | `{skill_name} · {task_title} — Advisor session active` |

**Skill-author events in the activity feed**:

Skill-author dispatch and completion are surfaced in the feed as system-level events — visually distinct from work task events. They carry a `system` tag and are rendered in a muted style (e.g. italic text, secondary colour, no agent avatar):

| Trigger | Feed entry text |
|---|---|
| Skill-author task dispatched | `⚙ [system] Authoring missing skill: {skill_name} — {N} task(s) blocked` |
| Skill-author task complete | `⚙ [system] Skill authored: {skill_name}.md — {N} task(s) unblocked` |

The dispatch entry explains why execution paused — without it, the human sees tasks sit in `waiting` with no cause. The completion entry is a positive signal: the system self-healed and blocked tasks are now eligible to run. Together they close the glass-box gap for infrastructure events.

Styling rules:
- No agent avatar / skill badge — system events are not agent turns
- `[system]` label replaces the skill badge, rendered in a muted secondary colour
- The `⚙` prefix mirrors the matrix meta-task icon for visual consistency
- System events may be filtered out via a "Hide system events" toggle in the feed toolbar (off by default — they should be visible)

The activity feed is the glass box. It answers: *is everything running as expected?*

### 2c. Concurrency Indicator

Visible within the project header: `● 5 / 5 agents` (running / limit). Clicking opens concurrency settings. The global indicator lives in the global bar.

**Data source**: the running count is `SELECT COUNT(*) FROM agents WHERE project_id=? AND status='running'`. The limit comes from the project's configured concurrency setting. The frontend receives updates via `poe-agent-started` and `poe-agent-exited` Tauri events — it increments/decrements the displayed count on each event rather than re-querying. On initial project load, the count is hydrated from the full state snapshot.

**Correctness requirement**: the count must reflect live processes only. Ghost agent rows (processes that died without cleanup) inflate the count and suppress task dispatch. Ghost-agent recovery at project-open time (Architecture.md §Recovery) keeps the `agents` table clean. The indicator is only trustworthy if ghost recovery runs before it is first displayed.

---

## Pane 3 — Queue + Advisor (right)

Always visible when the project is selected. Never in a tab.

### Decision Queue

> **Scope**: The Decision Queue handles `poe:decision` events from **autonomous agents** only — exception escalations where an agent cannot proceed without a human call. Collaborative agent turns (`poe:chat`) route to the Artifact Viewer chat panel and do not appear here.

Each queue item shows:
- The question raised by the agent
- The task and WBS context it came from
- Candidate options if the agent provided them
- Time waiting

The human can resolve directly (select an option or type a response), or engage the Advisor first.

### Conversational Queue Items

An autonomous agent may raise a series of sequential `poe:decision` events before it has enough context to proceed — each round building on the human's previous answer. This is rare but valid: a planning specialist might ask one structural question, receive an answer, discover a dependent question, and yield again. Each round is a separate yield/resume cycle; the agent sees the full session history each time.

When a task has prior resolved decisions AND a current pending decision, the queue item renders in **conversation thread mode**:

```
┌─ Plan Scope Clarification — product-manager ────────────┐
│                                                          │
│  ● Q1: Should Phase 2 include the admin portal?          │
│    A: No — defer to Phase 3.                             │
│                                                          │
│  ● Q2 (pending): Does the deferral affect the           │
│    auth model planned for Phase 2?                       │
│                                                          │
│  [ type your response...                              ]  │
│                                              [ Send ↵ ]  │
└──────────────────────────────────────────────────────────┘
```

Prior Q+A pairs are shown read-only above the current question. The current pending question is highlighted. The input is a free-text field (no option buttons — conversational questions rarely have enumerable options).

Detection: if `queue_items` table has > 0 resolved rows for this `task_id` AND 1 pending row, render in thread mode. Otherwise render as standard queue item.

> **Distinct from collaborative artifact building**: multi-round `poe:decision` in the Decision Queue is for autonomous agents encountering sequential structural blockers. CONOPS elicitation and other co-authoring sessions use `poe:chat` and appear in the Artifact Viewer — not here. See §Artifact Viewer and Flows.md §3.8.

### Queue Advisor Chatbot

Persistent chat interface associated with the queue. The human can direct it to research before deciding:

- *"What do the architecture constraints say about this?"*
- *"Has this come up in the knowledge register before?"*
- *"Check if library X supports feature Y."*

The Advisor has full access to the project's artifact corpus, knowledge register, and DAG context. It does not resolve queue items — it informs human decisions. The human resolves.

The Advisor is also available outside of queue items — for general project questions, scope exploration, or checking the state of the plan.

---

## Node-Scoped Agent Conversation

Any WBS node (epic, feature, task, bug) can have a specialist conversation opened against it. The conversation is pre-loaded with:

- The node's full context (title, description, WBS ancestry)
- Relevant artifacts and knowledge register entries
- The node's current status and agent brief (if an agent has run)

This is the bp6 pattern carried forward. It handles detailed design discussions, bug investigation, and the cases where the human needs to think through something with an expert before the orchestrator runs it.

**Mode is interactive.** The orchestrator prepends the interactive mode protocol block to the bundle. The conversation is multi-round — the human asks, the specialist responds.

**Mode guard**: if the task's assigned skill declares `modes: [autonomous]` only, the "Open conversation" button is disabled with the tooltip: *"This skill runs autonomously. View its activity feed to see what it did."* The human can still open the agent session handover (PTY resume) to read the raw session, but cannot initiate a new interactive conversation with that skill.

**Mode is implicit — no selector.** The human never chooses autonomous vs. interactive. Opening a conversation always means interactive. Scheduling via the orchestrator always means autonomous. The UX does not expose this distinction beyond the mode guard on incompatible skills.

---

## Manual DAG Editing

The Phase × Scope Matrix supports direct editing:

- Drag to reorder tasks within a phase
- Add / remove dependency edges
- Edit task title, description, skill assignment
- Cancel or re-open tasks
- Move tasks between phases

Manual edits trigger the orchestrator loop immediately. If an edit unblocks a task, the orchestrator picks it up. If an edit cancels a running task, the agent is interrupted.

This is the safety valve for when agents get the structure wrong.

---

## Project Terminal (Tmux)

Each project has a persistent background shell session — the bp6 tmux pattern carried forward. Auto-resumed on app restart, auto-switched when the active project changes. Available as a panel or full-screen from the project toolbar.

The project terminal is for code review, running commands, and manual investigation — not for launching agents. The orchestrator handles agent launching.

**Distinct from agent session handover**: the project terminal is a general-purpose shell (tmux). The agent session handover (accessible from any activity feed entry) is a `claude --resume` PTY connected specifically to one agent's Claude session. They are two separate surfaces. Do not conflate them in the implementation.

---

## Plan Composer

When a project is created (or when the human advances to plan a new phase), the Plan Composer replaces the Phase × Scope Matrix in Pane 2.

```
┌─ Plan Composer ───────────────────────────────────────────┐
│                                                            │
│  Stage Library          Plan Canvas (drag to add)         │
│  ─────────────          ────────────────────────          │
│  [ CONOPS         ]  →  [ CONOPS ] ──→ [ Guardrails ]    │
│  [ Guardrails     ]                        ↓              │
│  [ Increment Plan ]              [ Increment Planning ]   │
│  [ Execution      ]                        ↓              │
│  [ Plan Review    ]              [ Plan Review ]          │
│  [ Validity Anal. ]                        ↓              │
│  [ Retrospective  ]              [ Execution ]            │
│  [ Onboarding     ]                                       │
│                                  [ Validate Stage Plan ]  │
│                                  [ Start Project ]        │
└────────────────────────────────────────────────────────────┘
```

- Stage library shows all available stage types with their declared input/output artifact connectors.
- Canvas supports drag-and-drop composition; stages snap together when their artifact connectors are compatible.
- The validator checks that all declared input connectors are satisfied by upstream stage outputs before allowing execution to start.
- "Start Project" finalises the plan and creates the phase records in SQLite. The orchestrator begins with the first phase.

This is the primary entry point for all new projects. The Onboarding stage type handles the case where the human is joining an existing project mid-flight.

---

## Artifact Viewer

The Artifact Viewer is a single context-sensitive surface — read-only by default, chat-active when a collaborative session is in progress or the human initiates one.

Accessible from:
- The Phase × Scope Matrix — artifact icon on any completed phase
- The Stage Gate UI — "Review outputs" button before advancing
- The Advisor Chatbot — when an artifact is referenced in a conversation
- Activity feed entries with `poe:chat` events — opens directly in chat-active state

### Read-only state (default)

The viewer shows the artifact's content rendered from its markdown source, with metadata: type, producing task, phase, timestamp. Side-by-side comparison is available for versioned artifacts (e.g. revised `conops.md` vs. original).

Artifacts are produced by agents — they are not directly edited by the human. If the human wants to change an artifact, they either revise the task that produced it (Stage Gate: Revise) or initiate a chat session with the producing agent.

A **"Chat about this"** button in the viewer toolbar activates the chat panel. This transitions the viewer to chat-active state and dispatches a `poe:chat` session against the artifact's producing task (or a new specialist task if the producing task is complete).

### Chat-active state

Activated by either:
- **Agent-initiated**: the agent running a task emits `poe:chat` — the Artifact Viewer opens automatically in chat-active mode for that task's artifact
- **Human-initiated**: the human clicks "Chat about this" on any artifact in read-only state

```
┌─ Artifact Viewer ──────────────────────────────────────────────────────┐
│  [← Back to Matrix]   conops.md — in progress               [Close]   │
│ ┌───────────────────────────────────┬────────────────────────────────┐ │
│ │  Artifact (live)                  │  Chat                          │ │
│ │                                   │                                │ │
│ │  # Project CONOPS                 │  ● Analyst                     │ │
│ │                                   │  What problem does this        │ │
│ │  ## Problem Statement             │  system solve?                 │ │
│ │  [generating...]                  │                                │ │
│ │                                   │  ● You                         │ │
│ │  ## Users                         │  It's a tool for managing      │ │
│ │  ...                              │  agentic workflows...          │ │
│ │                                   │                                │ │
│ │                                   │  ● Analyst                     │ │
│ │                                   │  Who are the primary users?    │ │
│ │                                   │                                │ │
│ │                                   │  [ type your response...    ]  │ │
│ │                                   │                    [ Send ↵ ]  │ │
│ └───────────────────────────────────┴────────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────┘
```

**Left panel — artifact**: the document's current markdown content, rendered live. Updates whenever the agent emits `poe:artifact`. Shows `[generating...]` in sections not yet written. Read-only — artifact content is shaped through conversation, not direct human edit.

**Right panel — chat**: agent messages arrive via `poe:chat` events and display as agent turns. The human types in the input field and submits. Full scroll history with timestamps.

**Session persistence**: closing the viewer does not end the session. The agent's `session_id` and full conversation history (`chat_turns`) persist in SQLite. Re-opening resumes from the last turn.

**Completion**: when the agent emits `poe:done`, the viewer shows a completion banner with "Return to Matrix" and "View Artifact" options. The produced artifact is now part of the project corpus — accessible from the artifact browser and injectable into subsequent task bundles.

**Distinction from the Decision Queue**: the Artifact Viewer chat panel handles `poe:chat` events — sustained co-authoring of a specific artifact with a specific agent. The Decision Queue handles `poe:decision` events — exception escalation from autonomous agents. A task cannot appear in both simultaneously.

---

## Knowledge Register Panel

Accessible from the project toolbar (persistent icon, never in a tab for active projects).

The panel shows all knowledge entries for the current project:
- Key / value pairs in a table
- Filter by phase, author (agent or human), search by keyword
- Click an entry to see the full value and any entries it supersedes
- Entries are read-only in the panel — humans do not directly edit the register. Agents write via `poe:knowledge`. The human's role is to read, search, and promote.

**Promote to user-level**: entries can be flagged for promotion — this signals the Retrospective stage to export the entry to `~/.poe/knowledge/` so it applies across all projects. Promotion is logged and reversible.

---

## Stage Gate UI

When a phase reaches a human gate (end of Planning, end of Execution, Retrospective review), the project card in Pane 1 shows a clear gate indicator. The Phase × Scope Matrix dims the next phase until the gate is cleared. The gate action appears prominently in the project header — not buried in a menu.

Gate actions: **Advance / Revise / Re-run**. The human makes the call; the orchestrator responds.

**Before acting on a gate, the human reviews the stage outputs.** The gate panel shows:
- All artifacts produced by the completed stage, with direct links to the Artifact Viewer
- A summary of the inner loop state (how many plan review iterations ran, whether any review blocked)
- Open decisions resolved during this stage
- Agent activity summary (briefs and step counts)

The human is expected to read the artifacts before advancing. The gate should not be a rubber stamp — it is the quality checkpoint the CONOPS describes.

**Retrospective gate** — additional affordance: the Retrospective stage produces updated skill files and knowledge entries. The gate panel for a Retrospective shows a diff of skill changes for human review and one-click approval. Approved skill changes are written to `{project}/.poe/skills/`. The human can promote to `~/.poe/skills/` from the same panel.

---

## Agent Interrupt

Three levels of interrupt, available from the project header:

| Action | Effect |
|---|---|
| **Cancel task** | Cancel one running task. Agent receives SIGTERM. Task re-queues to Pending. Available from task detail panel. |
| **Pause stage** | SIGTERM all running agents in the current stage. Tasks re-queue to Pending. Stage enters Paused state. Human can resume or revise. |
| **Abort project** | SIGTERM all running agents across the project. All Running tasks → Pending. Project enters Paused state. Human reviews before restarting. |

Interrupt actions are confirmed with a brief modal. The confirmation shows the count of agents that will be stopped. Abort is never silent.

On resume after interrupt: the orchestrator attempts `--resume` for all previously-running tasks using their stored session IDs. If resume fails (session expired), tasks restart fresh from Pending.

---

## Key UX Principles

1. **The queue is never buried.** If something needs the human, it is visible immediately without hunting.
2. **Agents are assigned, not launched.** The human does not choose a specialist or open a terminal. The orchestrator does this. The human sees what's running.
3. **The matrix is the map.** Phase × Scope gives orientation in seconds. Drill down for detail.
4. **Observation is the default mode.** The human watches; the agents work. Intervention is the exception.
5. **The Advisor is always at hand.** Not a separate screen — present in the queue panel, available for any question.
6. **Agent session handover is a drill-down, not the surface.** Clicking into an agent's xterm.js session is one click away from any activity feed entry, but it is never the primary view. The structured event stream (poe: events) is what drives the UI.
7. **Manual override is always possible.** The DAG can be edited directly. The orchestrator responds immediately.
