# POE — UX Design Brief

**Status**: Draft
**Last updated**: 2026-03-08

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

Clicking a task opens a detail panel: full description, WBS ancestry, skill assignment, dependency chain, agent brief (`poe:brief`), event log, and the option to open a node-scoped agent conversation.

### 2b. Activity Feed

A live, structured stream of agent events across the selected project — not raw PTY output. Built from the `poe:` event log.

Each entry shows:
- Agent identifier and assigned skill
- Model override, if the skill declares one (e.g. `[claude-opus-4-6]`) — omitted when the skill uses the default
- Task name and WBS ancestry
- Event type: brief / step / artifact produced / decision raised / done
- Timestamp

Clicking an entry opens the **agent session handover** — an xterm.js panel that resumes the agent's Claude session (`claude --resume <session_id>`) in a PTY, bridged to the browser via WebSocket. The human can read the raw conversation, ask follow-up questions, or assist an agent that raised a decision. Closing the panel does not terminate the agent's session — the session_id persists in SQLite. Time filters carried forward from bp6: last hour, last 6 hours, since phase start.

The activity feed is the glass box. It answers: *is everything running as expected?*

### 2c. Concurrency Indicator

Visible within the project header: `● 5 / 5 agents` (running / limit). Clicking opens concurrency settings. The global indicator lives in the global bar.

---

## Pane 3 — Queue + Advisor (right)

Always visible when the project is selected. Never in a tab.

### Decision Queue

Each queue item shows:
- The question raised by the agent
- The task and WBS context it came from
- Candidate options if the agent provided them
- Time waiting

The human can resolve directly (select an option or type a response), or engage the Advisor first.

### Conversational Queue Items

Some agents (specifically the operational-analyst during CONOPS elicitation) conduct a multi-round conversation via sequential `poe:decision` events. Each round builds on the previous answers — the human needs the full exchange in view to answer round 3 coherently.

When a task has prior resolved decisions AND a current pending decision, the queue item renders in **conversation thread mode**:

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

Prior Q+A pairs are shown read-only above the current question. The current pending question is highlighted. The input is a free-text field (no option buttons — conversational questions rarely have enumerable options).

Detection: if `decisions` table has > 0 resolved rows for this `task_id` AND 1 pending row, render in thread mode. Otherwise render as standard queue item.

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

> **Gap addressed**: The CONOPS states "A project plan is a human-composed DAG of stage instances." The UX-Brief previously described the Phase × Scope Matrix for viewing an existing plan, but not how a human composes one.

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

> **Gap addressed**: The CONOPS describes artifacts as "the connectors between stages" and "structured documents that define the product." The UX-Brief previously did not describe a UI for reading artifacts.

Accessible from:
- The Phase × Scope Matrix — artifact icon on any completed phase
- The Stage Gate UI — "Review outputs" button before advancing
- The Advisor Chatbot — when an artifact is referenced in a conversation

The viewer shows the artifact's content rendered from its markdown source, with metadata: type, producing task, phase, timestamp. Side-by-side comparison is available for versioned artifacts (e.g. revised `conops.md` vs. original).

The artifact panel is a read-only viewer. Artifacts are produced by agents — they are not directly edited by the human. If the human wants to change an artifact, they either revise the task that produced it (Stage Gate: Revise) or create a new task.

---

## Knowledge Register Panel

> **Gap addressed**: Architecture.md describes the knowledge register as "surfaced in the UI as a browsable, searchable panel." This was absent from the UX-Brief.

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

> **Gap addressed**: The CONOPS describes the human being able to "pull the cord if something is going wrong." The UX-Brief previously described task cancellation (for individual tasks) but no emergency stop.

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
