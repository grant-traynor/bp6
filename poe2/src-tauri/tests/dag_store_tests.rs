//! Unit tests for dag_store SQLite CRUD helpers (bp6-m2f.11.1).
//! Property tests for `db_find_ready_tasks` — DAG topology and concurrency (bp6-m2f.11.3).
//!
//! Each test uses an independent in-memory SQLite database for isolation.
//! Schema is initialised via `schema::CREATE_TABLES` plus the `promoted`
//! column migration that mirrors what `open_project_db` does at runtime.

use poe2_lib::dag_store::{
    schema,
    types::{
        CreateKnowledgeInput, CreateNodeInput, EdgeType, NodeStatus, NodeType,
        UpdateNodeInput,
    },
    db_count_running_agents, db_count_unresolved_queue_items_for_task, db_create_agent,
    db_create_edge, db_create_knowledge, db_create_node, db_create_queue_item,
    db_find_ready_tasks, db_get_ancestry, db_get_node, db_list_phases, db_resolve_queue_item,
    db_update_node, db_get_agent_session_for_task,
};
use rusqlite::Connection;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Open a fresh in-memory database with the full schema applied.
fn new_mem_db() -> Connection {
    let conn = Connection::open_in_memory().expect("open_in_memory");
    conn.execute_batch(schema::CREATE_TABLES)
        .expect("apply CREATE_TABLES");
    // Mirror the runtime migration for the `promoted` column.
    let _ = conn.execute_batch(
        "ALTER TABLE knowledge ADD COLUMN promoted INTEGER NOT NULL DEFAULT 0",
    );
    conn
}

/// Insert a minimal project row and return its id.
fn insert_project(conn: &Connection, name: &str) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO projects (id, name, path, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, name, format!("/tmp/{}", id), now, now],
    )
    .expect("insert project");
    id
}

/// Insert a minimal phase row and return its id.
fn insert_phase(conn: &Connection, project_id: &str, number: i64, title: &str) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO phases (id, project_id, number, title, lifecycle_stage, gate_held, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'planning', 0, ?5, ?6)",
        rusqlite::params![id, project_id, number, title, now, now],
    )
    .expect("insert phase");
    id
}

// ── 1. Queue item lifecycle ───────────────────────────────────────────────────

#[test]
fn queue_item_create_has_null_resolved_at() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn, "proj-q");

    let item = db_create_queue_item(
        &conn,
        &project_id,
        None,
        Some("task-1"),
        "What colour?",
        Some(r#"["red","blue"]"#),
    )
    .expect("create queue item");

    assert_eq!(item.project_id, project_id);
    assert_eq!(item.question, "What colour?");
    assert!(item.resolution.is_none(), "resolution should be None");
    assert!(item.resolved_at.is_none(), "resolved_at should be None");
}

#[test]
fn queue_item_resolve_sets_resolution_and_resolved_at() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn, "proj-r");

    let item = db_create_queue_item(&conn, &project_id, None, None, "Pick one", None)
        .expect("create");

    let resolved = db_resolve_queue_item(&conn, &item.id, "blue").expect("resolve");

    assert_eq!(resolved.resolution.as_deref(), Some("blue"));
    assert!(
        resolved.resolved_at.is_some(),
        "resolved_at must be set after resolve"
    );
}

// ── 2. NodeStatus transitions ─────────────────────────────────────────────────

#[test]
fn node_status_transitions_pending_running_waiting_complete() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn, "proj-status");

    let node = db_create_node(
        &conn,
        &CreateNodeInput {
            project_id: project_id.clone(),
            phase_id: None,
            parent_id: None,
            node_type: NodeType::Task,
            title: "Status test task".to_owned(),
            description: None,
            skill_id: None,
        },
    )
    .expect("create node");

    assert_eq!(node.status, NodeStatus::Pending);

    // Transition → Running
    db_update_node(
        &conn,
        &node.id,
        &UpdateNodeInput {
            title: None,
            description: None,
            status: Some(NodeStatus::Running),
            skill_id: None,
            assignee: None,
        },
    )
    .expect("set running");
    let n = db_get_node(&conn, &node.id).expect("get");
    assert_eq!(n.status, NodeStatus::Running);

    // Transition → Waiting
    db_update_node(
        &conn,
        &node.id,
        &UpdateNodeInput {
            title: None,
            description: None,
            status: Some(NodeStatus::Waiting),
            skill_id: None,
            assignee: None,
        },
    )
    .expect("set waiting");
    let n = db_get_node(&conn, &node.id).expect("get");
    assert_eq!(n.status, NodeStatus::Waiting);

    // Transition → Complete
    db_update_node(
        &conn,
        &node.id,
        &UpdateNodeInput {
            title: None,
            description: None,
            status: Some(NodeStatus::Complete),
            skill_id: None,
            assignee: None,
        },
    )
    .expect("set complete");
    let n = db_get_node(&conn, &node.id).expect("get");
    assert_eq!(n.status, NodeStatus::Complete);
}

// ── 3. db_get_ancestry ────────────────────────────────────────────────────────

#[test]
fn ancestry_three_level_hierarchy_closest_first() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn, "proj-anc");

    let grandparent = db_create_node(
        &conn,
        &CreateNodeInput {
            project_id: project_id.clone(),
            phase_id: None,
            parent_id: None,
            node_type: NodeType::Epic,
            title: "Grandparent".to_owned(),
            description: None,
            skill_id: None,
        },
    )
    .expect("create grandparent");

    let parent = db_create_node(
        &conn,
        &CreateNodeInput {
            project_id: project_id.clone(),
            phase_id: None,
            parent_id: Some(grandparent.id.clone()),
            node_type: NodeType::Feature,
            title: "Parent".to_owned(),
            description: None,
            skill_id: None,
        },
    )
    .expect("create parent");

    let child = db_create_node(
        &conn,
        &CreateNodeInput {
            project_id: project_id.clone(),
            phase_id: None,
            parent_id: Some(parent.id.clone()),
            node_type: NodeType::Task,
            title: "Child".to_owned(),
            description: None,
            skill_id: None,
        },
    )
    .expect("create child");

    let ancestry = db_get_ancestry(&conn, &child.id).expect("get ancestry");

    assert_eq!(ancestry.len(), 3, "should have 3 nodes in chain");
    assert_eq!(ancestry[0].id, child.id, "first = child (closest)");
    assert_eq!(ancestry[1].id, parent.id, "second = parent");
    assert_eq!(ancestry[2].id, grandparent.id, "third = grandparent");
}

#[test]
fn ancestry_root_node_returns_single_entry() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn, "proj-root-anc");

    let root = db_create_node(
        &conn,
        &CreateNodeInput {
            project_id: project_id.clone(),
            phase_id: None,
            parent_id: None,
            node_type: NodeType::Epic,
            title: "Root epic".to_owned(),
            description: None,
            skill_id: None,
        },
    )
    .expect("create root");

    let ancestry = db_get_ancestry(&conn, &root.id).expect("get ancestry");

    assert_eq!(ancestry.len(), 1);
    assert_eq!(ancestry[0].id, root.id);
}

// ── 4. db_list_phases ─────────────────────────────────────────────────────────

#[test]
fn list_phases_scoped_to_project() {
    let conn = new_mem_db();
    let proj_a = insert_project(&conn, "proj-a");
    let proj_b = insert_project(&conn, "proj-b");

    insert_phase(&conn, &proj_a, 1, "Alpha Phase 1");
    insert_phase(&conn, &proj_a, 2, "Alpha Phase 2");
    insert_phase(&conn, &proj_b, 1, "Beta Phase 1");

    let phases_a = db_list_phases(&conn, &proj_a).expect("list phases a");
    assert_eq!(phases_a.len(), 2, "proj_a should have 2 phases");
    assert!(
        phases_a.iter().all(|p| p.project_id == proj_a),
        "all phases should belong to proj_a"
    );

    let phases_b = db_list_phases(&conn, &proj_b).expect("list phases b");
    assert_eq!(phases_b.len(), 1, "proj_b should have 1 phase");
}

#[test]
fn list_phases_ordered_by_number() {
    let conn = new_mem_db();
    let proj = insert_project(&conn, "proj-order");

    // Insert out of order.
    insert_phase(&conn, &proj, 3, "Phase 3");
    insert_phase(&conn, &proj, 1, "Phase 1");
    insert_phase(&conn, &proj, 2, "Phase 2");

    let phases = db_list_phases(&conn, &proj).expect("list phases");
    assert_eq!(phases.len(), 3);
    assert_eq!(phases[0].number, 1);
    assert_eq!(phases[1].number, 2);
    assert_eq!(phases[2].number, 3);
}

// ── 5. db_count_unresolved_queue_items_for_task ───────────────────────────────

#[test]
fn count_unresolved_queue_items_decrements_on_resolve() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn, "proj-count");

    let task_id = "task-count-test";

    let item1 = db_create_queue_item(&conn, &project_id, None, Some(task_id), "Q1", None)
        .expect("create item1");
    let item2 = db_create_queue_item(&conn, &project_id, None, Some(task_id), "Q2", None)
        .expect("create item2");

    // Both pending → count = 2
    let count = db_count_unresolved_queue_items_for_task(&conn, task_id).expect("count");
    assert_eq!(count, 2);

    // Resolve one → count = 1
    db_resolve_queue_item(&conn, &item1.id, "answered").expect("resolve item1");
    let count = db_count_unresolved_queue_items_for_task(&conn, task_id).expect("count");
    assert_eq!(count, 1);

    // Resolve both → count = 0
    db_resolve_queue_item(&conn, &item2.id, "also answered").expect("resolve item2");
    let count = db_count_unresolved_queue_items_for_task(&conn, task_id).expect("count");
    assert_eq!(count, 0);
}

// ── 6. db_get_agent_session_for_task ─────────────────────────────────────────

#[test]
fn agent_session_retrievable_for_task() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn, "proj-agent");

    // The agents table has a FK on task_id → nodes.id, so we need a real node.
    let node = db_create_node(
        &conn,
        &CreateNodeInput {
            project_id: project_id.clone(),
            phase_id: None,
            parent_id: None,
            node_type: NodeType::Task,
            title: "Agent task".to_owned(),
            description: None,
            skill_id: None,
        },
    )
    .expect("create node");

    let session_id = "session-abc-123";
    db_create_agent(&conn, &project_id, "planner", &node.id, Some(session_id))
        .expect("create agent");

    let found = db_get_agent_session_for_task(&conn, &node.id).expect("get session");
    assert_eq!(found.as_deref(), Some(session_id));
}

#[test]
fn agent_session_returns_none_when_no_agent() {
    let conn = new_mem_db();
    let result = db_get_agent_session_for_task(&conn, "nonexistent-task").expect("query");
    assert!(result.is_none());
}

// ── 7. promoted column migration idempotency ──────────────────────────────────

#[test]
fn promoted_column_migration_is_idempotent() {
    let conn = Connection::open_in_memory().expect("open_in_memory");
    conn.execute_batch(schema::CREATE_TABLES)
        .expect("apply schema first time");

    // First migration — should succeed.
    let r1 = conn.execute_batch(
        "ALTER TABLE knowledge ADD COLUMN promoted INTEGER NOT NULL DEFAULT 0",
    );
    assert!(r1.is_ok(), "first migration should succeed");

    // Second migration — column already exists; the runtime ignores the error with `let _`.
    // This test verifies the error does NOT panic and can be safely ignored.
    let r2 = conn.execute_batch(
        "ALTER TABLE knowledge ADD COLUMN promoted INTEGER NOT NULL DEFAULT 0",
    );
    // SQLite returns an error when the column already exists; the application
    // intentionally swallows it.  We just assert it doesn't panic.
    drop(r2);
}

#[test]
fn flag_knowledge_for_promotion_sets_promoted_column() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn, "proj-promo");

    // Create a knowledge entry.
    let entry = db_create_knowledge(
        &conn,
        &CreateKnowledgeInput {
            project_id: project_id.clone(),
            key: "arch-decision".to_owned(),
            value: "Use Riverpod for state".to_owned(),
            source: None,
            supersedes_id: None,
        },
    )
    .expect("create knowledge");

    // Directly apply the same SQL that `flag_knowledge_for_promotion` uses.
    conn.execute(
        "UPDATE knowledge SET promoted = 1 WHERE id = ?1",
        [&entry.id],
    )
    .expect("set promoted");

    // Read back and verify the column value.
    let promoted: i64 = conn
        .query_row(
            "SELECT promoted FROM knowledge WHERE id = ?1",
            [&entry.id],
            |row| row.get(0),
        )
        .expect("query promoted");

    assert_eq!(promoted, 1, "promoted should be 1 after flagging");
}

// ═══════════════════════════════════════════════════════════════════════════════
// bp6-m2f.11.3 — Property tests for `db_find_ready_tasks`
// ═══════════════════════════════════════════════════════════════════════════════

// ── Helpers specific to ready-task tests ──────────────────────────────────────

/// Create an in-memory DB, a project, and a phase in `execution` stage with
/// `gate_held = 0`.  Returns `(conn, project_id, phase_id)`.
fn setup_execution_db() -> (Connection, String, String) {
    let conn = new_mem_db();
    let project_id = insert_project(&conn, "proj-ready");
    let phase_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO phases (id, project_id, number, title, lifecycle_stage, gate_held, created_at, updated_at)
         VALUES (?1, ?2, 1, 'Execution Phase', 'execution', 0, ?3, ?3)",
        rusqlite::params![phase_id, project_id, now],
    )
    .expect("insert execution phase");
    (conn, project_id, phase_id)
}

/// Create a `task` node inside the given phase.  Returns its id.
fn add_task(conn: &Connection, project_id: &str, phase_id: &str, title: &str) -> String {
    db_create_node(
        conn,
        &CreateNodeInput {
            project_id: project_id.to_owned(),
            phase_id: Some(phase_id.to_owned()),
            parent_id: None,
            node_type: NodeType::Task,
            title: title.to_owned(),
            description: None,
            skill_id: None,
        },
    )
    .expect("create task node")
    .id
}

/// Set a node's status via raw SQL.
fn force_status(conn: &Connection, node_id: &str, status: &str) {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE nodes SET status = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![status, now, node_id],
    )
    .expect("force status");
}

/// Collect and sort node IDs from `db_find_ready_tasks`.
fn sorted_ready_ids(conn: &Connection, project_id: &str) -> Vec<String> {
    let mut ids: Vec<String> = db_find_ready_tasks(conn, project_id)
        .expect("db_find_ready_tasks")
        .into_iter()
        .map(|n| n.id)
        .collect();
    ids.sort();
    ids
}

// ── Test: linear chain A→B→C ──────────────────────────────────────────────────

#[test]
fn ready_tasks_linear_chain_dispatches_in_order() {
    let (conn, project_id, phase_id) = setup_execution_db();

    let a = add_task(&conn, &project_id, &phase_id, "A");
    let b = add_task(&conn, &project_id, &phase_id, "B");
    let c = add_task(&conn, &project_id, &phase_id, "C");

    // A → B → C (each depends on the previous).
    db_create_edge(&conn, &a, &b, EdgeType::DependsOn).expect("edge A→B");
    db_create_edge(&conn, &b, &c, EdgeType::DependsOn).expect("edge B→C");

    // Initially only A is unblocked.
    assert_eq!(
        sorted_ready_ids(&conn, &project_id),
        vec![a.clone()],
        "initially only A should be ready"
    );

    // After A completes, B is unblocked.
    force_status(&conn, &a, "complete");
    assert_eq!(
        sorted_ready_ids(&conn, &project_id),
        vec![b.clone()],
        "after A completes, only B should be ready"
    );

    // After B completes, C is unblocked.
    force_status(&conn, &b, "complete");
    assert_eq!(
        sorted_ready_ids(&conn, &project_id),
        vec![c.clone()],
        "after B completes, only C should be ready"
    );
}

// ── Test: diamond A→B, A→C, B+C→D ────────────────────────────────────────────

#[test]
fn ready_tasks_diamond_dispatches_correctly() {
    let (conn, project_id, phase_id) = setup_execution_db();

    let a = add_task(&conn, &project_id, &phase_id, "A");
    let b = add_task(&conn, &project_id, &phase_id, "B");
    let c = add_task(&conn, &project_id, &phase_id, "C");
    let d = add_task(&conn, &project_id, &phase_id, "D");

    db_create_edge(&conn, &a, &b, EdgeType::DependsOn).expect("A→B");
    db_create_edge(&conn, &a, &c, EdgeType::DependsOn).expect("A→C");
    db_create_edge(&conn, &b, &d, EdgeType::DependsOn).expect("B→D");
    db_create_edge(&conn, &c, &d, EdgeType::DependsOn).expect("C→D");

    // Only A is ready initially.
    assert_eq!(
        sorted_ready_ids(&conn, &project_id),
        vec![a.clone()],
        "initially only A ready"
    );

    // After A completes, both B and C are ready.
    force_status(&conn, &a, "complete");
    let mut ready = sorted_ready_ids(&conn, &project_id);
    let mut expected = vec![b.clone(), c.clone()];
    expected.sort();
    assert_eq!(ready, expected, "after A: B and C must both be ready");

    // After B completes (C still pending), D is NOT yet ready.
    force_status(&conn, &b, "complete");
    ready = sorted_ready_ids(&conn, &project_id);
    assert!(ready.contains(&c), "C must still be ready");
    assert!(!ready.contains(&d), "D must not be ready while C is pending");

    // After C completes, D becomes ready.
    force_status(&conn, &c, "complete");
    assert_eq!(
        sorted_ready_ids(&conn, &project_id),
        vec![d.clone()],
        "after B and C complete, only D should be ready"
    );
}

// ── Test: waiting node not dispatched ─────────────────────────────────────────

#[test]
fn ready_tasks_waiting_node_not_dispatched() {
    let (conn, project_id, phase_id) = setup_execution_db();

    let w = add_task(&conn, &project_id, &phase_id, "Waiting");
    let p = add_task(&conn, &project_id, &phase_id, "Pending");

    // Set w to Waiting — no blockers, but status is not 'pending'.
    force_status(&conn, &w, "waiting");

    let ready = sorted_ready_ids(&conn, &project_id);
    assert!(
        !ready.contains(&w),
        "waiting node must not appear in ready tasks"
    );
    assert!(
        ready.contains(&p),
        "pending node with no deps must appear in ready tasks"
    );
}

// ── Test: running node not double-spawned ─────────────────────────────────────

#[test]
fn ready_tasks_running_node_not_double_spawned() {
    let (conn, project_id, phase_id) = setup_execution_db();

    let r = add_task(&conn, &project_id, &phase_id, "Running");
    let p = add_task(&conn, &project_id, &phase_id, "Pending");

    force_status(&conn, &r, "running");

    let ready = sorted_ready_ids(&conn, &project_id);
    assert!(
        !ready.contains(&r),
        "running node must not appear in ready tasks (would cause double-spawn)"
    );
    assert!(
        ready.contains(&p),
        "pending node must still appear alongside a running node"
    );
}

// ── Test: gate_held blocks phase ──────────────────────────────────────────────

#[test]
fn ready_tasks_gate_held_blocks_phase() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn, "proj-gate");
    let now = chrono::Utc::now().to_rfc3339();

    // Phase with gate_held = 1 (blocked at gate).
    let held_phase_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO phases (id, project_id, number, title, lifecycle_stage, gate_held, created_at, updated_at)
         VALUES (?1, ?2, 1, 'Held', 'execution', 1, ?3, ?3)",
        rusqlite::params![held_phase_id, project_id, now],
    )
    .expect("insert held phase");

    // Phase with gate_held = 0 (open for dispatch).
    let open_phase_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO phases (id, project_id, number, title, lifecycle_stage, gate_held, created_at, updated_at)
         VALUES (?1, ?2, 2, 'Open', 'execution', 0, ?3, ?3)",
        rusqlite::params![open_phase_id, project_id, now],
    )
    .expect("insert open phase");

    let blocked_task = add_task(&conn, &project_id, &held_phase_id, "BlockedByGate");
    let open_task = add_task(&conn, &project_id, &open_phase_id, "OpenTask");

    let ready = sorted_ready_ids(&conn, &project_id);
    assert!(
        !ready.contains(&blocked_task),
        "task in gate_held=1 phase must NOT appear in ready tasks"
    );
    assert!(
        ready.contains(&open_task),
        "task in gate_held=0 phase must appear in ready tasks"
    );
}

// ── Test: concurrency limit respected ─────────────────────────────────────────

/// `db_find_ready_tasks` returns ALL eligible pending tasks without applying a
/// concurrency cap.  The orchestration layer enforces the cap by comparing the
/// result count with `db_count_running_agents`.  This test verifies:
///
///   - With 5 independent pending tasks and 1 agent already running, the full
///     set of 5 ready tasks is returned.
///   - The running task itself does NOT appear in ready tasks.
///   - `db_count_running_agents` correctly returns 1.
///   - Applying limit=2 and running=1 gives budget=1, so the caller dispatches
///     exactly 1 more task.
#[test]
fn ready_tasks_concurrency_limit_respected() {
    let (conn, project_id, phase_id) = setup_execution_db();

    // Create 5 independent pending tasks.
    let mut task_ids = Vec::new();
    for i in 0..5_usize {
        task_ids.push(add_task(&conn, &project_id, &phase_id, &format!("Task{}", i)));
    }

    // Create an additional node that will be set to running, simulating an
    // already-dispatched agent consuming one concurrency slot.
    let running_task = add_task(&conn, &project_id, &phase_id, "RunningTask");
    force_status(&conn, &running_task, "running");

    // Insert a running agent record pointing to the running task.
    let now = chrono::Utc::now().to_rfc3339();
    let agent_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO agents (id, project_id, skill_id, task_id, status, started_at)
         VALUES (?1, ?2, 'test-skill', ?3, 'running', ?4)",
        rusqlite::params![agent_id, project_id, running_task, now],
    )
    .expect("insert running agent");

    // All 5 pending tasks must appear in the ready list.
    let ready = db_find_ready_tasks(&conn, &project_id).expect("find ready tasks");
    let ready_id_set: std::collections::HashSet<String> =
        ready.iter().map(|n| n.id.clone()).collect();

    for id in &task_ids {
        assert!(
            ready_id_set.contains(id),
            "pending task {} must appear in ready list",
            id
        );
    }
    // The running task must not appear.
    assert!(
        !ready_id_set.contains(&running_task),
        "running task must not appear in ready tasks"
    );

    // The caller computes available budget.
    let concurrency_limit: usize = 2;
    let running_count =
        db_count_running_agents(&conn, &project_id).expect("count running agents");
    assert_eq!(running_count, 1, "exactly 1 agent should be running");

    let budget = concurrency_limit.saturating_sub(running_count);
    assert_eq!(budget, 1, "budget = limit(2) - running(1) = 1");

    // The caller dispatches at most `budget` tasks.
    let to_dispatch: Vec<_> = ready.into_iter().take(budget).collect();
    assert_eq!(
        to_dispatch.len(),
        1,
        "caller dispatches exactly 1 more task to stay within concurrency limit"
    );
}
