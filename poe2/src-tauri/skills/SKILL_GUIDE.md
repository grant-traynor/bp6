# POE v2 Skill Authoring Guide

**Canonical reference for Wave 1 skill authors.**

Skills are `.md` files with YAML frontmatter. The orchestrator loads them from the priority chain (see §2) and injects them into the agent's stdin bundle as the `# Skill` section. There is no installation ceremony — drop a `.md` file in the right directory and it is live.

Read `poe-base.md` before writing any skill. This guide adds authoring conventions; `poe-base.md` defines the event protocol and required sequence that every skill inherits.

---

## 1. Frontmatter Schema

```yaml
---
id: <skill-id>               # REQUIRED. Kebab-case. Must match the filename stem.
name: <Human Readable Name>  # REQUIRED. Displayed in the UI.
description: <one sentence>  # REQUIRED. What this specialist does. Used by the orchestrator
                             #   to surface the skill in the UI and log it in the event trail.
modes: [autonomous]          # Optional. One or more of: autonomous, interactive.
                             #   autonomous  — run via stream-json -p (no keyboard, poe: events expected)
                             #   interactive — run in a human conversation (poe: events only on concrete output)
                             #   Omitting modes: OR using modes: [] safely defaults to [autonomous].
                             #   Declare explicitly for clarity; the parser is lenient with absent or empty values.
                             #   The UI blocks interactive sessions for autonomous-only skills.
model: claude-opus-4-6       # Optional. Claude model ID for this skill's agent spawn.
                             #   When present, the orchestrator passes --model <value> to claude.
                             #   When absent, claude uses its configured default.
                             #   Use for high-stakes analysis skills that benefit from a more capable model.
tags: [poe, lifecycle, ...]  # Optional. Informational. Not parsed by the orchestrator.
applies_to: [WorkflowType]   # Optional. Informational. Not parsed by the orchestrator.
protocol_version: v2         # Optional. Convention — include for new skills to signal v2 format.
---
```

### Required fields summary

| Field | Required | Notes |
|---|---|---|
| `id` | Yes | Must match filename stem exactly |
| `name` | Yes | Human-readable display name |
| `description` | Yes | One sentence — what this specialist does |
| `modes` | No | Omitting or empty `[]` defaults to `["autonomous"]`; declare explicitly for clarity |
| `model` | No | Claude model ID; passes `--model` to claude spawn when present |
| `tags` | No | Informational only |
| `applies_to` | No | Informational only |
| `protocol_version` | No | Convention — include `v2` for new skills |

> **Note — existing skills**: All current skills (`implementer.md`, `planner.md`, etc.) include `modes:` and `protocol_version: v2`. New skills should include both for clarity, but omitting `modes:` or using `modes: []` is safe — the parser defaults to `["autonomous"]`.

---

## 2. Skill File Locations

The orchestrator resolves `<skill-id>` via a priority chain (first match wins):

1. `{project.path}/.poe/skills/<skill-id>.md` — project-local override
2. `~/.poe/skills/<skill-id>.md` — user-level override
3. App bundle `skills/` — these files (`poe2/src-tauri/skills/`) — the defaults

**For Wave 1**: write into `poe2/src-tauri/skills/`. The Retrospective stage may promote corrective skill updates to project-local or user-level; the mechanism is the same.

If no file is found for a task's assigned skill, the orchestrator aborts the task with an error — it does not run the agent with no skill.

---

## 3. Required Event Sequence

**Canonical reference**: `poe-base.md §Required Event Sequence`. Do not restate or paraphrase it in your skill — link to it.

Quick reference (for reading; follow poe-base.md for authoritative text):

```
1. poe:brief   — FIRST, always. Externalise your interpretation before starting work.
2. poe:step    — At each meaningful phase (2–5 typical).
3. Outputs     — poe:artifact, poe:task, poe:edge, poe:knowledge — interleaved with steps.
4. poe:decision — Only for genuine blockers. Continue unblocked work in parallel.
5. poe:done    — LAST, always.
```

**Senior engineer exception**: `senior-engineer` operates in both proactive mode (assigned to Guardrails review and Plan Review lifecycle task nodes) and reactive mode (spawned via `poe:review`). In both cases it follows the plan-review sequence, not the standard proactive sequence — see `senior-engineer.md` and `poe-base.md §Dual Activation Mode`.

### In your skill file

Include this comment in your `## poe: Event Protocol` section to signal v2 inheritance:

```markdown
<!-- Protocol: poe v2 — inherits poe-base.md -->
```

Do NOT copy-paste the full event reference from poe-base.md into your skill unless you are specialising it. Repeat only what your skill overrides or extends.

---

## 4. poe: Event Wire Format Quick Reference

One event per line. No multi-line JSON. No markdown wrappers. Events are embedded in assistant text — the ingester extracts them by looking for valid JSON containing a `"poe"` key.

```
{"poe": "brief",    "content": "..."}
{"poe": "step",     "name": "...", "detail": "..."}           // detail optional
{"poe": "artifact", "name": "<filename>", "artifact_type": "<type>"}  // write file first, then declare
{"poe": "knowledge","key": "<slug>", "content": "...", "supersedes": "<id>"}  // supersedes optional
{"poe": "skill",    "name": "<skill-id>", "content": "<full SKILL.md markdown>"}  // NOT automatic — emit only when pattern is worth capturing
{"poe": "decision", "question": "...", "options": ["A", "B"]}  // options optional
{"poe": "chat",     "content": "...", "id": "<turn-id>"}       // interactive mode only; id optional
{"poe": "yield"}                                               // suspend; derive reason from last substantive event
{"poe": "review",   "reviewer_skill": "<id>", "content": "...", "id": "<review-id>"}
{"poe": "task",     "id": "<uuid>", "title": "...", "description": "...", "skill": "<id>",
                    "type": "task", "parent_id": "<id>", "depends_on": ["<id>"]}
{"poe": "edge",     "from": "<task-id>", "to": "<task-id>"}  // finish-to-start: "from" must finish before "to" starts
{"poe": "done",     "summary": "..."}                          // summary optional
```

**Artifact content**: full document as a string, newlines escaped as `\n`, no whitespace between JSON fields. Do not wrap the artifact line in a code fence. Do not add any text after it.

Full event catalogue and ingester responsibilities: `doc-POE/Protocol.md §2`.

### Artifact type registry

| `artifact_type` | Filename | Produced by |
|---|---|---|
| `conops` | `conops.md` | operational-analyst |
| `architecture-constraints` | `architecture-constraints.md` | architecture-analyst |
| `interface-control` | `interface-control.md` | interface-analyst |
| `data-model` | `data-model.md` | data-model-analyst |
| `must-nots` | `must-nots.md` | must-not-analyst |
| `guardrails-review` | `guardrails-review.md` | senior-engineer |
| `phase-plan` | `phase-N-plan.md` | product-manager |
| `plan-review` | `phase-N-plan-review.md` | senior-engineer (plan-review mode) |
| `validity` | `phase-N-validity.md` | validity-analyst |
| `rca` | `phase-N-rca.md` | rca-analyst |
| `review` | `review-<task-id>.md` | senior-engineer (ad-hoc) |

Use exact strings from this table. The orchestrator stores `artifact_type` and uses it for corpus assembly.

---

## 5. Activation Patterns

### Proactive specialist (most skills)

Assigned to a task node. Runs lifecycle work and produces artifacts or task graphs. Follows the standard event sequence in §3.

```yaml
modes: [autonomous]
```

Examples: `operational-analyst`, `architecture-analyst`, `validity-analyst`.

### Reactive specialist (senior-engineer)

`senior-engineer` operates in two modes: **proactive** (assigned directly to lifecycle task nodes — Guardrails review, Plan Review inner loop) and **reactive** (spawned by the orchestrator in response to a `poe:review` event from another agent). In both cases it receives a stdin bundle with `**Type**: plan_review` or `**Type**: task` to indicate which mode is active.

```yaml
modes: [autonomous]
```

When operating as a reviewer (reactive), `senior-engineer` follows the plan-review sequence from §5, not the proactive sequence from §3. When operating as a proactive lifecycle specialist, it follows the standard sequence. See `senior-engineer.md` for full detail.

### Plan-review mode

Some proactive specialists also support `plan_review` activation — they can be spawned as reviewers. Detect mode by reading the T section header:

```
**Type**: plan_review  →  activate plan-review mode
**Type**: task         →  activate standard task execution mode
```

In plan-review mode:
1. Emit `poe:brief` summarising: "Reviewing plan subset for `<domain>` domain."
2. Read the `## Review Request` section.
3. Produce a single `poe:artifact` with `artifact_type: plan-review`.
4. Emit `poe:done`.

Do NOT emit `poe:task`, `poe:edge`, or domain artifact types in plan-review mode.

Skills that support plan-review mode: `senior-engineer`, `architecture-analyst`, `interface-analyst`, `data-model-analyst`.

Verdict values (machine-parsed by orchestrator):
```
APPROVED
APPROVED_WITH_CONDITIONS
BLOCKED
```

Full plan-review protocol: `poe-base.md §Dual Activation Mode`.

---

## 6. poe:yield — Blocking for Review or Decision

Use `poe:yield` when the agent must pause and wait for an asynchronous response before it can continue. It is the correct handoff event for any checkpoint where the task status must become `waiting`. It is **not** a completion event.

### When to emit poe:yield

| Situation | Correct pattern |
|---|---|
| Interactive mode — sent a `poe:chat` question, waiting for the human | Emit `poe:chat`, then `poe:yield` |
| Agent requests peer review and must wait for results | Emit `poe:review` event(s), then `poe:yield` |
| Agent raises a decision that blocks further progress | Emit `poe:decision` event, then `poe:yield` |
| Agent has completed all work | Emit `poe:done` — never `poe:yield` |

`poe:done` is reserved for **task completion only**. Never use `poe:done` as a checkpoint or to signal that the agent is waiting for something. An agent that emits `poe:done` is marked `status = done` — the orchestrator will not resume it.

**Note on the `reason` field**: Do NOT include a `reason` field in `poe:yield`. The ingester derives the yield reason automatically from the last substantive event emitted before it (`chat`, `decision`, or `review`). Any `reason` field you write is silently ignored.

### Required event sequence

The required ordering is strict:

1. Emit all `poe:chat` **or** `poe:review` **or** `poe:decision` events for this checkpoint first.
2. Emit `poe:yield` as the **last** event before the process exits.

The ingester logs each `poe:review` / `poe:decision` event as it arrives. When `poe:yield` is received, the ingester marks the task `waiting` and signals the orchestrator. The orchestrator then reads the accumulated `poe:review` events (or waits for human resolution of the `poe:decision`) before triggering SF-4 (Agent Continuation). See `doc-POE/Flows.md §SF-3` and `§SF-4` for the full orchestrator sequence.

### Wire format

```
{"poe": "yield", "reason": "review"}
{"poe": "yield", "reason": "decision"}
```

`reason` is required. Values: `"review"` | `"decision"`.

### Code example — chat yield (interactive mode)

```
// Correct: poe:chat sends the message, poe:yield suspends
{"poe": "chat", "content": "What problem does this project solve?", "id": "c1"}
{"poe": "yield"}

// Process exits. Orchestrator waits for human to respond via respond_to_chat.
// When human responds, orchestrator resumes via --resume with "Human: {response}".
// Agent's ENTIRE output on resume: optional poe:artifact, then poe:chat, then poe:yield.
// Write the draft to disk first, then declare it:
Write("docs/conops.md", "...draft...")
{"poe": "artifact", "name": "conops.md", "artifact_type": "conops"}
{"poe": "chat", "content": "Thanks. Next: who are the primary users?", "id": "c2"}
{"poe": "yield"}

// After enough rounds, write final version and conclude:
Write("docs/conops.md", "...final...")
{"poe": "artifact", "name": "conops.md", "artifact_type": "conops"}
{"poe": "done", "summary": "CONOPS complete."}
```

**Never write bare prose text in interactive mode.** It goes to the debug log, not to the user.

### Code example — review yield

```
// Correct: emit all poe:review events first, then yield
{"poe": "review", "reviewer_skill": "senior-engineer", "id": "r-eng", "content": "..."}
{"poe": "review", "reviewer_skill": "architecture-analyst", "id": "r-arch", "content": "..."}
{"poe": "yield", "reason": "review"}

// The process exits. Orchestrator dispatches reviewers in parallel (SF-3).
// When all reviews complete, orchestrator resumes via --resume (SF-4).
// Agent receives ReviewResult blocks in its continuation bundle and continues work.
// Agent completes remaining work and emits poe:done (task completion).
{"poe": "done", "summary": "..."}
```

### Code example — decision yield

```
// Correct: emit poe:decision, then yield to wait for human resolution
{"poe": "decision", "question": "Should Stage 1 include user account management?", "options": ["Include", "Defer"]}
{"poe": "yield", "reason": "decision"}

// The process exits. Orchestrator waits for human resolution (SF-3).
// When resolved, orchestrator resumes via --resume (SF-4).
// Agent receives "Human: {resolution}" in continuation bundle and continues.
// Agent completes work and emits poe:done.
{"poe": "done", "summary": "..."}
```

### Anti-patterns

```
// WRONG: poe:done after poe:decision — task is marked complete, orchestrator will not resume it
{"poe": "decision", "question": "..."}
{"poe": "done", "summary": "Awaiting decision."}   // ← BUG: use poe:yield instead

// WRONG: poe:yield after all work is done — task stays waiting forever
{"poe": "artifact", ...}
{"poe": "yield", "reason": "review"}   // ← BUG: use poe:done if no review was requested

// WRONG: poe:yield before the poe:review events — orchestrator sees no reviews to dispatch
{"poe": "yield", "reason": "review"}   // ← BUG: emit poe:review events first
{"poe": "review", "reviewer_skill": "...", "id": "r1", "content": "..."}
```

---

## 7. Common Patterns

### Single-pass, no wait

All skills in autonomous mode are single-pass. The agent reads its stdin bundle and produces output in one run. Do not write skills that ask questions and wait for typed answers — there is no keyboard on the autonomous path.

```
❌ "Please describe your system..." (then wait)
✓ Read the bundle. Reason from it. Write the full output now.
```

For missing information, write your best estimate and mark it:

```
[PENDING: specific question that would resolve this]
```

A document with substantive content and PENDING markers is far more valuable than a skeleton. PENDING markers direct the human's attention; skeletons block progress.

### Decision vs review escalation

| Situation | Event |
|---|---|
| Genuine blocker requiring human judgment (business decision, scope boundary, not resolvable from artifacts) | `poe:decision` |
| Technical question where a peer specialist's judgment is needed | `poe:review` |
| You know the answer | Act on it — do not escalate |

A high `poe:decision` volume during execution signals that the upstream CONOPS or Guardrails stage produced insufficient clarity — not a healthy working state.

### Quality checklist pattern

Every skill should include a quality checklist before `poe:done`. See `validity-analyst.md` or `poe-base.md §Quality Gate` for examples. Minimum checklist items:

- [ ] `poe:brief` was the first event emitted
- [ ] All major phases covered by `poe:step` events
- [ ] No empty or placeholder content in artifact fields
- [ ] `poe:done` is the final event emitted

---

## 8. Existing Skill Inventory

| Skill | Stage | Modes | Inconsistencies vs this guide |
|---|---|---|---|
| `poe-base` | Base (inherited by all) | autonomous | None — canonical reference |
| `operational-analyst` | CONOPS | autonomous, interactive | `model: claude-opus-4-6` set — proof-of-concept for multi-model routing |
| `architecture-analyst` | Guardrails | autonomous | None |
| `interface-analyst` | Guardrails | autonomous | None |
| `data-model-analyst` | Guardrails | autonomous | None |
| `must-not-analyst` | Guardrails | autonomous | None |
| `senior-engineer` | Guardrails review, Plan Review, ad-hoc | autonomous | None |
| `product-manager` | Increment Planning | autonomous | **v1 context model** — `Input Context` section injects `POE_WORKFLOW_ID`, `POE_NODE_ID`, `POE_NODE_DATA` env vars. POE2 delivers context via stdin bundle (Protocol.md §3), not env vars. Event emission section uses v1 payload format. Do not use as a format reference. Fix tracked in bp6-rub.5. BLOCKED path now correctly emits `poe:yield reason="decision"` (was incorrectly `poe:done` — fixed in bp6-m2f.17). |
| `validity-analyst` | Validity Analysis | autonomous | None |
| `rca-analyst` | Retrospective | autonomous | None |
| `implementer` | Execution | autonomous | None |
| `planner` | Execution | autonomous | None |
| `test` | Execution | autonomous | Detects project type and runs the appropriate test suite (cargo test, npm test, pytest, etc.). Emits poe:done on pass, poe:decision on failure. |
