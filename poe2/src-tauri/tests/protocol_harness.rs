//! Protocol integration test harness for the poe: event wire format.
//!
//! Spawns a real Claude process via stream-json transport (run_agent_capturing),
//! injects a deterministic test skill bundle, and validates that all 11 poe: event
//! types are correctly transmitted and parsed.
//!
//! The long-content artifact test (test-long.txt, 300 A-chars) confirms that the
//! stream-json transport does not truncate long lines. There is no PTY in this path;
//! the bp6-pdr.13 column-wrap bug cannot occur here by construction.
//!
//! All output is written to `target/protocol-harness.log` regardless of test outcome
//! or whether --nocapture is used. Monitor with: `tail -f target/protocol-harness.log`
//!
//! See tests/README.md for how to run and how to interpret output.

use poe2_lib::{agent_lifecycle, event_ingester};
use std::io::Write;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

// ── Harness logger ────────────────────────────────────────────────────────────
//
// Writes to target/protocol-harness.log AND stderr simultaneously.
// Cloneable so it can be shared into the PTY observer callback (different thread).
// Monitor live with: tail -f target/protocol-harness.log

#[derive(Clone)]
struct HarnessLog {
    file: Arc<Mutex<std::fs::File>>,
    path: std::path::PathBuf,
}

impl HarnessLog {
    fn create() -> Self {
        std::fs::create_dir_all("target").ok();
        let path = std::path::PathBuf::from("target/protocol-harness.log");
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .unwrap_or_else(|e| panic!("failed to open {}: {}", path.display(), e));
        let log = Self { file: Arc::new(Mutex::new(file)), path };
        log.line(&format!(
            "=== protocol_harness started at {} ===",
            chrono::Utc::now().to_rfc3339()
        ));
        log
    }

    fn line(&self, msg: &str) {
        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(f, "{}", msg);
            let _ = f.flush();
        }
        eprintln!("{}", msg);
    }

    fn sep(&self) {
        self.line("");
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

// ── Long content constant ─────────────────────────────────────────────────────
//
// 300 A-characters. When embedded in the long-artifact JSON line the total line
// length is ~384 chars — well past the 220-column PTY width.
//
// If the PTY column-wrap bug (bp6-pdr.13) is present, this JSON line is split
// across two lines at col 220. Neither fragment is valid JSON, so parse_poe_event
// returns None for both and the test-long.txt artifact event is never seen.

const LONG_CONTENT: &str = concat!(
    "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", // 50
    "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", // 100
    "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", // 150
    "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", // 200
    "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", // 250
    "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", // 300
);

// ── Helpers ───────────────────────────────────────────────────────────────────

fn claude_on_path() -> bool {
    std::process::Command::new("claude")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Build the embedded test skill. The long artifact JSON is constructed at
/// runtime so LONG_CONTENT is defined once and shared with the assertions.
fn test_skill(long_content: &str) -> String {
    // Note: double-braces {{ }} produce literal { } in format! strings.
    format!(
        r#"---
id: poe-test-harness
name: POE Protocol Test Harness
description: Deterministic test skill. Emits a fixed sequence of poe: events for protocol validation.
tags: [test]
protocol_version: v2
---

# POE Protocol Test Harness

You are running inside an automated integration test for the POE v2 protocol.
This is a deterministic test — not a real task.

## Strict Instructions

1. Output ONLY the JSON lines listed in "Required Output" below.
2. Do NOT add any explanation, commentary, or markdown formatting.
3. Do NOT wrap events in code fences or backticks.
4. Each JSON object must be on a SINGLE line — no newlines inside a JSON object.
5. Emit lines in EXACTLY the order shown.
6. Do NOT modify field values, including the long string of repeated A characters.

Any output that is not one of the listed JSON lines will be ignored by the test harness.

## Required Output

Output these lines in this exact order, each on its own line:

{{"poe": "brief", "content": "Protocol harness test: validating all poe event types."}}
{{"poe": "step", "name": "wire-format-coverage", "detail": "Emitting all poe event types in sequence."}}
{{"poe": "knowledge", "key": "test-key", "content": "test-value"}}
{{"poe": "artifact", "name": "test-short.txt", "artifact_type": "test", "content": "Short artifact content for protocol harness validation."}}
{{"poe": "task", "id": "test-task-001", "title": "Test subtask alpha", "description": "Harness subtask", "skill": "implementer", "type": "task"}}
{{"poe": "task:update", "id": "test-task-001", "title": "Updated test subtask alpha"}}
{{"poe": "task:cancel", "id": "test-task-001", "reason": "cancelled by harness"}}
{{"poe": "edge", "from": "test-task-001", "to": "test-task-002"}}
{{"poe": "edge:remove", "from": "test-task-001", "to": "test-task-002"}}
{{"poe": "decision", "question": "Harness test decision: which option?", "options": ["Option A", "Option B"]}}
{{"poe": "step", "name": "long-artifact", "detail": "Emitting artifact with content longer than 220 chars to test PTY line wrapping."}}
{{"poe": "artifact", "name": "test-long.txt", "artifact_type": "test", "content": "{long_content}"}}
{{"poe": "done", "summary": "Protocol harness test complete."}}
"#
    )
}

fn build_bundle(skill: &str) -> String {
    format!(
        "{skill}\n\n---\n\n# Task\n\n**Protocol Harness Validation**\n\n\
         Emit all required poe: events exactly as specified in your skill instructions. \
         Output nothing else.\n"
    )
}

// ── Test ──────────────────────────────────────────────────────────────────────

/// End-to-end protocol validation: spawn Claude, inject the test skill,
/// assert all 11 poe: event types are received and correctly parsed.
///
/// Skipped automatically if `claude` is not on PATH (CI-safe).
/// Run with: cargo test --test protocol_harness -- --nocapture
#[test]
fn protocol_end_to_end() {
    let log = HarnessLog::create();
    log.line(&format!("log: {}", log.path().display()));

    if !claude_on_path() {
        log.line("SKIP: claude not found on PATH — skipping protocol_end_to_end");
        return;
    }

    let tmp = TempDir::new().expect("failed to create temp dir");
    let skill = test_skill(LONG_CONTENT);
    let bundle = build_bundle(&skill);

    log.sep();
    log.line(&format!("=== BUNDLE ({} bytes) ===", bundle.len()));
    log.line(&bundle);

    // ── Spawn Claude — observer writes every PTY line to the log in real-time ──
    log.sep();
    log.line("=== SPAWNING CLAUDE ===");

    // Line counter and event parser run inside the observer so results are
    // visible in the log file as they arrive, not only after the run completes.
    let observer_log = log.clone();
    let observer_events: Arc<Mutex<Vec<(String, serde_json::Value)>>> =
        Arc::new(Mutex::new(Vec::new()));
    let observer_events_clone = observer_events.clone();
    let line_index = Arc::new(Mutex::new(0usize));
    let line_index_clone = line_index.clone();

    let observer = Box::new(move |line: String| {
        let idx = {
            let mut i = line_index_clone.lock().unwrap();
            let v = *i;
            *i += 1;
            v
        };
        // Parse first — annotate the log line with its event type if recognised.
        match event_ingester::parse_poe_event(&line) {
            Some((ref event_type, ref json)) => {
                observer_log.line(&format!("[{idx:4}] [poe:{event_type}] {json}"));
                observer_events_clone
                    .lock()
                    .unwrap()
                    .push((event_type.clone(), json.clone()));
            }
            None if line.trim().starts_with('{') => {
                let preview = &line[..line.len().min(120)];
                observer_log.line(&format!("[{idx:4}] [PARSE FAIL — possible PTY wrap fragment] {preview}"));
            }
            None => {
                observer_log.line(&format!("[{idx:4}] {line}"));
            }
        }
    });

    let lines = agent_lifecycle::run_agent_capturing(&bundle, tmp.path(), Some(observer))
        .expect("run_agent_capturing failed — is claude on PATH?");

    let events: Vec<(String, serde_json::Value)> =
        observer_events.lock().unwrap().clone();

    log.sep();
    log.line(&format!(
        "{} poe: events parsed from {} raw PTY lines",
        events.len(),
        lines.len()
    ));

    // ── Assertions ────────────────────────────────────────────────────────────
    log.sep();
    log.line("=== ASSERTIONS ===");

    assert!(!events.is_empty(), "No poe: events received — see {}", log.path().display());

    // poe:brief must be the first event
    log.line(&format!("  first event: poe:{}", events[0].0));
    assert_eq!(
        events[0].0, "brief",
        "First poe: event must be poe:brief, got poe:{} — see {}",
        events[0].0, log.path().display()
    );

    // poe:done must be the last event
    let last = events.last().unwrap();
    log.line(&format!("  last event:  poe:{}", last.0));
    assert_eq!(
        last.0, "done",
        "Last poe: event must be poe:done, got poe:{} — see {}",
        last.0, log.path().display()
    );

    // All 11 event types must appear (poe:review excluded — requires live orchestrator)
    let type_list: Vec<&str> = events.iter().map(|(t, _)| t.as_str()).collect();
    log.line(&format!("  event types seen: {:?}", type_list));
    for expected in &[
        "brief", "step", "knowledge", "artifact",
        "task", "task:update", "task:cancel",
        "edge", "edge:remove", "decision", "done",
    ] {
        let present = type_list.contains(expected);
        log.line(&format!("  poe:{:<14} {}", expected, if present { "OK" } else { "MISSING" }));
        assert!(
            present,
            "Missing event type poe:{expected} — see {}",
            log.path().display()
        );
    }

    // Exactly two artifact events (test-short.txt and test-long.txt)
    let artifacts: Vec<&serde_json::Value> = events
        .iter()
        .filter(|(t, _)| t == "artifact")
        .map(|(_, v)| v)
        .collect();
    log.line(&format!("  artifact count: {}", artifacts.len()));
    assert_eq!(
        artifacts.len(), 2,
        "Expected 2 poe:artifact events, got {}. See {}",
        artifacts.len(), log.path().display()
    );

    // Short artifact — name and non-empty content
    let short = artifacts
        .iter()
        .find(|a| a.get("name").and_then(|v| v.as_str()) == Some("test-short.txt"));
    assert!(short.is_some(), "test-short.txt artifact not found — see {}", log.path().display());
    let short_content = short.unwrap().get("content").and_then(|v| v.as_str()).unwrap_or("");
    log.line(&format!("  test-short.txt content length: {}", short_content.len()));
    assert!(!short_content.is_empty(), "test-short.txt content is empty — see {}", log.path().display());

    // Long artifact — confirms stream-json transport does not truncate long lines.
    let long = artifacts
        .iter()
        .find(|a| a.get("name").and_then(|v| v.as_str()) == Some("test-long.txt"));
    assert!(
        long.is_some(),
        "test-long.txt artifact not found. See {}",
        log.path().display()
    );
    let long_content = long.unwrap().get("content").and_then(|v| v.as_str()).unwrap_or("");
    log.line(&format!(
        "  test-long.txt content length: {} (expected {})",
        long_content.len(), LONG_CONTENT.len()
    ));
    assert!(
        long_content.len() > 220,
        "test-long.txt content is only {} chars (expected > 220) — see {}",
        long_content.len(), log.path().display()
    );

    log.sep();
    log.line("=== ALL ASSERTIONS PASSED ===");
}
