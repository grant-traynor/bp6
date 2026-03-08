//! Integration test: validates the full stream-json transport stack.
//!
//! Requires `claude` on PATH. Automatically skipped when absent.

use poe2_lib::agent_lifecycle::run_agent_capturing;
use poe2_lib::event_ingester::parse_poe_event;

/// A minimal skill + task bundle that asks the agent to do a trivial task
/// and emit poe:done. Designed to complete quickly with minimal tokens.
const TEST_BUNDLE: &str = r#"## Execution Protocol

You are running in autonomous mode — there is no human at the keyboard.

- Begin by emitting `poe:brief` describing your understanding of the task.
- Work from the context in this bundle. Do not ask questions.
- Raise genuine blockers via `poe:decision`, then continue with your best judgement.
- Emit `poe:done` as your final output. The process exits after this.

---

# Skill

You are a test agent. Your only job is to emit poe: events and nothing else.

---

# Task

**Test task**

Emit `poe:brief` with a one-sentence summary, then immediately emit `poe:done`.
Do not write any prose outside of poe: events.

Example output:
{"poe":"brief","content":"I will emit done immediately."}
{"poe":"done","summary":"test complete"}
"#;

#[test]
fn stream_json_transport_emits_poe_done() {
    // Skip if claude is not available.
    if std::process::Command::new("claude")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("[stdio_json_harness] claude not found on PATH — skipping");
        return;
    }

    let tmp = tempfile::TempDir::new().expect("failed to create temp dir");

    let lines = run_agent_capturing(
        TEST_BUNDLE,
        tmp.path(),
        Some(Box::new(|line| {
            eprintln!("[stdio_json_harness] line: {}", line);
        })),
    )
    .expect("run_agent_capturing failed");

    eprintln!(
        "[stdio_json_harness] total lines collected: {}",
        lines.len()
    );

    // Must contain at least one poe: event.
    let poe_events: Vec<_> = lines
        .iter()
        .filter_map(|l| parse_poe_event(l))
        .collect();

    assert!(
        !poe_events.is_empty(),
        "expected at least one poe: event in output, got none.\nLines:\n{}",
        lines.join("\n")
    );

    // Must contain poe:done.
    let has_done = poe_events.iter().any(|(t, _)| t == "done");
    assert!(
        has_done,
        "expected poe:done event in output.\nPoe events found: {:?}",
        poe_events.iter().map(|(t, _)| t).collect::<Vec<_>>()
    );
}
