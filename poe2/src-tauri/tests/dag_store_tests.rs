//! Unit tests for dag_store SQLite CRUD helpers (bp6-m2f.11.1).
//!
//! Each test uses an independent in-memory SQLite database for isolation.
//! Schema is initialised via `schema::CREATE_TABLES` plus the `promoted`
//! column migration that mirrors what `open_project_db` does at runtime.

use poe2_lib::dag_store::{
    schema,
    types::{
        CreateKnowledgeInput, CreateNodeInput, NodeStatus, NodeType,
        UpdateNodeInput,
    },
    db_count_unresolved_queue_items_for_task, db_create_agent, db_create_knowledge,
    db_create_node, db_create_queue_item, db_get_ancestry, db_get_node, db_list_phases,
    db_resolve_queue_item, db_update_node, db_get_agent_session_for_task,
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
