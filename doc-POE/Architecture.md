# POE — Pairti Orchestration Engine: Architect

**Status**: Draft
**Last updated**: 2026-03-08

---

## Overview

```mermaid
graph TB
    subgraph Project["Project (defined by CONOPS)"]
        KR["Knowledge Register\n(persistent, cross-cutting)"]

        subgraph Phase["Phase (unit of iteration)"]
            subgraph WBS["Work Breakdown"]
                Epic --> Feature
                Feature --> Task["Task / Bug / Chore"]
                Task -.->|"depends on"| Task2["Task / Bug / Chore"]
                Task --> Subtask["Subtask (rare)"]
            end

            subgraph Lifecycle["Phase Lifecycle"]
                Planning --> Execution --> Retrospective
            end
        end

        subgraph Artifacts["Artifact Corpus"]
            A1["CONOPS"]
            A2["Guardrails"]
            A3["Phase Review"]
        end
    end

    KR -.->|"context for all agents"| Lifecycle
    Artifacts -.->|"injected as context"| Lifecycle
    Execution -->|"produces"| Artifacts
    Retrospective -->|"writes"| KR
```

---

## Design Drivers

1. **Each phase runs two nested PDCA loops.** The outer loop checks and corrects what was built; the inner loop checks and corrects how it was planned and executed. Both loops must close. A system with only an outer loop learns slowly and wastes effort on rework that better planning would have prevented. A system with only an inner loop improves execution efficiency but never asks whether it is building the right thing.

   > **Design note**: The PDCA frame arrived late in the design session as an intuition, not as a starting point. It immediately validated the existing stage structure and became the primary conceptual anchor for the whole phase model. The two-loop refinement arrived even later — as a nagging intuition that a single loop felt incomplete. Once named, it resolved the ambiguity between Rework (inner Act) and Retrospective (outer Act), which had previously felt like overlapping concerns. They are not: they operate on different objects at different cadences.

   ### The Inner Loop — Plan Quality (pre-execution)

   Closes *before execution begins*. Checks and corrects the plan: whether work is well-decomposed, dependencies are correct, tasks are right-sized, and the right specialists are assigned. A plan that survives this loop produces much better execution output — because T is closer to T!.

   ```
   Plan  →  Increment Planning: decompose into epics, features, tasks; define T; assign skills S
   Check →  Plan Review: specialists (architect, engineer, PM) review the plan via poe:review;
             flag gaps, ambiguities, wrong skill assignments, missing dependencies
   Act   →  Plan Revision: planning specialist updates the DAG based on findings; may iterate
   Do    →  Execution begins on the approved plan
   ```

   The inner loop closes the gap in **T** (task quality) before it can corrupt output. Human input (**H**) is invested here — in the plan review, not in mid-execution firefighting. A busy decision queue during execution is usually a signal that the inner loop was skipped or was insufficient.

   **The loop may iterate.** A single review pass is the common case, but a plan with significant issues may require multiple cycles: Review → Revise → Review again. This is expected and correct — iteration is cheaper pre-execution than post. The planning specialist tracks unresolved findings and drives subsequent iterations autonomously via `poe:review`.

   **Escalation to human.** If the loop cannot converge — reviewers keep finding blockers, or reviewers disagree on a fundamental design question — the planning specialist escalates via `poe:decision`. The human takes the call, breaks the deadlock, and the loop resumes. This is the correct escalation point: a human decision that is structural, not incidental. The queue should be sparse; if plan reviews frequently escalate, it is a signal that the CONOPS or Guardrails stage produced insufficient clarity.

   > **Living proof**: the design session that produced these documents ran this exact loop. Docs and beads were drafted (Plan), an implementation engineer agent reviewed them and identified 4 blocking gaps (Check), the docs were revised and Protocol.md was written (Act). Execution proceeded on a much stronger foundation. The copy-paste between agent sessions that this required is what `poe:review` eliminates — the orchestrator routes the review automatically; the human watches in the activity feed.

   ### The Outer Loop — Deliverable and Process Quality (post-execution)

   Closes after execution. Checks and corrects what was built and how the agent team performed.

   ```
   Do    →  Execution: f(C, T, S, K, H) → C'  (inner loop already closed; T is solid)
   Check →  Validity Analysis: does C' satisfy C!? where is the gap between built and intended?
   Act   →  Retrospective: RCA on quality gaps → update S (skills), update K (knowledge register),
             tighten guardrails if needed; feed improvements into the next phase
   ```

   The outer Act corrects **S** (skill quality) and **K** (knowledge gaps). It does not re-execute tasks — that is Rework's territory when specific deliverable deficiencies are found. It prepares the agent team to perform better in the *next* phase.

   ### Why Both Loops Are Necessary

   | | Inner Loop | Outer Loop |
   |---|---|---|
   | **When** | Pre-execution — plan is checked before Do begins | Post-execution — output is checked after Do completes |
   | **Checks** | Plan quality — is the work correctly decomposed? | Deliverable quality — was the right thing built? |
   | **Acts on** | Task structure, dependencies, skill assignments | Skills, knowledge register, guardrails |
   | **Corrects** | **T** (task quality) + **H** (human investment upfront) | **S** (skills) + **K** (knowledge) |
   | **Stage types** | Plan Review → Plan Revision → [Execute] | Validity Analysis → Retrospective |
   | **Failure signal** | Busy decision queue during execution | Gap between C' and C! at validity check |

   A defect that the inner loop catches is a planning failure. A defect that only the outer loop catches is a skill or knowledge failure. This distinction drives where the correction goes — and makes the Retrospective's output (updated skills and knowledge) meaningful rather than generic "lessons learned."

2. **Plan broadly, implement narrowly, replan aggressively.** The CONOPS and Phase plan define the shape. Execution is focused and bounded. The Retrospective updates the plan before the next Phase begins.
3. **Local-first.** All project state lives in `{project}/.poe/` — portable, no central store.
4. **Implementation directory is `poe2/`**. `poe/` is the v1 app (Restate-based, retired). All POE v2 code — Rust backend, React frontend, skills — lives in `poe2/`. Do not modify `poe/`.
4. **Event-driven.** No polling. Agents emit structured events; the backend ingests them into SQLite and pushes deltas to the frontend via Tauri events.
6. **Agents run autonomously.** Human oversight is observational by default, not supervisory. The human invests effort before execution (planning, guardrails) so that execution can succeed without intervention.
7. **The knowledge register is institutional memory.** It accumulates across phases and is always current. Agents read it before acting; they write to it when they learn something worth preserving.

---

## The Unit of Work

The orchestrator's fundamental job is to assemble and dispatch units of work. Understanding what a unit of work *is* defines both what the orchestrator schedules and what the Retrospective corrects.

> **Design note**: This model was not designed top-down. It emerged during the design session when asking what the agent's CRUD protocol was actually operating on. The question "what is the orchestrator scheduling?" revealed the unit of work as the fundamental abstraction, and the imperfect-inputs framing followed naturally. This is the kind of insight that arrives late and clarifies everything — worth preserving as a reminder to hold design sessions open rather than closing on structure too early.

A unit of work is a function over imperfect inputs:

```
f(C, T, S, K, H) → C'

Ideal: f(C, T!, S!, K!, H!) → C!
```

| Input | What it is | How it can be imperfect |
|---|---|---|
| **C** | The codebase as it stands | Accumulated drift from prior imperfect outputs |
| **T** | The task — scope and description of the activity | Ambiguous, incomplete, or misjudged scope |
| **S** | The skill — how the specialist should approach the work | Wrong role, poorly defined behaviour, missing domain knowledge |
| **K** | The total project knowledge available at execution time | Gaps, outdated entries, missing context not yet discovered |
| **H** | Human input — guidance, decisions, corrections during execution | Contradictory, misremembered, or evolving — humans can misguide as well as guide |

The gap between C' (what was produced) and C! (what was intended) is the accumulated error from all five imperfect inputs. Each bad output is a diagnostic: which input was furthest from ideal?

### Implications for the Orchestrator

The orchestrator does not merely schedule tasks — it assembles the best possible input bundle `(C, T, S, K, H)` for each unit of work given what is currently available. The quality of that assembly determines output quality. This is why:

- **Artifacts flow forward as context** — they are the best current approximation of C for the next task
- **The knowledge register is injected at task start** — it closes gaps in K
- **Skills are loaded from the priority chain** — project-local overrides make S closer to S!
- **Planning is front-loaded** — the investment in T before execution reduces the cost of bad H mid-run

### Implications for the Retrospective

The Retrospective is fault attribution. Given C' ≠ C!, which input was responsible?

- T was poorly scoped → refine the task decomposition process
- S was wrong for this domain → update the skill file
- K had a gap → add a knowledge register entry
- H was contradictory → note the decision and its rationale

The Retrospective's outputs — updated skills, new knowledge entries, refined planning guidance — are systematic corrections that move each input closer to its ideal for the next phase. This is the feedback loop that makes the agent team better over time.

### The Two Sides of the Same Coin

The orchestrator (which assembles inputs and schedules work) and the orchestrated (the agent that executes) are two sides of the same coin. The orchestrator creates the conditions for success; the agent operates within them. When an agent underperforms, the question is rarely "was the agent bad?" — it is "which input was furthest from ideal, and whose responsibility was it to provide that input?"

---

## Work Breakdown Structure

The WBS is the vertical decomposition axis. Every node in the hierarchy knows its parent, giving agents (and humans) the full "why" chain at any level of zoom.

```
Project
  Phase
    Epic
      Feature
        Task | Bug | Chore
          Subtask             (rare — only for genuinely complex tasks)
```

### Definitions

**Project** — the top-level container. Defined by the CONOPS. A project has one artifact corpus, one knowledge register, and a sequence of phases.

**Phase** — a meaningful increment toward the CONOPS. The unit of iteration. Each phase gets a full lifecycle pass (Planning → Execution → Retrospective). Phases are planned at a high level upfront and refined as the project matures.

**Epic** — a major body of work within a phase. Groups related features that together deliver a significant capability. An epic is too large to execute directly; it exists to provide grouping and context.

**Feature** — a discrete, deliverable unit of functionality within an epic. A feature should be completable within a phase. It has a clear definition of done.

**Task / Bug / Chore** — the agent-executable leaf node.
- **Task**: new work that produces something
- **Bug**: a defect to be corrected
- **Chore**: maintenance, refactoring, or housekeeping that has no direct user-facing output

**Subtask** — a subdivision of a task used only when a task is genuinely too complex to assign to a single agent invocation. Subtasks are the exception, not the rule.

---

## Dependency Model

Dependencies are horizontal linkages between tasks (and optionally features) within a phase. They express execution ordering: *you cannot do A before B*.

- Dependencies are set during Phase Planning by the planning specialist.
- The execution engine resolves the DAG: tasks with no unmet dependencies are immediately eligible to run; dependent tasks wait.
- Independent tasks run in parallel.
- A dependency can exist between tasks within the same feature, across features within an epic, or across epics within a phase.
- Cross-phase dependencies are expressed at the Phase level in the project plan, not at the task level.

---

## Artifact Corpus

Artifacts are the structured documents that define the product. They are phase outputs — produced by a lifecycle stage and injected as context into subsequent stages.

### Artifact Flow

Artifacts flow forward through the lifecycle. Each stage receives all artifacts produced by prior stages as context. A stage cannot read future artifacts. There is no hardcoded ordering — context resolution matches artifacts by type and phase.

### Standard Artifacts

**Project-level** — produced once, injected into every subsequent stage's input bundle:

| Artifact | Produced by | Purpose |
|---|---|---|
| `conops.md` | CONOPS stage | Product concept, users, goals, operational context |
| `architecture-constraints.md` | Guardrails stage | Architecture decisions, patterns, must-use technologies |
| `interface-control.md` | Guardrails stage | External interface definitions — wire protocols, event formats, API contracts, inter-subsystem boundaries |
| `data-model.md` | Guardrails stage | Internal data structures — DB schema, type definitions, entity relationships |
| `design-system.md` | Guardrails stage | UI design tokens, component patterns, visual language |
| `user-analysis.md` | Guardrails stage | Personas, user journeys, feature priority matrix |
| `must-nots.md` | Guardrails stage | Hard constraints — security, compliance, things the system must never do |
| `guardrails-review.md` | Guardrails stage (Senior Engineer review) | Cross-cutting review of all guardrails artifacts for conflicts and gaps |

**Phase-level** — produced per phase:

| Artifact | Produced by | Purpose |
|---|---|---|
| `phase-N-plan.md` | Increment Planning stage | Epic/feature/task decomposition for this phase |
| `phase-N-plan-review.md` | Plan Review stage (inner loop Check) | Specialist findings on the plan before execution |
| `phase-N-data-model-delta.md` | Increment Planning stage (if schema changes) | Additions to the data model for this phase |
| `phase-N-validity.md` | Validity Analysis stage | Gap between C' and C! for this phase |
| `phase-N-rca.md` | Retrospective stage | Root cause analysis and corrective actions |

> **Note**: `interface-control.md` and `data-model.md` close the gap that existed in POE v1 — implementation agents had no authoritative spec for internal structure or interface contracts injected into their context, leading to protocol conflicts discovered only during coding. These documents are injected into every implementation task's input bundle via the standard K assembly.

### Storage

Artifact content lives in `{project}/docs/`. The project database tracks metadata: artifact type, producing stage, phase number, and timestamp. The database is the index; the filesystem holds the content.

---

## Knowledge Register

The knowledge register is the project's institutional memory. It is distinct from the artifact corpus:

| | Artifact Corpus | Knowledge Register |
|---|---|---|
| **Purpose** | Defines the product | Guides execution |
| **Lifecycle** | Phase outputs; superseded by later versions | Persistent; accumulates across all phases |
| **Writable by** | Agents (via `poe:artifact`) | Agents and humans |
| **Examples** | CONOPS, architecture doc, phase review | Architectural decisions, domain glossary, failed approaches, discovered constraints, integration notes |

### What belongs in the Knowledge Register

- Architectural decisions and their rationale ("we chose X over Y because Z")
- Domain terminology and glossary
- Things tried that did not work, and why
- Constraints discovered during execution (not known at planning time)
- Integration notes and gotchas
- Project-local agent skill overrides (written by agents via `poe:skill` during execution or the Retrospective stage)

### Structure

The knowledge register is a set of named entries, each with a key, a value, and a timestamp. Entries can be updated or superseded but are never deleted (history is preserved). Agents query the register by key or by full-text search before acting on tasks that may be affected.

### Storage

Knowledge register entries live in the project database alongside the WBS graph. They are surfaced in the UI as a browsable, searchable panel.

---

## Data Model

All project state is local-first, stored in `{project}/.poe/poe.db` (SQLite, WAL mode).

See `doc-POE/Protocol.md §1` for the complete CREATE TABLE schema. Key tables:

- `tasks` — WBS nodes (title, description, type, skill, status, parent\_id, phase\_id, session\_id)
- `edges` — directed dependency edges (from\_id, to\_id)
- `event_log` — append-only structured agent event log (never updated or deleted)
- `decisions` — human decision queue items (question, options, resolution)
- `artifacts` — artifact index (name, type, path, producing\_task\_id)
- `knowledge` — knowledge register entries (key, content, supersedes\_id)
- `phases` — phase definitions, stage type, PDCA state
- `projects` — project metadata

Every `tasks` row has a `parent_id` (full WBS hierarchy traversal) and a `phase_id`. Decision queue items reference the task that raised the question. The event log is the audit trail — every poe: event lands here in full.

---

## Agent Event Protocol

Agents communicate with POE via structured JSON events embedded in the `--output-format stream-json` transport — not PTY scraping. The orchestrator spawns agents with `claude --output-format stream-json -p --dangerously-skip-permissions [--model <id>]`, writes the T+S+K bundle to stdin, and reads newline-delimited JSON from stdout. The `--model` flag is included when the skill's frontmatter declares a `model:` field; otherwise the claude binary uses its configured default. poe: events are extracted from assistant text content via a line-accumulation buffer. See `doc-POE/Protocol.md §2` and `§5` for the full wire format and spawn model.

| Event | Purpose |
|---|---|
| `poe:brief` | Agent's interpretation of its task, written before execution begins. Drives the glass-box interpretation view. |
| `poe:step` | Named progress milestone during execution. |
| `poe:artifact` | Produce a named artifact. Written to `docs/`, indexed in the database. |
| `poe:task` | Create a WBS node (used by planning specialist to populate the task graph). |
| `poe:edge` | Create a dependency edge between two nodes. |
| `poe:knowledge` | Write an entry to the knowledge register. |
| `poe:skill` | Write a reusable pattern to `{project}/.poe/skills/<name>.md`. Closes the self-improvement loop: any agent that discovers a project-specific pattern can persist it as a local skill override without manual authoring. |
| `poe:decision` | Raise a question for the human decision queue. Agent then emits `poe:done`; orchestrator resumes via `--resume` with the human's resolution. |
| `poe:review` | Request a peer review from another specialist agent. Agent emits `poe:done` (awaiting review); orchestrator spawns reviewer, then resumes requesting agent via `--resume` with the review result. |
| `poe:done` | Signal task completion (or checkpoint when awaiting decision/review). |

Human access to the raw agent conversation is via **xterm.js session handover** — `claude --resume <session_id>` in a PTY, bridged to the browser via WebSocket. ANSI codes are rendered by xterm.js natively; no parsing occurs on this path. The structured event stream drives the UI; the xterm handover is a drill-down for human-in-the-loop interaction.

### Agent-to-Agent Review Cycle

Agents can request peer review from other specialist agents without human facilitation. This eliminates the need for a human to act as message relay between agents — the orchestrator routes the conversation.

**The pattern:**

```
Agent A (e.g. Architect) emits poe:review
  → Orchestrator creates a review task, assigns it to the named skill (e.g. tauri-engineer)
  → Agent A status = blocked (waiting for review)
  → Agent B (Tauri Engineer) receives full context: Agent A's brief + artifacts + the review question
  → Agent B produces a review artifact (poe:artifact) and signals poe:done
  → Orchestrator unblocks Agent A, injects Agent B's review into Agent A's context
  → Agent A continues with the reviewer's assessment in hand
```

**poe:review payload:**

```json
{"poe": "review", "reviewer_skill": "tauri-engineer", "content": "Are these 4 features ready for implementation? Flag any gaps."}
```

See `doc-POE/Protocol.md §2` for the full wire format for all events.

The human observes the entire exchange via the activity feed. Queue items only arrive if both agents hit genuine ambiguity neither can resolve — which is the correct escalation point.

**What this replaces:** the human reading one agent's output, copying it to another agent's terminal, reading the response, and copying it back. The orchestrator does this. The human watches.

### poe:brief

The `poe:brief` event is emitted by every agent at the start of execution, before any work begins. It externalises the agent's interpretation of its task so the human can verify intent asynchronously. The agent proceeds immediately after emitting the brief — it does not wait for human acknowledgement.

```json
{"poe": "brief", "content": "I understand this task as: ... I will: 1. ... 2. ..."}
```

---

## Human Oversight

### Activity Feed (Glass Box)

The activity feed shows, across all active agents and projects:

- The agent's `poe:brief` — what it understood the task to be and its plan
- `poe:step` milestones as they are emitted
- Task completion status
- Any questions raised to the human queue

The feed is built entirely from the structured event log. It is not derived from PTY output.

### Decision Queue

Agents that encounter genuine ambiguity emit a `poe:decision` event with a question and optionally a set of candidate options. This creates a queue item. The human resolves it; the agent is unblocked. Unrelated agents continue running in parallel.

A low queue volume is a quality signal: it means the planning and guardrails stages produced sufficient context. A high volume indicates the preconditions were insufficient.

### Queue Advisor (AI Decision Aid)

The decision queue is not a simple approval interface — it is a collaborative decision-making space. Each queue item has a chatbot advisor associated with it. When the human is uncertain how to resolve a question, they can instruct the advisor to help: *"Go check what the architecture constraints say about this"*, *"Has this come up before?"*, or *"Spawn a quick research task on X."*

The advisor is well-positioned to help because it has direct access to the inputs that should inform the decision:

- **K** — the knowledge register, searchable for prior decisions and discovered constraints
- **Artifacts** — the full project artifact corpus (CONOPS, guardrails, phase plans)
- **DAG context** — the blocked task, its dependencies, its parent feature and epic

The advisor researches; the human decides. The boundary is explicit: the advisor does not resolve queue items — it improves the quality of human input (H) before the human commits.

> **Design note**: The Queue Advisor's location evolved during design. The initial question was whether the *orchestrator* needed AI support for scheduling decisions. The answer was no — orchestration is deterministic given a well-formed DAG. The real judgment gap is in the human's decisions, not the engine's. Placing the advisor at the queue level rather than the task level keeps AI support where it actually reduces friction.

In terms of the formal model, the Queue Advisor is an **H-quality tool** — it exists to make human decisions better-informed before they enter the execution loop.

#### Evolution Path

```
Phase 1: Queue → Human decides alone
Phase 2: Queue + Advisor → Human asks it to research, then decides
Phase 3: Advisor proactively surfaces relevant K before human has to ask
Phase 4: Advisor auto-resolves low-stakes decisions, escalates genuine ambiguity only
```

The system starts simple. AI support is added where friction is actually felt, not speculatively.

---

## Orchestration Engine

The orchestrator is a Rust service inside Tauri. It is not a sidecar, not an external workflow engine. It starts with the app, recovers from SQLite on restart, and reacts to DAG changes. SQLite is the sole source of durable state.

### What Triggers It

The orchestrator is reactive — it wakes on events, not on a timer:

- `poe:done` received — a task completed, dependents may now be ready
- `poe:task` / `poe:task:cancel` — DAG structure changed
- `poe:edge` / `poe:edge:remove` — dependency graph changed
- Human resolves a queue item — a blocked agent can continue
- Human advances a stage gate — next stage becomes active
- App start — recover and resume from prior state

### The Core Loop

Every wake-up runs the same loop regardless of trigger:

```
1. Find all tasks where:
   - status = pending
   - all dependency tasks = complete
   - stage is in execution mode (not held at a human gate)
   - concurrency limit not reached

2. For each ready task:
   a. Assemble input bundle (T, S, K — C is implicit, H arrives during execution)
   b. Spawn agent
   c. Mark task as running

3. Emit frontend deltas
```

### Two Levels of Orchestration

- **Stage gates** — human-driven. The orchestrator holds until the human explicitly advances. No tasks start in the next stage until the gate is cleared. The human controls the stages.
- **Task scheduling** — dependency-driven. Within an active stage, the orchestrator runs everything the DAG says is ready, automatically. The DAG controls the tasks.

### Input Bundle Assembly

Before spawning an agent, the orchestrator assembles the context it needs:

| Input | Source |
|---|---|
| **C** (codebase) | Implicit — agent runs in the project directory |
| **T** (task) | Node record from SQLite — title, description, type, plus full WBS ancestry (task → feature → epic → phase) |
| **S** (skill) | Loaded from skill file priority chain (bundle → user → project-local) |
| **K** (knowledge) | Artifacts declared as inputs for this stage type (selective, not full corpus) + all knowledge register entries |
| **H** (human input) | Not assembled upfront — arrives during execution via `write_to_agent` if needed |

**WBS ancestry** is always injected. Knowing that a task belongs to feature X, epic Y, phase Z gives the agent the "why" chain that informs every decision it makes.

**K selectivity**: the stage type declares what artifact types it consumes. The orchestrator filters the artifact corpus to those types only. Knowledge register entries are always injected in full — they are designed to be concise and universally relevant.

### Event Ingester

The event ingester is the bridge between the agent's JSON stream and the DAG. It runs as part of the agent watchdog, processing the stream-json output:

```
JSON object received from agent stdout
  → type = "system", subtype = "init"  → extract and store session_id in nodes.session_id
  → type = "assistant" or "content_block_delta"
       → extract text → push to text_buf
       → for each complete line in text_buf:
           → no "poe" key  → discard (agent commentary)
           → "poe" key     → parse → write to SQLite → trigger orchestrator → emit Tauri event
  → type = "result"        → flush text_buf tail → agent session complete
```

The orchestrator is notified via a Tokio `mpsc` channel (`DagChanged` signal). Anything that mutates DAG structure or task status triggers it; everything else records and notifies the frontend only.

See `doc-POE/Protocol.md §2` for the full ingester responsibility table (SQLite writes, orchestrator triggers, and Tauri events emitted per event type).

### Recovery

On app restart:

```
1. Open all known project databases
2. Find tasks with status = running → agent process is gone
3. Attempt to resume each interrupted agent using its stored session ID (Claude --resume)
4. If resume fails → mark task back to pending, orchestrator re-spawns fresh
5. Queue items persist as-is
6. Artifacts persist on disk and in SQLite index
7. Trigger orchestrator loop → re-evaluates all ready tasks
```

Agent session IDs are captured from the `{"type":"system","subtype":"init","session_id":"..."}` JSON event at spawn time and stored in `nodes.session_id`. Resume is attempted first; clean restart is the fallback.

### Concurrency

Two levels of concurrency limit, both configurable and visible in the UI:

| Limit | Default | Description |
|---|---|---|
| Per-project | 5 | Max concurrent agents within a single project |
| Global | 15 | Max concurrent agents across all open projects |

The orchestrator respects both limits when selecting tasks to spawn in the core loop. If the limit is reached, ready tasks are queued in the DAG (status remains pending) until a running agent completes.

The UI displays a concurrency indicator — running count / limit — for each project and globally. Both limits are adjustable from the UI. Higher limits suit powerful machines with fast API access; lower limits suit constrained environments or when the human wants to keep queue volume manageable.

---

## Agent Tooling (MCP)

The `poe:` event protocol is one-way — agent writes, POE ingests. MCP (Model Context Protocol) makes the interface bidirectional, adding the Read side to the CRUD model and exposing project-specific tooling to agents.

### Complementary Roles

| Protocol | Direction | Purpose |
|---|---|---|
| `poe:` events | Agent → POE | Writes — DAG mutations, artifacts, progress, decisions |
| MCP tools | Agent ↔ POE | Reads + structured operations — queries, tooling, project utilities |

### Why This Matters

The input bundle assembled at task start is a snapshot. As execution progresses an agent may need information that wasn't knowable at start — especially in a living DAG where other agents are writing concurrently. MCP gives agents a structured way to query mid-task without relying on what was injected into the prompt upfront.

### Planned MCP Tool Categories

**DAG & Knowledge Queries**
- Query current task status and dependency state
- Search the knowledge register by keyword or topic
- Retrieve specific artifacts by name or type

**Project Tooling**
- Run the test suite and return structured results
- Check git status, diff, and recent history
- Execute project-specific build or validation commands

**Write Operations** (structured alternative to `poe:` events for complex mutations)
- Create or update WBS nodes with validation
- Resolve ambiguities against the knowledge register before raising a `poe:decision`

### Status

MCP tooling is a planned capability. The `poe:` event protocol is sufficient for the initial implementation. MCP is introduced once the core orchestration loop is stable, prioritising the tools that most reduce `poe:decision` queue volume.

---

## Skill System

Skills are the specialist definitions that agents execute under. Each skill is a markdown file with YAML frontmatter defining the role, behaviour, and expected outputs.

### Frontmatter Schema

Key fields parsed by the orchestrator at load time:

| Field | Required | Notes |
|---|---|---|
| `id` | Yes | Kebab-case; must match filename stem |
| `name` | Yes | Human-readable display name |
| `description` | Yes | One sentence — surfaced in UI and event trail |
| `modes` | Yes | `[autonomous]`, `[interactive]`, or both. Defaults to `[autonomous]` if absent |
| `model` | No | Claude model ID (e.g. `claude-opus-4-6`). When present, passed as `--model` to the claude spawn. When absent, claude uses its configured default |

Informational fields (`tags`, `applies_to`, `protocol_version`) are not parsed by the orchestrator.

### Load Order (highest priority wins)

1. App bundle defaults (`resources/skills/`)
2. User-level overrides (`~/.poe/skills/<skill-id>.md`)
3. Project-level overrides (`{project}/.poe/skills/<skill-id>.md`)

### Skill Evolution

After each phase, the Retrospective stage may update project-local skill files to capture lessons learned. The human can promote project-local improvements to the user level if they apply broadly. Skills improve across phases; the agent team becomes better at working on this specific project over time.

### Bootstrap Strategy — Existing Skills

POE v2 does not start from a blank skill library. Two skills from POE v1 (`poe/src-tauri/skills/`) are near-ready for direct port:

| Skill | Readiness | Notes |
|---|---|---|
| `operational-analyst` | 90% | Already emits `poe:artifact`. Needs event payload field rename (`type` → `event`) and removal of old lifecycle step references. Maps to the CONOPS stage. |
| `product-manager` | 85% | Already emits `poe:node` / `poe:edge` (rename to `poe:task` / `poe:edge`). Readiness check, phase decomposition, quality checklist, and `poe:decision` for scope choices are all correct. Maps to the Increment Planning stage. |

The `project-advisor` skill is a solid foundation for the Queue Advisor but needs extension — it is currently read-only and passive; the Queue Advisor needs search, research, and sub-task capability.

The bp6 persona template (`bp6/templates/personas/_TEMPLATE_EIAMOE.md`) defines the E-I-A-M-O-E framework (Entry / Inputs / Activities / Measurements / Outputs / Exit). The framework structure and mode selection concepts remain valid for authoring POE v2 skills. The beads-specific tooling (`bd` commands, C-E-P via `bd show`) is replaced by the `poe:` event protocol and orchestrator-injected input bundles — but the structural framework is worth keeping as an authoring guide.

**Recommended approach**: once `bp6-e1n.6` (Skill System) lands, run a skill authoring task that ports `operational-analyst` and `product-manager` to POE v2 format and writes the missing specialists from scratch using the updated EIAMOE framework as a guide. This is well-scoped work for a specialist agent.

Missing specialists to author for v2:

| Skill | Stage | Role |
|---|---|---|
| `senior-engineer` | Guardrails (review), Plan Review (inner loop Check), ad-hoc via `poe:review` | Resolves protocol conflicts, reviews plans for technical correctness, answers implementation questions from other agents. Primary target for `poe:review` from implementation agents. |
| `interface-analyst` | Guardrails | Authors `interface-control.md` — defines wire formats, event protocols, API contracts, inter-subsystem boundaries |
| `data-model-analyst` | Guardrails | Authors `data-model.md` — defines DB schema, type definitions, entity relationships |
| `architecture-analyst` | Guardrails | Authors `architecture-constraints.md` |
| `must-not-analyst` | Guardrails | Authors `must-nots.md` |
| `validity-analyst` | Validity Analysis (outer loop Check) | Authors `phase-N-validity.md` |
| `rca-analyst` | Retrospective (outer loop Act) | Authors `phase-N-rca.md`, updates skills and knowledge register |

---

## Process Architecture

```
Tauri App (Rust + React)
  ├── SQLite (poe.db)          — WBS graph, artifacts index, knowledge register, event log
  ├── AgentState               — active agent processes (stream-json), watchdog, stdout readers
  ├── ProjectState             — open projects, active project
  ├── EventIngester            — reads stream-json stdout, extracts poe: events, writes to SQLite, emits Tauri events
  └── Frontend (React)
        ├── ActivityFeed       — live agent event stream
        ├── DecisionQueue      — human decision queue
        ├── WBSView            — project / phase / epic / feature / task hierarchy
        ├── ArtifactsView      — artifact corpus browser
        ├── KnowledgeView      — knowledge register browser
        └── AgentHandover      — xterm.js PTY panel (--resume, human-in-the-loop)

Autonomous Agent (claude --output-format stream-json -p)
  — reads stdin bundle (T + S + K — see Protocol.md §3)
  — emits poe: events embedded in assistant text ({"poe": "<type>", ...})
  — process exits after {"type":"result",...}
  — session_id stored at spawn from {"type":"system","subtype":"init",...}

Interactive Agent (claude --resume <session_id>, PTY)
  — human handover only — not parsed by orchestrator
  — raw bytes → WebSocket → xterm.js in browser
  — used for check-in, decision-assist, direct exploration
  — cwd MUST match the project.path used for the original stream-json session
    (Claude scopes sessions by directory — mismatch → "No conversation found")
```

---

## Invariants

1. **One project per directory.** Project state is co-located with the project. No central store.
2. **Artifacts flow forward only.** A stage can read artifacts from prior stages but not future ones.
3. **Knowledge register is append-only.** Entries can be superseded but never deleted.
4. **The event log is the audit trail.** Every agent action that matters is recorded as a structured event.
5. **Human gates are explicit.** No phase advances without a human decision. The human can choose to skip a gate, but the skip is recorded.
6. **The queue should be sparse.** Frequent agent questions indicate insufficient preconditions, not a healthy workflow.
