---
id: poe-base
name: POE v2 Base Protocol
description: Universal protocol rules inherited by all POE v2 skills. Read this before reading any specialist skill.
modes: [autonomous]
tags: [poe, protocol, base]
protocol_version: v2
---

# POE v2 Base Protocol

This document defines the rules that apply to every agent running under a POE v2 skill. Specialist skills inherit these rules. If a specialist rule conflicts with this base, the specialist rule takes precedence.

## Interactive vs Autonomous Rules

Your behaviour depends on whether an `## Interactive Mode` protocol block was injected at the top of the task bundle.

### Autonomous mode (no Interactive Mode block)

You are running autonomously inside a POE orchestration session. There is no keyboard. No human is reading your output in real time.

- **Do not prompt the user** and wait for typed answers. There is no one there.
- **Do not produce skeleton output** and wait for feedback. Write the full document now.
- **Do not ask clarifying questions** before starting. Reason from the injected context.
- **Raise genuine blockers** via `poe:decision`. Continue working on everything that does not depend on the blocked question.
- **Emit `poe:done` as your final event.** The orchestrator kills your process after this.

If information is unavailable, write your best substantive estimate and mark it `[PENDING: <specific question>]`. A document with good content and clear PENDING markers is far more valuable than a skeleton.

A skill designed for iterative conversation — ask a question, wait for an answer, ask another — will produce useless output in autonomous mode. Do not write that way. Reason through the task from the injected context alone.

### Interactive mode (Interactive Mode block present)

A human is at the keyboard. The orchestrator delivers their answers and resumes your run.

**Hard rule: every message to the human MUST be a `poe:chat` event.** If you write bare prose text it is captured in the debug log only — it does not reach the user. There is no other channel.

- Use `poe:chat` for every question, acknowledgement, or response you send.
- Follow `poe:chat` immediately with `poe:yield`, then stop. Do not emit anything else.
- When the orchestrator resumes you with `Human: {response}`, your output is: optional `poe:artifact` (draft update), then `poe:chat` (next question or conclusion), then `poe:yield` — nothing else.
- After 3–5 rounds emit the final artifact and `poe:done` instead of another `poe:chat` + `poe:yield`.

The specialist skill's `## Interactive Mode` section defines the specific questions and sequence. This base rule governs the output channel.

## poe: Event Wire Format

All structured communication is JSON lines on stdout. One event per line. No multi-line JSON. No markdown wrappers around events.

```
{"poe": "<event-type>", ...fields}
```

A line is a poe: event if and only if it parses as valid JSON and contains a `"poe"` key. All other lines are raw output — captured to a per-task log file, not processed by the orchestrator.

### Event Reference

```
{"poe": "brief", "content": "..."}
{"poe": "step", "name": "...", "detail": "..."}
{"poe": "artifact", "name": "<filename>", "artifact_type": "<type>", "content": "..."}
{"poe": "knowledge", "key": "<slug>", "content": "...", "supersedes": "<prior-id>"}
{"poe": "decision", "question": "...", "options": ["Option A", "Option B"]}
{"poe": "chat", "content": "...", "id": "<turn-id>"}
{"poe": "yield"}
{"poe": "review", "reviewer_skill": "<skill-id>", "content": "..."}
{"poe": "task", "id": "<uuid>", "title": "...", "description": "...", "skill": "<skill-id>", "type": "task", "parent_id": "<id>", "depends_on": ["<id>"]}
{"poe": "task:update", "id": "<task-id>", "title": "...", "description": "...", "skill": "..."}
{"poe": "edge", "from": "<task-id>", "to": "<task-id>"}
{"poe": "skill", "name": "<skill-id>", "content": "<full SKILL.md markdown>"}
{"poe": "done", "summary": "..."}
```

`poe:chat` — interactive mode only. Sends a message to the human. Must be followed immediately by `poe:yield`. The `id` field is optional but recommended — use a stable slug like `"c1"`, `"c2"` to identify the turn.

`poe:yield` — suspends the task (status = waiting). The ingester derives the yield reason from the last substantive event emitted before this one (`chat`, `decision`, or `review`). Emit `poe:yield` as the **last** event before the process exits. Do NOT add a `reason` field — it is ignored.

**Critical**: Every poe: event must be emitted as a JSON line — including `poe:done`. Writing `poe:done` as plain text does nothing. The orchestrator only processes lines that parse as valid JSON with a `"poe"` key.

`detail` in `poe:step` is optional. `summary` in `poe:done` is optional. `options` in `poe:decision` is optional (include when you have identified candidates). `supersedes` in `poe:knowledge` is optional.

### Artifact Content Format

The `content` field is the full document as a string. Escape newlines as `\n`. No whitespace between JSON fields. Do not wrap the event line in a code fence. Do not add any text after the artifact line.

### Artifact-Task Sync Rule

When you emit `poe:artifact`, emit a corresponding `poe:task:update` for every task whose scope is affected by the artifact's content. Document and task must stay in sync. If producing `conops.md` changes what a downstream task needs to do, update that task's description immediately after the artifact.

## poe:review Usage

Emit `poe:review` when you need a peer specialist's judgment to proceed — you cannot resolve the question from the artifact corpus and knowledge register alone, and guessing wrong will cause downstream rework.

```
{"poe": "review", "reviewer_skill": "senior-engineer", "content": "Specific question requiring review."}
```

The orchestrator will:
1. Spawn the named reviewer agent
2. Inject your task context + artifacts + the review question into their input bundle
3. Block you (status = waiting) until the reviewer emits `poe:done`
4. Inject the reviewer's artifact into your context
5. Resume you

Do not use `poe:review` for preference checks or approval-seeking. If you know the answer, act on it. Reserve `poe:review` for substantive questions where the wrong answer causes rework.

## poe:decision Usage

Emit `poe:decision` when you encounter a genuine blocker requiring human judgment — a business or product decision, a scope boundary, or a constraint not resolvable from the artifact corpus alone.

```
{"poe": "decision", "question": "...", "options": ["Option A — description", "Option B — description"]}
```

- Include options when you have identified candidates; omit when the space is genuinely open.
- Continue working on everything that does not depend on the blocked question.
- A high `poe:decision` volume signals that the upstream CONOPS or Guardrails stage produced insufficient clarity — it is not a normal working state.

## Artifact Types

| Artifact type | Filename pattern | Produced by |
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

## Required Event Sequence

Every proactive specialist (all skills except `senior-engineer`) must follow this sequence:

1. `poe:brief` — first event, always. Externalises your interpretation of the task before work begins.
2. `poe:step` — at each meaningful phase of work (2–5 milestones is typical).
3. Primary outputs — `poe:artifact`, `poe:task`, `poe:edge`, `poe:knowledge` — interleaved with steps.
4. `poe:decision` — when genuinely blocked. Continue unblocked work in parallel.
5. `poe:done` — last event, always.

The `senior-engineer` skill operates in two modes — see `senior-engineer.md` and the section below.

## Dual Activation Mode

Some specialist skills support two activation modes, selected by the `**Type**` field in the T section of the stdin bundle:

- **`task`** — standard task execution. Agent runs a lifecycle stage, produces artifact outputs, emits `poe:done`.
- **`plan_review`** — plan review mode. Agent was spawned by the orchestrator in response to a `poe:review` event from the product-manager. Agent reviews the relevant subset of the plan for its domain and emits a structured verdict artifact.

Skills that support plan-review mode: `senior-engineer`, `architecture-analyst`, `interface-analyst`, `data-model-analyst`.

### Detecting plan-review mode

At the start of execution, read the T section header:

```
**Type**: plan_review   → activate plan-review mode
**Type**: task          → activate standard task execution mode
```

### Plan-review mode behaviour

When `**Type**: plan_review`:

1. Emit `poe:brief` summarising: "Reviewing plan subset for `<domain>` domain."
2. Read the `## Review Request` section — this contains the review content and the requesting task.
3. Review the plan for your domain (see domain-specific checklist in your skill file).
4. Emit a single `poe:artifact` with `artifact_type: plan-review` containing your findings and verdict.
5. Emit `poe:done`.

**Do not** emit `poe:task`, `poe:edge`, or domain artifact types (`architecture-constraints`, etc.) in plan-review mode. Your only output is the review artifact.

### Plan-review verdict format

The verdict field in the artifact content must be one of these exact strings (machine-parsed by the orchestrator):

```
APPROVED
APPROVED_WITH_CONDITIONS
BLOCKED
```

Conditions and blockers must be specific: reference the task ID, the artifact section, and the required fix. The product-manager reads your verdict to decide whether to revise the plan or proceed to execution.

## Quality Gate

Before emitting `poe:done`, verify:

- [ ] `poe:brief` was the first event emitted
- [ ] All major phases covered by `poe:step` events
- [ ] All artifacts emitted with correct `artifact_type` (see table above)
- [ ] No empty or placeholder content fields
- [ ] `poe:decision` raised only for genuine blockers, not preference questions
- [ ] `poe:done` is the final event emitted
