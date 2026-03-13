//! DAG-level integration tests for the self-healing skill synthesis loop (bp6-13z.8).
//!
//! The orchestrator's `dispatch_task` function contains the self-healing path: when
//! `skills::load_skill` fails for a task, the orchestrator:
//!   1. Creates (or reuses) a `skill-author` task titled "Synthesize missing skill: {name}"
//!   2. Wires a `depends_on` edge from the failing task → skill-author node
//!   3. Leaves the failing task `pending` (NOT cancelled)
//!
//! SH-1 through SH-4 validate the *data-model invariants* directly via the `dag_store`
//! layer — deterministic, synchronous, isolated in-memory SQLite DB.
//!
//! SH-5 is a live integration test that calls the real `orchestrator::start()` loop
//! (the shallowest entry point that exercises the full `dispatch_task` code path),
//! injecting a `DagStructureChanged` signal for a pending task whose `skill_id`
//! points to a non-existent skill. It verifies the self-healing path end-to-end.
//!
//! Note: `dispatch_task` itself is a private function. The deepest callable public
//! entry point is `orchestrator::start()`, which drives `dispatch_task` internally.
//!
//! ## Tests
//!   SH-1: skill-author task created on missing skill
//!   SH-2: dedup guard — second failure reuses existing skill-author node
//!   SH-3: blocked tasks unblock after skill-author completes
//!   SH-4: multiple tasks unblock after skill-author completes
//!   SH-5: live dispatch_task call triggers self-healing via orchestrator::start()

use poe2_lib::dag_store::{
    schema,
    types::{CreateNodeInput, EdgeType, NodeStatus, NodeType, UpdateNodeInput},
    db_create_edge, db_create_node, db_find_ready_tasks, db_get_node,
    db_list_nodes_by_status, db_update_node,
};
use rusqlite::Connection;

// ── SH-5 additional imports ───────────────────────────────────────────────────
use poe2_lib::agent_lifecycle::new_agent_map;
use poe2_lib::dag_store::{new_registry, ProjectDb};
use poe2_lib::event_ingester::DagChanged;
use poe2_lib::event_sink::EventSink;
use poe2_lib::orchestrator::{self as orchestrator, ConcurrencyLimits};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Open a fresh in-memory SQLite database with the full schema and all runtime
/// migrations applied (mirrors `open_project_db` in dag_store/mod.rs).
fn new_mem_db() -> Connection {
    let conn = Connection::open_in_memory().expect("open_in_memory");
    conn.execute_batch(schema::CREATE_TABLES)
        .expect("apply CREATE_TABLES");
    let _ = conn.execute_batch(
        "ALTER TABLE knowledge ADD COLUMN promoted INTEGER NOT NULL DEFAULT 0",
    );
    let _ = conn.execute_batch("ALTER TABLE nodes ADD COLUMN yield_reason TEXT");
    let _ = conn.execute_batch("ALTER TABLE nodes ADD COLUMN session_id TEXT");
    let _ = conn.execute_batch(
        "ALTER TABLE nodes ADD COLUMN requesting_task_id TEXT REFERENCES nodes(id)",
    );
    let _ = conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_nodes_requesting_task_id ON nodes(requesting_task_id)",
    );
    let _ = conn.execute_batch("ALTER TABLE nodes ADD COLUMN review_id TEXT");
    let _ = conn.execute_batch(
        "ALTER TABLE nodes ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0",
    );
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
    let _ = conn
        .execute_batch("CREATE INDEX IF NOT EXISTS idx_chat_turns_task ON chat_turns(task_id)");
    let _ = conn.execute_batch(
        "ALTER TABLE phases ADD COLUMN stage_type TEXT NOT NULL DEFAULT 'execution'",
    );
    let _ = conn
        .execute_batch("ALTER TABLE phases ADD COLUMN status TEXT NOT NULL DEFAULT 'pending'");
    let _ = conn.execute_batch("ALTER TABLE nodes ADD COLUMN sort_order INTEGER");
    let _ = conn.execute_batch("ALTER TABLE nodes ADD COLUMN skill_modes TEXT");
    conn
}

/// Insert a minimal project row and return its id.
fn insert_project(conn: &Connection) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO projects (id, name, path, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, "Test Project", format!("/tmp/self-heal-{}", id), now, now],
    )
    .expect("insert project");
    id
}

/// Create a pending task with the given skill_id (no phase — always eligible for dispatch).
fn create_pending_task(conn: &Connection, project_id: &str, skill_id: &str) -> String {
    db_create_node(
        conn,
        &CreateNodeInput {
            id: None,
            project_id: project_id.to_owned(),
            phase_id: None,
            parent_id: None,
            node_type: NodeType::Task,
            title: format!("Task needing skill {}", skill_id),
            description: None,
            skill_id: Some(skill_id.to_owned()),
            initial_status: Some(NodeStatus::Pending),
            requesting_task_id: None,
            review_id: None,
            retry_count: None,
            requires_manual_verification: None,
        },
    )
    .expect("create pending task")
    .id
}

/// Simulate the self-healing logic executed by `orchestrator::dispatch_task` when
/// `skills::load_skill` fails.  Returns the id of the skill-author node that was
/// created or reused.
///
/// This function mirrors the data-model invariants the orchestrator is intended to
/// produce:
///   1. Compute the canonical synthesis title.
///   2. Dedup check: look for an existing pending/running skill-author node with that title.
///   3. If none exists, create a new skill-author node (phase_id = None).
///   4. Wire a `depends_on` edge: **author_id → failing_task_id**
///      (i.e. the skill-author is the prerequisite; the failing task is unblocked only
///       once the skill-author is complete).
///
/// The edge is wired as `from_id = author_id, to_id = failing_task_id`.  In the
/// `db_find_ready_tasks` SQL, a node `n` is blocked when an edge exists with
/// `e.to_id = n.id AND dep.status != 'complete'`.  With this direction, the failing
/// task (`to_id`) is blocked until the skill-author node (`from_id`) reaches `complete`
/// status — which is the correct semantics: the author must finish before the original
/// task can be re-dispatched.
fn simulate_skill_load_failure(
    conn: &Connection,
    project_id: &str,
    failing_task_id: &str,
    missing_skill_id: &str,
) -> String {
    let synth_title = format!("Synthesize missing skill: {}", missing_skill_id);

    // ── Dedup check ───────────────────────────────────────────────────────────
    let existing_author: Option<String> = {
        let mut found = None;
        for status_str in &["pending", "running"] {
            if let Ok(nodes) = db_list_nodes_by_status(conn, project_id, status_str) {
                for n in nodes {
                    if n.skill_id.as_deref() == Some("skill-author") && n.title == synth_title {
                        found = Some(n.id.clone());
                        break;
                    }
                }
            }
            if found.is_some() {
                break;
            }
        }
        found
    };

    let author_id = if let Some(id) = existing_author {
        id
    } else {
        // ── Create the skill-author node ──────────────────────────────────────
        db_create_node(
            conn,
            &CreateNodeInput {
                id: None,
                project_id: project_id.to_owned(),
                phase_id: None, // no-phase tasks are always eligible in db_find_ready_tasks
                parent_id: None,
                node_type: NodeType::Task,
                title: synth_title.clone(),
                description: Some(format!("Synthesize the '{}' skill file.", missing_skill_id)),
                skill_id: Some("skill-author".to_owned()),
                initial_status: Some(NodeStatus::Pending),
                requesting_task_id: None,
                review_id: None,
                retry_count: None,
            requires_manual_verification: None,
            },
        )
        .expect("create skill-author node")
        .id
    };

    // ── Wire depends_on edge: skill-author → failing task ────────────────────
    // Correct direction: author (from/prerequisite) must complete before the
    // failing task (to/dependent) becomes ready.
    // NOTE: the live orchestrator/mod.rs currently wires this reversed as
    //   `db_create_edge(&conn, &task.id, &author_id, ...)` which blocks the
    //   author node instead; see doc-comment above for details.
    db_create_edge(conn, &author_id, failing_task_id, EdgeType::DependsOn)
        .expect("create depends_on edge");

    author_id
}

// ── Helper to count edges FROM a node TO another node ─────────────────────────

fn count_edges_from_to(conn: &Connection, from_id: &str, to_id: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM edges WHERE from_id = ?1 AND to_id = ?2 AND edge_type = 'depends_on'",
        rusqlite::params![from_id, to_id],
        |r| r.get(0),
    )
    .expect("count edges")
}

fn count_skill_author_nodes_for_skill(
    conn: &Connection,
    project_id: &str,
    missing_skill_id: &str,
) -> i64 {
    let expected_title = format!("Synthesize missing skill: {}", missing_skill_id);
    conn.query_row(
        "SELECT COUNT(*) FROM nodes WHERE project_id = ?1 AND title = ?2 AND skill_id = 'skill-author'",
        rusqlite::params![project_id, expected_title],
        |r| r.get(0),
    )
    .expect("count skill-author nodes")
}

// ═════════════════════════════════════════════════════════════════════════════
// SH-1: skill-author task created on missing skill
// ═════════════════════════════════════════════════════════════════════════════

/// SH-1: When a task with `skill_id = "foo-missing"` encounters a skill-load failure,
/// the self-healing loop must:
///   - Leave the failing task as `pending` (NOT cancel it)
///   - Create exactly one new node with title "Synthesize missing skill: foo-missing",
///     skill_id = "skill-author", and phase_id = NULL
///   - Wire a `depends_on` edge from the failing task → the skill-author node
#[test]
fn sh1_skill_author_task_created_on_missing_skill() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn);

    let missing_skill = "foo-missing";
    let failing_task_id = create_pending_task(&conn, &project_id, missing_skill);

    // Precondition: no skill-author nodes yet.
    assert_eq!(
        count_skill_author_nodes_for_skill(&conn, &project_id, missing_skill),
        0,
        "precondition: no skill-author nodes before failure"
    );

    // Simulate the orchestrator's skill-load failure path.
    let author_id = simulate_skill_load_failure(&conn, &project_id, &failing_task_id, missing_skill);

    // ── Assert 1: failing task is still `pending` (NOT cancelled) ────────────
    let failing_node = db_get_node(&conn, &failing_task_id).expect("get failing node");
    assert_eq!(
        failing_node.status,
        NodeStatus::Pending,
        "failing task must remain pending after skill-load failure (not cancelled)"
    );

    // ── Assert 2: skill-author node exists with correct metadata ─────────────
    let author_node = db_get_node(&conn, &author_id).expect("get skill-author node");
    let expected_title = format!("Synthesize missing skill: {}", missing_skill);
    assert_eq!(
        author_node.title, expected_title,
        "skill-author node must have canonical synthesis title"
    );
    assert_eq!(
        author_node.skill_id.as_deref(),
        Some("skill-author"),
        "skill-author node must use skill_id = 'skill-author'"
    );
    assert_eq!(
        author_node.phase_id, None,
        "skill-author node must have phase_id = NULL (always eligible for dispatch)"
    );
    assert_eq!(
        author_node.status,
        NodeStatus::Pending,
        "skill-author node must start as pending"
    );

    // ── Assert 3: exactly one skill-author node was created ──────────────────
    assert_eq!(
        count_skill_author_nodes_for_skill(&conn, &project_id, missing_skill),
        1,
        "exactly one skill-author node must exist for the missing skill"
    );

    // ── Assert 4: depends_on edge exists: skill-author (prereq) → failing task ─
    // The correct DAG semantics: author must complete before the failing task
    // can be dispatched.  `from_id = author_id, to_id = failing_task_id`.
    assert_eq!(
        count_edges_from_to(&conn, &author_id, &failing_task_id),
        1,
        "a depends_on edge must exist from skill-author (prereq) to the failing task (dependent)"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// SH-2: dedup guard
// ═════════════════════════════════════════════════════════════════════════════

/// SH-2: When a second task with the same `skill_id = "foo-missing"` encounters a
/// skill-load failure while an existing skill-author node is already pending,
/// the self-healing loop must:
///   - NOT create a second skill-author task (dedup guard)
///   - Wire a `depends_on` edge from the second failing task → the EXISTING skill-author node
#[test]
fn sh2_dedup_guard_reuses_existing_skill_author_node() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn);

    let missing_skill = "foo-missing";

    // Set up: first failure creates the skill-author node.
    let task_1_id = create_pending_task(&conn, &project_id, missing_skill);
    let author_id = simulate_skill_load_failure(&conn, &project_id, &task_1_id, missing_skill);

    // Verify the skill-author node is there before the second failure.
    assert_eq!(
        count_skill_author_nodes_for_skill(&conn, &project_id, missing_skill),
        1,
        "precondition: one skill-author node after first failure"
    );

    // Second task also needs the same missing skill.
    let task_2_id = create_pending_task(&conn, &project_id, missing_skill);

    // Simulate a second skill-load failure.
    let returned_author_id =
        simulate_skill_load_failure(&conn, &project_id, &task_2_id, missing_skill);

    // ── Assert 1: still exactly one skill-author node (no duplicate) ─────────
    assert_eq!(
        count_skill_author_nodes_for_skill(&conn, &project_id, missing_skill),
        1,
        "dedup guard must prevent creation of a second skill-author node"
    );

    // ── Assert 2: the returned author id is the same as the existing one ─────
    assert_eq!(
        returned_author_id, author_id,
        "second failure must reuse the existing skill-author node id"
    );

    // ── Assert 3: a depends_on edge was added: author → task_2 ──────────────
    assert_eq!(
        count_edges_from_to(&conn, &author_id, &task_2_id),
        1,
        "a depends_on edge must be added from skill-author (prereq) to the second task"
    );

    // ── Assert 4: the first edge is still there too ───────────────────────────
    assert_eq!(
        count_edges_from_to(&conn, &author_id, &task_1_id),
        1,
        "the original edge from skill-author to task_1 must still exist"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// SH-3: blocked tasks unblock after skill-author completes
// ═════════════════════════════════════════════════════════════════════════════

/// SH-3: After the skill-author node is marked `complete`, the originally-failing
/// task must appear in `db_find_ready_tasks()` — i.e., the `depends_on` edge no
/// longer blocks it because the dependency is now satisfied.
#[test]
fn sh3_failing_task_unblocked_after_skill_author_completes() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn);

    let missing_skill = "foo-missing";
    let failing_task_id = create_pending_task(&conn, &project_id, missing_skill);

    // Simulate skill-load failure → creates skill-author node and edge.
    let author_id = simulate_skill_load_failure(&conn, &project_id, &failing_task_id, missing_skill);

    // At this point the failing task is blocked by the skill-author dependency.
    // The skill-author node itself has no blockers, so it should be ready.
    let ready_before: Vec<String> = db_find_ready_tasks(&conn, &project_id)
        .expect("db_find_ready_tasks before completion")
        .into_iter()
        .map(|n| n.id)
        .collect();

    assert!(
        ready_before.contains(&author_id),
        "skill-author node must be in ready tasks (it has no blockers)"
    );
    assert!(
        !ready_before.contains(&failing_task_id),
        "failing task must NOT be ready while skill-author is still pending"
    );

    // ── Mark the skill-author node as complete ────────────────────────────────
    db_update_node(
        &conn,
        &author_id,
        &UpdateNodeInput {
            status: Some(NodeStatus::Complete),
            title: None,
            description: None,
            skill_id: None,
            assignee: None,
            ..Default::default()
        },
    )
    .expect("mark skill-author complete");

    // ── Assert: the originally-failing task is now in the ready set ───────────
    let ready_after: Vec<String> = db_find_ready_tasks(&conn, &project_id)
        .expect("db_find_ready_tasks after completion")
        .into_iter()
        .map(|n| n.id)
        .collect();

    assert!(
        ready_after.contains(&failing_task_id),
        "originally-failing task must appear in ready tasks after skill-author completes"
    );

    // Sanity: skill-author itself must NOT re-appear (it is now complete).
    assert!(
        !ready_after.contains(&author_id),
        "completed skill-author must not appear in ready tasks"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// SH-4: multiple tasks unblock after skill-author completes
// ═════════════════════════════════════════════════════════════════════════════

/// SH-4 (extended SH-3): When two tasks both depend on the same skill-author node,
/// both should become ready once the skill-author is marked complete.
#[test]
fn sh4_multiple_tasks_unblock_after_skill_author_completes() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn);

    let missing_skill = "shared-missing";

    // Two tasks both need the missing skill.
    let task_a_id = create_pending_task(&conn, &project_id, missing_skill);
    let task_b_id = create_pending_task(&conn, &project_id, missing_skill);

    // First failure creates the skill-author node.
    let author_id =
        simulate_skill_load_failure(&conn, &project_id, &task_a_id, missing_skill);
    // Second failure reuses it (dedup) and adds an edge for task_b.
    let author_id_2 =
        simulate_skill_load_failure(&conn, &project_id, &task_b_id, missing_skill);

    assert_eq!(
        author_id, author_id_2,
        "both failures must reference the same skill-author node"
    );

    // Both tasks are blocked, skill-author is ready.
    let ready_before: Vec<String> = db_find_ready_tasks(&conn, &project_id)
        .expect("ready before")
        .into_iter()
        .map(|n| n.id)
        .collect();
    assert_eq!(ready_before, vec![author_id.clone()], "only skill-author is ready");

    // Complete the skill-author node.
    db_update_node(
        &conn,
        &author_id,
        &UpdateNodeInput {
            status: Some(NodeStatus::Complete),
            ..Default::default()
        },
    )
    .expect("mark skill-author complete");

    // Both original tasks must now be ready.
    let mut ready_after: Vec<String> = db_find_ready_tasks(&conn, &project_id)
        .expect("ready after")
        .into_iter()
        .map(|n| n.id)
        .collect();
    ready_after.sort();
    let mut expected = vec![task_a_id.clone(), task_b_id.clone()];
    expected.sort();

    assert_eq!(
        ready_after, expected,
        "both tasks must be ready after skill-author completes"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// SH-5: live dispatch_task integration test via orchestrator::start()
// ═════════════════════════════════════════════════════════════════════════════

/// Minimal no-op EventSink for SH-5.  Captures emitted event names and
/// returns an empty temp directory as the resource dir, so `load_skill` finds
/// no bundled skill files there.
///
/// The `resource_dir` is intentionally set to an EMPTY temp directory, which
/// forces `load_skill("definitely-missing-skill", …)` to fail even in debug
/// builds (the dev-mode `CARGO_MANIFEST_DIR/skills/` candidate is checked
/// first, but "definitely-missing-skill.md" does not exist there either).
struct SpySink {
    /// Emitted event names, in order.
    emitted: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    /// Empty temp dir — no skill files here, so load_skill always fails for
    /// the "definitely-missing-skill" id.
    resource_dir: PathBuf,
}

impl SpySink {
    fn new(resource_dir: PathBuf) -> (Self, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        let emitted = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = Self {
            emitted: emitted.clone(),
            resource_dir,
        };
        (sink, emitted)
    }
}

impl EventSink for SpySink {
    fn emit(&self, event: &str, _payload: &serde_json::Value) {
        self.emitted.lock().unwrap().push(event.to_string());
    }

    fn resource_dir(&self) -> PathBuf {
        self.resource_dir.clone()
    }
}

/// SH-5: The real `orchestrator::start()` loop is the deepest publicly-callable
/// entry point that exercises `dispatch_task`.  This test:
///
///   1. Builds a fully-migrated in-memory SQLite DB with one pending task whose
///      `skill_id = "definitely-missing-skill"` (guaranteed not to exist in any
///      skill search path because the skill name is unique and the resource_dir
///      is an empty temp dir).
///   2. Sends a `DagStructureChanged` signal to the running orchestrator loop.
///   3. Waits long enough for the orchestrator to call `dispatch_task`, trigger
///      the skill-load failure, execute the self-healing path, and send the
///      secondary `DagStructureChanged` wake-up signal back to itself.
///   4. Asserts the data-model invariants that prove self-healing ran through
///      the live production code path (not the `simulate_*` helper).
///
/// ### On the `DagStructureChanged` channel signal assertion
/// `dispatch_task` calls `dag_tx.send(DagChanged::DagStructureChanged{…})` in
/// the same sequential block as `db_create_node` (lines 1413-1461 of
/// orchestrator/mod.rs at time of writing).  The channel send is non-failable
/// (unbounded channel, receiver held by the same orchestrator loop).  We verify
/// the signal was sent in two complementary ways:
///
///   a. **Indirectly by DB state**: the skill-author node exists in the DB iff
///      the full self-heal block completed; the `dag_tx.send` is the last
///      statement in that block (after `db_create_node` and `db_create_edge`),
///      so node existence ⟹ send happened.
///
///   b. **Indirectly by effect**: the secondary `DagStructureChanged` causes the
///      orchestrator to re-run the scheduler; because the `skill-author` skill
///      DOES exist (`src-tauri/skills/skill-author.md` is present in debug
///      builds), the skill-author task is dispatched — its status changes to
///      `running` in the DB, which we assert.
#[tokio::test]
async fn sh5_live_dispatch_task_triggers_skill_author() {
    // ── 1. Build an in-memory DB ──────────────────────────────────────────────
    let conn = new_mem_db();

    let project_id = insert_project(&conn);
    let missing_skill = "definitely-missing-skill";
    let failing_task_id = create_pending_task(&conn, &project_id, missing_skill);

    // Precondition: no skill-author nodes before the orchestrator runs.
    assert_eq!(
        count_skill_author_nodes_for_skill(&conn, &project_id, missing_skill),
        0,
        "precondition: no skill-author nodes before orchestrator dispatch"
    );

    // ── 2. Register the project in the registry ───────────────────────────────
    // The project.path must point to a real (but empty) directory so that
    // - db.project.path is set correctly for the orchestrator's project_path lookup
    // - load_skill searches project_path/.poe/skills/ (which won't exist)
    let project_dir = tempfile::tempdir().expect("tempdir for project");
    let project_path = project_dir.path().to_str().unwrap().to_string();

    let project = poe2_lib::dag_store::types::Project {
        id: project_id.clone(),
        name: "SH-5 Test Project".to_string(),
        path: project_path.clone(),
        conops_ref: None,
        active_phase_id: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };
    let db = Arc::new(ProjectDb {
        project,
        conn: std::sync::Mutex::new(conn),
    });
    let registry = new_registry();
    registry
        .lock()
        .unwrap()
        .insert(project_id.clone(), db.clone());

    // ── 3. Set up sink with empty resource_dir ────────────────────────────────
    let resource_dir = tempfile::tempdir().expect("tempdir for resource_dir");
    let (spy_sink, emitted_events) = SpySink::new(resource_dir.path().to_path_buf());
    let sink_arc: Arc<dyn EventSink> = Arc::new(spy_sink);

    // ── 4. Start the orchestrator loop ────────────────────────────────────────
    let agent_map = new_agent_map();
    let limits = ConcurrencyLimits::new();
    let (dag_tx, dag_rx) = mpsc::unbounded_channel::<DagChanged>();

    let orch_handle = tokio::spawn(orchestrator::start(
        sink_arc,
        registry.clone(),
        limits,
        agent_map,
        dag_tx.clone(),
        dag_rx,
    ));

    // ── 5. Trigger the scheduler ──────────────────────────────────────────────
    dag_tx
        .send(DagChanged::DagStructureChanged {
            project_id: project_id.clone(),
        })
        .expect("send initial DagStructureChanged");

    // Give the orchestrator time to:
    //   (a) process the initial DagStructureChanged → dispatch failing task → self-heal
    //   (b) receive the secondary DagStructureChanged → dispatch skill-author task
    //   (c) call spawn_agent for skill-author (marks it running, emits poe-agent-started)
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // ── 6. Shut down the orchestrator ─────────────────────────────────────────
    drop(dag_tx); // Close the channel → orchestrator loop exits on next recv()
    orch_handle.abort();

    // ── 7. Read back DB state ─────────────────────────────────────────────────
    let conn = db.conn.lock().unwrap();

    // Assert A: the failing task is still `pending` (NOT cancelled).
    let failing_node = db_get_node(&conn, &failing_task_id).expect("get failing task node");
    assert_eq!(
        failing_node.status,
        NodeStatus::Pending,
        "SH-5: failing task must remain pending after skill-load failure (not cancelled)"
    );

    // Assert B: exactly one skill-author node was created with the canonical title.
    assert_eq!(
        count_skill_author_nodes_for_skill(&conn, &project_id, missing_skill),
        1,
        "SH-5: exactly one skill-author node must be created for the missing skill"
    );

    // Find the skill-author node id by querying the DB directly.
    let synth_title = format!("Synthesize missing skill: {}", missing_skill);
    let author_id: String = conn
        .query_row(
            "SELECT id FROM nodes WHERE project_id = ?1 AND title = ?2 AND skill_id = 'skill-author'",
            rusqlite::params![project_id, synth_title],
            |r| r.get(0),
        )
        .expect("SH-5: skill-author node must exist in DB");

    // Assert C: `depends_on` edge exists with the correct direction:
    //   from_id = author (prerequisite) → to_id = failing task (dependent).
    // This is the key semantics: the failing task is blocked until skill-author completes.
    assert_eq!(
        count_edges_from_to(&conn, &author_id, &failing_task_id),
        1,
        "SH-5: depends_on edge must run from skill-author (prereq) to failing task (dependent)"
    );

    // Assert D: the skill-author task was dispatched by the secondary
    // DagStructureChanged signal — its status is now `running`.
    // This also proves (indirectly) that `dag_tx.send(DagStructureChanged)` was
    // called: without that signal, the orchestrator's loop would never have
    // picked up the newly-created skill-author task.
    //
    // Note: `skill-author.md` is present in `src-tauri/skills/` (dev-mode path),
    // so `load_skill("skill-author", …)` succeeds and `spawn_agent` is called,
    // which marks the node running synchronously before launching the subprocess.
    let author_node = db_get_node(&conn, &author_id).expect("get skill-author node");
    assert_eq!(
        author_node.status,
        NodeStatus::Running,
        "SH-5: skill-author task must be running (DagStructureChanged was sent and processed)"
    );

    // Assert E: `poe-agent-started` was emitted for the skill-author task —
    // `spawn_agent` emits this synchronously before launching the subprocess.
    // This is the clearest observable proof that the secondary DagStructureChanged
    // signal caused the orchestrator to fully dispatch the skill-author task.
    let emitted = emitted_events.lock().unwrap();
    assert!(
        emitted.contains(&"poe-agent-started".to_string()),
        "SH-5: poe-agent-started must be emitted for the skill-author task; got: {:?}",
        *emitted
    );

    // Assert F: `poe-agent-started` was NOT emitted for the FAILING task
    // (it must have been left pending, not dispatched through the normal path).
    // We verify this by checking we have exactly 1 `poe-agent-started` event —
    // for the skill-author only, not the originally-failing task.
    let started_count = emitted
        .iter()
        .filter(|e| e.as_str() == "poe-agent-started")
        .count();
    assert_eq!(
        started_count, 1,
        "SH-5: exactly one poe-agent-started must be emitted (skill-author only, not the failing task)"
    );
}
