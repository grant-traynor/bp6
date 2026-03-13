# After-Action Review — wordle_005
**Date**: 2026-03-13
**Status**: Draft
**Run target**: Wordle clone (Stage 1 MVP) via POE2 orchestrator
**Baseline**: wordle_004 (42:51 total, ≤5-minute target)

---

## Observations (Live Run)

### Regressions / Bugs

| # | Observation | Severity | Data-confirmed? |
|---|---|---|---|
| 1 | No "thinking" indicator in CONOPS chat resume — silent pause between human reply and agent response | High | Behaviour observed |
| 2 | Activity feed panel not discoverable | High | Behaviour observed |
| 3/4 | Agent count stale in top bar and project level — shows "No agents running" while agents active | High | Behaviour observed |
| 9/11/17/24 | Every `poe:step` and `poe:brief` appearing twice in activity feed | Critical | **Confirmed frontend** — events table has zero duplicate entries per agent; DB is clean. Double-subscription on `poe-agent-activity` Tauri listener in frontend |
| 13 | BLOCKED verdict not escalating to human decision queue | High | **Confirmed** — `queue_items` table has 0 rows for entire run |
| 19 | ReviewResult bundle not carrying approved artifact reference to PM — PM re-submits wrong baseline after BLOCKED | Critical | Behaviour observed; supported by multiple BLOCKED plan review cycles in data |
| 20 | No convergence guard on BLOCKED planning loop | High | Plan review nodes show retry_count 1–2; loop did eventually resolve but required extra cycles |
| 22/23/39 | Stage gate not blocking execution dispatch | Critical | **Confirmed** — `phases` table: ALL phases have `gate_held=0` for entire run. Gate was never set. |
| 28 | Must-not open questions not surfaced to decision queue | High | **Confirmed** — `queue_items` table: 0 rows total. No decisions ever created. |
| 32/55 | WBS showing FEAT/EPIC nodes as open after all children complete | High | **Corrected** — nodes table shows all epics and features are `complete` in DB. u7s.5 worked correctly. This is a **frontend rendering bug** — WBS not reflecting DB state. |
| 52 | Agent context injection thin — agents re-detecting project from scratch | Medium | Behaviour observed |
| 58 | Advance button wired to wrong signal (`DagStructureChanged` not `advance_phase`) | Critical | Confirmed in logs |
| 59 | "Ask Advisor" spawns agents but no advisor UI displayed | High | **Confirmed** — two advisor nodes with `status='running'` in nodes table (created 23:55, 23:56), never resolved. Agents spawned, frontend never rendered interaction panel. |
| 34 | `brew upgrade claude-code` leaking into agent output stream | Medium | Behaviour observed |

### UX Gaps

| # | Observation |
|---|---|
| 6/7 | Phase labels ("Do"/"Plan") not meaningful; PDCA mini-track with current phase highlighted suggested |
| 8 | No feedback during DAG mutation (poe:task / poe:edge events) — silent planning phase |
| 31 | WBS shows no in-progress state — no running indicator for active agents |
| 33 | No dependency visibility in WBS — Gantt-style grid with column=dependency depth, colour=state requested |
| 41 | Inter-agent context passing opaque — cannot inspect T+S+K bundle or artifact handoffs |
| 42 | All generated code in project root — no output path convention |
| 47 | Activity feed missing agent/skill identity per entry; task context bleeding into every subtitle row |
| 50 | No drill-down on WBS items |

### Positives

| # | Observation |
|---|---|
| 5 | CONOPS → must-nots → PM workflow sequencing correct |
| 10/16/44 | `poe:review-outcome` working correctly across plan and execution review phases |
| 15 | Artifact versioning working (review-r-eng.md / review-r-eng-2.md) |
| 21/30 | Self-healing loop confirmed end-to-end |
| 25/27/29 | Parallel execution working — multiple agents active simultaneously |
| 46/49 | APPROVED_WITH_CONDITIONS triggering rework; test→review→fix→retest loop working |
| 51/54 | All tests passing; arch/scope verification checks passing |
| u7s.5 | Hierarchy auto-close working correctly in DB — epics and features all closed. Frontend rendering was the issue, not the orchestrator. |

---

## Data Analysis

### Run Time & Cost

| Metric | wordle_004 | wordle_005 | Delta |
|---|---|---|---|
| Total elapsed | 42:51 | **81:06** | +38 min (89% worse) |
| Start | — | 22:24:02 | — |
| End | — | 23:45:10 | — |
| Total cost | — | **$20.82** | — |

**Significantly worse than baseline.** Target was ≤5 minutes. Root cause breakdown follows.

#### Token Breakdown (wordle_005)

| Token type | Count | Cost | Notes |
|---|---|---|---|
| Input (non-cached) | 6,592 | $0.02 | Minimal — cache working well |
| Output | 485,508 | $7.28 | Dominant cost driver |
| Cache write | 1,833,849 | $6.88 | T+S+K bundles written to cache |
| Cache read | 22,119,282 | $6.64 | 10× cheaper than raw input |
| **Total** | | **$20.82** | 58 agents, Sonnet 4.6 pricing |

Cache is functioning correctly — 22M cache read tokens vs 6.6K raw input is a 10× cost saving on context. Output tokens ($7.28) are the largest single cost; reducing agent retries and improving first-pass quality is the primary lever for cost reduction. The 4× `t-anim-test` retry cycle alone likely contributed ~$1.50–2.00 in unnecessary output.

### Phase Timeline

| Phase | Start | End | Duration | Notes |
|---|---|---|---|---|
| CONOPS | 22:24:02 | 22:25:58 | 1:56 | Chat cycle with human |
| Guardrails | 22:27:54 | 22:27:54 | ~0 | Instant — likely single must-nots agent |
| Increment Planning | 22:31:50 | 22:48:18 | **16:28** | Multiple BLOCKED cycles, 8 plan review agents |
| Gap (gate bypass) | 22:48:18 | 22:49:44 | 1:26 | No human approval — execution started anyway |
| Execution | 22:49:44 | 23:45:10 | **55:26** | Includes rework and post-execution review |

### Time Cost Breakdown — Execution Phase

| Cause | Estimated Cost | Evidence |
|---|---|---|
| t-anim-test: 4 failed retries before pass | ~14 min | agents table: 5 rows for t-anim-test (22:02–23:16) |
| Post-execution review + rework cycle | ~22 min | poe:brief at 23:23, final poe:done at 23:45 |
| Increment planning — BLOCKED loop | ~10 min | 8 plan_review agents, retry_count up to 2 |
| Normal execution (parallel tasks) | ~20 min | 22:49 to 23:02 first review trigger |

### Duplicate Agent Analysis

**Finding: duplicate agents are legitimate sequential retries, not race condition spawns.**

The events table has **zero duplicate `poe:step` entries per agent** — every agent's events are unique. The double-rendering in the UI is a frontend double-subscription bug on `poe-agent-activity`, not an orchestrator problem.

The agents table does show multiple agents per node — but inspection confirms they are sequential (each starts when the previous exits), not simultaneous. These are correct retry cycles (u7s.2 working).

Notable retry counts:
- `t-anim-test`: 4 retries (animation smoke test requires a live browser — test agent cannot run this headlessly; the skill needs fixing)
- Plan review nodes: 1–2 retries (BLOCKED → revise → retry — expected)
- Test nodes `t-a11y-keyboard`, `t-input-test`: 2 retries each

### Stage Gate

**`gate_held=0` on all phases for the entire run.** The gate flag was never set. This means the execution phase was always technically dispatchable — the gate enforcement logic is either not setting the flag or not checking it in `db_find_ready_tasks`.

### Decision Queue

**0 queue items created during the entire run.** **Confirmed: must-not analyst emitted zero `poe:decision` events.** Open questions were resolved autonomously — the skill is not escalating ambiguous constraints to the human queue. Skill behaviour issue, not an ingester bug.

### Advisor

Two advisor nodes stuck in `running` status, created after the run was otherwise complete. The agents were spawned (session IDs assigned) but the frontend never displayed the interaction panel. The advisor sessions are orphaned.

### Hierarchy Close (u7s.5)

**Working correctly in the orchestrator.** All 4 epics and all 10 features are `complete` in the nodes table. The WBS appearing to show them open was a frontend rendering issue — the UI was not subscribing to the node status change events that close container nodes, or not re-rendering the WBS after those events.

---

## Root Causes

### RC-1: 81-minute run time (vs 42-minute baseline)
Primary drivers:
1. `t-anim-test` failing 4× due to skill mismatch (animation smoke test requires live browser — test agent cannot satisfy this headlessly). **14 minutes wasted.**
2. Post-execution review + rework cycle took 22 minutes — longer than the execution itself.
3. Increment planning BLOCKED loop added ~10 minutes.

The u7s fixes did not cause regression — the extra time is new failure modes in task execution and review cycles.

### RC-2: Stage gate bypass
`gate_held` never set to 1 on any phase. The phase activation logic does not write `gate_held=1` when a phase with a human gate reaches its gate point. The Advance button sending `DagStructureChanged` instead of `advance_phase` is a separate but related bug — even if the gate were correctly set, the Advance button couldn't clear it.

### RC-3: Frontend double-rendering
Single-subscription architecture assumption violated somewhere. `poe-agent-activity` listener being registered twice — likely on component mount without cleanup on unmount, causing accumulation on navigation.

### RC-4: Activity feed WBS not reactive
Node status changes from `close_completed_ancestors` (u7s.5) not propagating to the WBS view. The orchestrator correctly closes containers in the DB but the Tauri event that should trigger a WBS refresh is either not being emitted for container closures or the frontend is not subscribed to it.

### RC-5: Advisor UI missing
The `advisor` node type has a backend path (nodes created, agents spawned, sessions assigned) but no frontend rendering path. The UI panel for advisor interactions was not built or not wired to the advisor node events.

### RC-6: Decision queue not populated
**Confirmed: zero `poe:decision` events emitted by must-not analyst.** Skill is resolving ambiguous constraints autonomously instead of escalating. Fix: update must-not-analyst skill to emit `poe:decision` when a constraint has genuine ambiguity rather than making assumptions.

---

## Proposed Beads — Phase 4.3: Orchestrator & UX Hardening

### P0 — Correctness (block next test run)

| Bead | Title | Root Cause |
|---|---|---|
| bp6-u8a.1 | Fix stage gate: set `gate_held=1` on phase activation and enforce in dispatch | RC-2 |
| bp6-u8a.2 | Fix Advance button: wire to `advance_phase` command not `DagStructureChanged` | RC-2 |
| bp6-u8b.1 | ReviewResult bundle: include reviewer artifact paths so PM can locate approved baseline | RC-1 (planning loop) |
| bp6-u8c.1 | Fix `poe-agent-activity` double-subscription in frontend | RC-3 |
| bp6-u8d.1 | Fix WBS reactivity: emit Tauri event on container node closure, subscribe in WBS view | RC-4 |

### P1 — High Value

| Bead | Title |
|---|---|
| bp6-u8e.1 | Advisor UI: render advisor interaction panel when advisor node is active |
| bp6-u8e.2 | Decision queue routing: verify `poe:decision` → `queue_items` pipeline and surface to UI |
| bp6-u8f.1 | Agent identity in activity feed: show skill/task name per event entry |
| bp6-u8f.2 | WBS in-progress state: show Running indicator on active tasks |
| bp6-u8f.3 | CONOPS chat thinking indicator: spinner during resume processing |
| bp6-u8f.4 | Agent count reactive: fix stale counter in top bar and project level |
| bp6-u8g.1 | Fix `t-anim-test` skill: animation smoke test must not require live browser or must be scoped correctly |

### P2 — Later Iteration

| Bead | Title |
|---|---|
| bp6-u8h.1 | Gantt dependency grid view in WBS |
| bp6-u8h.2 | WBS drill-down detail panel |
| bp6-u8h.3 | Inter-agent context inspector |
| bp6-u8h.4 | Output path convention for generated code |
| bp6-u8h.5 | Terminal resize + unicode handling: port from bp6 |
| bp6-u8h.6 | Context injection quality: richer T+S+K bundle |
| bp6-u8h.7 | Idle sweep: close terminal containers on 0-running/0-ready condition |
| bp6-u8h.8 | BLOCKED verdict loop convergence guard: escalate to human after N cycles |

---

## Open Questions (Resolved)

| Question | Answer |
|---|---|
| Duplicate spawns: real or frontend? | **Frontend double-subscription.** DB has zero duplicate events. |
| Were hierarchy containers actually closed? | **Yes.** All epics/features complete in DB. Frontend rendering bug only. |
| Was gate_held ever set? | **No.** 0 on all phases entire run. |
| Did decision queue get populated? | **No.** 0 queue_items rows. |
| What happened with Ask Advisor? | Two advisor nodes stuck running, no UI rendered. |
| Total elapsed vs wordle_004? | **81 min vs 43 min — 89% worse.** Primary driver: t-anim-test 4× retry (14 min) + long rework cycle (22 min). |
