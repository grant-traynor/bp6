# POE — Pairti Orchestration Engine: Concept of Operations


**Status**: Draft
**Last updated**: 2026-03-15 (rev 2026-03-15: DAG Service replaces poe:task/poe:edge; reviewers read DAG directly; Phase/Stage model clarified — phases are iterations, stages are process steps within a phase)

---

## Overview

```mermaid
graph TB
    Human["Human"] -->|"defines scope"| Plan

    subgraph Plan["Project Plan — a sequence of Phases"]
        direction LR
        PA["Phase 1\n(Initial Prototype)"] --> PB["Phase 2\n(Feature A)"]
        PB --> PC["Phase 3\n(Feature B)"]
    end

    subgraph Phase["Inside Every Phase"]
        direction TB
        subgraph Stages["Stages (process steps)"]
            direction LR
            S1["Increment\nPlanning"] --> S2["Execution"] --> S3["Retrospective"]
        end
        subgraph WBS["Phase Scope (DAG)"]
            direction TB
            Planner["Product Manager\n(creates task DAG via DAG Service)"]
            Planner --> T1["Task — Skill A"]
            Planner --> T2["Task — Skill B"]
            T1 & T2 --> Art["Artifacts\n(docs, code, data)"]
        end
        S1 -->|"builds"| WBS
        S2 -->|"executes"| WBS
    end

    Art -->|"injected as context"| NextPhase["Next Phase"]

    subgraph Oversight["Human Oversight"]
        Feed["Activity Feed\n(glass box)"]
        Queue["Decision Queue\n(agent asks for help)"]
    end

    T1 & T2 -.->|"structured events"| Feed
    T1 & T2 -.->|"raise questions"| Queue
    Human -.->|"observes"| Feed
    Human -.->|"resolves"| Queue
```

---

## What is POE?

POE is a human-in-the-loop agentic coding and orchestration tool that provides full lifecycle oversight — from initial concept through to delivery.

The human's role changes across the lifecycle:

- **Before execution**: collaborative, high-bandwidth. The human seeds the idea, shapes the plan, defines constraints. This front-loading is the primary quality investment — clarity here directly determines how well agents execute later.
- **During execution**: low-bandwidth, observational. Agents run autonomously. The human can inspect activity at any time (glass box) and can pull the cord if something is going wrong. The human resolves agent questions when they arise, but this should be rare if the preconditions were good.
- **Between stages**: review and gate. The human reviews what was produced and decides whether to advance, revise, or re-run.

The goal is to make autonomous execution succeed by setting it up well, not to supervise it step-by-step.

---

## The Plan Model

A project plan is a **sequence of phases** — each phase a meaningful increment of the total scope. Within each phase, the orchestrator progresses through a series of **stages**: planning the work, executing it, validating it, and learning from it.

Key properties:

- **Phases are scope increments.** "Initial Prototype", "Feature A", "Performance Hardening" are phases. A phase defines a chunk of the product to build. Its scope is expressed as a WBS: epics → features → tasks.
- **Stages are process steps.** Within each phase, the team moves through stages: `increment_planning` to decompose the scope into a task DAG, `execution` to build it, `retrospective` to learn from it. Stage types have defined input connectors (artifacts they need) and output connectors (artifacts they produce).
- **Non-linear by design.** The stage sequence within a phase is not fixed. You can skip stages, repeat them, or compose them in any valid dependency structure. An `onboarding` stage can be inserted at the front of any phase to orient a new agent team.
- **One plan per project, growing over time.** The plan grows as the project progresses. Completing a phase informs the scope of the next.

---

## Core Primitives

### Phase

A phase is a meaningful increment of the total project scope. "Initial Prototype", "Feature A", "Performance Hardening" are phases. A phase owns a work breakdown structure (WBS): its epics, features, and tasks. The phase's scope is decomposed during planning and executed during execution.

Phases progress through stages. A phase is complete when all its stages have been worked through and any final human gate resolved.

### Stage

A stage is a process step within a phase. It is self-contained: it knows what it needs, what it does, and what it produces. Stages compose multiple specialists working in sequence.

Every stage follows the same internal pattern:

1. **Planning specialist** reads the stage definition and all available input artifacts, then produces a task list with specialist assignments (for execution stages, this task list is the phase WBS built via the DAG Service).
2. **Specialist agents** execute the tasks, possibly in parallel where dependencies allow.
3. **Artifacts** are produced as tasks complete.
4. **Human gate** (if the stage defines one): human reviews outputs and decides to advance, revise, or re-run.

A stage is complete when its declared output artifacts exist and any human gate has been resolved.

### Artifact

An artifact is a structured document produced by a stage — a CONOPS, an architecture doc, a design system, a task plan, a code module, a review report. Artifacts are the connectors between stages.

When a stage starts, context resolution automatically gathers all available artifacts that match the stage's declared inputs, regardless of which prior stage produced them. There is no hardcoded ordering — the artifact graph is the source of truth.

Artifacts live in the project directory (`docs/`, or alongside code) and are tracked in the project database with metadata (type, producing stage, timestamp).

### Task

A task is a discrete unit of work within a stage, assigned to a single specialist. Tasks are:

- **Small and focused** — a specialist should be able to execute a task without needing to resolve ambiguity.
- **Dependency-aware** — tasks within a stage can have dependencies; independent tasks run in parallel.
- **Artifact-producing** — every task has a declared output (a document, a code change, a review).

Tasks are created by the planning specialist during the `increment_planning` stage, not predefined. The plan knows what phases and stages to run; the planning specialist decides how to decompose each phase's scope into tasks given the current context. Tasks belong to the phase, not the stage — they persist across stage transitions and are dispatched during the `execution` stage.

### Skill

A skill is a specialist definition: the role the agent plays, how it behaves, what it emits, what it knows. Skills are markdown files with structured frontmatter, loaded from a priority chain:

1. App bundle defaults
2. `~/.poe/skills/<skill-id>.md` — user-level overrides
3. `<project>/.poe/skills/<skill-id>.md` — project-level overrides

Skills improve over time. Any agent can emit `poe:skill` during execution to write a project-local skill file capturing a discovered pattern (e.g. "this project targets embedded devices — never propose cloud-native patterns"). The Retrospective stage does this systematically. The human can promote project-local improvements to the user level if they apply broadly.

---

## Human Oversight

### Glass Box

Agents run autonomously. The human is not blocking their progress. Instead, the human can look in at any time via a live activity feed that shows:

- What each active agent is currently doing
- The agent's interpretation of its task (written at task start, before execution begins)
- Structured progress events emitted by the agent as it works
- Which tasks are complete, in progress, or waiting

The activity feed is built on **structured events** emitted by agents via the stream-json transport, not raw terminal output. When deeper inspection is needed, the human can open an xterm.js terminal that resumes the agent's session directly — but this is a drill-down, not the primary signal.

### Decision Queue

Agents can raise questions to the human queue: *"I've encountered an ambiguity — here are my three options, I need a call."* The human resolves the question; the agent unblocks. Unrelated agents continue running in parallel.

The queue should be sparse. A busy queue is a signal that the stage's preconditions were insufficient.

---

## Built-in Stage Types

The following stage types form the initial library. They are the process steps available within any phase. Most phases will use a subset in roughly the order shown, but there is no enforced sequence.

Each phase runs two nested PDCA loops. Stage types map to these loops as follows — see Architecture.md for the full model.

**Setup (establishes the outer loop baseline):**

| Stage Type | Purpose | Key Specialists | Primary Output |
|---|---|---|---|
| Project Onboarding | Join or resume a project; synthesise existing artifacts into a shared understanding | Operational Analyst | `onboarding-summary.md` |
| CONOPS | Define the product concept, users, goals, and operational context | Operational Analyst | `conops.md` |
| Guardrails | Define architecture, interface contracts, data model, design system, user model, and must-nots | Architecture, Interface, Data Model, Design, User, Must-Not Analysts + Senior Engineer review | `architecture-constraints.md`, `interface-control.md`, `data-model.md`, `design-system.md`, `user-analysis.md`, `must-nots.md`, `guardrails-review.md` |

**Inner loop — Plan quality (closes before execution begins):**

| Stage Type | PDCA | Purpose | Key Specialists | Primary Output |
|---|---|---|---|---|
| Increment Planning | Plan | Select a meaningful next increment; decompose into epics, features, tasks, and dependency edges via the DAG Service | Product Manager | WBS: populated task graph in `dag.db` |
| Plan Review | Check | Specialists review the WBS *directly from the DAG* before execution: flag gaps, wrong skill assignments, missing dependencies, ambiguous tasks. Directed by the planning specialist via `poe:review` (specifying task IDs and focus area); reviewer reads the live task graph via DAG Service tools. Runs autonomously — no human relay required. | Architect, Engineer, PM (per plan type) | `phase-N-plan-review.md` |
| Plan Revision | Act | Planning specialist updates the DAG via DAG Service tools based on review findings. | Product Manager | Updated task graph in `dag.db` |

The inner loop may iterate — Review → Revise → Review again — until the plan is clean. If the loop cannot converge (reviewers disagree on a structural question, or blockers persist after N cycles) the planning specialist escalates via `poe:decision` and the human breaks the deadlock. Once approved, execution begins.

**Execution (Do — runs on the approved plan):**

| Stage Type | PDCA | Purpose | Key Specialists | Primary Output |
|---|---|---|---|---|
| Execution | Do | Run the approved task set autonomously | Task specialists (per task type) | Code + artifacts |

**Outer loop — Deliverable and process quality (closes after execution):**

| Stage Type | PDCA | Purpose | Key Specialists | Primary Output |
|---|---|---|---|---|
| Validity Analysis | Check | Validate what was built against the CONOPS; identify the gap between C' and C! | Validity Analyst | `phase-N-validity.md` |
| Rework | Act (targeted) | Address specific deliverable deficiencies found in validity analysis; minimal targeted task plan | Product Manager + task specialists | Updated artifacts (per task) |
| Retrospective | Act (systemic) | Root cause analysis on quality gaps; update skills, knowledge register, and guardrails to prevent recurrence | RCA Analyst | `phase-N-rca.md`, updated skills, updated knowledge |

---

## Agent Protocol

Agents interact with POE through two complementary channels:

- **DAG Service** — an MCP server embedded in POE and injected into every agent's tool set. Agents call DAG Service tools to create tasks, add edges, query the WBS, update nodes, and cancel work. The DAG Service commits changes to SQLite and notifies the orchestrator directly. This is the primary read/write interface for all project state. See Architecture.md §Agent Tooling (MCP) for the full tool surface.

- **`poe:` protocol** — structured JSON events embedded in the `--output-format stream-json` transport. The `poe:` protocol is **control flow and observability only**: progress milestones, decisions, reviews, and completion signals. It does not carry data. Agents are invoked with `claude --output-format stream-json -p --dangerously-skip-permissions`; the T+S+K bundle is written to stdin (then closed), and the orchestrator reads poe: events from the JSON output stream. See `doc-POE/Protocol.md §2` and `§5` for the full wire format and spawn model.

The DAG is not a static plan — it evolves as execution reveals new information, scope is refined, and dependencies are discovered or invalidated. Agents have full mutation rights over the graph via DAG Service tools.

### Artifacts & Knowledge

| Event | Operation | Purpose |
|---|---|---|
| `poe:artifact` | Create / Update | Produce or revise a named artifact (written to `docs/`, indexed in DB) |
| `poe:knowledge` | Create / Supersede | Write an entry to the knowledge register |
| `poe:skill` | Write | Write a reusable skill pattern to `{project}/.poe/skills/<name>.md` (project-level override, loadable immediately) |

### Execution & Oversight

| Event | Purpose |
|---|---|
| `poe:brief` | Agent's interpretation of its task — written before execution begins, non-blocking |
| `poe:step` | Named progress milestone during execution |
| `poe:decision` | Raise a question for the human queue, with candidate options if available. For autonomous agents only — routes to the Decision Queue. |
| `poe:chat` | Collaborative turn in a co-authoring session. For interactive agents only — routes to the Artifact Viewer chat panel, not the Decision Queue. The agent drives the conversation to build an artifact together with the human. See Architecture.md §Two Human Interaction Models. |
| `poe:advisor` | Advisor turn in a Queue Advisor session. Routes to Pane 3 advisor panel. Structurally identical to poe:chat but for a different surface and purpose (decision research, not artifact co-authoring). |
| `poe:review` | Request a peer review. The `content` field is a review *directive* — task IDs and focus area, not a plan transcription. The reviewer reads the live WBS directly from the DAG via DAG Service tools. Agent emits `poe:yield` after all review requests. |
| `poe:review-outcome` | Reviewer signals its verdict before yielding: APPROVED, APPROVED_WITH_CONDITIONS, BLOCKED, or FAILED. Orchestrator uses this to build the ReviewResult bundle for the resumed task. Missing verdict defaults to BLOCKED. |
| `poe:yield` | Yield control while awaiting a review, decision, chat, or advisor response. Task status → waiting. |
| `poe:done` | Signal task completion (all work done). |

### Skill Self-Healing

When the orchestrator cannot load a required skill at dispatch time, it does not cancel the task. Instead it auto-creates a `skill-author` task as a prerequisite, wires it as a dependency of the blocked task, and dispatches it. The `skill-author` agent produces a project-local skill file via `poe:skill`, after which the originally blocked task is dispatched normally. This is the system's self-repair mechanism — any missing skill in the library can be synthesised at runtime without human intervention. See Architecture.md §Skill System for the full self-healing loop.

### The Living DAG

Dependencies and scope change as a project progresses. The planning specialist creates the initial DAG for a phase, but execution agents refine it as they encounter new information. The Retrospective stage is not a special process — it is the planning specialist running again with full mutation rights, pruning and extending the graph based on what was learned.

This is what "replan aggressively" means in practice: the DAG is always the current best understanding, not a frozen snapshot from planning time.

---

## Multi-Project Concurrency

POE manages multiple projects concurrently. Each project has its own plan, its own artifact corpus, and its own set of active agents. The human sees a unified view across all projects — a single activity feed and a single decision queue, scoped by project.

Project state is local-first: all state lives in `{project}/.poe/` — a SQLite database for the plan, task graph, and artifact index, plus the `docs/` directory for artifact content. No central store. Projects are portable.

---

## Future State: POE as a Client/Server Application

POE is architecturally a client/server application. This is not a planned change — it is an observation about what has already emerged.

The Rust backend owns all state, all decisions, and all agent processes. It is the server. The JavaScript frontend renders state, surfaces decisions, and sends commands. It is the client. The Tauri IPC channels are the current transport — an implementation detail, not the architecture.

Every `#[tauri::command]` is an API endpoint. Every `app.emit()` is a server-sent event. The separation is already clean.

### The Thin Binding Layer

When the time is right, Tauri's role narrows to a thin native shell:

- Provides OS integration (filesystem trust, process management, auto-updates, native menus)
- Hosts the webview
- Exposes a binding layer that maps IPC channels ↔ HTTP/WebSocket

The orchestrator remains Rust. The frontend remains JavaScript. The transport becomes HTTP+WebSocket alongside Tauri IPC. Nothing in the business logic changes.

This opens without architectural disruption:

- The localhost REST API (`/api/v1/*`) connects to the same orchestrator state
- A web or mobile client can consume the same API
- The Rust core is independently testable without any UI
- Third-party integrations (CI/CD hooks, terminal dashboards, editor plugins) become straightforward

### Why Tauri Stays

Tauri is not incidental. It provides things a pure web server cannot:

- Trusted filesystem access without browser sandbox restrictions
- Ability to spawn and manage privileged subprocesses (the Claude CLI)
- No CORS complications for localhost agent communication
- Native auto-update delivery
- Single distributable binary with embedded frontend

The future state is not "replace Tauri with a web server." It is "Tauri as the native shell for a server that also speaks HTTP."

### When This Matters

Not yet. The current architecture is correct for the current stage. This future state becomes relevant when:

- A second client surface is needed (mobile, web, editor plugin)
- The REST API (Phase 5, Feature 6) proves that external consumers add real value
- The merge between Tauri IPC and HTTP becomes the obvious next simplification

No action required. The architecture will pull in this direction naturally.
