# Lifecycle Workflow Engine — Design Document

**Epic**: bp6-ims
**Status**: Planned
**Last updated**: 2026-03-06

---

## Purpose

This document is the authoritative design reference for the Lifecycle Workflow Engine. It is written for an agent with no prior context. Read it fully before writing any code.

The goal is to replace the broken `poe/src-tauri/src/workflow.rs` with a durable, opinionated V-model lifecycle engine backed by Restate. Users work a project from initial concept through deployment using specialist AI agents, guided by user-gated phase transitions.

---

## Codebase Overview

The tool is a Tauri desktop app (macOS) with:

- **Frontend**: React + TypeScript (`poe/src/`)
- **Backend**: Rust (`poe/src-tauri/src/`)
- **Database**: SQLite via rusqlite, accessed through `dag/mod.rs` (the DagStore)
- **Agent execution**: PTY-based processes via `portable_pty` in `agents.rs`
- **Workflow durability**: Restate server (side process, ports 9070 admin / 9080 services)
- **Human queue**: Axum HTTP server at port 9082, registered as Restate virtual object (`queue_service.rs`)
- **Skills system**: Markdown files with YAML frontmatter, loaded from app resources / `~/.poe/skills/` / `<project>/.poe/skills/` (`skills.rs`)

### Key existing files (keep these)

| File | Role |
|------|------|
| `src-tauri/src/agents.rs` | PTY agent spawn, watchdog, poe: event parsing |
| `src-tauri/src/dag/mod.rs` | DagStore — SQLite nodes, edges, queue items |
| `src-tauri/src/project.rs` | Tauri-managed ProjectState, open/close project commands |
| `src-tauri/src/queue_service.rs` | HTTP server for human decision queue |
| `src-tauri/src/restate.rs` | Restate server process spawn/health check |
| `src-tauri/src/skills.rs` | Skill loading and prompt injection |
| `src/` | React frontend |

### Files to replace

| File | Replacement |
|------|-------------|
| `src-tauri/src/workflow.rs` | `src-tauri/src/lifecycle/` module (Restate workflows) |

---

## The V-Model Lifecycle

The lifecycle has six steps. Each step has entry criteria (what must exist before it runs), an exit criteria (what the user approves), and on-exit actions (what gets created for the next step). Completing one step automatically creates the conditions for the next.

### Step 1 — Concept Development

**Specialist**: Operational Analysis Expert
**Entry criteria**: New project opened (no prior artefacts)
**Activity**: Agent researches the domain, leads structured Q&A to elicit the user's idea, users, integrations, and high-level goals.
**Output artefact**: `docs/conops.md` — Concept of Operations document
**Exit criteria**: User reviews draft CONOPS and explicitly approves it
**On exit**: CONOPS injected as context into all subsequent steps

---

### Step 2 — Guardrails Definition

**Parent workflow**: GuardrailsWorkflow
**Entry criteria**: `docs/conops.md` exists and is approved
**Activity**: Four sequential substeps, each gated by user approval:

| Substep | Specialist | Output artefact |
|---------|-----------|-----------------|
| 2.1 | Architecture Analyst | `docs/architecture-constraints.md` |
| 2.2 | Design System Analyst | `docs/design-system.md` |
| 2.3 | User Analyst | `docs/user-analysis.md` |
| 2.4 | Must-Not Analyst | `docs/must-nots.md` |

After all four substeps: an **Engineering Manager agent** reviews all four docs plus the CONOPS for consistency and completeness, producing `docs/guardrails-review.md`.
**Exit criteria**: User approves the EM review report
**On exit**: All five guardrail documents injected into Step 3 agent context

---

### Step 3 — Stage Planning

**Specialist**: Product Manager
**Entry criteria**: All Step 2 artefacts exist and are approved
**Activity**: PM agent reads the full artefact corpus, proposes a **minimal meaningful deliverable** for the next implementation stage. Decomposes into epics → features → tasks. Tasks are discrete, small, specialist-assignable. Explicitly includes review, integration, test, and documentation tasks. Sets task dependencies.
**Output**: DAG nodes (Epic / Feature / Task) with dependency edges
**Exit criteria**: User reviews and approves the stage plan
**On exit**: Task list passed to Step 4 for execution

---

### Step 4 — Autonomous Execution

**Orchestrator**: ExecutionWorkflow
**Entry criteria**: Approved task list from Step 3
**Activity**:
- Independent tasks execute in parallel
- Dependent tasks wait on their blockers (managed by Restate)
- Agent queries surface in the human feedback queue without blocking unrelated tasks
- Agent failures escalate to the human feedback queue
- On all tasks complete: PM agent produces `docs/phase-N-review.md`

**Exit criteria**: Human reviews PM phase review report and makes a decision:
- **Deploy** → proceed to Step 5 (or skip to Step 6 if no rework)
- **Rework** → proceed to Step 5

**On exit**: Deploy decision recorded; lifecycle advances

---

### Step 5 — Rework (optional)

**Entry criteria**: Human testing has produced rework items
**Activity**: Human submits rework items. PM agent reviews and creates a minimal rework plan (tasks only). Tasks execute via the same ExecutionWorkflow mechanism.
**Skip condition**: If no rework items submitted, this step is skipped automatically
**Exit criteria**: All rework tasks complete
**On exit**: Lifecycle advances to Step 6

---

### Step 6 — Replanning & QA

**Specialists**: Validity Check Analyst, RCA Analyst
**Entry criteria**: Step 5 complete (or skipped)
**Activity**:
1. **Validity check**: Agent reviews CONOPS + guardrail docs against what was built. Flags drift or invalidated assumptions.
2. **RCA**: Agent analyses execution logs (task failures, human interventions, blocked agents). Produces root cause report with process improvement recommendations.
3. **Skill tuning**: RCA agent writes updated specialist skill files to `<project>/.poe/skills/` — project-local overrides that tune the agent team based on what was learned this stage. These are loaded automatically by `skills.rs` at the start of the next stage (load order: app bundle → `~/.poe/skills/` → `<project>/.poe/skills/`).
4. **Skill promotion**: Human is offered the option to promote any project-local skill improvement to app-level (`~/.poe/skills/`). This is always a human-gated decision — not automatic — because not all projects look the same. A pattern worth promoting is one that applies regardless of project context.
5. **Metrics**: Key metrics recorded per stage (tasks completed, failure rate, human interventions, agent blocks, cycle time) and compared to prior stages.

**Exit criteria**: Human reviews both reports, agrees on skill changes, and optionally promotes improvements. Updated artefacts written back to `docs/`.
**On exit**: Loop back to Step 3 with a tuned specialist team

**Key insight**: Skill files are not static templates. They are living documents that accumulate project-specific wisdom across iterations. The RCA agent is not just reporting — it is improving the agent team for the next stage.

---

## Architecture

### Process Map

```
Tauri app (Rust)
  ├── DagStore (SQLite)           — nodes, edges, queue items, project state
  ├── AgentState                  — active PTY processes, watchdog
  ├── ProjectState                — open projects, active project
  ├── RestateState                — restate-server child process
  ├── Restate Service (HTTP)      — lifecycle workflows (NEW)
  └── Queue Service (HTTP :9082)  — human decision queue

Restate server (:9070 admin, :9080 services)
  — calls into Restate Service to execute workflow steps
  — provides durable state, awakeables, sub-workflow orchestration
```

### Restate Service

The new `src-tauri/src/lifecycle/` module embeds `restate-sdk` (Rust) and registers an HTTP endpoint that Restate calls to execute workflow steps. This replaces `workflow.rs` entirely.

**Workflows defined**:

```
ProjectLifecycleWorkflow        — top-level state machine per project
  └── ConceptDevelopmentWorkflow    (Step 1)
  └── GuardrailsWorkflow            (Step 2)
        └── ArchitectureConstraintsWorkflow  (Step 2.1)
        └── DesignSystemWorkflow             (Step 2.2)
        └── UserAnalysisWorkflow             (Step 2.3)
        └── MustNotsWorkflow                 (Step 2.4)
        └── EngineeringManagerWorkflow       (Step 2 review)
  └── StagePlanningWorkflow         (Step 3)
  └── ExecutionWorkflow             (Step 4)
        └── TaskWorkflow             (one per task, parallel)
  └── ReworkWorkflow                (Step 5)
  └── ReplanningWorkflow            (Step 6)
        └── ValidityCheckWorkflow
        └── RCAWorkflow
```

---

## Key Patterns

### User Gate (Awakeable)

Every step that requires user approval uses a Restate durable promise:

```rust
// In the workflow run handler:
let (approval_id, approval_promise) = ctx.promise::<ApprovalResult>("user-approval").await;

// Emit queue item for human review (resolved by queue_service.rs)
ctx.run(|| create_queue_item(approval_id, question, options)).await;

// Block durably until human resolves
let result = approval_promise.await;
```

The queue service resolves the promise when the user clicks Approve in the UI:

```rust
// In queue_service.rs resolve handler:
restate_client.resolve_awakeable(approval_id, resolution).await;
```

### Agent Spawn (Durable Side Effect)

Spawning a PTY agent is a durable side effect so it only happens once even on replay:

```rust
ctx.run(|| {
    spawn_agent_internal(SpawnAgentParams {
        cmd: "claude".to_string(),
        args: vec![task_prompt],
        skill_ids: vec!["operational-analyst"],
        ..Default::default()
    })
}).await;
```

### Artefact Injection

Before spawning each step's agent, the workflow retrieves all prior artefacts and prepends them to the agent's prompt:

```rust
let artefacts = ctx.run(|| dag_store.get_artefacts_for_steps(prior_steps)).await;
let enriched_prompt = build_artefact_context(artefacts) + "\n\n" + &task_prompt;
```

### Fan-out / Fan-in (Step 4)

Independent tasks execute in parallel using Restate's parallel call pattern:

```rust
let task_futures: Vec<_> = independent_tasks
    .iter()
    .map(|task| ctx.call(TaskWorkflow::run, task.id.clone(), task.clone()))
    .collect();

// Await all — Restate handles durability of each
for fut in task_futures {
    fut.await?;
}
```

Dependent tasks wait on their blockers: ExecutionWorkflow tracks completed task IDs in Restate state and starts a task only when all its dependencies are in the completed set.

---

## Agent Protocol (poe: events)

Agents communicate with the backend via structured JSON lines on stdout, prefixed `poe:`. Existing parsing is in `agents.rs`.

| Event | Purpose |
|-------|---------|
| `poe:step` | Update current step label in UI |
| `poe:artifact` | Create a KnowledgeArtifact DAG node and write file to `docs/` |
| `poe:node` | Create a DAG node (used by PM in Step 3 to create tasks) |
| `poe:edge` | Create a DAG edge (used by PM to set task dependencies) |
| `poe:decision` | Raise a question for the human feedback queue |
| `poe:done` | Signal workflow step completion |

**poe:artifact payload**:
```json
{
  "event": "poe:artifact",
  "filename": "conops.md",
  "title": "Concept of Operations",
  "content": "...",
  "step": 1
}
```

---

## Artefact Corpus

Documents live in the project's `docs/` directory. The DagStore tracks them as `KnowledgeArtifact` nodes with a `step` field indicating which lifecycle step produced them.

**Naming convention**:
```
docs/conops.md                   — Step 1
docs/architecture-constraints.md — Step 2.1
docs/design-system.md            — Step 2.2
docs/user-analysis.md            — Step 2.3
docs/must-nots.md                — Step 2.4
docs/guardrails-review.md        — Step 2 EM review
docs/phase-N-review.md           — Step 4 PM review (N = stage number)
docs/phase-N-validity.md         — Step 6 validity check
docs/phase-N-rca.md              — Step 6 RCA report
```

**Context injection**: `get_artefacts_for_step(step)` returns all artefacts produced in steps prior to the given step. These are concatenated and prepended to the agent's system prompt via the existing `build_enriched_args` pattern.

---

## Specialist Roles

Each step maps to a named skill. Skills are markdown files with YAML frontmatter loaded by `skills.rs`.

| Step | Skill ID | File |
|------|----------|------|
| 1 | `operational-analyst` | `skills/operational-analyst.md` |
| 2.1 | `architecture-analyst` | `skills/architecture-analyst.md` |
| 2.2 | `design-system-analyst` | `skills/design-system-analyst.md` |
| 2.3 | `user-analyst` | `skills/user-analyst.md` |
| 2.4 | `must-not-analyst` | `skills/must-not-analyst.md` |
| 2 review | `engineering-manager` | `skills/engineering-manager.md` |
| 3 | `product-manager` | `skills/product-manager.md` |
| 4 tasks | per task type (existing) | — |
| 6 validity | `validity-analyst` | `skills/validity-analyst.md` |
| 6 RCA | `rca-analyst` | `skills/rca-analyst.md` |

**Skill load order** (highest priority wins, per existing `skills.rs` behaviour):
1. App bundle defaults
2. `~/.poe/skills/<skill-id>.md` — user-level overrides
3. `<project>/.poe/skills/<skill-id>.md` — project-level overrides (written by RCA agent each stage)

The `STEP_SPECIALIST_MAP` in `lifecycle/mod.rs` maps step/substep to skill ID. The lifecycle workflow passes `skill_ids` to `build_enriched_args` when spawning each agent.

**Skill evolution**: After each stage, the RCA agent may update project-local skill files to capture lessons learned (e.g. "this project is a single-user desktop app — do not propose microservices architectures"). The human can optionally promote these to `~/.poe/skills/` if the lesson is broadly applicable. Skills improve across stages; the agent team gets better at working on this specific project over time.

---

## Conversational UI

Steps 1–3 are interactive conversations, not fire-and-forget tasks. The frontend detects the active lifecycle step type and renders accordingly:

- **Conversational steps (1–3)**: `ConversationalView.tsx` — chat bubbles, user input box, artifact preview cards
- **Execution steps (4–5)**: existing `AgentActivityView.tsx` — parallel task status, agent logs
- **Review steps (6)**: `ConversationalView.tsx` with read-only report display

**User input**: `invoke('write_to_agent', { agentId, input })` — the existing `write_to_agent` Tauri command writes to the PTY stdin.

**Artifact approval**: When a `poe:artifact` event arrives during a conversational step, the UI renders an inline document preview with Approve / Request Revision buttons. Approve resolves the user gate; Request Revision writes feedback back to the agent via `write_to_agent`.

---

## Frontend Views

| View | Tab | Purpose |
|------|-----|---------|
| `ConversationalView` | (auto, replaces queue for Steps 1-3) | Chat with specialist agent |
| `QueueView` | Queue | Human feedback queue items |
| `GanttView` | DAG | Task dependency graph |
| `AgentActivityView` | Agents | Parallel execution monitor |
| `RestateView` | Restate | Restate service inspector |
| `ArtifactsView` | (new panel) | Browse project artefacts |
| `MetricsView` | (new panel) | Per-stage metrics and trends |

---

## Implementation Order

Features are tracked as beads under epic `bp6-ims`. Implement in this order:

### Phase A — Foundation (no blockers, can parallelise)
1. **bp6-ims.1**: Restate SDK integration — embed `restate-sdk`, remove `workflow.rs`, register the new lifecycle service module
2. **bp6-ims.3**: Artefact corpus — extend `poe:artifact` handling, `get_artefacts_for_step()`, docs/ file writing
3. **bp6-ims.4**: Specialist agent roles — create skill markdown files, `STEP_SPECIALIST_MAP`

### Phase B — State machine (depends on Phase A)
4. **bp6-ims.2**: `ProjectLifecycleWorkflow` — top-level state machine, step transitions, `ctx.promise()` gates, `get_status()` shared handler

### Phase C — Step workflows (sequential, each depends on previous)
5. **bp6-ims.5**: `ConceptDevelopmentWorkflow` (Step 1)
6. **bp6-ims.6**: `GuardrailsWorkflow` (Step 2 + substeps + EM review)
7. **bp6-ims.7**: `StagePlanningWorkflow` (Step 3)
8. **bp6-ims.8**: `ExecutionWorkflow` (Step 4, fan-out/fan-in)

### Phase D — UI and polish
9. **bp6-ims.9**: `ConversationalView` — chat UI for Steps 1–3
10. **bp6-ims.10**: `ReworkWorkflow` (Step 5)
11. **bp6-ims.11**: `ReplanningWorkflow` (Step 6, RCA + metrics)

---

## Key Cargo Dependencies to Add

```toml
# src-tauri/Cargo.toml
restate-sdk = "0.4"           # check crates.io for latest
tokio = { version = "1", features = ["full"] }
```

The Restate service HTTP server runs on a new port (e.g. 9083) alongside the queue service (9082). It is started in `lib.rs` alongside `spawn_queue_service`.

---

## What NOT to Touch

- `agents.rs` — PTY agent spawning works; only extend the `poe:` event set
- `dag/mod.rs` — DagStore is the source of truth for project state; add queries but don't restructure
- `queue_service.rs` — human queue works; only add the promise resolution hook for lifecycle gates
- `restate.rs` — Restate server spawn/health check is fine as-is
- `skills.rs` — add skill files, don't change loading logic
- The React frontend structure — add components, don't restructure the app shell

---

## Acceptance Test for the Concept

The tool's own development is the proof of concept.

The design session that produced this document (2026-03-06) ran the exact lifecycle the tool is designed to orchestrate — manually, in a chat window:

1. **Concept Development** — the idea was explained, an expert asked clarifying questions, a shared understanding emerged
2. **Guardrails** — architecture constraints (Restate + Rust), design decisions, users, and must-nots were defined
3. **Planning** — the work was decomposed into a prioritised, sequenced epic with 11 features (bp6-ims)
4. **Execution** — this design document is the brief for an implementation agent

The first real validation of the engine is to use it to plan and build the next feature of itself. If the structured, agent-driven lifecycle produces output of equal or better quality than this manual session, the concept is proven.

---

## Invariants

1. **One lifecycle per project**: `ProjectLifecycleWorkflow` is keyed by `project_id`. Only one instance runs per project at a time.
2. **User always gates phase transitions**: No step advances without an explicit human approval via a resolved queue item.
3. **Artefacts flow forward, never backward**: Each step receives all prior artefacts as context. Steps cannot read future artefacts.
4. **Restate owns durability**: All workflow state lives in Restate. The SQLite DagStore owns the project graph (nodes, edges, artefacts). These are complementary, not redundant.
5. **PTY agents are ephemeral**: Agents can be restarted; their outputs are persisted as DAG nodes. The workflow (in Restate) is the durable record; the agent is the executor.
