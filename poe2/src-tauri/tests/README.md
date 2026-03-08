# POE2 Integration Tests

End-to-end protocol tests that require a live `claude` binary on PATH.
All tests skip gracefully when `claude` is absent — they do not fail CI.

---

## Quick reference

| Harness | File | Auto / Manual | Log |
|---|---|---|---|
| `protocol_harness` | `tests/protocol_harness.rs` | Automated | `target/protocol-harness.log` |
| `stdio_json_harness` | `tests/stdio_json_harness.rs` | Automated | stderr only |
| `stream_json_integration` | `tests/stream_json_integration.rs` | Automated | `target/stream-json-*.log` |
| `decision_handoff_harness` | `tests/decision_handoff_harness.rs` | **Manual (browser)** | `target/decision-handoff.log` |
| `session_handoff_harness` | `tests/session_handoff_harness.rs` | Manual (browser) | `target/session-handoff.log` |

Run all automated tests:

```bash
cd poe2/src-tauri
cargo test --test protocol_harness --test stdio_json_harness --test stream_json_integration
```

---

## protocol_harness

**File**: `tests/protocol_harness.rs`
**Bead**: `bp6-pdr.17`

Validates the full poe: event wire format from bundle injection through the transport layer to parsed events. Spawns a real Claude process via `run_agent_capturing`, injects a deterministic test skill, and asserts that all 11 poe: event types are correctly transmitted and parsed.

Also tests for **PTY column-wrap bug bp6-pdr.13**: injects a 300-character artifact whose JSON line is ~384 chars. If the PTY wraps at col 220, the line is fragmented and the artifact event is never seen.

### How to run

```bash
cd poe2/src-tauri

cargo test --test protocol_harness

# Monitor output live while the test runs
tail -f target/protocol-harness.log

# See output inline (log file is also always written)
cargo test --test protocol_harness -- --nocapture
```

### What each assertion tests

| Assertion | Tests |
|---|---|
| First event is `poe:brief` | Protocol ordering rule: brief must be first |
| Last event is `poe:done` | Protocol ordering rule: done must be last |
| All 11 event types present | Wire format coverage: every event type parses correctly |
| Exactly 2 `poe:artifact` events | Short and long artifact both transmitted |
| `test-short.txt` content non-empty | Short artifact content field intact |
| `test-long.txt` artifact present | PTY column-wrap bug (bp6-pdr.13): long JSON line not fragmented |
| `test-long.txt` content > 220 chars | Artifact content not truncated at PTY column boundary |

`poe:review` is excluded — it requires a live orchestrator.

### Log structure

The log has three sections visible with `--nocapture`:

1. **BUNDLE** — the full test skill injected into Claude
2. **SPAWNING CLAUDE** — one line per PTY output, numbered. Lines marked `[PARSE FAIL — possible PTY wrap fragment]` are JSON-looking lines that failed to parse.
3. **ASSERTIONS** — each event type checked with OK / MISSING

### Common failure modes

**"test-long.txt artifact not found"**
The PTY column-wrap bug (bp6-pdr.13) is present. The `test-long.txt` JSON line (~384 chars) was split at col 220. Fix: increase PTY column width in `agent_lifecycle::spawn_agent`.

**"Missing event type poe:X"**
Claude did not emit that event type. Check the raw PTY output for the corresponding JSON fragment. The test skill may need adjustment.

**"run_agent_capturing failed"**
`claude` could not be spawned. Check that `claude --dangerously-skip-permissions` works from the terminal.

**"Last event is not poe:done" / poe:done missing from observer**
`poe:done` is typically the last line Claude emits and often lacks a trailing newline, so it lives in the `TextBufExtractor` tail until `on_complete` flushes it. If the observer never receives the tail, check that `run_agent_capturing`'s `on_complete` callback calls `on_line` with the flushed tail (this was fixed; the original only pushed to `lines`).

---

## stdio_json_harness

**File**: `tests/stdio_json_harness.rs`

Smoke test for the stream-json transport stack. Uses `run_agent_capturing` — the same path as production `spawn_agent` — with a minimal bundle that asks Claude to emit `poe:done` immediately. Validates that the transport end-to-end works at all.

This is a **smoke test**, not a protocol test. It does not check event types beyond `poe:done`, does not capture `session_id`, and does not exercise the `--resume` path.

### How to run

```bash
cd poe2/src-tauri
cargo test --test stdio_json_harness -- --nocapture
```

### What it asserts

- At least one `poe:` event is received
- `poe:done` is among the received events

### Common failure modes

**"expected poe:done event in output"**
Claude ran but did not emit `poe:done`. Increase verbosity by adding `eprintln!` to the `on_line` callback, or switch to `stream_json_integration` for more detailed diagnostics.

---

## stream_json_integration

**File**: `tests/stream_json_integration.rs`

Integration tests that call `JsonStreamTransport::run()` **directly** — below `run_agent_capturing` — exercising the full callback chain (`on_pid`, `on_session_id`, `on_text_chunk`, `on_raw_json`, `on_complete`) with a live Claude process. Text is routed through `TextBufExtractor → parse_poe_event`, mirroring the production path in `agent_lifecycle::spawn_agent`.

These tests cover what `protocol_harness` and `stdio_json_harness` cannot: `session_id` capture, `on_complete` firing, JSON field-level correctness, long-line integrity confirmation for stream-json (no PTY, so bp6-pdr.13 cannot occur), and the `--resume` continuation path.

### How to run

```bash
cd poe2/src-tauri

# All three tests
cargo test --test stream_json_integration -- --nocapture

# Individual tests
cargo test --test stream_json_integration session_id_captured_from_real_stream -- --nocapture
cargo test --test stream_json_integration full_protocol_payload_correctness -- --nocapture
cargo test --test stream_json_integration resume_continuation -- --nocapture

# Monitor a test live
tail -f target/stream-json-protocol.log
```

### Tests

#### `session_id_captured_from_real_stream`

**Log**: `target/stream-json-session-id.log`

Sends a minimal bundle and asserts that `on_session_id` fires with a non-empty UUID-format string. This validates that the `{"type":"system","subtype":"init","session_id":"..."}` NDJSON event emitted by Claude is correctly parsed by `JsonStreamTransport`.

#### `full_protocol_payload_correctness`

**Log**: `target/stream-json-protocol.log`

Injects the same 13-line deterministic test skill as `protocol_harness` and asserts:

| Assertion | Tests |
|---|---|
| `on_session_id` fires | Init event parsed, session_id captured |
| `on_complete` fires | Result event received, transport exits cleanly |
| First event is `poe:brief` | Protocol ordering |
| Last event is `poe:done` | Protocol ordering |
| All 11 event types present | Wire format coverage |
| `poe:task` has `id == "test-task-001"` | JSON field values correct, not just event type |
| Exactly 2 `poe:artifact` events | Both artifacts received |
| `test-long.txt` content == full 300-char string | Stream-json does not truncate long lines (no PTY, so bp6-pdr.13 cannot occur; failure here indicates a transport regression) |

#### `resume_continuation_captures_new_session_and_emits_done`

**Log**: `target/stream-json-resume.log`

Two-phase test for the `--resume` continuation path:

1. **Session 1**: sends a minimal bundle; asserts `poe:done` arrives and captures the `session_id`.
2. **Session 2**: calls `JsonStreamTransport::run()` with `resume_session_id = Some(&sid)`; asserts `on_session_id` fires, `poe:done` is received, and `on_complete` fires.

This is the only test that exercises the resume path used for `poe:decision` and `poe:review` continuations.

### Common failure modes

**"on_session_id never fired"**
The `{"type":"system","subtype":"init"}` NDJSON event was not emitted or not parsed. Check that `claude --output-format stream-json` emits the init event on startup.

**"on_complete never fired"**
The `{"type":"result"}` NDJSON event was not received. Claude may have exited without completing, or the process was killed.

**"test-long.txt content does not match"**
The 300-char artifact was truncated or fragmented. Stream-json transport reads raw stdout with no PTY — line truncation at this layer would indicate a regression in `process_ndjson_reader` or the `TextBufExtractor`.

**Session 2: "on_session_id never fired"**
`--resume <sid>` did not produce an init event. Verify the session_id from session 1 is valid and that `claude --resume` is supported in the installed version.

---

## decision_handoff_harness

**File**: `tests/decision_handoff_harness.rs`

**Manual / human-runnable** — opens a browser window. Not part of automated CI.

End-to-end test of the full decision → PTY handover → stream-json resume cycle:

1. **Session 1 (stream-json)**: Claude emits `poe:brief`, `poe:decision`, `poe:done`. The `session_id` is captured from the stream-json init event.
2. **PTY handover**: `claude --resume <session_id>` opens in an xterm.js browser window. You can read Claude's decision question and type any response. Close the tab when done.
3. **Session 2 (stream-json)**: Resumes with `--resume <session_id>`. Asserts `poe:done` arrives and `on_complete` fires.

This is the only test that exercises the full loop: stream-json spawn → `poe:decision` event → PTY handover → stream-json resume → `poe:done`.

### How to run

```bash
cd poe2/src-tauri
cargo test --test decision_handoff_harness -- --nocapture

# The test opens a browser window automatically.
# Read Claude's decision question, type any response, then close the tab.

# Monitor live
tail -f target/decision-handoff.log
```

### What it asserts

| Assertion | Tests |
|---|---|
| Session 1: `on_session_id` fires | session_id captured from init event |
| Session 1: `poe:decision` received | decision event emitted and parsed |
| Session 1: `poe:done` received | session 1 completes cleanly |
| PTY: browser WebSocket connects | PTY spawned, xterm.js loaded, WS handshake complete |
| Session 2: `on_session_id` fires | `--resume` path receives init event |
| Session 2: `poe:done` received | resumed session completes |
| Session 2: `on_complete` fires | result event received after resume |

### What the human should do

When the browser window opens, you will see a terminal with Claude's session history. The decision question ("Which path should the orchestrator take?") will be visible. You may:

- Simply read it and close the tab immediately
- Type a response and press Enter, then close the tab

The test continues automatically when the tab is closed (or after a 10-minute timeout).

### Common failure modes

**"No conversation found with session ID: ..."**
All three steps (session 1, PTY, session 2) must use the same working directory. Claude scopes sessions to project directories by cwd hash — if the PTY uses a different cwd than session 1, `--resume` cannot find the session. The test uses `std::env::current_dir()` throughout; running it from an unexpected directory can trigger this.

**"timed out waiting for browser WebSocket connection (60 s)"**
The browser did not open or did not connect. Check the URL printed to the log and open it manually.

**"Session 2: poe:done not received"**
The resumed session did not complete. Check `target/decision-handoff.log` for what Claude emitted in session 2.

---

## session_handoff_harness

**File**: `tests/session_handoff_harness.rs`

**Manual / developer tool** — not part of automated CI.

Runs a stream-json session to completion, then resumes that session interactively in an xterm.js browser window bridged via WebSocket. Lets you inspect session state and continue talking to Claude after an automated test run.

### How to run

```bash
cd poe2/src-tauri

# Fastest: run the stream-json flow inline, then open xterm.js
cargo test --test session_handoff_harness -- --nocapture

# Reuse a session from a previous run (skips the stream-json step)
# The session_id is saved to target/stdio-json-session-id.txt automatically.
cargo test --test session_handoff_harness -- --nocapture
```

The test opens a browser window automatically (`open` on macOS, `xdg-open` on Linux). The xterm.js terminal connects to the resumed Claude session via WebSocket. Close the browser tab to end the session; the test times out after 10 minutes.

### Flow

1. **Session acquisition**: reads `target/stdio-json-session-id.txt` if present; otherwise runs the full protocol test inline and saves the resulting `session_id`.
2. **PTY spawn**: `claude --resume <session_id> --dangerously-skip-permissions` in a 220×50 PTY.
3. **WebSocket bridge**: PTY output forwarded to browser as binary frames; keyboard input and resize events forwarded from browser to PTY.
4. **HTTP server**: serves a single-page xterm.js app that connects to the WS bridge.
5. **Browser launch**: opens the URL automatically.
6. **Shutdown**: Ctrl-C → SIGTERM → SIGKILL sequence; PTY master dropped to release controlling terminal.

### Log: `target/session-handoff.log`

```
tail -f target/session-handoff.log
```

Contains numbered PTY output lines, WebSocket connection events, resize events, and shutdown sequence.

### Diagnostics

**"timed out waiting for browser WebSocket connection (30s)"**
The browser did not connect within 30 s. Check that the URL printed to the log was opened and the WebSocket port is reachable.

**"run_agent_capturing failed"**
`claude` could not be spawned for the initial stream-json step.

**Browser connects but terminal is blank**
PTY reader EOF before the WebSocket connected — Claude may have exited immediately. Check `target/session-handoff.log` for `[pty] reader EOF`.
