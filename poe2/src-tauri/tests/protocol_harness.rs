//! Protocol integration test harness for the poe: event wire format.
//!
//! Spawns a real Claude process via PTY, injects a deterministic test skill bundle,
//! and validates that all poe: event types are correctly transmitted and parsed.
//!
//! See tests/README.md for how to run and how to interpret output.

use poe2_lib::{agent_lifecycle, event_ingester};
use tempfile::TempDir;

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
    if !claude_on_path() {
        eprintln!("claude not found on PATH — skipping protocol_end_to_end");
        return;
    }

    let tmp = TempDir::new().expect("failed to create temp dir");
    let skill = test_skill(LONG_CONTENT);
    let bundle = build_bundle(&skill);

    eprintln!("\n=== BUNDLE ({} bytes) ===", bundle.len());

    let lines = agent_lifecycle::run_agent_capturing(&bundle, tmp.path())
        .expect("run_agent_capturing failed — is claude on PATH?");

    // ── Log all raw PTY lines (always visible via --nocapture) ────────────────
    eprintln!("\n=== RAW PTY OUTPUT ({} lines) ===", lines.len());
    for (i, line) in lines.iter().enumerate() {
        eprintln!("[{:4}] {}", i, line);
    }

    // ── Parse events, log which lines were recognised ─────────────────────────
    let mut events: Vec<(String, serde_json::Value)> = Vec::new();
    eprintln!("\n=== POE EVENT PARSE RESULTS ===");
    for line in &lines {
        match event_ingester::parse_poe_event(line) {
            Some((event_type, json)) => {
                eprintln!("  [poe:{event_type}] {json}");
                events.push((event_type, json));
            }
            None if line.trim().starts_with('{') => {
                // A JSON-looking line that didn't parse — likely a PTY-wrap fragment.
                let preview = &line[..line.len().min(120)];
                eprintln!("  [PARSE FAIL — possible PTY wrap fragment] {preview}");
            }
            None => {} // raw PTY output (prompts, banners, etc.) — expected
        }
    }
    eprintln!(
        "\n{} poe: events parsed from {} raw PTY lines\n",
        events.len(),
        lines.len()
    );

    // ── Assertions ────────────────────────────────────────────────────────────

    assert!(!events.is_empty(), "No poe: events received — check raw PTY output above");

    // poe:brief must be the first event
    assert_eq!(
        events[0].0, "brief",
        "First poe: event must be poe:brief, got poe:{}",
        events[0].0
    );

    // poe:done must be the last event
    let last = events.last().unwrap();
    assert_eq!(
        last.0, "done",
        "Last poe: event must be poe:done, got poe:{}",
        last.0
    );

    // All 11 event types must appear (poe:review excluded — requires live orchestrator)
    let type_list: Vec<&str> = events.iter().map(|(t, _)| t.as_str()).collect();
    for expected in &[
        "brief",
        "step",
        "knowledge",
        "artifact",
        "task",
        "task:update",
        "task:cancel",
        "edge",
        "edge:remove",
        "decision",
        "done",
    ] {
        assert!(
            type_list.contains(expected),
            "Missing event type poe:{expected} — check raw PTY output and parse results above"
        );
    }

    // Exactly two artifact events (test-short.txt and test-long.txt)
    let artifacts: Vec<&serde_json::Value> = events
        .iter()
        .filter(|(t, _)| t == "artifact")
        .map(|(_, v)| v)
        .collect();
    assert_eq!(
        artifacts.len(),
        2,
        "Expected 2 poe:artifact events (test-short.txt and test-long.txt), got {}. \
         If only 1 received, the long-artifact JSON line was likely fragmented by PTY \
         column wrap (bp6-pdr.13).",
        artifacts.len()
    );

    // Short artifact — name and non-empty content
    let short = artifacts
        .iter()
        .find(|a| a.get("name").and_then(|v| v.as_str()) == Some("test-short.txt"));
    assert!(short.is_some(), "test-short.txt artifact not found in parsed events");
    let short_content = short
        .unwrap()
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(!short_content.is_empty(), "test-short.txt content field is empty");

    // Long artifact — primary assertion for bp6-pdr.13 (PTY column-wrap bug)
    let long = artifacts
        .iter()
        .find(|a| a.get("name").and_then(|v| v.as_str()) == Some("test-long.txt"));
    assert!(
        long.is_some(),
        "test-long.txt artifact not found — PTY column-wrap bug (bp6-pdr.13) is present: \
         the {}-char JSON line was split at col 220 and neither fragment parsed as JSON.",
        82 + LONG_CONTENT.len() + 2 // structure prefix + content + closing "}
    );
    let long_content = long
        .unwrap()
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        long_content.len() > 220,
        "test-long.txt content is only {} chars (expected > 220) — content may have been \
         truncated. Full expected length: {} chars.",
        long_content.len(),
        LONG_CONTENT.len()
    );
}
