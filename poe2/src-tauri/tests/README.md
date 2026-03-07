# POE2 Integration Tests

End-to-end protocol tests that require a live `claude` binary on PATH.

---

## protocol_harness

**File**: `tests/protocol_harness.rs`
**Bead**: `bp6-pdr.17`

Validates the full poe: event wire format from bundle injection through PTY to parsed events. Spawns a real Claude process, injects a deterministic test skill, and asserts that all 11 event types are correctly transmitted and parsed.

### How to run

```bash
cd poe2/src-tauri

# Run the test (output always written to target/protocol-harness.log)
cargo test --test protocol_harness

# Monitor output live while the test runs
tail -f target/protocol-harness.log

# Run and see output inline (log file is also written)
cargo test --test protocol_harness -- --nocapture
```

All diagnostic output — raw PTY lines, parse results, assertion outcomes — is written to
`target/protocol-harness.log` unconditionally, regardless of `--nocapture` or test outcome.
The log path is printed in every assertion failure message.

### Prerequisites

- `claude` binary on PATH (`claude --version` must succeed)
- Test is skipped automatically if `claude` is not found — it does not fail CI

### What each assertion tests

| Assertion | Tests |
|---|---|
| First event is `poe:brief` | Protocol ordering rule: brief must be first |
| Last event is `poe:done` | Protocol ordering rule: done must be last |
| All 11 event types present | Wire format coverage: every event type parses correctly |
| Exactly 2 `poe:artifact` events | Short and long artifact both transmitted |
| `test-short.txt` content non-empty | Short artifact content field intact |
| `test-long.txt` artifact present | **PTY column-wrap bug (bp6-pdr.13)**: if present, the long-artifact JSON line (~384 chars) is fragmented at col 220 and never parses |
| `test-long.txt` content > 220 chars | Artifact content not truncated at PTY column boundary |

Event type `poe:review` is excluded — it requires a live orchestrator to resolve the review dependency chain.

### How to interpret failure output

The test logs three sections to stderr (visible with `--nocapture`):

1. **RAW PTY OUTPUT** — every line the PTY emitted, numbered. Look here if events are missing.
2. **POE EVENT PARSE RESULTS** — which lines were recognised as poe: events vs raw output. Lines marked `[PARSE FAIL — possible PTY wrap fragment]` are JSON-looking lines that failed to parse, usually because a long JSON line was split at the PTY column boundary.
3. **Summary line** — count of parsed events vs total raw lines.

### Common failure modes

**"test-long.txt artifact not found"**
The PTY column-wrap bug (bp6-pdr.13) is present. The `test-long.txt` JSON line (~384 chars) was split at col 220, producing two non-JSON fragments. Fix: increase PTY column width in `agent_lifecycle::spawn_agent`.

**"Missing event type poe:X"**
Claude did not emit that event type. Check the raw PTY output for the corresponding JSON fragment. The test skill may need adjustment if Claude consistently omits an event.

**"run_agent_capturing failed"**
The PTY failed to open or `claude` could not be spawned. Check that `claude --dangerously-skip-permissions` works from the terminal.
