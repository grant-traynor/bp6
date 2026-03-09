---
id: phase-2.3-plan
title: POE Phase 2.3 — Interactive CONOPS & Wire Protocol Proof
status: Draft
date: 2026-03-10
---

# POE Phase 2.3 — Interactive CONOPS & Wire Protocol Proof

## 1. Objective

Deliver a working end-to-end demonstration of a human and an agent co-authoring a CONOPS document through the Artifact Viewer's chat panel. By the end of this phase:

- A human opens POE, creates a project, and describes a concept in a few lines.
- The orchestrator dispatches the operational-analyst via SF-1 with no PTY bypass.
- The agent elicits requirements through a `poe:chat` round-trip conversation.
- The Artifact Viewer opens automatically in chat-active mode; the human sees the evolving artifact on the left and the conversation on the right.
- After sufficient rounds, the agent writes the final `conops.md` and emits `poe:done`.
- The artifact is stored and visible in the Artifact Viewer.

This proves the full interactive wire protocol, the SF-1 → poe:chat → SF-4 orchestrator path, the Artifact Viewer dual-state UI, and the `operational-analyst` skill's interactive mode. No PTY interfaces are involved.

---

## 2. Scope

### In scope

| Area | Item |
|---|---|
| Protocol | `poe:chat` ingestion, `chat_turns` schema, `respond_to_chat` command, `poe://chat-turn` Tauri event |
| Protocol | Remove `reason` field from `poe:yield`; derive intent from `yield_reason` column only |
| Orchestrator | SF-4 chat path — detect `yield_reason='chat'`, await responded turn, assemble `Human: {response}` continuation |
| Orchestrator | SF-1 dispatch fix — ConopsLauncher must create task as `pending`, not `waiting`; no direct PTY call |
| Skill | Rewrite `operational-analyst` for interactive mode: `poe:chat` round-trip elicitation |
| UI | Artifact Viewer: add chat panel (right side), activate on `poe://chat-turn`, `respond_to_chat` submit |
| UI | Artifact Viewer: "Chat about this" button for human-initiated sessions on completed artifacts |
| UI | Activity feed: add `poe:chat` entry type |

### Out of scope

- PTY / `agent_handover_open` interfaces — deferred
- Reviewer watchdog and `poe:review` path — already implemented, not touched
- Decision Queue conversational thread mode (bp6-7ct.7) — separate
- Plan Composer, Stage Gate, Knowledge Register UI — Phase 3
- Skill authoring for any skill other than `operational-analyst`

---

## 3. Current State

### What exists

| Component | State |
|---|---|
| `JsonStreamTransport` | Complete — stream-json, `--resume`, model override |
| `event_ingester` | Handles `poe:brief`, `poe:step`, `poe:artifact`, `poe:yield`, `poe:done`, `poe:decision`, `poe:review`, `poe:task`, `poe:edge`, `poe:knowledge`, `poe:skill` |
| `orchestrator` (SF-1) | Dispatches `pending` tasks — T+S+K bundle assembly, skill mode guard, concurrency limits |
| `orchestrator` (SF-2) | Agent completion handling |
| `orchestrator` (SF-3) | Yield detection — reads `yield_reason` |
| `orchestrator` (SF-4) | Decision + review continuation via `--resume` — only `yield_reason='decision'` and `yield_reason='review'` paths |
| `orchestrator` recovery | Waiting task recovery on restart |
| `ArtifactViewer.tsx` | Read-only artifact viewer — no chat panel |
| `QueuePanel.tsx` | Decision queue — `poe:decision` items |
| `ActivityFeed.tsx` | Feeds from `poe:` event log — no `poe:chat` entry |
| `operational-analyst` skill | **Single-pass autonomous** — rewrites required |
| `ConopsLauncher` | Creates task with `initialStatus: 'waiting'`, immediately calls `agent_handover_open` — **bypasses orchestrator** |

### What is missing

| Gap | Description |
|---|---|
| `poe:chat` ingester handler | No handler for `{"poe": "chat", ...}` events |
| `chat_turns` table | Not in SQLite schema |
| `poe://chat-turn` Tauri event | Not emitted |
| `respond_to_chat` command | Not implemented |
| SF-4 chat path | Orchestrator has no logic for `yield_reason='chat'` |
| Artifact Viewer chat panel | Not built |
| `operational-analyst` interactive mode | Skill written for single-pass; no `poe:chat` usage |
| ConopsLauncher fix | Wrong `initialStatus`, wrong launch path |
| `poe:yield reason` removal | `reason` field still present; agreed to remove as redundant |

---

## 4. UX Components

### 4a. Artifact Viewer (modified)

**Current**: read-only viewer, renders artifact markdown, no interaction.

**Phase 2.3 changes**:

- Add **chat panel** (right-side, collapsible). Renders `chat_turns` for the current task.
- Panel activates automatically when `poe://chat-turn` arrives for the viewed task's `task_id`.
- Each turn shows: agent message (agent turn) or human message (human turn), with timestamp.
- Input field + "Send" button at bottom of chat panel. On submit: `invoke("respond_to_chat", {turn_id, response})`.
- Left panel (artifact) updates live on each `poe://event` of type `poe:artifact`.
- **"Chat about this" button** in viewer toolbar — visible when the artifact's producing task is `done`. Clicking dispatches a new interactive session.
- Chat panel closes (collapses, state preserved) when task reaches `done`. Shows completion banner.

**Layout** (chat-active state):

```
┌─ Artifact Viewer ──────────────────────────────────────────────────────┐
│  [← Back]   conops.md                              [Chat about this]   │
│ ┌───────────────────────────────────┬────────────────────────────────┐ │
│ │  Artifact                         │  Chat                          │ │
│ │  ─────────                        │  ──────                        │ │
│ │  (rendered markdown, live)        │  ● Analyst: question text      │ │
│ │                                   │  ● You: your reply             │ │
│ │                                   │  ● Analyst: next question      │ │
│ │                                   │                                │ │
│ │                                   │  [ type response...         ]  │ │
│ │                                   │                    [ Send ↵ ]  │ │
│ └───────────────────────────────────┴────────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────┘
```

### 4b. Activity Feed (modified)

Add `poe:chat` entry type:

| Event | Feed entry |
|---|---|
| `poe:chat` | `Chat turn — {first 60 chars of content}` → click opens Artifact Viewer in chat-active mode |

### 4c. ConopsLauncher (fix)

Replace:
```typescript
initialStatus: 'waiting',
// ...
onOpenSession(node.id);        // PTY bypass — remove
```

With:
```typescript
initialStatus: 'pending',
// onOpenSession call removed — orchestrator handles dispatch
```

The Artifact Viewer opens automatically when the orchestrator dispatches the task and the first `poe://chat-turn` arrives. The human does not need to trigger it.

---

## 5. Protocol Elements

### 5a. New: `poe:chat` event

```json
{"poe": "chat", "content": "What problem does this system solve?", "id": "c1"}
```

- Emitted by interactive agents before `poe:yield`.
- `id` is optional; when present, links the turn to its response in `chat_turns`.
- Routes to Artifact Viewer chat panel. Does not appear in Decision Queue.

### 5b. Remove: `reason` field from `poe:yield`

**Before** (current):
```json
{"poe": "yield", "reason": "chat"}
```

**After** (Phase 2.3):
```json
{"poe": "yield"}
```

The `yield_reason` column on `nodes` is set by the ingester based on what the agent emitted *before* yielding:
- If the agent emitted `poe:decision` before yielding → `yield_reason = 'decision'`
- If the agent emitted `poe:chat` before yielding → `yield_reason = 'chat'`
- If the agent emitted `poe:review` before yielding → `yield_reason = 'review'`
- Otherwise → `yield_reason = NULL` (plain yield — waiting for external event)

The ingester tracks "last substantive poe: event before poe:yield" to set `yield_reason`. The `reason` field on the wire is eliminated.

### 5c. New: `chat_turns` table

```sql
CREATE TABLE IF NOT EXISTS chat_turns (
    id          TEXT PRIMARY KEY,          -- matches poe:chat "id" field; generated if absent
    task_id     TEXT NOT NULL,
    content     TEXT NOT NULL,             -- agent's message
    response    TEXT,                      -- human's reply; NULL until responded
    created_at  TEXT NOT NULL,
    responded_at TEXT                      -- NULL until responded
);
```

### 5d. New: `poe://chat-turn` Tauri event

```
poe://chat-turn  →  {turn_id, task_id, content}
```

Emitted by the ingester when a `poe:chat` event is processed. Frontend listens to open/update the Artifact Viewer chat panel.

### 5e. New: `respond_to_chat` Tauri command

```
invoke("respond_to_chat", {project_id, turn_id, response: "..."})
```

Rust handler:
1. Updates `chat_turns` — set `response` and `responded_at`.
2. Signals orchestrator via `DagChanged::QueueItemResolved` — reuse this variant (`turn_id → item_id`, `response text → resolution`). No new variant needed.
3. Orchestrator wakes, identifies waiting task with `yield_reason='chat'`, triggers SF-4.

---

## 6. Runtime Flows

### SF-3 (updated) — Yield dispatch

Current SF-3 reads `yield_reason` and routes. After Phase 2.3:

```
yield_reason = 'decision'  → await resolve_decision() → SF-4
yield_reason = 'review'    → dispatch reviewer task → SF-4 (on reviewer done)
yield_reason = 'chat'      → await respond_to_chat() → SF-4     ← NEW
yield_reason = NULL         → wait (no auto-resume)
```

### SF-4 (updated) — Agent continuation

The chat path continuation bundle is identical to decision resolution:

```
stdin input:  "Human: {response text}"
spawn:        claude --resume {session_id} --output-format stream-json -p --dangerously-skip-permissions
```

SF-4 checks:
- `yield_reason = 'chat'` AND `chat_turns` row with `task_id = node.id` AND `responded_at IS NOT NULL` → assemble bundle and resume.

### SF-1 (fix) — Task dispatch via orchestrator

ConopsLauncher creates a `pending` task. The orchestrator's normal scheduling loop picks it up, assembles the T+S+K bundle with the **interactive mode protocol block** prepended (because `operational-analyst` declares `modes: [autonomous, interactive]` and the task is human-initiated), and spawns the agent.

The interactive mode protocol block (from `agent_lifecycle::assemble_bundle`) instructs the agent to use `poe:chat` for elicitation rather than writing directly.

Full flow for the CONOPS case:

```
Human submits concept text
  → ConopsLauncher: create_node(pending, skill=operational-analyst)
  → Orchestrator: SF-1 picks up pending task
  → Orchestrator: assemble T+S+K bundle (interactive mode block prepended)
  → agent_lifecycle: spawn_agent (fresh — no session_id)
  → Agent reads bundle, emits poe:brief
  → Agent emits poe:chat "What problem does this solve?" + poe:yield
  → Ingester: INSERT chat_turns, UPDATE nodes SET yield_reason='chat', status='waiting'
  → Ingester: emit poe://chat-turn
  → Frontend: Artifact Viewer opens in chat-active mode
  → Human types response → invoke("respond_to_chat")
  → Orchestrator: DagChanged → SF-4
  → agent_lifecycle: spawn_agent --resume, stdin "Human: {response}"
  → Agent continues — emits poe:artifact (draft), then poe:chat "next question" + poe:yield
  → [cycle repeats]
  → Agent emits final poe:artifact + poe:done
  → Ingester: SF-2 — task done
  → Frontend: completion banner
```

---

## 7. Architecture Components

| Component | Role in Phase 2.3 | Change |
|---|---|---|
| `event_ingester` | Parse `poe:chat`; INSERT `chat_turns`; emit `poe://chat-turn`; track last-event-before-yield to set `yield_reason` | **Add** `poe:chat` handler; **modify** `poe:yield` handler to derive `yield_reason` from prior event |
| `dag_store` | `chat_turns` table; `respond_to_chat` write path | **Add** table + RPC |
| `orchestrator` (SF-3) | Route `yield_reason='chat'` | **Add** chat arm |
| `orchestrator` (SF-4) | Check `chat_turns` for responded turn; assemble continuation | **Add** chat arm |
| `agent_lifecycle` | Bundle assembly — interactive mode block for `poe:chat` agents | **Already implemented** — no change needed |
| `skills/mod.rs` | Mode guard, bundle assembly | **Already implemented** — no change needed |
| `operational-analyst.md` | Rewrite for interactive `poe:chat` elicitation | **Full rewrite** |
| `App.tsx` / `ConopsLauncher` | Fix `initialStatus`, remove PTY call | **Fix** |
| `ArtifactViewer.tsx` | Add chat panel, `poe://chat-turn` listener, `respond_to_chat` invoke | **Extend** |
| `ActivityFeed.tsx` | Add `poe:chat` entry type | **Add** |

---

## 8. Implementation Tasks (ordered)

### Milestone A — Schema & Protocol (backend)

1. **Schema**: add `chat_turns` table to SQLite migrations.
2. **Remove `reason` from `poe:yield`**: update `PoeYield` struct to remove `reason` field. Update ingester `poe:yield` handler to derive `yield_reason` from ingester state (last emitted event type before the yield).
3. **Add `poe:chat` ingester handler**: INSERT into `chat_turns`, emit `poe://chat-turn` Tauri event, log to `event_log`.
4. **Add `respond_to_chat` Tauri command**: write response to `chat_turns`, signal `DagChanged`.

### Milestone B — Orchestrator (SF-3 + SF-4)

5. **SF-3 chat arm**: when `yield_reason='chat'`, wait for `respond_to_chat` signal (no auto-dispatch).
6. **SF-4 chat arm**: detect chat-waiting task, check `chat_turns` for responded turn, assemble `Human: {response}` bundle, spawn `--resume`.

### Milestone C — Skill

7. **Rewrite `operational-analyst`**: interactive mode `poe:chat` elicitation. Emit `poe:chat` + `poe:yield` per round. Build artifact progressively with `poe:artifact`. Emit `poe:done` when complete. Keep autonomous mode path (single-pass) for non-interactive dispatch.

### Milestone D — Frontend

8. **Fix `ConopsLauncher`**: `initialStatus: 'pending'`, remove `onOpenSession` call.
9. **`ArtifactViewer` chat panel**: right-side panel, `poe://chat-turn` listener, renders `chat_turns`, submit → `invoke("respond_to_chat")`.
10. **Activity feed**: add `poe:chat` entry type.

### Milestone E — Integration test

11. **End-to-end smoke test**: create project, submit concept, verify full `poe:chat` round-trip, verify `conops.md` artifact stored, verify task reaches `done`.

---

## 9. Success Criteria

- [ ] Human submits a concept text; orchestrator dispatches `operational-analyst` via SF-1 with no PTY involvement.
- [ ] Agent emits `poe:chat` + `poe:yield`; task status shows `waiting`; Artifact Viewer opens in chat-active mode automatically.
- [ ] Human types response; `respond_to_chat` wakes SF-4; agent resumes via `--resume`.
- [ ] At least 3 full `poe:chat` round-trips complete without error.
- [ ] Agent emits progressive `poe:artifact` updates visible in left panel on each round.
- [ ] Agent emits `poe:done`; `conops.md` stored in SQLite `artifacts` table and rendered in Artifact Viewer.
- [ ] Activity feed shows all `poe:chat` entries with correct task context.
- [ ] `poe:yield` on the wire has no `reason` field; ingester still sets `yield_reason` correctly.
- [ ] No PTY / `agent_handover_open` calls in the CONOPS path.
