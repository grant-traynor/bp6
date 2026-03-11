//! Real-agent integration tests: Phase 2.3 interactive CONOPS protocol.
//!
//! These tests spawn a real `claude` process with the production `operational-analyst`
//! skill and validate the poe:chat + poe:yield + resume cycle end-to-end.
//! They exist specifically to catch protocol mismatches that mocked tests cannot —
//! e.g. incorrect event names, wrong payload structure, yield without preceding chat.
//!
//! Tests covered (Phase 2.3 success criteria):
//!
//!   SC-1  interactive mode activates: first run emits poe:chat + poe:yield, NOT poe:done
//!   SC-2  chat content is clean prose, not raw protocol JSON
//!   SC-3  yield follows chat in the same session (tracker survives multi-chunk output)
//!   SC-4  resume after human response emits next poe:chat or poe:done
//!   SC-5  autonomous mode completes with poe:artifact + poe:done, no poe:chat
//!
//! REQUIRES: `claude` binary on PATH with a valid ANTHROPIC_API_KEY.
//!
//! Run:
//!   cd poe2/src-tauri
//!   cargo test --test interactive_conops_test -- --nocapture
//!
//! All tests skip gracefully when `claude` is not on PATH.
//! Logs: target/conops-interactive-*.log

use poe2_lib::agent::text_extractor::TextBufExtractor;
use poe2_lib::agent::transport::{JsonStreamTransport, StreamCallbacks};
use poe2_lib::event_ingester::parse_poe_event;
use poe2_lib::skills::{assemble_input_bundle, load_skill, SpawnMode};
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

// ── Log ───────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct Log {
    file: Arc<Mutex<std::fs::File>>,
}

impl Log {
    fn open(name: &str) -> Self {
        std::fs::create_dir_all("target").ok();
        let path = format!("target/conops-{}.log", name);
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .unwrap_or_else(|e| panic!("log open {}: {}", path, e));
        Self { file: Arc::new(Mutex::new(file)) }
    }

    fn line(&self, msg: &str) {
        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(f, "{}", msg);
            let _ = f.flush();
        }
        eprintln!("{}", msg);
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

// ── Agent run helper ──────────────────────────────────────────────────────────

struct AgentRun {
    /// Session id from the init event — needed for --resume.
    session_id: Option<String>,
    /// All poe: events received, in emission order: (event_type, full_json_value).
    poe_events: Vec<(String, serde_json::Value)>,
    /// True when the transport received the result event and on_complete fired.
    on_complete_fired: bool,
}

impl AgentRun {
    fn event_types(&self) -> Vec<&str> {
        self.poe_events.iter().map(|(t, _)| t.as_str()).collect()
    }

    fn chat_events(&self) -> Vec<&serde_json::Value> {
        self.poe_events.iter()
            .filter(|(t, _)| t == "chat")
            .map(|(_, v)| v)
            .collect()
    }

    fn has(&self, event_type: &str) -> bool {
        self.poe_events.iter().any(|(t, _)| t == event_type)
    }

    /// Position of first occurrence of event_type in emission order.
    fn pos(&self, event_type: &str) -> Option<usize> {
        self.poe_events.iter().position(|(t, _)| t == event_type)
    }
}

fn run_agent(
    bundle: &str,
    resume_session_id: Option<&str>,
    cwd: &Path,
    log: &Log,
) -> anyhow::Result<AgentRun> {
    let session_id: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let poe_events: Arc<Mutex<Vec<(String, serde_json::Value)>>> =
        Arc::new(Mutex::new(Vec::new()));
    let on_complete = Arc::new(Mutex::new(false));

    let sid_w = session_id.clone();
    let evts_chunk = poe_events.clone();
    let evts_complete = poe_events.clone();
    let done_w = on_complete.clone();

    let extractor = Arc::new(Mutex::new(TextBufExtractor::new()));
    let ex_chunk = extractor.clone();
    let ex_complete = extractor.clone();

    let log_chunk = log.file.clone();
    let log_raw = log.file.clone();

    JsonStreamTransport::run(
        bundle,
        resume_session_id,
        Some("claude-opus-4-6"),
        cwd,
        StreamCallbacks {
            on_pid: None,
            on_session_id: Box::new(move |sid| {
                *sid_w.lock().unwrap() = Some(sid);
            }),
            on_text_chunk: Box::new(move |text| {
                let mut ex = ex_chunk.lock().unwrap();
                let lines = ex.push(&text);
                drop(ex);
                let mut evts = evts_chunk.lock().unwrap();
                let mut f = log_chunk.lock().unwrap();
                for line in lines {
                    let _ = writeln!(f, "[text] {}", &line[..line.len().min(300)]);
                    if let Some((et, jv)) = parse_poe_event(&line) {
                        let _ = writeln!(f, "[poe:{}] (parsed)", et);
                        evts.push((et, jv));
                    }
                }
            }),
            on_raw_json: Box::new(move |json| {
                let s = serde_json::to_string(&json).unwrap_or_default();
                let mut f = log_raw.lock().unwrap();
                let _ = writeln!(f, "[raw] {}", &s[..s.len().min(250)]);
            }),
            on_complete: Box::new(move || {
                // Flush TextBufExtractor tail for events without trailing newline.
                if let Some(tail) = ex_complete.lock().unwrap().flush() {
                    if !tail.trim().is_empty() {
                        if let Some((et, jv)) = parse_poe_event(&tail) {
                            evts_complete.lock().unwrap().push((et, jv));
                        }
                    }
                }
                *done_w.lock().unwrap() = true;
            }),
        },
    )?;

    let session_id_val = session_id.lock().unwrap().clone();
    let poe_events_val = poe_events.lock().unwrap().clone();
    let on_complete_fired_val = *on_complete.lock().unwrap();

    Ok(AgentRun {
        session_id: session_id_val,
        poe_events: poe_events_val,
        on_complete_fired: on_complete_fired_val,
    })
}

// ── Skill loader ──────────────────────────────────────────────────────────────

fn operational_analyst() -> poe2_lib::skills::ResolvedSkill {
    // In debug builds, load_skill checks CARGO_MANIFEST_DIR/skills/ first.
    // Pass a dummy project path and empty resource dir — dev-mode path wins.
    let dummy_project = TempDir::new().unwrap();
    let dummy_resources = TempDir::new().unwrap();
    load_skill("operational-analyst", dummy_project.path(), dummy_resources.path())
        .unwrap_or_else(|e| panic!("load operational-analyst skill: {}", e))
}

// ── SC-1 / SC-2 / SC-3 ───────────────────────────────────────────────────────

/// SC-1/2/3: operational-analyst in interactive mode emits poe:brief → poe:chat → poe:yield.
///
/// Validates:
///   SC-1  interactive mode activates (poe:chat present, poe:done absent)
///   SC-2  poe:chat content is clean prose, not raw JSON protocol output
///   SC-3  poe:yield follows poe:chat in emission order
#[test]
#[ignore = "real Claude API test — run explicitly: cargo test --test interactive_conops_test -- --ignored --test-threads=1 --nocapture"]
fn interactive_mode_emits_chat_then_yields_not_done() {
    if !claude_on_path() {
        eprintln!("SKIP: claude not on PATH");
        return;
    }

    let log = Log::open("interactive-round1");
    log.line("=== SC-1/2/3: interactive mode first run ===");

    let skill = operational_analyst();
    log.line(&format!(
        "skill loaded: {} modes={:?} model={:?}",
        skill.skill_id, skill.modes, skill.model
    ));

    let cwd = TempDir::new().expect("tempdir");
    let bundle = assemble_input_bundle(
        &SpawnMode::Interactive,
        &skill,
        "Develop CONOPS",
        Some("A JavaScript clone of Wordle for learning purposes."),
        &[],
        &[],
        &[],
    );

    log.line("--- BUNDLE (first 800 chars) ---");
    log.line(&bundle[..bundle.len().min(800)]);
    log.line("--- END BUNDLE ---");

    let run = run_agent(&bundle, None, cwd.path(), &log)
        .expect("run_agent failed — is claude on PATH with a valid API key?");

    let etypes = run.event_types();
    log.line(&format!("poe: events in order: {:?}", etypes));

    // ── Transport ───────────────────────────────────────────────────────────
    assert!(
        run.on_complete_fired,
        "on_complete must fire — transport must receive the result event"
    );
    assert!(
        run.session_id.is_some(),
        "session_id must be captured from init event (needed for resume)"
    );

    // ── SC-1: interactive mode activated ────────────────────────────────────
    let chat_events = run.chat_events();
    assert!(
        !chat_events.is_empty(),
        "SC-1 FAIL: operational-analyst in interactive mode must emit at least one \
         poe:chat event. Received events: {:?}",
        etypes
    );
    assert!(
        !run.has("done"),
        "SC-1 FAIL: poe:done must NOT be emitted in the first interactive run — \
         the agent must pause and wait for the human. Received: {:?}",
        etypes
    );

    // ── SC-2: poe:chat content is clean prose ───────────────────────────────
    for (i, chat_val) in chat_events.iter().enumerate() {
        let content = chat_val["content"]
            .as_str()
            .unwrap_or_else(|| panic!("poe:chat[{}] must have a string 'content' field", i));

        log.line(&format!(
            "poe:chat[{}] content ({} chars): {}",
            i,
            content.len(),
            &content[..content.len().min(200)]
        ));

        assert!(
            content.len() > 15,
            "SC-2 FAIL: poe:chat[{}] content too short ({} chars) — expected a real question. \
             Got: {:?}",
            i, content.len(), content
        );
        assert!(
            !content.trim_start().starts_with('{'),
            "SC-2 FAIL: poe:chat[{}] content starts with '{{' — the agent output is raw JSON, \
             not extracted prose. The poe:chat handler or ingester is broken. Got: {:?}",
            i, content
        );
        assert!(
            !content.contains("\"poe\""),
            "SC-2 FAIL: poe:chat[{}] content contains '\"poe\"' — looks like raw protocol JSON \
             leaked into the content field. Got: {:?}",
            i, content
        );
    }

    // ── SC-3: poe:yield follows poe:chat ────────────────────────────────────
    let chat_pos = run.pos("chat").expect("poe:chat position");
    let yield_pos = run.pos("yield").unwrap_or_else(|| {
        panic!(
            "SC-3 FAIL: poe:yield not found — agent never yielded after asking a question. \
             Events: {:?}",
            etypes
        )
    });
    assert!(
        yield_pos > chat_pos,
        "SC-3 FAIL: poe:yield (pos={}) must come AFTER poe:chat (pos={}). \
         Full event order: {:?}",
        yield_pos, chat_pos, etypes
    );

    log.line("SC-1/2/3 PASSED");
}

// ── SC-4 ─────────────────────────────────────────────────────────────────────

/// SC-4: resumed session after human response emits next poe:chat or poe:done.
///
/// Two-phase test:
///   Phase 1: interactive run → poe:chat + poe:yield + session_id captured
///   Phase 2: --resume with "Human: <response>" → poe:chat (next question)
///            OR poe:artifact + poe:done (CONOPS complete in 2 rounds)
#[test]
#[ignore = "real Claude API test — run explicitly: cargo test --test interactive_conops_test -- --ignored --test-threads=1 --nocapture"]
fn resume_after_human_response_continues_or_completes() {
    if !claude_on_path() {
        eprintln!("SKIP: claude not on PATH");
        return;
    }

    let log = Log::open("interactive-resume");
    log.line("=== SC-4: resume after human response ===");

    let skill = operational_analyst();
    let cwd = TempDir::new().expect("tempdir");
    let bundle = assemble_input_bundle(
        &SpawnMode::Interactive,
        &skill,
        "Develop CONOPS",
        Some("A JavaScript clone of Wordle for learning purposes."),
        &[],
        &[],
        &[],
    );

    // Phase 1 ─────────────────────────────────────────────────────────────────
    log.line("--- Phase 1: initial interactive run ---");
    let run1 = run_agent(&bundle, None, cwd.path(), &log)
        .expect("Phase 1 run_agent failed");

    let etypes1 = run1.event_types();
    log.line(&format!("Phase 1 events: {:?}", etypes1));

    let sid = run1.session_id.clone().unwrap_or_else(|| {
        panic!("Phase 1 must capture session_id (needed for resume)")
    });

    assert!(
        run1.has("chat"),
        "Phase 1 must emit poe:chat before resume test can proceed. Got: {:?}", etypes1
    );
    assert!(
        !run1.has("done"),
        "Phase 1 must NOT emit poe:done. Got: {:?}", etypes1
    );

    // Phase 2 ─────────────────────────────────────────────────────────────────
    log.line("--- Phase 2: resume with human response ---");
    log.line(&format!("resuming session_id: {}", sid));

    // Bundle format matches what resume_chat_agent in the orchestrator sends.
    let response_bundle = "---\nHuman: It's a learning project. \
        Done means core Wordle gameplay works in a browser — 6 guesses, \
        5-letter words, colour-coded feedback. No backend needed.\n";

    let run2 = run_agent(&response_bundle, Some(&sid), cwd.path(), &log)
        .expect("Phase 2 run_agent (resume) failed");

    let etypes2 = run2.event_types();
    log.line(&format!("Phase 2 events: {:?}", etypes2));

    // SC-4a: transport completes cleanly
    assert!(
        run2.on_complete_fired,
        "SC-4 FAIL: Phase 2 on_complete must fire. Got events: {:?}", etypes2
    );

    // SC-4b: Phase 2 must either ask another question or produce the CONOPS
    let has_chat = run2.has("chat");
    let has_done = run2.has("done");
    assert!(
        has_chat || has_done,
        "SC-4 FAIL: Phase 2 must emit poe:chat (next question) OR poe:done (CONOPS complete). \
         Got: {:?}",
        etypes2
    );

    // SC-4c: if Phase 2 asks another question, content must be clean prose
    if has_chat {
        log.line("Phase 2: agent asked another question (normal — ≥3 rounds expected)");
        for chat_val in run2.chat_events() {
            let content = chat_val["content"].as_str().unwrap_or("");
            log.line(&format!("Phase 2 poe:chat content: {}", &content[..content.len().min(200)]));
            assert!(
                !content.contains("\"poe\""),
                "SC-4 FAIL: Phase 2 poe:chat content contains raw JSON. Got: {:?}", content
            );
            assert!(
                content.len() > 10,
                "SC-4 FAIL: Phase 2 poe:chat content too short: {:?}", content
            );
        }
        // Yield must follow chat in Phase 2 as well
        let chat_pos = run2.pos("chat").unwrap();
        let yield_pos = run2.pos("yield");
        assert!(
            yield_pos.map(|p| p > chat_pos).unwrap_or(false),
            "SC-4 FAIL: Phase 2 poe:yield must follow poe:chat. Events: {:?}", etypes2
        );
    }

    // SC-4d: if Phase 2 completes, a CONOPS artifact should exist
    if has_done {
        log.line("Phase 2: agent completed (CONOPS produced in 2 rounds)");
        assert!(
            run2.has("artifact"),
            "SC-4 FAIL: poe:done without poe:artifact — CONOPS document not produced. \
             Got: {:?}", etypes2
        );
    }

    log.line("SC-4 PASSED");
}

// ── SC-5 ─────────────────────────────────────────────────────────────────────

/// SC-5: operational-analyst WITHOUT interactive mode block runs autonomously —
/// emits poe:artifact + poe:done without any poe:chat turns.
#[test]
#[ignore = "real Claude API test — run explicitly: cargo test --test interactive_conops_test -- --ignored --test-threads=1 --nocapture"]
fn autonomous_mode_completes_without_chat() {
    if !claude_on_path() {
        eprintln!("SKIP: claude not on PATH");
        return;
    }

    let log = Log::open("autonomous-mode");
    log.line("=== SC-5: autonomous mode completes without poe:chat ===");

    let skill = operational_analyst();
    let cwd = TempDir::new().expect("tempdir");
    let bundle = assemble_input_bundle(
        &SpawnMode::Autonomous,
        &skill,
        "Develop CONOPS",
        Some("A JavaScript clone of Wordle for learning purposes."),
        &[],
        &[],
        &[],
    );

    log.line("--- BUNDLE mode block (first 300 chars) ---");
    log.line(&bundle[..bundle.len().min(300)]);
    log.line("---");

    let run = run_agent(&bundle, None, cwd.path(), &log)
        .expect("run_agent failed");

    let etypes = run.event_types();
    log.line(&format!("poe: events: {:?}", etypes));

    // SC-5a: transport completes
    assert!(
        run.on_complete_fired,
        "SC-5 FAIL: on_complete must fire in autonomous mode"
    );

    // SC-5b: poe:done present — autonomous mode must complete in one pass
    assert!(
        run.has("done"),
        "SC-5 FAIL: autonomous mode must emit poe:done. Got: {:?}", etypes
    );

    // SC-5c: NO poe:chat — autonomous agents do not ask questions
    assert!(
        !run.has("chat"),
        "SC-5 FAIL: autonomous mode must NOT emit poe:chat (the interactive mode \
         block was not injected). Got: {:?}", etypes
    );

    // SC-5d: poe:artifact — CONOPS document must be produced
    assert!(
        run.has("artifact"),
        "SC-5 FAIL: autonomous mode must produce a poe:artifact (the CONOPS). \
         Got: {:?}", etypes
    );

    log.line("SC-5 PASSED");
}
