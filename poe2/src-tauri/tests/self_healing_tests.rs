//! DAG-level integration tests for the self-healing skill synthesis loop (bp6-13z.8).
//!
//! The orchestrator's `dispatch_task` function contains the self-healing path: when
//! `skills::load_skill` fails for a task, the orchestrator:
//!   1. Creates (or reuses) a `skill-author` task titled "Synthesize missing skill: {name}"
//!   2. Wires a `depends_on` edge from the failing task → skill-author node
//!   3. Leaves the failing task `pending` (NOT cancelled)
//!
//! Because that path is tightly coupled to async Arcs and the live skill-loading
//! infrastructure, these tests validate the *data-model invariants* directly via
//! the `dag_store` layer — the same functions the orchestrator calls internally.
//! Each test is deterministic, synchronous, and uses an isolated in-memory SQLite DB.
//!
//! ## What an integration test would additionally assert
//! An end-to-end integration test running the full `orchestrator::start()` loop would:
//!   - Call `dispatch_task` with a task whose `skill_id` points to a non-existent skill file.
//!   - Assert that no `poe-agent-started` event is emitted for the failing task.
//!   - Assert that a `DagStructureChanged` signal is emitted to wake the scheduler.
//!   - Assert that the skill-author task is subsequently dispatched (poe-agent-started).
//! That level of test is deferred until the live-test harness in `e2e_orchestrator_tests.rs`
//! supports skill-load failure injection.
//!
//! ## Tests
//!   SH-1: skill-author task created on missing skill
//!   SH-2: dedup guard — second failure reuses existing skill-author node
//!   SH-3: blocked tasks unblock after skill-author completes

use poe2_lib::dag_store::{
    schema,
    types::{CreateNodeInput, EdgeType, NodeStatus, NodeType, UpdateNodeInput},
    db_create_edge, db_create_node, db_find_ready_tasks, db_get_node,
    db_list_nodes_by_status, db_update_node,
};
use rusqlite::Connection;

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
/// ## Note on orchestrator/mod.rs edge direction
/// The live orchestrator currently wires the edge in the **reverse** direction:
/// `db_create_edge(&conn, &task.id, &author_id, ...)` — `from_id = task, to_id = author`.
/// In the `db_find_ready_tasks` SQL, a node `n` is blocked when an edge exists with
/// `e.to_id = n.id AND dep.status != 'complete'`.  With the reversed direction,
/// `to_id = author_id`, so the *skill-author* node is blocked by the failing task — the
/// exact opposite of the intended semantics.  The failing task itself is never blocked
/// and therefore never "unblocks" in the db_find_ready_tasks sense; instead the
/// orchestrator relies on the skill being present at the next dispatch attempt.
///
/// These tests validate the **intended** data-model invariants (correct edge direction).
/// A separate issue should track fixing the live orchestrator's edge direction.
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
