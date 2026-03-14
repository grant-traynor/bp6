# bp6-7vp — Move DAG Management Into MCP Tooling

**Status**: Open
**Priority**: P1
**Type**: Feature
**Owner**: Grant Traynor
**Created**: 2026-03-14
**Last updated**: 2026-03-15

---

## Overview

Replace the `poe:task` / `poe:edge` stdout event mechanism with a DAG Service MCP server embedded in POE. Agents interact with the task graph directly via MCP tool calls instead of emitting structured events into their stdout stream. This work also introduced the Phase/Stage model split across all four POE design documents.

---

## Background

The prior design routed all DAG mutations (create task, add edge, update node, cancel task) through `poe:task` and `poe:edge` events in the agent's `--output-format stream-json` stdout. This created a failure mode observed in test run wordle_009: agents could narrate intended mutations as `poe:step` detail text or review content without emitting the actual protocol events — the plan existed only in narrative, not in the database. The execution phase activated with zero tasks as a result.

---

## What Changes

### New: poe-dag-mcp binary

A new MCP server binary (`poe-dag-mcp`) is added to the app bundle. Claude spawns a new `poe-dag-mcp` subprocess per agent invocation via the MCP stdio model (one subprocess per running agent). POE does not pre-spawn a singleton — it writes `mcp-config.json` and listens on `dag.sock` before any agent is dispatched. Multiple concurrent `poe-dag-mcp` instances share the same `dag.db` (safe under SQLite WAL) and the same `dag.sock`.

### New: mcp-config.json

The orchestrator writes `{project.path}/.poe/mcp-config.json` before first agent spawn. Every agent on the project receives `--mcp-config {project.path}/.poe/mcp-config.json` in its spawn command. Format:

```json
{
  "mcpServers": {
    "poe": {
      "command": "{app.resource_dir}/poe-dag-mcp",
      "args": ["--project-id", "{id}", "--db", "{path}/.poe/dag.db", "--socket", "{path}/.poe/dag.sock"]
    }
  }
}
```

### DAG Service Tool Surface (14 tools)

Agents call tools prefixed `poe__<name>`:

**Task CRUD**: `create_task`, `get_task`, `get_phase_wbs`, `query_tasks`, `update_task`, `cancel_task`
**Edges**: `add_edge`, `remove_edge`
**Knowledge/Artifacts**: `write_knowledge`, `query_knowledge`, `register_artifact`, `get_artifact`
**Execution support**: `run_tests`, `git_status`

After every mutation, the DAG Service commits to SQLite, sends a `DagChanged` notification to the orchestrator via `dag.sock`, and relays a Tauri event to the frontend.

### Removed: poe:task and poe:edge protocol events

`poe:task`, `poe:task:update`, `poe:task:cancel`, `poe:edge`, and `poe:edge:remove` are removed from the `poe:` protocol entirely. The ingester no longer handles these event types. The `poe:` protocol is control flow and observability only.

### Updated: Agent spawn command

```
claude --output-format stream-json --verbose -p --dangerously-skip-permissions \
       --mcp-config {project.path}/.poe/mcp-config.json
```

### Updated: Orchestrator notification

The orchestrator scheduling loop now wakes from four sources:

1. **DAG Service socket notification** — after any MCP mutation tool call
2. **poe: ingester** — on `poe:done`, `poe:yield`
3. **Human gate commands** — `advance_stage`, `revise_stage`, `rerun_stage`, `activate_phase`, `advance_phase`
4. **Decision/chat resolution** — `resolve_decision`, `respond_to_chat`, `respond_to_advisor`

### Updated: Reviewer flow

Reviewers call `get_phase_wbs` to read the live task graph directly. The `poe:review` content field is a review directive (task IDs + focus area), not a plan transcription. Reviewers do not call mutation tools.

### Updated: Skills

- `product-manager.md`: replace Phase 3–6 instructions (epic/feature/task creation, dependency edges) from `poe:task`/`poe:edge` emission to `create_task`, `add_edge` MCP tool calls
- Reviewer skills: replace bundle-content reading with `get_phase_wbs`/`get_task` tool calls

### Updated: Phase/Stage model

Introduced the correct two-level model across all design documents:

- **Phase** — a scope iteration ("Initial Prototype", "Feature A"). Owns the WBS; `nodes.phase_id` references `phases(id)`.
- **Stage** — a process step within a phase (`increment_planning`, `execution`, `retrospective`, etc.). Lives in the `stages` table with its own lifecycle (`pending → running → gate → complete`).

The schema now has separate `phases` and `stages` tables. `projects` tracks both `active_phase_id` and `active_stage_id`. Stage gate (`status='gate'`) lives on `stages`, not `phases`. Phase advances only when all its stages are complete. Bootstrap is per-stage (`maybe_bootstrap_stage`), triggered by `activate_stage`.

---

## Why This Fixes the Root Cause

With the DAG Service, the write is the proof. If the product-manager hasn't called `create_task`, the reviewer calling `get_phase_wbs` finds an empty graph — there is no middle ground where the plan exists as text but not as data. The narrate-instead-of-emit failure mode is architecturally eliminated.

---

## Docs Updated (all committed)

| Document | Changes |
|---|---|
| `doc-POE/Concept.md` | Phase/Stage model; Agent Protocol section; stage type table |
| `doc-POE/Architecture.md` | Phase/Stage schema; Stages section added; MCP subprocess model corrected; DAG Service as primary write interface |
| `doc-POE/Protocol.md` | §6 DAG Service (MCP) added; `phases`+`stages` schema; `poe:task`/`poe:edge` removed; `poe-stage-update` event; Tauri commands for stage/phase lifecycle; `status='complete'` throughout nodes table |
| `doc-POE/Flows.md` | DAG Service model throughout; SF-5 phases+stages activation; SF-7 invariants; SF-2 stage completion gate corrected; SF-3/§3.1 `status='complete'` in reviewer checks; §3.5 `pause_stage` corrected to `stage_id`; §3.7 Stage Closure & Gate rewrite; §4.6 Stage Activation (not Phase); SF-1 gate commands corrected |

---

## Implementation Scope

1. Implement `poe-dag-mcp` binary (Rust) with all 14 tools
2. Add `dag.sock` Unix socket server to orchestrator
3. Write `mcp-config.json` at project open time
4. Update agent spawn command to include `--mcp-config`
5. Remove `poe:task`/`poe:edge` ingester handlers
6. Update `product-manager.md` skill prompt
7. Update reviewer skill prompts
8. Implement `phases` + `stages` tables; migrate from single `phases` table
9. Implement stage lifecycle Tauri commands: `create_stage`, `activate_stage`, `advance_stage`, `revise_stage`, `rerun_stage`
10. Add `active_stage_id` to `projects` table

---

## Notes

### Phase/Stage Model Corrections (Flows.md)

Additional fixes to Flows.md after Phase/Stage model introduction:

- **SF-2 stage completion**: "transition phase → status=gate" corrected to "transition stage → status=gate"; emit `poe-stage-update` (not `poe-phase-update`)
- **SF-2/SF-3/§3.1 reviewer completion checks**: `status IN ('done','cancelled')` → `status IN ('complete','cancelled')` throughout — this was a live scheduler bug that would have prevented dependent tasks ever unblocking
- **§3.5 Pause/Abort**: `pause_stage` now takes `stage_id` not `phase_id`; `UPDATE stages` (not `phases`) SET `status='paused'`; Tauri event corrected to `poe-stage-update`; walkthrough text corrected
- **SF-1 human gate commands**: removed `revise_phase`/`rerun_phase` (eliminated by Phase/Stage split); corrected to `advance_stage`/`revise_stage`/`rerun_stage`/`activate_phase`/`advance_phase`
- **§4.6**: "Phase Activation with No Bootstrap Skill" → "Stage Activation with No Bootstrap Skill"; trigger corrected to `activate_stage`/`advance_stage`; `maybe_bootstrap_phase` → `maybe_bootstrap_stage` throughout
- **§3.7**: Full rewrite of Stage Closure & Gate — gate lives on `stages`, not `phases`; `advance_stage`/`revise_stage`/`rerun_stage` commands; cascade to phase advance when all stages complete
- **Flow index**: "Plan Composer → Phase Activation" → "Plan Composer → Stage Activation"
