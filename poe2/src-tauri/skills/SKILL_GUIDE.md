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
modes: [autonomous]          # REQUIRED. One or more of: autonomous, interactive.
                             #   autonomous  — run via stream-json -p (no keyboard, poe: events expected)
                             #   interactive — run in a human conversation (poe: events only on concrete output)
                             #   If omitted, orchestrator assumes [autonomous] only.
                             #   The UI blocks interactive sessions for autonomous-only skills.
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
| `modes` | Yes | Declare explicitly — do not rely on the autonomous default |
| `tags` | No | Informational only |
| `applies_to` | No | Informational only |
| `protocol_version` | No | Convention — include `v2` for new skills |

> **Inconsistency note — existing skills**: `implementer.md` and `planner.md` are missing `modes:` and `protocol_version:`. They should be treated as `modes: [autonomous]` per Protocol.md §3. New skills must include `modes:` explicitly.

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

**Senior engineer exception**: `senior-engineer` is reactive (spawned by `poe:review`), not proactive. It follows a different sequence — see `senior-engineer.md` and `poe-base.md §Dual Activation Mode`.

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
{"poe": "artifact", "name": "<filename>", "artifact_type": "<type>", "content": "..."}
{"poe": "knowledge","key": "<slug>", "content": "...", "supersedes": "<id>"}  // supersedes optional
{"poe": "decision", "question": "...", "options": ["A", "B"]}  // options optional
{"poe": "review",   "reviewer_skill": "<id>", "content": "...", "id": "<review-id>"}
{"poe": "task",     "id": "<uuid>", "title": "...", "description": "...", "skill": "<id>",
                    "type": "task", "parent_id": "<id>", "depends_on": ["<id>"]}
{"poe": "edge",     "from": "<task-id>", "to": "<task-id>"}
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

Do not follow the proactive event sequence for reactive skills. See `senior-engineer.md` for the correct pattern.

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

## 6. Common Patterns

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

Every skill should include a quality checklist before `poe:done`. See `product-manager.md` or `validity-analyst.md` for examples. Minimum checklist items:

- [ ] `poe:brief` was the first event emitted
- [ ] All major phases covered by `poe:step` events
- [ ] No empty or placeholder content in artifact fields
- [ ] `poe:done` is the final event emitted

---

## 7. Existing Skill Inventory

| Skill | Stage | Modes | Inconsistencies vs this guide |
|---|---|---|---|
| `poe-base` | Base (inherited by all) | autonomous | None — canonical reference |
| `operational-analyst` | CONOPS | autonomous, interactive | None |
| `architecture-analyst` | Guardrails | autonomous | None |
| `interface-analyst` | Guardrails | autonomous | None |
| `data-model-analyst` | Guardrails | autonomous | None |
| `must-not-analyst` | Guardrails | autonomous | None |
| `senior-engineer` | Guardrails review, Plan Review, ad-hoc | autonomous | None |
| `product-manager` | Increment Planning | autonomous | **v1 event format** — uses `poe:node` (not `poe:task`), `type:` prefix on all events, `POE_WORKFLOW_ID` env var injection. Do not use as a format reference. Fix tracked in bp6-rub.5. |
| `validity-analyst` | Validity Analysis | autonomous | None |
| `rca-analyst` | Retrospective | autonomous | None |
| `implementer` | Execution | autonomous | Missing `modes:` and `protocol_version:`; simplified event protocol section does not reference poe-base.md |
| `planner` | Execution | autonomous | Missing `modes:` and `protocol_version:`; simplified event protocol section does not reference poe-base.md |

**For new skills**: follow this guide. Do not mirror the format from `implementer.md` or `planner.md`.
