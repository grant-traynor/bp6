//! Integration tests: stream-json transport with real Claude interactions.
//!
//! These tests call `JsonStreamTransport::run()` directly — exercising the full
//! callback chain (on_pid, on_session_id, on_text_chunk, on_raw_json, on_complete)
//! with a live claude process, without going through the Tauri app layer.
//!
//! Three tests:
//!   - session_id_captured_from_real_stream  — on_session_id fires with a valid UUID
//!   - full_protocol_payload_correctness     — all 11 poe: event types, JSON field
//!                                             values, and stream-json long-line fix
//!   - resume_continuation_captures_new_session_and_emits_done — --resume path
//!
//! All tests skip gracefully when `claude` is not on PATH.
//! Output is logged to `target/stream-json-integration.log` regardless of outcome.
//! Monitor live with: tail -f target/stream-json-integration.log

use poe2_lib::agent::text_extractor::TextBufExtractor;
use poe2_lib::agent::transport::{JsonStreamTransport, StreamCallbacks};
use poe2_lib::event_ingester::parse_poe_event;
use std::io::Write;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

// ── Logger ────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct Log {
    file: Arc<Mutex<std::fs::File>>,
    path: std::path::PathBuf,
}

impl Log {
    fn open(name: &str) -> Self {
        std::fs::create_dir_all("target").ok();
        let path = std::path::PathBuf::from(format!("target/{name}.log"));
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .unwrap_or_else(|e| panic!("failed to open {}: {}", path.display(), e));
        let log = Self { file: Arc::new(Mutex::new(file)), path };
        log.line(&format!("=== {name} started ==="));
        log
    }

    fn line(&self, msg: &str) {
        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(f, "{msg}");
            let _ = f.flush();
        }
        eprintln!("{msg}");
    }

    fn sep(&self) {
        self.line("");
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

// ── Guard ─────────────────────────────────────────────────────────────────────

fn claude_on_path() -> bool {
    std::process::Command::new("claude")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ── Shared test data ──────────────────────────────────────────────────────────

// 300 A-characters. Embedded in the poe:knowledge event to confirm that stream-json
// transport does not fragment long lines. poe:artifact events are content-free;
// the long value is carried by knowledge instead.
const LONG_CONTENT: &str = concat!(
    "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", // 50
    "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", // 100
    "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", // 150
    "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", // 200
    "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", // 250
    "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", // 300
);

fn full_skill_bundle(long_content: &str) -> String {
    // Note: double-braces {{ }} produce literal { } in format! strings.
    let skill = format!(
        r#"---
id: poe-test-harness
name: POE Protocol Test Harness
description: Deterministic test skill. Emits a fixed sequence of poe: events.
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
{{"poe": "knowledge", "key": "test-key", "content": "{long_content}"}}
{{"poe": "artifact", "name": "test-short.txt", "artifact_type": "test"}}
{{"poe": "task", "id": "test-task-001", "title": "Test subtask alpha", "description": "Harness subtask", "skill": "implementer", "type": "task"}}
{{"poe": "task:update", "id": "test-task-001", "title": "Updated test subtask alpha"}}
{{"poe": "task:cancel", "id": "test-task-001", "reason": "cancelled by harness"}}
{{"poe": "edge", "from": "test-task-001", "to": "test-task-002"}}
{{"poe": "edge:remove", "from": "test-task-001", "to": "test-task-002"}}
{{"poe": "decision", "question": "Harness test decision: which option?", "options": ["Option A", "Option B"]}}
{{"poe": "step", "name": "long-knowledge", "detail": "Long knowledge value above was emitted with 300 A-chars to confirm no line truncation."}}
{{"poe": "artifact", "name": "test-long.txt", "artifact_type": "test"}}
{{"poe": "done", "summary": "Protocol harness test complete."}}
"#
    );

    format!(
        "{skill}\n\n---\n\n# Task\n\n**Protocol Harness Validation**\n\n\
         Emit all required poe: events exactly as specified in your skill instructions. \
         Output nothing else.\n"
    )
}

// ── Transport helper ──────────────────────────────────────────────────────────

struct RunResult {
    session_ids: Vec<String>,
    poe_events: Vec<(String, serde_json::Value)>,
    completed: bool,
}

/// Drive `JsonStreamTransport::run()` and collect all callbacks into a `RunResult`.
///
/// Text is routed through `TextBufExtractor → parse_poe_event`, mirroring the
/// production path in `agent_lifecycle::spawn_agent`. The `on_complete` callback
/// flushes the extractor tail so a final event lacking a trailing newline is
/// still captured.
fn run_transport(
    bundle: &str,
    resume_session_id: Option<&str>,
    cwd: &std::path::Path,
    log: &Log,
) -> anyhow::Result<RunResult> {
    let session_ids: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let poe_events: Arc<Mutex<Vec<(String, serde_json::Value)>>> = Arc::new(Mutex::new(Vec::new()));
    let completed: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

    let session_ids_c = session_ids.clone();
    let poe_for_chunk = poe_events.clone();
    let poe_for_complete = poe_events.clone();
    let completed_c = completed.clone();
    let log_pid = log.clone();
    let log_session = log.clone();
    let log_chunk = log.clone();
    let log_complete = log.clone();

    let extractor = Arc::new(Mutex::new(TextBufExtractor::new()));
    let extractor_chunk = extractor.clone();
    let extractor_complete = extractor.clone();

    JsonStreamTransport::run(
        bundle,
        resume_session_id,
        None, // model — use claude's default
        cwd,
        StreamCallbacks {
            on_pid: Some(Box::new(move |pid| {
                log_pid.line(&format!("[transport] pid={pid}"));
            })),
            on_session_id: Box::new(move |sid| {
                log_session.line(&format!("[transport] session_id={sid}"));
                session_ids_c.lock().unwrap().push(sid);
            }),
            on_text_chunk: Box::new(move |text| {
                let lines = extractor_chunk.lock().unwrap().push(&text);
                for line in lines {
                    match parse_poe_event(&line) {
                        Some((ref t, ref j)) => {
                            log_chunk.line(&format!("[poe:{t}] {j}"));
                            poe_for_chunk.lock().unwrap().push((t.clone(), j.clone()));
                        }
                        None if line.trim().starts_with('{') => {
                            log_chunk.line(&format!(
                                "[text/json?] {}",
                                &line[..line.len().min(120)]
                            ));
                        }
                        None => {
                            log_chunk.line(&format!("[text] {}", &line[..line.len().min(120)]));
                        }
                    }
                }
            }),
            on_raw_json: Box::new(|_| {}),
            on_complete: Box::new(move || {
                // Flush tail — the last poe: event may lack a trailing newline.
                let tail = extractor_complete.lock().unwrap().flush();
                if let Some(ref t) = tail {
                    if !t.trim().is_empty() {
                        match parse_poe_event(t) {
                            Some((ref et, ref ej)) => {
                                log_complete.line(&format!("[poe:{et}] (tail) {ej}"));
                                poe_for_complete.lock().unwrap().push((et.clone(), ej.clone()));
                            }
                            None => {
                                log_complete.line(&format!("[tail] {}", &t[..t.len().min(120)]));
                            }
                        }
                    }
                }
                *completed_c.lock().unwrap() = true;
                log_complete.line("[transport] on_complete fired");
            }),
        },
    )?;

    let out_session_ids = session_ids.lock().unwrap().clone();
    let out_poe_events = poe_events.lock().unwrap().clone();
    let out_completed = *completed.lock().unwrap();
    Ok(RunResult {
        session_ids: out_session_ids,
        poe_events: out_poe_events,
        completed: out_completed,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Verify that the `{"type":"system","subtype":"init","session_id":"..."}` NDJSON
/// event emitted by claude is parsed by JsonStreamTransport and delivered via
/// `on_session_id` as a non-empty UUID-format string.
#[test]
fn session_id_captured_from_real_stream() {
    let log = Log::open("stream-json-session-id");
    if !claude_on_path() {
        log.line("SKIP: claude not on PATH — skipping session_id_captured_from_real_stream");
        return;
    }

    // Minimal bundle — just get claude to emit poe:done so the session exits cleanly.
    let bundle = concat!(
        "You are a test agent. Emit exactly one line, then stop.\n\n",
        "---\n\n",
        "# Task\n\n",
        "Emit this exact line and nothing else:\n",
        r#"{"poe":"done","summary":"session-id test complete"}"#,
        "\n"
    );

    let tmp = TempDir::new().unwrap();
    log.sep();
    log.line("=== SPAWNING CLAUDE ===");

    let result = run_transport(bundle, None, tmp.path(), &log)
        .expect("run_transport failed");

    log.sep();
    log.line("=== ASSERTIONS ===");
    log.line(&format!("  session_ids: {:?}", result.session_ids));

    assert!(
        !result.session_ids.is_empty(),
        "on_session_id was never fired — session_id not extracted from init event. \
         See {}",
        log.path().display()
    );

    let sid = &result.session_ids[0];
    assert!(
        !sid.is_empty(),
        "session_id is empty string. See {}",
        log.path().display()
    );
    // Claude session IDs are UUID-format (8-4-4-4-12 hex groups separated by hyphens).
    assert!(
        sid.len() >= 8 && sid.contains('-'),
        "session_id '{}' does not look like a UUID (expected hyphens and ≥ 8 chars). \
         See {}",
        sid,
        log.path().display()
    );

    log.line(&format!("  session_id OK: {sid}"));
    log.sep();
    log.line("=== ALL ASSERTIONS PASSED ===");
}

/// Drive all 11 poe: event types through `JsonStreamTransport::run()` directly
/// (not via `run_agent_capturing`) and assert:
///
/// - `on_session_id` fires with a valid session_id
/// - `on_complete` fires (result event received)
/// - All 11 event types arrive in the correct order (brief first, done last)
/// - Specific JSON field values are correct (e.g. poe:task id == "test-task-001")
/// - The long-content knowledge value (300 A-chars) arrives untruncated — confirming
///   that stream-json transport does not fragment long lines. poe:artifact events are
///   content-free; artifact names only are verified.
#[test]
fn full_protocol_payload_correctness() {
    let log = Log::open("stream-json-protocol");
    if !claude_on_path() {
        log.line("SKIP: claude not on PATH — skipping full_protocol_payload_correctness");
        return;
    }

    let bundle = full_skill_bundle(LONG_CONTENT);

    log.sep();
    log.line(&format!("=== BUNDLE ({} bytes) ===", bundle.len()));
    log.line(&bundle);
    log.sep();
    log.line("=== SPAWNING CLAUDE ===");

    let tmp = TempDir::new().unwrap();
    let result = run_transport(&bundle, None, tmp.path(), &log)
        .expect("run_transport failed");

    let events = &result.poe_events;
    log.sep();
    log.line(&format!("{} poe: events parsed", events.len()));
    log.line("=== ASSERTIONS ===");

    // on_session_id must fire
    assert!(
        !result.session_ids.is_empty(),
        "on_session_id never fired — init event not parsed. See {}",
        log.path().display()
    );
    log.line(&format!("  session_id: {}", result.session_ids[0]));

    // on_complete must fire
    assert!(
        result.completed,
        "on_complete never fired — result event missing from stream. See {}",
        log.path().display()
    );
    log.line("  on_complete: OK");

    // Must have received poe: events
    assert!(
        !events.is_empty(),
        "No poe: events received at all. See {}",
        log.path().display()
    );

    // poe:brief must be first
    log.line(&format!("  first event: poe:{}", events[0].0));
    assert_eq!(
        events[0].0, "brief",
        "First poe: event must be poe:brief, got poe:{}. See {}",
        events[0].0,
        log.path().display()
    );

    // poe:done must be last
    let last = events.last().unwrap();
    log.line(&format!("  last event:  poe:{}", last.0));
    assert_eq!(
        last.0, "done",
        "Last poe: event must be poe:done, got poe:{}. See {}",
        last.0,
        log.path().display()
    );

    // All 11 event types must be present
    let type_list: Vec<&str> = events.iter().map(|(t, _)| t.as_str()).collect();
    log.line(&format!("  event types: {:?}", type_list));
    for expected in &[
        "brief", "step", "knowledge", "artifact",
        "task", "task:update", "task:cancel",
        "edge", "edge:remove", "decision", "done",
    ] {
        let present = type_list.contains(expected);
        log.line(&format!(
            "  poe:{:<14} {}",
            expected,
            if present { "OK" } else { "MISSING" }
        ));
        assert!(
            present,
            "Missing event type poe:{expected}. See {}",
            log.path().display()
        );
    }

    // poe:task must carry the correct id field
    let task_event = events
        .iter()
        .find(|(t, _)| t == "task")
        .expect("poe:task event not found (already checked above)");
    assert_eq!(
        task_event.1.get("id").and_then(|v| v.as_str()),
        Some("test-task-001"),
        "poe:task id field mismatch. See {}",
        log.path().display()
    );
    log.line("  poe:task id: OK");

    // Exactly two artifact events (test-short.txt and test-long.txt); both are content-free.
    let artifacts: Vec<&serde_json::Value> = events
        .iter()
        .filter(|(t, _)| t == "artifact")
        .map(|(_, v)| v)
        .collect();
    log.line(&format!("  artifact count: {}", artifacts.len()));
    assert_eq!(
        artifacts.len(),
        2,
        "Expected 2 poe:artifact events, got {}. See {}",
        artifacts.len(),
        log.path().display()
    );

    // Verify both artifact names arrived.
    let short = artifacts.iter().find(|a| a.get("name").and_then(|v| v.as_str()) == Some("test-short.txt"));
    assert!(short.is_some(), "test-short.txt artifact not found. See {}", log.path().display());
    log.line("  test-short.txt: OK");

    let long_art = artifacts.iter().find(|a| a.get("name").and_then(|v| v.as_str()) == Some("test-long.txt"));
    assert!(long_art.is_some(), "test-long.txt artifact not found. See {}", log.path().display());
    log.line("  test-long.txt: OK");

    // Long knowledge value: stream-json must deliver the full 300-char payload verbatim.
    // If this assertion fails, a transport regression introduced line truncation.
    let knowledge_event = events.iter().find(|(t, v)| {
        t == "knowledge" && v.get("key").and_then(|k| k.as_str()) == Some("test-key")
    });
    assert!(
        knowledge_event.is_some(),
        "poe:knowledge (test-key) not found. See {}",
        log.path().display()
    );
    let knowledge_content = knowledge_event.unwrap().1.get("content").and_then(|v| v.as_str()).unwrap_or("");
    log.line(&format!(
        "  knowledge content len: {} (expected {})",
        knowledge_content.len(),
        LONG_CONTENT.len()
    ));
    assert_eq!(
        knowledge_content, LONG_CONTENT,
        "knowledge content does not match expected (len {} vs {}). \
         Stream-json transport should not truncate or fragment long JSON lines. See {}",
        knowledge_content.len(),
        LONG_CONTENT.len(),
        log.path().display()
    );
    log.line("  knowledge long content: OK");

    log.sep();
    log.line("=== ALL ASSERTIONS PASSED ===");
}

/// Exercise the `--resume <session_id>` continuation path end-to-end.
///
/// Session 1: minimal bundle that emits poe:done. Captures the session_id.
/// Session 2: calls `JsonStreamTransport::run()` with `resume_session_id = Some(sid)`
///            and a new prompt; asserts poe:done arrives and `on_complete` fires.
///
/// This validates both that session_id is surfaced from the init event AND that
/// the resume path correctly continues a conversation.
#[test]
fn resume_continuation_captures_new_session_and_emits_done() {
    let log = Log::open("stream-json-resume");
    if !claude_on_path() {
        log.line(
            "SKIP: claude not on PATH — skipping \
             resume_continuation_captures_new_session_and_emits_done",
        );
        return;
    }

    let tmp = TempDir::new().unwrap();

    // ── Session 1 ────────────────────────────────────────────────────────────

    let bundle_1 = concat!(
        "You are a test agent operating in autonomous mode.\n\n",
        "# Task\n\n",
        "Emit exactly these two lines in this order, nothing else:\n",
        r#"{"poe":"brief","content":"session one starting"}"#,
        "\n",
        r#"{"poe":"done","summary":"session one complete"}"#,
        "\n"
    );

    log.sep();
    log.line("=== SESSION 1 ===");

    let result_1 = run_transport(bundle_1, None, tmp.path(), &log)
        .expect("session 1: run_transport failed");

    assert!(
        !result_1.session_ids.is_empty(),
        "Session 1: on_session_id never fired. See {}",
        log.path().display()
    );
    let session_id = result_1.session_ids[0].clone();
    log.line(&format!("Session 1 session_id: {session_id}"));

    assert!(
        result_1.poe_events.iter().any(|(t, _)| t == "done"),
        "Session 1: poe:done not received. Events: {:?}. See {}",
        result_1.poe_events.iter().map(|(t, _)| t).collect::<Vec<_>>(),
        log.path().display()
    );
    log.line("Session 1 poe:done: OK");

    // ── Session 2 (resume) ────────────────────────────────────────────────────

    let bundle_2 = concat!(
        "This is a resumed session. Emit exactly this line and nothing else:\n",
        r#"{"poe":"done","summary":"resumed session complete"}"#,
        "\n"
    );

    log.sep();
    log.line("=== SESSION 2 (resume) ===");
    log.line(&format!("  resuming session_id: {session_id}"));

    let result_2 = run_transport(bundle_2, Some(&session_id), tmp.path(), &log)
        .expect("session 2: run_transport failed");

    log.sep();
    log.line("=== SESSION 2 ASSERTIONS ===");
    log.line(&format!(
        "  events: {}, completed: {}",
        result_2.poe_events.len(),
        result_2.completed
    ));

    // Session 2 must receive a session_id from the resumed stream.
    assert!(
        !result_2.session_ids.is_empty(),
        "Session 2: on_session_id never fired — init event not received for resumed session. \
         See {}",
        log.path().display()
    );
    log.line(&format!("  session_id: {}", result_2.session_ids[0]));

    // poe:done must arrive in session 2.
    assert!(
        result_2.poe_events.iter().any(|(t, _)| t == "done"),
        "Session 2: poe:done not received. Events: {:?}. See {}",
        result_2.poe_events.iter().map(|(t, _)| t).collect::<Vec<_>>(),
        log.path().display()
    );
    log.line("  poe:done: OK");

    // on_complete must fire in session 2 (result event present).
    assert!(
        result_2.completed,
        "Session 2: on_complete never fired. See {}",
        log.path().display()
    );
    log.line("  on_complete: OK");

    log.sep();
    log.line("=== ALL ASSERTIONS PASSED ===");
}
