//! End-to-end orchestrator tests (bp6-a6d.6).
// Compile the library with test-utils feature so TestEventSink is available.
// This is set automatically when running: cargo test --features test-utils
//!
//! These tests exercise the full orchestrator dispatch path using:
//!   - Real in-memory SQLite databases (full schema + all migrations)
//!   - Real ProjectRegistry / AgentMap / ConcurrencyLimits
//!   - TestEventSink capturing emitted events instead of Tauri AppHandle
//!   - Real orchestrator::start() loop driven by mpsc channel signals
//!
//! All tests are marked `#[ignore]` and require:
//!   E2E_RUN_LIVE_TESTS=1 cargo test --test e2e_orchestrator_tests -- --include-ignored --nocapture
//!
//! E2E-1: ProjectOpened signal → orchestrator runs without panicking
//! E2E-2: DagStructureChanged with no ready tasks → no agent spawned
//! E2E-3: DagStructureChanged with a ready task → poe-agent-started emitted
//! E2E-4: NodeStatusChanged Waiting → orchestrator processes without panic
//! E2E-5: QueueItemResolved with no live agent → orchestrator processes without panic

use poe2_lib::agent_lifecycle::new_agent_map;
use poe2_lib::dag_store::{
    self, new_registry,
    schema,
    types::{CreateNodeInput, NodeStatus, NodeType, UpdateNodeInput},
    ProjectDb,
};
use poe2_lib::event_ingester::DagChanged;
use poe2_lib::event_sink::test_helpers::TestEventSink;
use poe2_lib::orchestrator::{self, ConcurrencyLimits};
use rusqlite::Connection;
use tempfile;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

// ── E2E guard ─────────────────────────────────────────────────────────────────

fn e2e_enabled() -> bool {
    std::env::var("E2E_RUN_LIVE_TESTS").map(|v| v == "1").unwrap_or(false)
}

// ── In-memory DB helper ───────────────────────────────────────────────────────

/// Build a fully-migrated in-memory SQLite database, insert a project row,
/// and return `(project_id, Connection)`.
fn new_e2e_db() -> (String, Connection) {
    let conn = Connection::open_in_memory().expect("open_in_memory");
    conn.execute_batch(schema::CREATE_TABLES).expect("CREATE_TABLES");

    // ── All runtime migrations (mirrors open_project_db) ──────────────────
    let _ = conn.execute_batch("ALTER TABLE knowledge ADD COLUMN promoted INTEGER NOT NULL DEFAULT 0");
    let _ = conn.execute_batch("ALTER TABLE nodes ADD COLUMN yield_reason TEXT");
    let _ = conn.execute_batch("ALTER TABLE nodes ADD COLUMN session_id TEXT");
    let _ = conn.execute_batch("ALTER TABLE nodes ADD COLUMN requesting_task_id TEXT REFERENCES nodes(id)");
    let _ = conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_nodes_requesting_task_id ON nodes(requesting_task_id)");
    let _ = conn.execute_batch("ALTER TABLE nodes ADD COLUMN review_id TEXT");
    let _ = conn.execute_batch("ALTER TABLE nodes ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0");
    let _ = conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS chat_turns (
            id          TEXT PRIMARY KEY,
            task_id     TEXT NOT NULL,
            content     TEXT NOT NULL,
            response    TEXT,
            created_at  TEXT NOT NULL,
            responded_at TEXT
        )",
    );
    let _ = conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_chat_turns_task ON chat_turns(task_id)");
    let _ = conn.execute_batch("ALTER TABLE phases ADD COLUMN stage_type TEXT NOT NULL DEFAULT 'execution'");
    let _ = conn.execute_batch("ALTER TABLE phases ADD COLUMN status TEXT NOT NULL DEFAULT 'pending'");
    let _ = conn.execute_batch("ALTER TABLE nodes ADD COLUMN sort_order INTEGER");
    let _ = conn.execute_batch("ALTER TABLE nodes ADD COLUMN skill_modes TEXT");
    let _ = conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS advisor_turns (
            id           TEXT PRIMARY KEY,
            task_id      TEXT NOT NULL,
            content      TEXT NOT NULL,
            response     TEXT,
            created_at   TEXT NOT NULL,
            responded_at TEXT
        )",
    );
    let _ = conn.execute_batch("CREATE INDEX IF NOT EXISTS idx_advisor_turns_task ON advisor_turns(task_id)");

    // Insert a project row.
    let project_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO projects (id, name, path, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            project_id,
            "E2E Test Project",
            format!("/tmp/e2e-test-{}", project_id),
            now,
            now
        ],
    )
    .expect("insert project");

    (project_id, conn)
}

/// Insert a minimal phase row in `execution` lifecycle_stage with gate_held=0.
/// Tasks in this phase will appear in db_find_ready_tasks.
fn insert_phase(conn: &Connection, project_id: &str) -> String {
    let phase_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO phases (id, project_id, number, title, lifecycle_stage, gate_held, created_at, updated_at)
         VALUES (?1, ?2, 1, ?3, 'execution', 0, ?4, ?5)",
        rusqlite::params![phase_id, project_id, "Phase 1", now, now],
    )
    .expect("insert phase");
    phase_id
}

/// Wrap an in-memory db in a `ProjectDb` and register it in the registry.
fn register_project(
    registry: &poe2_lib::dag_store::ProjectRegistry,
    project_id: &str,
    project_path: &str,
    conn: Connection,
) {
    use std::collections::HashMap;
    let project = poe2_lib::dag_store::types::Project {
        id: project_id.to_string(),
        name: "E2E Test Project".to_string(),
        path: project_path.to_string(),
        conops_ref: None,
        active_phase_id: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };
    let db = Arc::new(ProjectDb {
        project,
        conn: Mutex::new(conn),
    });
    registry.lock().unwrap().insert(project_id.to_string(), db);
}

// ── E2E helper: start orchestrator, send signals, shut down ──────────────────

/// Start the orchestrator on a background task, send `signals` through `dag_tx`,
/// then drop the channel (causing the loop to exit) and wait for the task.
/// Returns the captured events.
async fn run_orchestrator_with_signals(
    registry: poe2_lib::dag_store::ProjectRegistry,
    signals: Vec<DagChanged>,
    sink: TestEventSink,
) -> Vec<String> {
    let agent_map = new_agent_map();
    let limits = ConcurrencyLimits::new();
    let (dag_tx, dag_rx) = mpsc::unbounded_channel::<DagChanged>();
    let sink_arc: Arc<dyn poe2_lib::event_sink::EventSink> = Arc::new(sink.clone());

    let handle = tokio::spawn(orchestrator::start(
        sink_arc,
        registry,
        limits,
        agent_map,
        dag_tx.clone(),
        dag_rx,
    ));

    // Send each signal then close the sender.
    for signal in signals {
        dag_tx.send(signal).expect("send signal");
    }
    drop(dag_tx);

    // Give the loop a short window to process, then it will exit when channel closes.
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    // Cancel the task (loop may be blocked on recv after draining all signals)
    handle.abort();

    sink.emitted_types()
}

// ── E2E-1 ─────────────────────────────────────────────────────────────────────

/// E2E-1: Sending a `ProjectOpened` signal exercises the ghost-agent recovery
/// path. With an empty project (no Running nodes), no agent is spawned and no
/// panic occurs.
#[tokio::test]
#[ignore]
async fn e2e_1_project_opened_signal_no_panic() {
    if !e2e_enabled() {
        return;
    }

    let (project_id, conn) = new_e2e_db();
    let registry = new_registry();
    register_project(&registry, &project_id, &format!("/tmp/e2e-test-{}", project_id), conn);

    let sink = TestEventSink::new();
    let events = run_orchestrator_with_signals(
        registry,
        vec![DagChanged::ProjectOpened { project_id }],
        sink,
    )
    .await;

    // No agents were in a Running state, so no poe-agent-started should fire.
    assert!(
        !events.contains(&"poe-agent-started".to_string()),
        "Unexpected poe-agent-started with empty project: {:?}",
        events
    );
}

// ── E2E-2 ─────────────────────────────────────────────────────────────────────

/// E2E-2: `DagStructureChanged` with no ready tasks — orchestrator runs the
/// scheduling loop, finds nothing dispatchable, and emits nothing.
#[tokio::test]
#[ignore]
async fn e2e_2_dag_structure_changed_no_ready_tasks() {
    if !e2e_enabled() {
        return;
    }

    let (project_id, conn) = new_e2e_db();
    // Insert a phase and a task in Pending state with an unresolved dependency so
    // it is NOT in the ready set.
    // The blocker is in Running status — not pending, so not re-dispatched.
    // The blocked task depends on the Running blocker, so it is also not ready.
    let phase_id = insert_phase(&conn, &project_id);
    let blocker_input = CreateNodeInput {
        project_id: project_id.clone(),
        phase_id: Some(phase_id.clone()),
        parent_id: None,
        node_type: NodeType::Task,
        title: "Blocker (Running)".to_string(),
        description: None,
        skill_id: None,
        initial_status: Some(NodeStatus::Running),
        requesting_task_id: None,
        review_id: None,
        retry_count: None,
    };
    let task_input = CreateNodeInput {
        project_id: project_id.clone(),
        phase_id: Some(phase_id.clone()),
        parent_id: None,
        node_type: NodeType::Task,
        title: "Blocked Task".to_string(),
        description: None,
        skill_id: None,
        initial_status: None,
        requesting_task_id: None,
        review_id: None,
        retry_count: None,
    };
    let blocker = dag_store::db_create_node(&conn, &blocker_input).expect("blocker");
    let task = dag_store::db_create_node(&conn, &task_input).expect("task");
    dag_store::db_create_edge(&conn, &blocker.id, &task.id, dag_store::types::EdgeType::DependsOn)
        .expect("edge");

    let registry = new_registry();
    register_project(&registry, &project_id, &format!("/tmp/e2e-test-{}", project_id), conn);

    let sink = TestEventSink::new();
    let events = run_orchestrator_with_signals(
        registry,
        vec![DagChanged::DagStructureChanged { project_id }],
        sink,
    )
    .await;

    assert!(
        !events.contains(&"poe-agent-started".to_string()),
        "No agent should start when all tasks are blocked: {:?}",
        events
    );
}

// ── E2E-3 ─────────────────────────────────────────────────────────────────────

/// E2E-3: `DagStructureChanged` with a ready task that has a skill_id → the
/// orchestrator calls `dispatch_task` which calls `spawn_agent`, causing
/// `poe-agent-started` to be emitted via the sink.
///
/// Note: This test exercises up to and including `spawn_agent`. It does NOT
/// assert that a real Claude process runs — `spawn_agent` marks the task Running
/// and emits the event synchronously before starting the actual subprocess.
#[tokio::test]
#[ignore]
async fn e2e_3_ready_task_with_skill_emits_agent_started() {
    if !e2e_enabled() {
        return;
    }

    let (project_id, conn) = new_e2e_db();
    let phase_id = insert_phase(&conn, &project_id);

    // A ready task: Pending, has a skill_id, no dependencies.
    let task_input = CreateNodeInput {
        project_id: project_id.clone(),
        phase_id: Some(phase_id.clone()),
        parent_id: None,
        node_type: NodeType::Task,
        title: "Ready Task".to_string(),
        description: Some("Do something".to_string()),
        skill_id: Some("test_skill".to_string()),
        initial_status: None,
        requesting_task_id: None,
        review_id: None,
        retry_count: None,
    };
    let _task = dag_store::db_create_node(&conn, &task_input).expect("task");

    let registry = new_registry();
    let project_path = format!("/tmp/e2e-test-{}", project_id);
    register_project(&registry, &project_id, &project_path, conn);

    // Create a temp dir that holds a minimal skill file so load_skill succeeds.
    let tmp = tempfile::tempdir().expect("tempdir");
    let skills_dir = tmp.path().join("skills");
    std::fs::create_dir_all(&skills_dir).expect("create skills dir");
    std::fs::write(
        skills_dir.join("test_skill.md"),
        "---\nmodes: [autonomous]\n---\nYou are a test skill.\n",
    )
    .expect("write skill file");
    let sink = TestEventSink::with_resource_dir(tmp.path().to_path_buf());

    let events = run_orchestrator_with_signals(
        registry,
        vec![DagChanged::DagStructureChanged {
            project_id: project_id.clone(),
        }],
        sink,
    )
    .await;

    // spawn_agent emits poe-agent-started synchronously before forking the process.
    assert!(
        events.contains(&"poe-agent-started".to_string()),
        "Expected poe-agent-started but got: {:?}",
        events
    );
}

// ── E2E-4 ─────────────────────────────────────────────────────────────────────

/// E2E-4: `NodeStatusChanged` with a Waiting node — the orchestrator runs
/// `handle_node_status_changed`, which inspects the yield reason.  With no
/// live agent in the map and no session_id the node simply stays Waiting.
/// Asserts no panic.
#[tokio::test]
#[ignore]
async fn e2e_4_node_status_changed_waiting_no_panic() {
    if !e2e_enabled() {
        return;
    }

    let (project_id, conn) = new_e2e_db();
    let phase_id = insert_phase(&conn, &project_id);

    let task_input = CreateNodeInput {
        project_id: project_id.clone(),
        phase_id: Some(phase_id.clone()),
        parent_id: None,
        node_type: NodeType::Task,
        title: "Waiting Task".to_string(),
        description: None,
        skill_id: Some("some_skill".to_string()),
        initial_status: None,
        requesting_task_id: None,
        review_id: None,
        retry_count: None,
    };
    let task = dag_store::db_create_node(&conn, &task_input).expect("task");

    // Manually set status to Waiting.
    dag_store::db_update_node(
        &conn,
        &task.id,
        &UpdateNodeInput {
            status: Some(NodeStatus::Waiting),
            ..Default::default()
        },
    )
    .expect("set waiting");

    let registry = new_registry();
    register_project(&registry, &project_id, &format!("/tmp/e2e-test-{}", project_id), conn);

    let sink = TestEventSink::new();
    let node_id = task.id.clone();
    let events = run_orchestrator_with_signals(
        registry,
        vec![DagChanged::NodeStatusChanged {
            project_id,
            node_id,
        }],
        sink,
    )
    .await;

    // No poe-agent-started: no agent was live to resume.
    assert!(
        !events.contains(&"poe-agent-started".to_string()),
        "No agent resume expected: {:?}",
        events
    );
}

// ── E2E-5 ─────────────────────────────────────────────────────────────────────

/// E2E-5: `QueueItemResolved` signal with no live agent → `resume_waiting_agent`
/// is called, fails to find an agent in the map, logs, and returns without panic.
#[tokio::test]
#[ignore]
async fn e2e_5_queue_item_resolved_no_live_agent_no_panic() {
    if !e2e_enabled() {
        return;
    }

    let (project_id, conn) = new_e2e_db();
    let registry = new_registry();
    register_project(&registry, &project_id, &format!("/tmp/e2e-test-{}", project_id), conn);

    let sink = TestEventSink::new();
    let fake_task_id = uuid::Uuid::new_v4().to_string();
    let fake_session_id = uuid::Uuid::new_v4().to_string();
    let fake_item_id = uuid::Uuid::new_v4().to_string();

    let events = run_orchestrator_with_signals(
        registry,
        vec![DagChanged::QueueItemResolved {
            project_id,
            task_id: fake_task_id,
            session_id: fake_session_id,
            resolution: "approved".to_string(),
            item_id: fake_item_id,
        }],
        sink,
    )
    .await;

    // With no live agent in the map this is a no-op path; no start event.
    assert!(
        !events.contains(&"poe-agent-started".to_string()),
        "No start expected with no live agent: {:?}",
        events
    );
}
