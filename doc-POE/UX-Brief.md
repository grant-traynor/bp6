# POE — UX Design Brief

**Status**: Draft
**Last updated**: 2026-03-07

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
- Task name and WBS ancestry
- Event type: brief / step / artifact produced / decision raised / done
- Timestamp

Clicking an entry drills into the agent's PTY session for raw output if needed. Time filters carried forward from bp6: last hour, last 6 hours, since phase start.

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

Each project has a persistent background terminal session — the bp6 tmux pattern carried forward. Auto-resumed on app restart, auto-switched when the active project changes. Available as a panel or full-screen from the project toolbar.

The terminal is for code review and manual investigation — not for launching agents. The orchestrator handles agent launching.

---

## Stage Gate UI

When a phase reaches a human gate (end of Planning, end of Execution, Retrospective review), the project card in Pane 1 shows a clear gate indicator. The Phase × Scope Matrix dims the next phase until the gate is cleared. The gate action appears prominently in the project header — not buried in a menu.

Gate actions are simple: Advance / Revise / Re-run. The human makes the call; the orchestrator responds.

---

## Key UX Principles

1. **The queue is never buried.** If something needs the human, it is visible immediately without hunting.
2. **Agents are assigned, not launched.** The human does not choose a specialist or open a terminal. The orchestrator does this. The human sees what's running.
3. **The matrix is the map.** Phase × Scope gives orientation in seconds. Drill down for detail.
4. **Observation is the default mode.** The human watches; the agents work. Intervention is the exception.
5. **The Advisor is always at hand.** Not a separate screen — present in the queue panel, available for any question.
6. **PTY is a drill-down, not the surface.** Raw terminal output is one click away but never the primary view.
7. **Manual override is always possible.** The DAG can be edited directly. The orchestrator responds immediately.
