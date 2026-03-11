//! Decision → scripted resume integration test.
//!
//! Run:  cargo test --test decision_handoff_harness -- --nocapture
//! Log:  target/decision-handoff.log
//!
//! Flow:
//!   1. Stream-json session 1: Claude emits poe:brief + poe:decision + poe:done.
//!      The session_id is captured from the init event.
//!   2. Scripted resume: stream-json --resume <session_id> injects the human
//!      response as bundle text (no PTY, no browser, no human required).
//!   3. Session 2 asserts poe:done received and on_complete fired.
//!
//! What the test asserts:
//!   - Session 1: on_session_id fires, poe:decision received, poe:done received.
//!   - Session 2: on_session_id fires, poe:done received, on_complete fires.
//!
//! NOTE — manual xterm.js UX test:
//!   The PTY + WebSocket + browser handoff path is tested in session_handoff_harness,
//!   which is correctly marked #[ignore] (manual). This test proves protocol
//!   correctness only and runs in CI without --ignored.

use poe2_lib::agent::text_extractor::TextBufExtractor;
use poe2_lib::agent::transport::{JsonStreamTransport, StreamCallbacks};
use poe2_lib::event_ingester::parse_poe_event;
use std::io::Write;
use std::sync::{Arc, Mutex};


// ── Logger ────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct Log {
    file: Arc<Mutex<std::fs::File>>,
    path: std::path::PathBuf,
}

impl Log {
    fn create(name: &str) -> Self {
        std::fs::create_dir_all("target").ok();
        let path = std::path::PathBuf::from(format!("target/{name}.log"));
        let file = std::fs::OpenOptions::new()
            .create(true).write(true).truncate(true)
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

    fn sep(&self) { self.line(""); }
    fn path(&self) -> &std::path::Path { &self.path }
}

// ── Guards ────────────────────────────────────────────────────────────────────

fn claude_on_path() -> bool {
    std::process::Command::new("claude").arg("--version")
        .output().map(|o| o.status.success()).unwrap_or(false)
}

// ── Transport runner (inline — test files cannot share code) ──────────────────

struct RunResult {
    session_ids: Vec<String>,
    poe_events: Vec<(String, serde_json::Value)>,
    completed: bool,
}

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
    let log_pid = log.clone(); let log_session = log.clone();
    let log_chunk = log.clone(); let log_complete = log.clone();

    let extractor = Arc::new(Mutex::new(TextBufExtractor::new()));
    let ex_chunk = extractor.clone();
    let ex_complete = extractor.clone();

    JsonStreamTransport::run(
        bundle, resume_session_id, None, cwd,
        StreamCallbacks {
            on_pid: Some(Box::new(move |pid| {
                log_pid.line(&format!("[transport] pid={pid}"));
            })),
            on_session_id: Box::new(move |sid| {
                log_session.line(&format!("[transport] session_id={sid}"));
                session_ids_c.lock().unwrap().push(sid);
            }),
            on_text_chunk: Box::new(move |text| {
                let lines = ex_chunk.lock().unwrap().push(&text);
                for line in lines {
                    match parse_poe_event(&line) {
                        Some((ref t, ref j)) => {
                            log_chunk.line(&format!("[poe:{t}] {j}"));
                            poe_for_chunk.lock().unwrap().push((t.clone(), j.clone()));
                        }
                        None if line.trim().starts_with('{') => {
                            log_chunk.line(&format!("[text/json?] {}", &line[..line.len().min(120)]));
                        }
                        None => {
                            log_chunk.line(&format!("[text] {}", &line[..line.len().min(80)]));
                        }
                    }
                }
            }),
            on_raw_json: Box::new(|_| {}),
            on_complete: Box::new(move || {
                let tail = ex_complete.lock().unwrap().flush();
                if let Some(ref t) = tail {
                    if !t.trim().is_empty() {
                        if let Some((ref et, ref ej)) = parse_poe_event(t) {
                            log_complete.line(&format!("[poe:{et}] (tail) {ej}"));
                            poe_for_complete.lock().unwrap().push((et.clone(), ej.clone()));
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
    Ok(RunResult { session_ids: out_session_ids, poe_events: out_poe_events, completed: out_completed })
}

// ── Test ──────────────────────────────────────────────────────────────────────

/// End-to-end decision → scripted resume protocol test.
///
/// CI-safe: runs without human interaction. Session 1 raises poe:decision;
/// Session 2 injects the human response as a bundle and asserts poe:done.
///
/// Run: cargo test --test decision_handoff_harness -- --nocapture
/// Log: target/decision-handoff.log
#[test]
fn decision_handoff_scripted_resume() {
    let log = Log::create("decision-handoff");
    log.line(&format!("log: {}", log.path().display()));

    if !claude_on_path() {
        log.line("SKIP: claude not on PATH — skipping decision_handoff_scripted_resume");
        return;
    }

    // Use the same cwd for both sessions so Claude's session scoping (by project
    // directory) can find the session when resuming.
    let cwd = std::env::current_dir().expect("current_dir");

    // ── Session 1: emit poe:decision then poe:done ────────────────────────────
    log.sep();
    log.line("=== SESSION 1: emit poe:decision ===");

    let bundle_1 = concat!(
        "You are a test agent in autonomous mode.\n\n",
        "# Task\n\n",
        "Emit exactly these three lines in order, nothing else:\n",
        r#"{"poe":"brief","content":"Evaluating options for the handoff test."}"#, "\n",
        r#"{"poe":"decision","question":"Which path should the orchestrator take?","options":["Continue","Pause"]}"#, "\n",
        r#"{"poe":"done","summary":"Decision raised — session paused for human input."}"#, "\n",
    );

    let result_1 = run_transport(bundle_1, None, &cwd, &log)
        .expect("session 1 run_transport failed");

    assert!(
        !result_1.session_ids.is_empty(),
        "Session 1: no session_id captured. See {}",
        log.path().display()
    );
    let session_id = result_1.session_ids[0].clone();
    log.line(&format!("Session 1 session_id: {session_id}"));

    assert!(
        result_1.poe_events.iter().any(|(t, _)| t == "decision"),
        "Session 1: poe:decision not received. Events: {:?}. See {}",
        result_1.poe_events.iter().map(|(t, _)| t).collect::<Vec<_>>(),
        log.path().display()
    );
    log.line("Session 1 poe:decision: OK");

    assert!(
        result_1.poe_events.iter().any(|(t, _)| t == "done"),
        "Session 1: poe:done not received. See {}",
        log.path().display()
    );
    log.line("Session 1 poe:done: OK");

    // ── Session 2: scripted resume — inject human response as bundle ──────────
    log.sep();
    log.line("=== SESSION 2: scripted resume (no PTY, no browser) ===");
    log.line(&format!("Resuming session {session_id} via stream-json…"));

    let bundle_2 = concat!(
        "Human: My scripted response to the decision — Continue.\n\n",
        "Emit this line and nothing else:\n",
        r#"{"poe":"done","summary":"Decision acknowledged — task resumed and complete."}"#, "\n",
    );

    let result_2 = run_transport(bundle_2, Some(&session_id), &cwd, &log)
        .expect("session 2 run_transport failed");

    log.sep();
    log.line("=== SESSION 2 ASSERTIONS ===");
    log.line(&format!("  events: {}, completed: {}", result_2.poe_events.len(), result_2.completed));

    assert!(
        !result_2.session_ids.is_empty(),
        "Session 2: on_session_id never fired. See {}",
        log.path().display()
    );
    log.line(&format!("  session_id: {}", result_2.session_ids[0]));

    assert!(
        result_2.poe_events.iter().any(|(t, _)| t == "done"),
        "Session 2: poe:done not received. Events: {:?}. See {}",
        result_2.poe_events.iter().map(|(t, _)| t).collect::<Vec<_>>(),
        log.path().display()
    );
    log.line("  poe:done: OK");

    assert!(
        result_2.completed,
        "Session 2: on_complete never fired. See {}",
        log.path().display()
    );
    log.line("  on_complete: OK");

    log.sep();
    log.line("=== ALL ASSERTIONS PASSED ===");
}
