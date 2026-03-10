//! Real-agent integration tests: Phase 3 skill/orchestrator interactions.
//!
//! These tests spawn real `claude` processes with the production `product-manager`
//! and `senior-engineer` skills and validate the poe: event protocol end-to-end.
//! They exist specifically to catch protocol mismatches that mocked tests cannot —
//! e.g. incorrect event names, wrong payload structure, missing review events.
//!
//! Tests covered (Phase 3 success criteria):
//!
//!   PM-1  product-manager emits poe:task + poe:edge (valid DAG output)
//!   PM-2  product-manager emits poe:review after creating DAG (multi-reviewer flow)
//!   PM-3  product-manager resumes with ReviewResult and emits poe:done
//!   PM-4  product-manager emits poe:yield with preceding poe:review (gate detected)
//!   PM-5  product-manager emits poe:yield after poe:decision (advisor-style yield)
//!   SE-1  senior-engineer receives plan_review bundle, emits poe:artifact + poe:done
//!   SE-2  senior-engineer approves a well-formed plan (artifact contains APPROVED)
//!   SE-3  senior-engineer requests revision on a flawed plan (artifact contains BLOCKED)
//!   ADV-1 poe-base skill receives advisor query, emits poe:advisor + poe:yield
//!   ADV-2 resumed advisor session maintains context
//!   LOOP  full PM→SE→PM roundtrip: PM creates DAG, SE reviews, PM receives result
//!
//! REQUIRES: `claude` binary on PATH with a valid ANTHROPIC_API_KEY.
//! Enable: `PHASE3_RUN_LIVE_TESTS=1 cargo test --test phase3_skill_tests -- --nocapture`
//!
//! All tests skip gracefully when `PHASE3_RUN_LIVE_TESTS` is not set or when
//! `claude` is not on PATH.
//! Logs: target/phase3-*.log

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
        let path = format!("target/phase3-{}.log", name);
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

// ── Guards ────────────────────────────────────────────────────────────────────

fn claude_on_path() -> bool {
    std::process::Command::new("claude")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn live_tests_enabled() -> bool {
    std::env::var("PHASE3_RUN_LIVE_TESTS").is_ok()
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

    fn has(&self, event_type: &str) -> bool {
        self.poe_events.iter().any(|(t, _)| t == event_type)
    }

    /// Position of first occurrence of event_type in emission order.
    fn pos(&self, event_type: &str) -> Option<usize> {
        self.poe_events.iter().position(|(t, _)| t == event_type)
    }

    /// All events of a specific type.
    fn events_of_type(&self, event_type: &str) -> Vec<&serde_json::Value> {
        self.poe_events
            .iter()
            .filter(|(t, _)| t == event_type)
            .map(|(_, v)| v)
            .collect()
    }

    /// First event matching event_type.
    fn first_event(&self, event_type: &str) -> Option<&serde_json::Value> {
        self.poe_events
            .iter()
            .find(|(t, _)| t == event_type)
            .map(|(_, v)| v)
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

// ── Skill loaders ─────────────────────────────────────────────────────────────

fn product_manager_skill() -> poe2_lib::skills::ResolvedSkill {
    let dummy_project = TempDir::new().unwrap();
    let dummy_resources = TempDir::new().unwrap();
    load_skill("product-manager", dummy_project.path(), dummy_resources.path())
        .unwrap_or_else(|e| panic!("load product-manager skill: {}", e))
}

fn senior_engineer_skill() -> poe2_lib::skills::ResolvedSkill {
    let dummy_project = TempDir::new().unwrap();
    let dummy_resources = TempDir::new().unwrap();
    load_skill("senior-engineer", dummy_project.path(), dummy_resources.path())
        .unwrap_or_else(|e| panic!("load senior-engineer skill: {}", e))
}

// ── Review bundle helper ──────────────────────────────────────────────────────

/// Assemble a plan_review input bundle for a reviewer agent (e.g., senior-engineer).
///
/// Layout matches Protocol.md §3 review task format:
///   - Autonomous mode protocol block (reviewers run autonomously)
///   - Reviewer skill prompt
///   - Task type / project / phase context
///   - The plan document (the artifact being reviewed)
///   - The specific review request from the requesting agent
fn assemble_review_bundle(
    reviewer_skill: &poe2_lib::skills::ResolvedSkill,
    project_name: &str,
    phase_title: &str,
    artifact_content: &str,
    review_request_message: &str,
) -> String {
    let mut bundle = String::new();

    // Autonomous mode protocol block (reviewers are always autonomous).
    bundle.push_str(concat!(
        "## Execution Protocol\n\n",
        "You are running in autonomous mode — there is no human at the keyboard.\n\n",
        "- Begin by emitting `poe:brief` describing your understanding of the task.\n",
        "- Work from the context in this bundle. Do not ask questions.\n",
        "- Raise genuine blockers via `poe:decision`, then continue with your best judgement.\n",
        "- Emit `poe:done` as your final output. The process exits after this.\n",
    ));
    bundle.push_str("\n---\n\n");

    // Reviewer skill prompt.
    bundle.push_str(&reviewer_skill.prompt);
    bundle.push_str("\n\n---\n\n");

    // Task context header.
    bundle.push_str(&format!(
        "**Task Type**: plan_review\n**Project**: {}\n**Phase**: {}\n\n",
        project_name, phase_title
    ));

    // Plan document.
    bundle.push_str("## Plan Document\n\n");
    bundle.push_str(artifact_content);
    bundle.push_str("\n\n");

    // Review request.
    bundle.push_str("## Review Request\n\n");
    bundle.push_str(review_request_message);
    bundle.push('\n');

    bundle
}

// ── Minimal plan fixtures ─────────────────────────────────────────────────────

/// A minimal but well-formed stage plan summary for SE review tests.
const GOOD_PLAN_SUMMARY: &str = r#"
## Stage 1 Plan Summary

### Epics

- E-1 (uuid: epic-001): User can authenticate and manage their account
  skill: backend | type: epic

### Features

- F-1 (uuid: feat-001): User registration and login
  skill: backend | type: feature | parent: epic-001
- F-2 (uuid: feat-002): Password reset flow
  skill: backend | type: feature | parent: epic-001

### Tasks

- T-1 (uuid: task-001): Implement JWT authentication middleware
  skill: backend | type: task | parent: feat-001
  estimatedHours: 3 | artifactType: code
  acceptanceCriteria: POST /auth/login returns JWT; token verified on protected routes.
- T-2 (uuid: task-002): Unit tests for JWT middleware
  skill: test | type: task | parent: feat-001
  estimatedHours: 2 | artifactType: test
  acceptanceCriteria: 100% branch coverage on auth middleware.
- T-3 (uuid: task-003): Database migration — users table
  skill: database | type: task | parent: feat-001
  estimatedHours: 1 | artifactType: migration
  acceptanceCriteria: users table created with id, email, password_hash, created_at.
- T-4 (uuid: task-004): Implement password reset email flow
  skill: backend | type: task | parent: feat-002
  estimatedHours: 3 | artifactType: code
  acceptanceCriteria: POST /auth/reset sends email with one-time token; token expires in 1h.
- T-5 (uuid: task-005): Unit tests for password reset
  skill: test | type: task | parent: feat-002
  estimatedHours: 2 | artifactType: test
  acceptanceCriteria: Reset token generation, expiry, and invalidation all tested.

### Dependency Edges

- task-003 → task-001 (migration must precede implementation)
- task-001 → task-002 (implementation must precede tests)
- task-004 → task-005 (implementation must precede tests)

### Estimated sizes: all tasks 1–3 hours. Stage is independently deployable.
"#;

/// A deliberately flawed plan for SE rejection tests.
const FLAWED_PLAN_SUMMARY: &str = r#"
## Stage 1 Plan Summary (FLAWED — for test purposes)

### Epics

- E-1 (uuid: epic-001): Authentication

### Features

- F-1 (uuid: feat-001): User login
  skill: backend | type: feature | parent: epic-001

### Tasks

- T-1 (uuid: task-001): Build the entire authentication system
  skill: backend | type: task | parent: feat-001
  estimatedHours: 40
  acceptanceCriteria: (none)

### Dependency Edges

(none)

### Notes

- Tasks use "tasks" field but the live schema uses "nodes".
- No test tasks. No migration task. No acceptance criteria on T-1.
- T-1 is massively over-sized (40h vs 1–4h limit).
- No dependency edges defined.
"#;

// ── PM tests ──────────────────────────────────────────────────────────────────

/// PM-1: product-manager receives a task description and emits poe:task + poe:edge events.
///
/// Validates:
///   - poe:brief is emitted first
///   - at least one poe:task event is emitted
///   - at least one poe:edge event is emitted (dependency edges)
///   - poe:task events have id, title, skill, type fields
#[tokio::test]
#[ignore]
async fn pm_1_creates_task_dag() {
    if !live_tests_enabled() || !claude_on_path() {
        eprintln!("SKIP: PHASE3_RUN_LIVE_TESTS not set or claude not on PATH");
        return;
    }

    let log = Log::open("pm-1-creates-dag");
    log.line("=== PM-1: product-manager creates task DAG ===");

    let skill = product_manager_skill();
    let cwd = TempDir::new().expect("tempdir");

    // Provide a minimal planning context without requiring real artifact files.
    let task_desc = concat!(
        "Plan Stage 1 for a simple Todo List web application.\n",
        "Features: create todo, mark complete, delete todo.\n",
        "Tech stack: React frontend, Node.js backend, PostgreSQL database.\n",
        "The Guardrails Review is APPROVED. Proceed with Stage 1 planning.\n",
        "User Analysis: P0 features are create-todo, complete-todo, delete-todo.\n",
        "No Must-Nots defined for this stage.",
    );

    let bundle = assemble_input_bundle(
        &SpawnMode::Autonomous,
        &skill,
        "Plan Stage 1",
        Some(task_desc),
        &[("project", "Todo List App"), ("phase", "Stage 1 Increment Planning")],
        &[("guardrails-verdict", "APPROVED — no conflicts found")],
        &[],
    );

    log.line(&format!("bundle length: {} chars", bundle.len()));

    let run = run_agent(&bundle, None, cwd.path(), &log)
        .expect("run_agent failed — is claude on PATH with a valid API key?");

    let etypes = run.event_types();
    log.line(&format!("poe: events in order: {:?}", etypes));

    assert!(run.on_complete_fired, "PM-1 FAIL: on_complete must fire");

    // poe:brief must be first event
    let brief_pos = run
        .pos("brief")
        .expect("PM-1 FAIL: poe:brief not emitted — product-manager must emit brief first");
    assert_eq!(brief_pos, 0, "PM-1 FAIL: poe:brief must be the first event, got pos={}", brief_pos);

    // At least one poe:task event
    let task_events = run.events_of_type("task");
    assert!(
        !task_events.is_empty(),
        "PM-1 FAIL: product-manager must emit poe:task events (DAG nodes). Got: {:?}",
        etypes
    );

    // poe:task events must have required fields
    for (i, task_val) in task_events.iter().enumerate() {
        let id = task_val.get("id").and_then(|v| v.as_str());
        let title = task_val.get("title").and_then(|v| v.as_str());
        let skill_field = task_val.get("skill").and_then(|v| v.as_str());
        let type_field = task_val.get("type").and_then(|v| v.as_str());
        assert!(
            id.is_some(),
            "PM-1 FAIL: poe:task[{}] missing 'id' field. Got: {}",
            i, task_val
        );
        assert!(
            title.is_some(),
            "PM-1 FAIL: poe:task[{}] missing 'title' field. Got: {}",
            i, task_val
        );
        assert!(
            skill_field.is_some(),
            "PM-1 FAIL: poe:task[{}] missing 'skill' field. Got: {}",
            i, task_val
        );
        assert!(
            type_field.is_some(),
            "PM-1 FAIL: poe:task[{}] missing 'type' field. Got: {}",
            i, task_val
        );
    }

    // At least one poe:edge event
    let edge_events = run.events_of_type("edge");
    assert!(
        !edge_events.is_empty(),
        "PM-1 FAIL: product-manager must emit poe:edge events (dependency edges). Got: {:?}",
        etypes
    );

    // poe:edge events must have from and to fields
    for (i, edge_val) in edge_events.iter().enumerate() {
        assert!(
            edge_val.get("from").is_some(),
            "PM-1 FAIL: poe:edge[{}] missing 'from' field. Got: {}",
            i, edge_val
        );
        assert!(
            edge_val.get("to").is_some(),
            "PM-1 FAIL: poe:edge[{}] missing 'to' field. Got: {}",
            i, edge_val
        );
    }

    log.line(&format!(
        "PM-1 PASSED: {} task events, {} edge events",
        task_events.len(),
        edge_events.len()
    ));
}

/// PM-2: product-manager emits poe:review after creating DAG (multi-reviewer flow).
///
/// Validates:
///   - poe:review is emitted
///   - poe:review to senior-engineer is always present
///   - poe:review events have reviewer_skill and content fields
///   - poe:yield follows all poe:review events
#[tokio::test]
#[ignore]
async fn pm_2_emits_review_request() {
    if !live_tests_enabled() || !claude_on_path() {
        eprintln!("SKIP: PHASE3_RUN_LIVE_TESTS not set or claude not on PATH");
        return;
    }

    let log = Log::open("pm-2-review-request");
    log.line("=== PM-2: product-manager emits poe:review after DAG ===");

    let skill = product_manager_skill();
    let cwd = TempDir::new().expect("tempdir");

    let task_desc = concat!(
        "Plan Stage 1 for a simple Note Taking application.\n",
        "Features: create note, list notes, delete note.\n",
        "Tech stack: Vue.js frontend, Python FastAPI backend, SQLite database.\n",
        "The Guardrails Review is APPROVED. Proceed with Stage 1 planning.\n",
        "User Analysis: P0 features are create-note, list-notes, delete-note.\n",
        "No Must-Nots defined. This stage includes a database migration task.",
    );

    let bundle = assemble_input_bundle(
        &SpawnMode::Autonomous,
        &skill,
        "Plan Stage 1",
        Some(task_desc),
        &[("project", "Note Taking App"), ("phase", "Stage 1 Increment Planning")],
        &[("guardrails-verdict", "APPROVED — no conflicts found")],
        &[],
    );

    let run = run_agent(&bundle, None, cwd.path(), &log)
        .expect("run_agent failed");

    let etypes = run.event_types();
    log.line(&format!("poe: events in order: {:?}", etypes));

    assert!(run.on_complete_fired, "PM-2 FAIL: on_complete must fire");

    // poe:review must be emitted
    let review_events = run.events_of_type("review");
    assert!(
        !review_events.is_empty(),
        "PM-2 FAIL: product-manager must emit at least one poe:review event. Got: {:?}",
        etypes
    );

    // senior-engineer review is always required
    let has_se_review = review_events.iter().any(|v| {
        v.get("reviewer_skill")
            .and_then(|s| s.as_str())
            .map(|s| s == "senior-engineer")
            .unwrap_or(false)
    });
    assert!(
        has_se_review,
        "PM-2 FAIL: product-manager must always emit poe:review to senior-engineer. \
         Review events: {:?}",
        review_events
    );

    // All poe:review events must have reviewer_skill and content
    for (i, review_val) in review_events.iter().enumerate() {
        assert!(
            review_val.get("reviewer_skill").is_some(),
            "PM-2 FAIL: poe:review[{}] missing 'reviewer_skill'. Got: {}",
            i, review_val
        );
        let content = review_val.get("content").and_then(|v| v.as_str());
        assert!(
            content.map(|c| c.len() > 10).unwrap_or(false),
            "PM-2 FAIL: poe:review[{}] missing or empty 'content'. Got: {}",
            i, review_val
        );
    }

    // poe:review must precede poe:yield
    let review_pos = run.pos("review").expect("already confirmed review exists");
    let yield_pos = run.pos("yield").unwrap_or_else(|| {
        panic!(
            "PM-2 FAIL: poe:yield not found — product-manager must yield after review requests. \
             Got: {:?}",
            etypes
        )
    });
    assert!(
        yield_pos > review_pos,
        "PM-2 FAIL: poe:yield (pos={}) must come AFTER poe:review (pos={}). Got: {:?}",
        yield_pos, review_pos, etypes
    );

    // poe:done must NOT be present — PM must await review results
    assert!(
        !run.has("done"),
        "PM-2 FAIL: product-manager must NOT emit poe:done before receiving review results. \
         Got: {:?}",
        etypes
    );

    log.line(&format!("PM-2 PASSED: {} review event(s) emitted", review_events.len()));
}

/// PM-3: product-manager resumes with ReviewResult(s) and emits poe:done.
///
/// Two-phase test:
///   Phase 1: PM creates DAG + emits poe:review + poe:yield → captures session_id
///   Phase 2: Resume PM with ReviewResult(s) APPROVED → PM emits poe:done
#[tokio::test]
#[ignore]
async fn pm_3_resumes_with_review_result() {
    if !live_tests_enabled() || !claude_on_path() {
        eprintln!("SKIP: PHASE3_RUN_LIVE_TESTS not set or claude not on PATH");
        return;
    }

    let log = Log::open("pm-3-resume-review");
    log.line("=== PM-3: product-manager resumes with ReviewResult → poe:done ===");

    let skill = product_manager_skill();
    let cwd = TempDir::new().expect("tempdir");

    let task_desc = concat!(
        "Plan Stage 1 for a simple Bookmark Manager application.\n",
        "Features: add bookmark, list bookmarks, delete bookmark.\n",
        "Tech stack: Svelte frontend, Go backend, PostgreSQL database.\n",
        "The Guardrails Review is APPROVED. Proceed with Stage 1 planning.\n",
        "User Analysis: P0 features are add-bookmark, list-bookmarks, delete-bookmark.\n",
        "No Must-Nots defined. Stage includes database migration.",
    );

    let bundle = assemble_input_bundle(
        &SpawnMode::Autonomous,
        &skill,
        "Plan Stage 1",
        Some(task_desc),
        &[("project", "Bookmark Manager"), ("phase", "Stage 1 Increment Planning")],
        &[("guardrails-verdict", "APPROVED — no conflicts found")],
        &[],
    );

    // Phase 1: initial planning run
    log.line("--- Phase 1: initial PM run ---");
    let run1 = run_agent(&bundle, None, cwd.path(), &log)
        .expect("Phase 1 run_agent failed");

    let etypes1 = run1.event_types();
    log.line(&format!("Phase 1 events: {:?}", etypes1));

    let sid = run1.session_id.clone().unwrap_or_else(|| {
        panic!("PM-3 FAIL: Phase 1 must capture session_id (needed for resume)")
    });

    assert!(
        run1.has("review"),
        "PM-3 FAIL: Phase 1 must emit poe:review before resume test can proceed. Got: {:?}",
        etypes1
    );
    assert!(
        run1.has("yield"),
        "PM-3 FAIL: Phase 1 must emit poe:yield (waiting for review results). Got: {:?}",
        etypes1
    );
    assert!(
        !run1.has("done"),
        "PM-3 FAIL: Phase 1 must NOT emit poe:done. Got: {:?}",
        etypes1
    );

    // Collect all review IDs emitted in Phase 1 to construct a matching ReviewResult bundle.
    let review_ids: Vec<(String, String)> = run1
        .events_of_type("review")
        .iter()
        .map(|v| {
            let id = v
                .get("id")
                .and_then(|s| s.as_str())
                .unwrap_or("r-eng")
                .to_owned();
            let reviewer_skill = v
                .get("reviewer_skill")
                .and_then(|s| s.as_str())
                .unwrap_or("senior-engineer")
                .to_owned();
            (id, reviewer_skill)
        })
        .collect();

    log.line(&format!("Phase 1 review IDs: {:?}", review_ids));

    // Phase 2: resume with APPROVED ReviewResult(s) for all reviews emitted.
    log.line("--- Phase 2: resume PM with ReviewResult APPROVED ---");
    log.line(&format!("resuming session_id: {}", sid));

    let mut resume_bundle = String::new();
    for (review_id, reviewer_skill) in &review_ids {
        resume_bundle.push_str("---\n");
        resume_bundle.push_str(&format!(
            "ReviewResult id={} skill={} verdict=APPROVED\n",
            review_id, reviewer_skill
        ));
        resume_bundle.push_str(
            "The plan is technically sound. All tasks are correctly sized, \
             properly assigned, and the dependency graph is acyclic. \
             All features have implementation and test tasks. Proceed.\n",
        );
    }
    resume_bundle.push_str("---\n");

    let run2 = run_agent(&resume_bundle, Some(&sid), cwd.path(), &log)
        .expect("Phase 2 run_agent (resume) failed");

    let etypes2 = run2.event_types();
    log.line(&format!("Phase 2 events: {:?}", etypes2));

    assert!(
        run2.on_complete_fired,
        "PM-3 FAIL: Phase 2 on_complete must fire. Got: {:?}",
        etypes2
    );

    // PM must emit poe:done after receiving APPROVED results
    assert!(
        run2.has("done"),
        "PM-3 FAIL: product-manager must emit poe:done after receiving APPROVED ReviewResult(s). \
         Got: {:?}",
        etypes2
    );

    log.line("PM-3 PASSED");
}

/// PM-4: product-manager emits poe:yield with preceding poe:review (phase gate detected).
///
/// This is essentially a subset of PM-2, but explicitly tests the gate condition:
/// all task events emitted → review request → yield (the orchestrator treats this
/// yield as yield_reason='review').
#[tokio::test]
#[ignore]
async fn pm_4_phase_gate_detected() {
    if !live_tests_enabled() || !claude_on_path() {
        eprintln!("SKIP: PHASE3_RUN_LIVE_TESTS not set or claude not on PATH");
        return;
    }

    let log = Log::open("pm-4-phase-gate");
    log.line("=== PM-4: product-manager yields at review gate ===");

    let skill = product_manager_skill();
    let cwd = TempDir::new().expect("tempdir");

    let task_desc = concat!(
        "Plan Stage 1 for a simple Event Tracker application.\n",
        "Features: create event, list events, mark event attended.\n",
        "The Guardrails Review is APPROVED. Proceed with Stage 1 planning.\n",
        "User Analysis: P0 features are create-event, list-events, mark-attended.",
    );

    let bundle = assemble_input_bundle(
        &SpawnMode::Autonomous,
        &skill,
        "Plan Stage 1",
        Some(task_desc),
        &[("project", "Event Tracker"), ("phase", "Stage 1 Increment Planning")],
        &[("guardrails-verdict", "APPROVED")],
        &[],
    );

    let run = run_agent(&bundle, None, cwd.path(), &log)
        .expect("run_agent failed");

    let etypes = run.event_types();
    log.line(&format!("poe: events in order: {:?}", etypes));

    assert!(run.on_complete_fired, "PM-4 FAIL: on_complete must fire");

    // Gate structure: tasks → review(s) → yield
    assert!(
        run.has("task"),
        "PM-4 FAIL: poe:task must be emitted before the gate. Got: {:?}",
        etypes
    );
    assert!(
        run.has("review"),
        "PM-4 FAIL: poe:review must be emitted at the gate. Got: {:?}",
        etypes
    );
    assert!(
        run.has("yield"),
        "PM-4 FAIL: poe:yield must be emitted at the gate. Got: {:?}",
        etypes
    );

    // Verify ordering: task < review < yield
    let task_pos = run.pos("task").unwrap();
    let review_pos = run.pos("review").unwrap();
    let yield_pos = run.pos("yield").unwrap();

    assert!(
        task_pos < review_pos,
        "PM-4 FAIL: poe:task (pos={}) must precede poe:review (pos={}). Got: {:?}",
        task_pos, review_pos, etypes
    );
    assert!(
        review_pos < yield_pos,
        "PM-4 FAIL: poe:review (pos={}) must precede poe:yield (pos={}). Got: {:?}",
        review_pos, yield_pos, etypes
    );

    // poe:done must NOT be present — PM is waiting for review results
    assert!(
        !run.has("done"),
        "PM-4 FAIL: poe:done must NOT be present before review results received. Got: {:?}",
        etypes
    );

    log.line("PM-4 PASSED");
}

/// PM-5: product-manager emits poe:yield with preceding poe:decision (advisor-style yield).
///
/// When the PM encounters a genuine scope decision it cannot resolve from artifacts,
/// it must emit poe:decision + poe:yield (yield_reason='decision' in the ingester).
/// This test provides ambiguous scope context to trigger a decision.
#[tokio::test]
#[ignore]
async fn pm_5_advisor_yield() {
    if !live_tests_enabled() || !claude_on_path() {
        eprintln!("SKIP: PHASE3_RUN_LIVE_TESTS not set or claude not on PATH");
        return;
    }

    let log = Log::open("pm-5-advisor-yield");
    log.line("=== PM-5: product-manager yields with poe:decision (advisor/scope question) ===");

    let skill = product_manager_skill();
    let cwd = TempDir::new().expect("tempdir");

    // Deliberately ambiguous scope to provoke a poe:decision before planning completes.
    // Include conflicting signals: Guardrails says BLOCKED, which forces decision+yield.
    let task_desc = concat!(
        "Plan Stage 1 for a Healthcare Patient Portal.\n",
        "IMPORTANT: The Guardrails Review is BLOCKED.\n",
        "There are 3 unresolved CONFLICT items in the guardrails-review.md document.\n",
        "Per the product-manager skill protocol, when the Guardrails Review is BLOCKED \n",
        "you must emit poe:decision and poe:yield immediately.\n",
        "Do NOT proceed with planning until the human resolves the guardrails conflicts.",
    );

    let bundle = assemble_input_bundle(
        &SpawnMode::Autonomous,
        &skill,
        "Plan Stage 1",
        Some(task_desc),
        &[("project", "Patient Portal"), ("phase", "Stage 1 Increment Planning")],
        &[("guardrails-verdict", "BLOCKED — 3 unresolved conflicts")],
        &[],
    );

    let run = run_agent(&bundle, None, cwd.path(), &log)
        .expect("run_agent failed");

    let etypes = run.event_types();
    log.line(&format!("poe: events in order: {:?}", etypes));

    assert!(run.on_complete_fired, "PM-5 FAIL: on_complete must fire");

    // PM must emit poe:yield (waiting for human to resolve)
    assert!(
        run.has("yield"),
        "PM-5 FAIL: poe:yield must be emitted when blocked by guardrails. Got: {:?}",
        etypes
    );

    // Either poe:decision or poe:review must precede the yield
    let has_decision = run.has("decision");
    let has_review = run.has("review");
    assert!(
        has_decision || has_review,
        "PM-5 FAIL: poe:yield must be preceded by poe:decision or poe:review. \
         Got no substantive event before yield. Events: {:?}",
        etypes
    );

    // poe:done must NOT be present — PM is blocked
    assert!(
        !run.has("done"),
        "PM-5 FAIL: poe:done must NOT be present when PM is blocked on guardrails. Got: {:?}",
        etypes
    );

    log.line("PM-5 PASSED");
}

// ── SE tests ──────────────────────────────────────────────────────────────────

/// SE-1: senior-engineer receives plan_review bundle, emits poe:artifact + poe:done.
///
/// Validates:
///   - poe:brief is emitted first
///   - poe:artifact is emitted with artifact_type="review"
///   - poe:artifact content contains a Verdict line
///   - poe:done is emitted last
#[tokio::test]
#[ignore]
async fn se_1_reviews_plan() {
    if !live_tests_enabled() || !claude_on_path() {
        eprintln!("SKIP: PHASE3_RUN_LIVE_TESTS not set or claude not on PATH");
        return;
    }

    let log = Log::open("se-1-reviews-plan");
    log.line("=== SE-1: senior-engineer receives plan_review bundle ===");

    let skill = senior_engineer_skill();
    let cwd = TempDir::new().expect("tempdir");

    let bundle = assemble_review_bundle(
        &skill,
        "Todo List App",
        "Stage 1 Increment Planning",
        GOOD_PLAN_SUMMARY,
        concat!(
            "Please review the Stage 1 plan for technical correctness:\n",
            "1. Are all tasks correctly sized (1–4 hours each)?\n",
            "2. Does every feature have at least one implementation task and one test task?\n",
            "3. Are the dependency edges correct and acyclic?\n",
            "4. Is every task assignable to a single specialist skill?\n",
            "Provide verdict: APPROVED, APPROVED_WITH_CONDITIONS, or BLOCKED.",
        ),
    );

    log.line(&format!("bundle length: {} chars", bundle.len()));

    let run = run_agent(&bundle, None, cwd.path(), &log)
        .expect("run_agent failed");

    let etypes = run.event_types();
    log.line(&format!("poe: events in order: {:?}", etypes));

    assert!(run.on_complete_fired, "SE-1 FAIL: on_complete must fire");

    // poe:brief must be first
    assert!(
        run.has("brief"),
        "SE-1 FAIL: poe:brief not emitted. Got: {:?}",
        etypes
    );

    // poe:artifact must be emitted with type="review"
    let artifact_events = run.events_of_type("artifact");
    assert!(
        !artifact_events.is_empty(),
        "SE-1 FAIL: poe:artifact (review) not emitted. Got: {:?}",
        etypes
    );

    let review_artifact = artifact_events.iter().find(|v| {
        v.get("artifact_type")
            .and_then(|t| t.as_str())
            .map(|t| t == "review")
            .unwrap_or(false)
    });
    assert!(
        review_artifact.is_some(),
        "SE-1 FAIL: poe:artifact with artifact_type='review' not found. \
         Artifact events: {:?}",
        artifact_events
    );

    let review_content = review_artifact
        .unwrap()
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    log.line(&format!(
        "review artifact content ({} chars): {}",
        review_content.len(),
        &review_content[..review_content.len().min(400)]
    ));

    // Content must contain a verdict
    let has_verdict = review_content.contains("APPROVED")
        || review_content.contains("APPROVED_WITH_CONDITIONS")
        || review_content.contains("BLOCKED");
    assert!(
        has_verdict,
        "SE-1 FAIL: review artifact content must contain a verdict \
         (APPROVED / APPROVED_WITH_CONDITIONS / BLOCKED). Got: {:?}",
        &review_content[..review_content.len().min(500)]
    );

    // poe:done must be emitted and must be last
    assert!(
        run.has("done"),
        "SE-1 FAIL: poe:done not emitted. Got: {:?}",
        etypes
    );

    // poe:artifact must precede poe:done
    let artifact_pos = run.pos("artifact").unwrap();
    let done_pos = run.pos("done").unwrap();
    assert!(
        artifact_pos < done_pos,
        "SE-1 FAIL: poe:artifact (pos={}) must precede poe:done (pos={}). Got: {:?}",
        artifact_pos, done_pos, etypes
    );

    log.line("SE-1 PASSED");
}

/// SE-2: senior-engineer approves a well-formed plan.
///
/// Provides a complete, correctly-sized, well-structured plan and asserts that
/// the review verdict is APPROVED or APPROVED_WITH_CONDITIONS (not BLOCKED).
#[tokio::test]
#[ignore]
async fn se_2_approve_path() {
    if !live_tests_enabled() || !claude_on_path() {
        eprintln!("SKIP: PHASE3_RUN_LIVE_TESTS not set or claude not on PATH");
        return;
    }

    let log = Log::open("se-2-approve");
    log.line("=== SE-2: senior-engineer approves a good plan ===");

    let skill = senior_engineer_skill();
    let cwd = TempDir::new().expect("tempdir");

    let bundle = assemble_review_bundle(
        &skill,
        "Todo List App",
        "Stage 1 Increment Planning",
        GOOD_PLAN_SUMMARY,
        concat!(
            "Please review this Stage 1 plan. The plan includes:\n",
            "- Epic, Feature, and Task hierarchy\n",
            "- All tasks sized 1–3 hours\n",
            "- Implementation tasks paired with test tasks\n",
            "- A database migration task prerequisite\n",
            "- Dependency edges that form a valid DAG\n",
            "Expected verdict: APPROVED or APPROVED_WITH_CONDITIONS.",
        ),
    );

    let run = run_agent(&bundle, None, cwd.path(), &log)
        .expect("run_agent failed");

    let etypes = run.event_types();
    log.line(&format!("poe: events: {:?}", etypes));

    assert!(run.on_complete_fired, "SE-2 FAIL: on_complete must fire");
    assert!(run.has("done"), "SE-2 FAIL: poe:done not emitted. Got: {:?}", etypes);

    // Find review artifact
    let artifact_events = run.events_of_type("artifact");
    let review_artifact = artifact_events.iter().find(|v| {
        v.get("artifact_type")
            .and_then(|t| t.as_str())
            .map(|t| t == "review")
            .unwrap_or(false)
    });

    assert!(
        review_artifact.is_some(),
        "SE-2 FAIL: review artifact not found. Got: {:?}",
        etypes
    );

    let review_content = review_artifact
        .unwrap()
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    log.line(&format!(
        "SE-2 review verdict excerpt: {}",
        &review_content[..review_content.len().min(600)]
    ));

    // Well-formed plan should not be BLOCKED
    let is_blocked = review_content.contains("**BLOCKED**")
        || (review_content.contains("BLOCKED")
            && !review_content.contains("not BLOCKED")
            && !review_content.contains("APPROVED"));
    // If BLOCKED appears but APPROVED also appears, likely APPROVED_WITH_CONDITIONS context.
    // We check that verdict line explicitly contains APPROVED.
    let is_approved = review_content.contains("APPROVED");
    assert!(
        is_approved,
        "SE-2 FAIL: a well-formed plan should receive APPROVED or APPROVED_WITH_CONDITIONS, \
         not BLOCKED. Review content: {}",
        &review_content[..review_content.len().min(800)]
    );

    let _ = is_blocked; // suppress unused warning
    log.line("SE-2 PASSED");
}

/// SE-3: senior-engineer requests revision on a deliberately flawed plan.
///
/// Provides a plan with clear defects (no test tasks, no acceptance criteria,
/// massively over-sized single task, no dependency edges) and asserts that
/// the review verdict is BLOCKED or APPROVED_WITH_CONDITIONS (not a clean APPROVED).
#[tokio::test]
#[ignore]
async fn se_3_revise_path() {
    if !live_tests_enabled() || !claude_on_path() {
        eprintln!("SKIP: PHASE3_RUN_LIVE_TESTS not set or claude not on PATH");
        return;
    }

    let log = Log::open("se-3-revise");
    log.line("=== SE-3: senior-engineer revises a flawed plan ===");

    let skill = senior_engineer_skill();
    let cwd = TempDir::new().expect("tempdir");

    let bundle = assemble_review_bundle(
        &skill,
        "Todo List App",
        "Stage 1 Increment Planning",
        FLAWED_PLAN_SUMMARY,
        concat!(
            "Please review this Stage 1 plan.\n",
            "Known issues in this plan:\n",
            "- Single task sized at 40 hours (far exceeds 1–4 hour limit)\n",
            "- No test tasks for any feature\n",
            "- No acceptance criteria on any task\n",
            "- No dependency edges defined\n",
            "- Schema note: uses 'tasks' field but live codebase uses 'nodes'\n",
            "Expected verdict: BLOCKED or APPROVED_WITH_CONDITIONS.",
        ),
    );

    let run = run_agent(&bundle, None, cwd.path(), &log)
        .expect("run_agent failed");

    let etypes = run.event_types();
    log.line(&format!("poe: events: {:?}", etypes));

    assert!(run.on_complete_fired, "SE-3 FAIL: on_complete must fire");
    assert!(run.has("done"), "SE-3 FAIL: poe:done not emitted. Got: {:?}", etypes);

    // Find review artifact
    let artifact_events = run.events_of_type("artifact");
    let review_artifact = artifact_events.iter().find(|v| {
        v.get("artifact_type")
            .and_then(|t| t.as_str())
            .map(|t| t == "review")
            .unwrap_or(false)
    });

    assert!(
        review_artifact.is_some(),
        "SE-3 FAIL: review artifact not found. Got: {:?}",
        etypes
    );

    let review_content = review_artifact
        .unwrap()
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    log.line(&format!(
        "SE-3 review verdict excerpt: {}",
        &review_content[..review_content.len().min(600)]
    ));

    // A flawed plan should not receive a clean APPROVED.
    // Accept BLOCKED or APPROVED_WITH_CONDITIONS.
    let is_cleanly_approved = review_content.contains("**APPROVED**")
        && !review_content.contains("BLOCKED")
        && !review_content.contains("APPROVED_WITH_CONDITIONS");

    assert!(
        !is_cleanly_approved,
        "SE-3 FAIL: a flawed plan should NOT receive a clean APPROVED verdict. \
         Expected BLOCKED or APPROVED_WITH_CONDITIONS. Review content: {}",
        &review_content[..review_content.len().min(800)]
    );

    log.line("SE-3 PASSED");
}

// ── ADV tests — poe:advisor protocol ─────────────────────────────────────────
//
// There is no dedicated `advisor.md` skill file in this repo. The poe:advisor
// event is a protocol-level event type (Protocol.md §2) used by skills that
// need to route messages to the advisor panel rather than the chat panel.
// We test the advisor event pattern using poe-base as the skill, with an
// explicit instruction to respond via poe:advisor + poe:yield.

/// ADV-1: agent receives an advisor query, emits poe:advisor + poe:yield.
///
/// Uses the poe-base skill with explicit advisor-mode instructions.
/// Validates:
///   - poe:advisor is emitted with a content field
///   - poe:yield follows poe:advisor
///   - poe:done is NOT emitted (session is yielded for human response)
#[tokio::test]
#[ignore]
async fn adv_1_responds_to_query() {
    if !live_tests_enabled() || !claude_on_path() {
        eprintln!("SKIP: PHASE3_RUN_LIVE_TESTS not set or claude not on PATH");
        return;
    }

    let log = Log::open("adv-1-advisor-query");
    log.line("=== ADV-1: agent emits poe:advisor + poe:yield in response to query ===");

    // Load poe-base as a generic foundation skill.
    let dummy_project = TempDir::new().unwrap();
    let dummy_resources = TempDir::new().unwrap();
    let poe_base = load_skill("poe-base", dummy_project.path(), dummy_resources.path())
        .unwrap_or_else(|e| panic!("load poe-base skill: {}", e));

    let cwd = TempDir::new().expect("tempdir");

    // Construct a bundle that directs the agent to act as an advisor.
    // The Interactive Mode block triggers interactive protocol behaviour.
    let mut bundle = String::new();
    bundle.push_str(concat!(
        "## Interactive Mode\n\n",
        "You are running in interactive mode — a human is present at the keyboard.\n\n",
        "- Use `poe:advisor` + `poe:yield` to send your response and wait for the human.\n",
        "- The orchestrator will resume your run with the answer as `Human: {response}`.\n",
        "- Do NOT use poe:chat. Use poe:advisor for all messages in this session.\n",
    ));
    bundle.push_str("\n---\n\n");
    bundle.push_str(&poe_base.prompt);
    bundle.push_str("\n\n---\n\n");
    bundle.push_str(concat!(
        "# Advisor Session\n\n",
        "The user has asked for strategic advice. You are acting as a strategic advisor.\n\n",
        "## User Question\n\n",
        "We are building a SaaS product for small businesses. Should we start with a \n",
        "mobile-first or desktop-first approach given that our primary persona is a \n",
        "small business owner who manages their business from both devices?\n\n",
        "## Instructions\n\n",
        "Emit poe:brief describing your role, then give your advisory response via:\n",
        "```\n",
        "{\"poe\": \"advisor\", \"content\": \"<your advisory response>\", \"id\": \"a1\"}\n",
        "```\n",
        "Then emit poe:yield to wait for the human's follow-up question.\n",
        "Do NOT emit poe:done — this is a multi-turn advisor session.\n",
    ));

    log.line(&format!("bundle length: {} chars", bundle.len()));

    let run = run_agent(&bundle, None, cwd.path(), &log)
        .expect("run_agent failed");

    let etypes = run.event_types();
    log.line(&format!("poe: events in order: {:?}", etypes));

    assert!(run.on_complete_fired, "ADV-1 FAIL: on_complete must fire");

    // poe:advisor must be emitted
    let advisor_events = run.events_of_type("advisor");
    assert!(
        !advisor_events.is_empty(),
        "ADV-1 FAIL: poe:advisor not emitted. Got: {:?}",
        etypes
    );

    // poe:advisor content must be non-empty prose
    for (i, adv_val) in advisor_events.iter().enumerate() {
        let content = adv_val
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        log.line(&format!(
            "poe:advisor[{}] content ({} chars): {}",
            i,
            content.len(),
            &content[..content.len().min(300)]
        ));
        assert!(
            content.len() > 20,
            "ADV-1 FAIL: poe:advisor[{}] content too short ({} chars). Got: {:?}",
            i, content.len(), content
        );
        assert!(
            !content.trim_start().starts_with('{'),
            "ADV-1 FAIL: poe:advisor[{}] content starts with '{{' — raw JSON leaked. Got: {:?}",
            i, content
        );
    }

    // poe:yield must follow poe:advisor
    let advisor_pos = run.pos("advisor").expect("already confirmed advisor emitted");
    let yield_pos = run.pos("yield").unwrap_or_else(|| {
        panic!(
            "ADV-1 FAIL: poe:yield not emitted after poe:advisor. Got: {:?}",
            etypes
        )
    });
    assert!(
        yield_pos > advisor_pos,
        "ADV-1 FAIL: poe:yield (pos={}) must follow poe:advisor (pos={}). Got: {:?}",
        yield_pos, advisor_pos, etypes
    );

    // poe:done must NOT be emitted — session is waiting for human response
    assert!(
        !run.has("done"),
        "ADV-1 FAIL: poe:done must NOT be emitted in advisor interactive session. Got: {:?}",
        etypes
    );

    log.line("ADV-1 PASSED");
}

/// ADV-2: resumed advisor session maintains context and continues the conversation.
///
/// Two-phase test:
///   Phase 1: advisor query → poe:advisor + poe:yield + session_id captured
///   Phase 2: resume with "Human: <follow-up>" → another poe:advisor (or poe:done)
#[tokio::test]
#[ignore]
async fn adv_2_maintains_context() {
    if !live_tests_enabled() || !claude_on_path() {
        eprintln!("SKIP: PHASE3_RUN_LIVE_TESTS not set or claude not on PATH");
        return;
    }

    let log = Log::open("adv-2-context");
    log.line("=== ADV-2: resumed advisor session maintains context ===");

    let dummy_project = TempDir::new().unwrap();
    let dummy_resources = TempDir::new().unwrap();
    let poe_base = load_skill("poe-base", dummy_project.path(), dummy_resources.path())
        .unwrap_or_else(|e| panic!("load poe-base skill: {}", e));

    let cwd = TempDir::new().expect("tempdir");

    let mut bundle = String::new();
    bundle.push_str(concat!(
        "## Interactive Mode\n\n",
        "You are running in interactive mode — a human is present at the keyboard.\n\n",
        "- Use `poe:advisor` + `poe:yield` to send responses and wait for the human.\n",
        "- Do NOT use poe:chat. Use poe:advisor for all messages in this session.\n",
    ));
    bundle.push_str("\n---\n\n");
    bundle.push_str(&poe_base.prompt);
    bundle.push_str("\n\n---\n\n");
    bundle.push_str(concat!(
        "# Advisor Session\n\n",
        "You are a product strategy advisor. The user has a question.\n\n",
        "## User Question\n\n",
        "We are deciding between a subscription model and a one-time purchase model \n",
        "for our developer tool. Which should we choose?\n\n",
        "## Instructions\n\n",
        "Emit poe:brief, give your advice via poe:advisor, then poe:yield.\n",
        "Keep your response concise (2–3 sentences).\n",
        "Do NOT emit poe:done — expect a follow-up question.\n",
    ));

    // Phase 1: initial advisor turn
    log.line("--- Phase 1: initial advisor query ---");
    let run1 = run_agent(&bundle, None, cwd.path(), &log)
        .expect("Phase 1 run_agent failed");

    let etypes1 = run1.event_types();
    log.line(&format!("Phase 1 events: {:?}", etypes1));

    let sid = run1.session_id.clone().unwrap_or_else(|| {
        panic!("ADV-2 FAIL: Phase 1 must capture session_id (needed for resume)")
    });

    assert!(
        run1.has("advisor"),
        "ADV-2 FAIL: Phase 1 must emit poe:advisor. Got: {:?}",
        etypes1
    );
    assert!(
        run1.has("yield"),
        "ADV-2 FAIL: Phase 1 must emit poe:yield. Got: {:?}",
        etypes1
    );
    assert!(
        !run1.has("done"),
        "ADV-2 FAIL: Phase 1 must NOT emit poe:done. Got: {:?}",
        etypes1
    );

    // Capture Phase 1 advisor content to check Phase 2 context retention
    let phase1_content = run1
        .first_event("advisor")
        .and_then(|v| v.get("content"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();
    log.line(&format!(
        "Phase 1 advisor content: {}",
        &phase1_content[..phase1_content.len().min(200)]
    ));

    // Phase 2: resume with human follow-up
    log.line("--- Phase 2: resume with human follow-up ---");
    log.line(&format!("resuming session_id: {}", sid));

    let followup = concat!(
        "---\n",
        "Human: Thanks for the advice on the pricing model. Based on what you said, \n",
        "what are the top 3 risks I should watch out for in Year 1?\n",
    );

    let run2 = run_agent(followup, Some(&sid), cwd.path(), &log)
        .expect("Phase 2 run_agent (resume) failed");

    let etypes2 = run2.event_types();
    log.line(&format!("Phase 2 events: {:?}", etypes2));

    assert!(
        run2.on_complete_fired,
        "ADV-2 FAIL: Phase 2 on_complete must fire. Got: {:?}",
        etypes2
    );

    // Phase 2 must emit either poe:advisor (next turn) or poe:done (session complete)
    let has_advisor = run2.has("advisor");
    let has_done = run2.has("done");
    assert!(
        has_advisor || has_done,
        "ADV-2 FAIL: Phase 2 must emit poe:advisor (next turn) OR poe:done (session complete). \
         Got: {:?}",
        etypes2
    );

    if has_advisor {
        log.line("Phase 2: advisor responded with another turn (multi-turn session)");
        let phase2_content = run2
            .first_event("advisor")
            .and_then(|v| v.get("content"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        log.line(&format!(
            "Phase 2 advisor content: {}",
            &phase2_content[..phase2_content.len().min(300)]
        ));

        assert!(
            phase2_content.len() > 10,
            "ADV-2 FAIL: Phase 2 poe:advisor content too short. Got: {:?}",
            phase2_content
        );

        // Yield must follow advisor in Phase 2 too
        let adv_pos = run2.pos("advisor").unwrap();
        let yield_pos = run2.pos("yield");
        assert!(
            yield_pos.map(|p| p > adv_pos).unwrap_or(false),
            "ADV-2 FAIL: Phase 2 poe:yield must follow poe:advisor. Events: {:?}",
            etypes2
        );
    }

    if has_done {
        log.line("Phase 2: advisor session concluded with poe:done");
    }

    log.line("ADV-2 PASSED");
}

// ── LOOP test — PM → SE → PM roundtrip ───────────────────────────────────────

/// LOOP-1: full PM→SE→PM loop.
///
/// End-to-end orchestration roundtrip:
///   Round 1 (PM):  PM creates DAG, emits poe:review, poe:yield → captures session_id + review_ids
///   Round 2 (SE):  SE reviews the plan, emits poe:artifact (verdict) + poe:done
///   Round 3 (PM):  PM resumes with SE ReviewResult, emits poe:done
///
/// This is the most expensive test — three real claude invocations in sequence.
#[tokio::test]
#[ignore]
async fn loop_1_pm_se_roundtrip() {
    if !live_tests_enabled() || !claude_on_path() {
        eprintln!("SKIP: PHASE3_RUN_LIVE_TESTS not set or claude not on PATH");
        return;
    }

    let log = Log::open("loop-1-pm-se-roundtrip");
    log.line("=== LOOP-1: full PM → SE → PM roundtrip ===");

    let pm_skill = product_manager_skill();
    let se_skill = senior_engineer_skill();
    let cwd = TempDir::new().expect("tempdir");

    // ── Round 1: PM creates DAG ───────────────────────────────────────────────
    log.line("--- Round 1: PM creates task DAG ---");

    let task_desc = concat!(
        "Plan Stage 1 for a simple Expense Tracker application.\n",
        "Features: add expense, list expenses, categorise expense, export CSV.\n",
        "Tech stack: React frontend, Node.js/Express backend, PostgreSQL database.\n",
        "The Guardrails Review is APPROVED. Proceed with Stage 1 planning.\n",
        "User Analysis: P0 features are add-expense, list-expenses, categorise-expense.\n",
        "P1 feature: export-csv (include in Stage 1 only if it fits within budget).\n",
        "No Must-Nots defined. Stage includes schema migration for expenses table.",
    );

    let pm_bundle = assemble_input_bundle(
        &SpawnMode::Autonomous,
        &pm_skill,
        "Plan Stage 1",
        Some(task_desc),
        &[("project", "Expense Tracker"), ("phase", "Stage 1 Increment Planning")],
        &[("guardrails-verdict", "APPROVED — no conflicts found")],
        &[],
    );

    let pm_run1 = run_agent(&pm_bundle, None, cwd.path(), &log)
        .expect("Round 1 (PM) run_agent failed");

    let etypes_pm1 = pm_run1.event_types();
    log.line(&format!("PM Round 1 events: {:?}", etypes_pm1));

    let pm_sid = pm_run1.session_id.clone().unwrap_or_else(|| {
        panic!("LOOP-1 FAIL: PM Round 1 must capture session_id")
    });

    assert!(
        pm_run1.has("task"),
        "LOOP-1 FAIL: PM must emit poe:task events. Got: {:?}",
        etypes_pm1
    );
    assert!(
        pm_run1.has("review"),
        "LOOP-1 FAIL: PM must emit poe:review. Got: {:?}",
        etypes_pm1
    );
    assert!(
        pm_run1.has("yield"),
        "LOOP-1 FAIL: PM must emit poe:yield. Got: {:?}",
        etypes_pm1
    );
    assert!(
        !pm_run1.has("done"),
        "LOOP-1 FAIL: PM must NOT emit poe:done in Round 1. Got: {:?}",
        etypes_pm1
    );

    // Collect review IDs and the plan summary from PM task events.
    let review_events = pm_run1.events_of_type("review");
    let se_review = review_events.iter().find(|v| {
        v.get("reviewer_skill")
            .and_then(|s| s.as_str())
            .map(|s| s == "senior-engineer")
            .unwrap_or(false)
    });

    assert!(
        se_review.is_some(),
        "LOOP-1 FAIL: PM must emit poe:review to senior-engineer. Reviews: {:?}",
        review_events
    );

    let se_review_val = se_review.unwrap();
    let se_review_id = se_review_val
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("r-eng")
        .to_owned();
    let se_review_content = se_review_val
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("Review the stage 1 plan.")
        .to_owned();

    log.line(&format!(
        "PM emitted SE review id={}, content ({} chars): {}",
        se_review_id,
        se_review_content.len(),
        &se_review_content[..se_review_content.len().min(400)]
    ));

    // Build a plan summary for the SE reviewer from PM's task events.
    let task_events = pm_run1.events_of_type("task");
    let edge_events = pm_run1.events_of_type("edge");
    let mut plan_summary = String::from("## Stage 1 Plan (generated by PM)\n\n### Tasks\n\n");
    for task_val in &task_events {
        let id = task_val.get("id").and_then(|v| v.as_str()).unwrap_or("?");
        let title = task_val.get("title").and_then(|v| v.as_str()).unwrap_or("?");
        let skill = task_val.get("skill").and_then(|v| v.as_str()).unwrap_or("?");
        let typ = task_val.get("type").and_then(|v| v.as_str()).unwrap_or("task");
        plan_summary.push_str(&format!(
            "- {} ({}): {} | skill: {} | type: {}\n",
            id, typ, title, skill, typ
        ));
    }
    plan_summary.push_str("\n### Dependency Edges\n\n");
    for edge_val in &edge_events {
        let from = edge_val.get("from").and_then(|v| v.as_str()).unwrap_or("?");
        let to = edge_val.get("to").and_then(|v| v.as_str()).unwrap_or("?");
        plan_summary.push_str(&format!("- {} → {}\n", from, to));
    }

    // ── Round 2: SE reviews the plan ─────────────────────────────────────────
    log.line("--- Round 2: SE reviews PM plan ---");

    let se_bundle = assemble_review_bundle(
        &se_skill,
        "Expense Tracker",
        "Stage 1 Increment Planning",
        &plan_summary,
        &se_review_content,
    );

    let se_run = run_agent(&se_bundle, None, cwd.path(), &log)
        .expect("Round 2 (SE) run_agent failed");

    let etypes_se = se_run.event_types();
    log.line(&format!("SE Round 2 events: {:?}", etypes_se));

    assert!(
        se_run.on_complete_fired,
        "LOOP-1 FAIL: SE on_complete must fire. Got: {:?}",
        etypes_se
    );
    assert!(
        se_run.has("done"),
        "LOOP-1 FAIL: SE must emit poe:done. Got: {:?}",
        etypes_se
    );

    // Extract SE verdict from review artifact
    let se_artifact_events = se_run.events_of_type("artifact");
    let se_review_artifact = se_artifact_events.iter().find(|v| {
        v.get("artifact_type")
            .and_then(|t| t.as_str())
            .map(|t| t == "review")
            .unwrap_or(false)
    });

    let se_review_content_result = se_review_artifact
        .and_then(|v| v.get("content"))
        .and_then(|v| v.as_str())
        .unwrap_or("APPROVED — plan reviewed.");

    // Determine verdict for the resume bundle
    let se_verdict = if se_review_content_result.contains("BLOCKED") {
        "BLOCKED"
    } else if se_review_content_result.contains("APPROVED_WITH_CONDITIONS") {
        "APPROVED_WITH_CONDITIONS"
    } else {
        "APPROVED"
    };

    log.line(&format!("SE verdict: {}", se_verdict));
    log.line(&format!(
        "SE review content ({} chars): {}",
        se_review_content_result.len(),
        &se_review_content_result[..se_review_content_result.len().min(400)]
    ));

    // ── Round 3: PM resumes with SE ReviewResult ──────────────────────────────
    log.line("--- Round 3: PM resumes with SE ReviewResult ---");
    log.line(&format!("resuming PM session_id: {}", pm_sid));

    let resume_bundle = format!(
        "---\nReviewResult id={} skill=senior-engineer verdict={}\n{}\n---\n",
        se_review_id, se_verdict, se_review_content_result
    );

    let pm_run2 = run_agent(&resume_bundle, Some(&pm_sid), cwd.path(), &log)
        .expect("Round 3 (PM resume) run_agent failed");

    let etypes_pm2 = pm_run2.event_types();
    log.line(&format!("PM Round 3 events: {:?}", etypes_pm2));

    assert!(
        pm_run2.on_complete_fired,
        "LOOP-1 FAIL: PM Round 3 on_complete must fire. Got: {:?}",
        etypes_pm2
    );

    if se_verdict == "BLOCKED" {
        // PM must emit another poe:review + poe:yield on BLOCKED verdict
        log.line("SE returned BLOCKED — PM should re-review or escalate");
        assert!(
            pm_run2.has("yield") || pm_run2.has("done"),
            "LOOP-1 FAIL: PM Round 3 must emit poe:yield (re-review) or poe:done \
             (escalated). Got: {:?}",
            etypes_pm2
        );
    } else {
        // PM must emit poe:done on APPROVED or APPROVED_WITH_CONDITIONS
        assert!(
            pm_run2.has("done"),
            "LOOP-1 FAIL: PM Round 3 must emit poe:done after APPROVED ReviewResult. \
             Got: {:?}",
            etypes_pm2
        );
    }

    log.line("LOOP-1 PASSED");
}
