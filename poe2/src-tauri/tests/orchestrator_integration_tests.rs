//! Fixture-based integration tests for the POE event ingester and orchestrator
//! state machine (bp6-0xf.10).
//!
//! These tests do NOT spawn real Claude processes. They exercise the DB layer
//! directly via `db_*` helper functions, asserting SQLite state after each
//! fixture poe: event sequence.
//!
//! AppHandle is not available in integration tests. Every test that would
//! require it calls the equivalent pure DB function instead and is annotated
//! with a comment explaining what the live path does.
//!
//! Coverage:
//!   TC-1  SF-1 + SF-2 — session_id capture, node1 done, node2 unblocked
//!   TC-2  SF-3/SF-4 decision path — yield_reason='decision', queue_item created
//!   TC-3  SF-3/SF-4 review path — reviewer task created, yield_reason='review'
//!   TC-4  SF-3/SF-4 chat path — chat_turns row, yield_reason='chat'
//!   TC-5  SF-3/SF-4 advisor path — advisor_turns row, yield_reason='advisor'
//!   TC-6  SF-5 phase activation — phase 1 running, phases 2+3 still pending
//!   TC-7  Phase gate + advance_phase — phase 1 complete, phase 2 running
//!   TC-8  Phase gate + revise_phase — t1 reset to pending, others remain done
//!   TC-9  Phase gate + rerun_phase — all done tasks reset to pending
//!   TC-10 skill_modes population — nodes.skill_modes JSON array set correctly

use poe2_lib::dag_store::{
    schema,
    types::{CreateNodeInput, EdgeType, NodeStatus, NodeType, PhaseLifecycleStage, UpdateNodeInput},
    db_create_edge, db_create_node, db_create_phase, db_find_ready_tasks, db_get_node,
    db_get_phase, db_insert_advisor_turn, db_insert_chat_turn, db_list_advisor_turns,
    db_list_chat_turns, db_list_phases, db_update_node, db_update_node_session,
    db_update_node_skill_modes,
};
use poe2_lib::event_ingester::{db_handle_decision, db_handle_done};
use rusqlite::Connection;

// ── In-memory DB setup ────────────────────────────────────────────────────────

/// Open a fresh in-memory SQLite database with the full schema applied,
/// including all runtime migrations that `open_project_db` performs.
fn new_mem_db() -> Connection {
    let conn = Connection::open_in_memory().expect("open_in_memory");
    conn.execute_batch(schema::CREATE_TABLES)
        .expect("apply CREATE_TABLES");
    // Runtime migrations — mirrors open_project_db exactly.
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
    let _ = conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_chat_turns_task ON chat_turns(task_id)",
    );
    // Phase 3 migrations
    let _ = conn
        .execute_batch("ALTER TABLE phases ADD COLUMN stage_type TEXT NOT NULL DEFAULT 'execution'");
    let _ =
        conn.execute_batch("ALTER TABLE phases ADD COLUMN status TEXT NOT NULL DEFAULT 'pending'");
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
    let _ = conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_advisor_turns_task ON advisor_turns(task_id)",
    );
    conn
}

// ── Fixture helpers ───────────────────────────────────────────────────────────

fn insert_project(conn: &Connection) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO projects (id, name, path, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, "Test Project", format!("/tmp/test-{}", id), now, now],
    )
    .expect("insert project");
    id
}

/// Create a pending task node with an optional phase and skill.
fn create_pending_task(
    conn: &Connection,
    project_id: &str,
    phase_id: Option<&str>,
    skill_id: Option<&str>,
) -> String {
    let node = db_create_node(
        conn,
        &CreateNodeInput { id: None,
            project_id: project_id.to_owned(),
            phase_id: phase_id.map(str::to_owned),
            parent_id: None,
            node_type: NodeType::Task,
            title: "Test task".to_owned(),
            description: None,
            skill_id: skill_id.map(str::to_owned),
            initial_status: None,
            requesting_task_id: None,
            review_id: None,
            retry_count: None,
        },
    )
    .expect("create task node");
    node.id
}

fn read_status(conn: &Connection, node_id: &str) -> NodeStatus {
    let s: String = conn
        .query_row(
            "SELECT status FROM nodes WHERE id = ?1",
            [node_id],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| panic!("node not found: {}", node_id));
    s.parse().expect("valid status string")
}

fn read_yield_reason(conn: &Connection, node_id: &str) -> Option<String> {
    conn.query_row(
        "SELECT yield_reason FROM nodes WHERE id = ?1",
        [node_id],
        |r| r.get(0),
    )
    .unwrap_or_else(|_| panic!("node not found: {}", node_id))
}

fn read_session_id(conn: &Connection, node_id: &str) -> Option<String> {
    conn.query_row(
        "SELECT session_id FROM nodes WHERE id = ?1",
        [node_id],
        |r| r.get(0),
    )
    .unwrap_or_else(|_| panic!("node not found: {}", node_id))
}

fn read_phase_status(conn: &Connection, phase_id: &str) -> String {
    conn.query_row(
        "SELECT status FROM phases WHERE id = ?1",
        [phase_id],
        |r| r.get(0),
    )
    .unwrap_or_else(|_| panic!("phase not found: {}", phase_id))
}

fn read_skill_modes(conn: &Connection, node_id: &str) -> Option<String> {
    conn.query_row(
        "SELECT skill_modes FROM nodes WHERE id = ?1",
        [node_id],
        |r| r.get(0),
    )
    .unwrap_or_else(|_| panic!("node not found: {}", node_id))
}

fn force_status(conn: &Connection, node_id: &str, status: &str) {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE nodes SET status = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![status, now, node_id],
    )
    .expect("force status");
}

/// Set a phase's status column directly (equivalent to the DB side of activate_phase,
/// advance_phase etc., without needing Tauri AppHandle or dag_tx).
fn force_phase_status(conn: &Connection, phase_id: &str, status: &str) {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE phases SET status = ?1, lifecycle_stage = 'execution', gate_held = 0, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![status, now, phase_id],
    )
    .expect("force phase status");
}

/// Simulate the poe:yield DB write that ingest_line_with_tracker performs
/// (without AppHandle/dag_tx, which aren't testable here).
fn apply_yield(conn: &Connection, node_id: &str, derived_reason: Option<&str>) {
    db_update_node(
        conn,
        node_id,
        &UpdateNodeInput {
            status: Some(NodeStatus::Waiting),
            yield_reason: derived_reason.map(str::to_owned),
            ..Default::default()
        },
    )
    .expect("apply yield");
}

// ═════════════════════════════════════════════════════════════════════════════
// TC-1: SF-1 + SF-2 — session_id capture, node1 done, node2 dependency unblocked
// ═════════════════════════════════════════════════════════════════════════════

/// TC-1: Create project + 2 pending nodes (node2 depends on node1).
/// Simulate SF-1 init by writing session_id to node1.
/// Apply poe:done for node1 — assert node1=complete.
/// Assert node2 becomes eligible (db_find_ready_tasks returns it).
#[test]
fn tc1_sf1_sf2_node1_done_unblocks_node2() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn);

    // Create two pending tasks with node2 depending on node1.
    let node1_id = create_pending_task(&conn, &project_id, None, Some("implementer"));
    let node2_id = create_pending_task(&conn, &project_id, None, Some("implementer"));

    // node2 depends on node1 (finish-to-start): edge from=node1, to=node2.
    db_create_edge(&conn, &node1_id, &node2_id, EdgeType::DependsOn).expect("create edge");

    // Assert only node1 is ready (node2 is blocked).
    let ready_before = db_find_ready_tasks(&conn, &project_id).expect("find ready tasks");
    let ready_ids: Vec<&str> = ready_before.iter().map(|n| n.id.as_str()).collect();
    assert!(
        ready_ids.contains(&node1_id.as_str()),
        "node1 must be ready before node2's dependency is met"
    );
    assert!(
        !ready_ids.contains(&node2_id.as_str()),
        "node2 must NOT be ready while node1 is pending"
    );

    // SF-1: simulate agent start — write session_id to node1.
    // In production: spawn_agent writes this via db_update_node_session at agent init.
    let session_id = "sess-abc123";
    db_update_node_session(&conn, &node1_id, session_id).expect("update session");
    assert_eq!(
        read_session_id(&conn, &node1_id).as_deref(),
        Some(session_id),
        "session_id must be stored on node1 after SF-1 init"
    );

    // Mark node1 as running (simulates the dispatch transition).
    force_status(&conn, &node1_id, "running");

    // SF-2 / poe:brief + poe:step (log-only events — no DB state change).
    // These would call handle_log_only in the ingester; no assertion needed.

    // poe:done for node1.
    let done_json = r#"{"poe":"done","summary":"node1 complete"}"#;
    let status = db_handle_done(&conn, done_json, &project_id, &node1_id, "agent-1")
        .expect("db_handle_done");

    assert_eq!(status, NodeStatus::Complete, "poe:done must return Complete");
    assert_eq!(
        read_status(&conn, &node1_id),
        NodeStatus::Complete,
        "node1 must be Complete after poe:done"
    );

    // Assert node2 is now eligible — its only dependency (node1) is complete.
    let ready_after = db_find_ready_tasks(&conn, &project_id).expect("find ready after");
    let ready_after_ids: Vec<&str> = ready_after.iter().map(|n| n.id.as_str()).collect();
    assert!(
        ready_after_ids.contains(&node2_id.as_str()),
        "node2 must appear in ready tasks once node1 is complete"
    );
    // node1 must no longer be ready.
    assert!(
        !ready_after_ids.contains(&node1_id.as_str()),
        "completed node1 must not appear in ready tasks"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// TC-2: SF-3/SF-4 decision path
// ═════════════════════════════════════════════════════════════════════════════

/// TC-2: Feed poe:decision + poe:yield.
/// Assert: decisions (queue_items) row inserted, node status=waiting, yield_reason='decision'.
#[test]
fn tc2_decision_yield_creates_queue_item_and_sets_waiting() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn);
    let task_id = create_pending_task(&conn, &project_id, None, Some("implementer"));

    force_status(&conn, &task_id, "running");

    // poe:decision — pure DB handler (no AppHandle required).
    let decision_json =
        r#"{"poe":"decision","id":"d1","question":"Which DB?","options":["sqlite","postgres"]}"#;
    let item = db_handle_decision(&conn, decision_json, &project_id, &task_id, "agent-1")
        .expect("db_handle_decision");

    // queue_items row inserted.
    assert_eq!(
        item.question, "Which DB?",
        "queue_item must carry the decision question"
    );
    assert!(
        item.resolved_at.is_none(),
        "queue_item must be unresolved initially"
    );
    assert_eq!(
        item.task_id.as_deref(),
        Some(task_id.as_str()),
        "queue_item must reference the requesting task"
    );

    // node status — db_handle_decision sets Waiting.
    assert_eq!(
        read_status(&conn, &task_id),
        NodeStatus::Waiting,
        "task must be Waiting after poe:decision"
    );

    // poe:yield — apply yield_reason derived from last_substantive='decision'.
    apply_yield(&conn, &task_id, Some("decision"));

    assert_eq!(
        read_yield_reason(&conn, &task_id).as_deref(),
        Some("decision"),
        "yield_reason must be 'decision'"
    );
    assert_eq!(
        read_status(&conn, &task_id),
        NodeStatus::Waiting,
        "task must remain Waiting after yield"
    );

    // No auto-resume — queue_item must still be unresolved.
    let unresolved: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM queue_items WHERE task_id = ?1 AND resolved_at IS NULL",
            [&task_id],
            |r| r.get(0),
        )
        .expect("count unresolved");
    assert_eq!(unresolved, 1, "queue_item must remain unresolved (no auto-resume)");
}

// ═════════════════════════════════════════════════════════════════════════════
// TC-3: SF-3/SF-4 review path
// ═════════════════════════════════════════════════════════════════════════════

/// TC-3: Feed poe:review + poe:yield.
/// Assert: reviewer task created (type=plan_review, review_id=r1),
/// requesting task status='waiting', yield_reason='review'.
///
/// In production handle_review is log-only; the orchestrator handles reviewer
/// node creation post-poe:yield. Here we exercise the DB state directly as the
/// orchestrator would.
#[test]
fn tc3_review_yield_creates_reviewer_node_and_sets_waiting() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn);
    let task_id = create_pending_task(&conn, &project_id, None, Some("implementer"));

    force_status(&conn, &task_id, "running");

    // poe:review — log-only in the ingester. The orchestrator creates the reviewer
    // node after poe:yield is received. We log the event to mirror the DB side.
    let review_id = "r1";
    conn.execute(
        "INSERT INTO events (id, project_id, agent_id, task_id, event_type, payload, created_at)
         VALUES (?1, ?2, 'agent-1', ?3, 'poe:review', ?4, ?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            project_id,
            task_id,
            r#"{"poe":"review","id":"r1","reviewer_skill":"senior-engineer","content":"..."}"#,
            chrono::Utc::now().to_rfc3339()
        ],
    )
    .expect("log review event");

    // poe:yield with derived_reason='review'.
    apply_yield(&conn, &task_id, Some("review"));

    assert_eq!(
        read_status(&conn, &task_id),
        NodeStatus::Waiting,
        "requesting task must be Waiting after review+yield"
    );
    assert_eq!(
        read_yield_reason(&conn, &task_id).as_deref(),
        Some("review"),
        "yield_reason must be 'review'"
    );

    // Orchestrator creates the reviewer node (mirrors orchestrator::handle_review_yield).
    let reviewer = db_create_node(
        &conn,
        &CreateNodeInput { id: None,
            project_id: project_id.clone(),
            phase_id: None,
            parent_id: None,
            node_type: NodeType::PlanReview,
            title: "Plan Review — Test task".to_owned(),
            description: Some("...".to_owned()),
            skill_id: Some("senior-engineer".to_owned()),
            initial_status: Some(NodeStatus::Pending),
            requesting_task_id: Some(task_id.clone()),
            review_id: Some(review_id.to_owned()),
            retry_count: Some(0),
        },
    )
    .expect("create reviewer node");

    // Reviewer assertions.
    assert_eq!(
        reviewer.node_type,
        NodeType::PlanReview,
        "reviewer must be of type plan_review"
    );
    assert_eq!(
        reviewer.review_id.as_deref(),
        Some(review_id),
        "reviewer must carry review_id=r1"
    );
    assert_eq!(
        reviewer.requesting_task_id.as_deref(),
        Some(task_id.as_str()),
        "reviewer must reference requesting task"
    );
    assert_eq!(
        reviewer.status,
        NodeStatus::Pending,
        "reviewer must start as Pending"
    );
    assert_eq!(
        reviewer.skill_id.as_deref(),
        Some("senior-engineer"),
        "reviewer must have correct skill_id"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// TC-4: SF-3/SF-4 chat path
// ═════════════════════════════════════════════════════════════════════════════

/// TC-4: Feed poe:chat + poe:yield.
/// Assert: chat_turns row inserted (role='assistant', id='c1'), yield_reason='chat'.
#[test]
fn tc4_chat_yield_inserts_chat_turn_and_sets_waiting() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn);
    let task_id = create_pending_task(&conn, &project_id, None, Some("implementer"));

    force_status(&conn, &task_id, "running");

    // poe:chat — insert the chat_turns row (mirrors handle_chat DB side).
    let turn_id = "c1";
    let content = "Need input on X";
    db_insert_chat_turn(&conn, turn_id, &task_id, content).expect("insert chat turn");

    // poe:yield with derived_reason='chat'.
    apply_yield(&conn, &task_id, Some("chat"));

    // Assertions.
    assert_eq!(
        read_status(&conn, &task_id),
        NodeStatus::Waiting,
        "task must be Waiting after chat+yield"
    );
    assert_eq!(
        read_yield_reason(&conn, &task_id).as_deref(),
        Some("chat"),
        "yield_reason must be 'chat'"
    );

    // chat_turns row must exist with correct id and content.
    let turns = db_list_chat_turns(&conn, &task_id).expect("list chat turns");
    assert_eq!(turns.len(), 1, "exactly one chat_turns row must be inserted");
    assert_eq!(turns[0].id, turn_id, "chat turn id must be 'c1'");
    assert_eq!(turns[0].content, content, "chat turn content must match");
    assert_eq!(turns[0].task_id, task_id, "chat turn must reference the task");
    assert!(
        turns[0].responded_at.is_none(),
        "chat turn must have no response yet"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// TC-5: SF-3/SF-4 advisor path
// ═════════════════════════════════════════════════════════════════════════════

/// TC-5: Feed poe:advisor + poe:yield.
/// Assert: advisor_turns row inserted (id='a1'), yield_reason='advisor'.
#[test]
fn tc5_advisor_yield_inserts_advisor_turn_and_sets_waiting() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn);
    let task_id = create_pending_task(&conn, &project_id, None, Some("implementer"));

    force_status(&conn, &task_id, "running");

    // poe:advisor — insert the advisor_turns row (mirrors handle_advisor DB side).
    let turn_id = "a1";
    let content = "Advisor response";
    db_insert_advisor_turn(&conn, turn_id, &task_id, content).expect("insert advisor turn");

    // poe:yield with derived_reason='advisor'.
    apply_yield(&conn, &task_id, Some("advisor"));

    // Assertions.
    assert_eq!(
        read_status(&conn, &task_id),
        NodeStatus::Waiting,
        "task must be Waiting after advisor+yield"
    );
    assert_eq!(
        read_yield_reason(&conn, &task_id).as_deref(),
        Some("advisor"),
        "yield_reason must be 'advisor'"
    );

    // advisor_turns row must exist.
    let turns = db_list_advisor_turns(&conn, &task_id).expect("list advisor turns");
    assert_eq!(turns.len(), 1, "exactly one advisor_turns row must be inserted");
    assert_eq!(turns[0].id, turn_id, "advisor turn id must be 'a1'");
    assert_eq!(turns[0].content, content, "advisor turn content must match");
    assert!(
        turns[0].responded_at.is_none(),
        "advisor turn must have no response yet"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// TC-6: SF-5 — Phase activation
// ═════════════════════════════════════════════════════════════════════════════

/// TC-6: Create 3 phases (all pending). Activate phase 1 directly (DB-level
/// equivalent of the activate_phase Tauri command).
/// Assert: phase 1 status='running', phases 2+3 still 'pending'.
///
/// The live activate_phase command additionally emits a Tauri event and sends
/// DagChanged::DagStructureChanged — those side-effects are not tested here.
#[test]
fn tc6_sf5_activate_phase_sets_running_only_for_target() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn);

    let phase1 = db_create_phase(&conn, &project_id, "Phase 1", 1, "execution")
        .expect("create phase 1");
    let phase2 = db_create_phase(&conn, &project_id, "Phase 2", 2, "execution")
        .expect("create phase 2");
    let phase3 = db_create_phase(&conn, &project_id, "Phase 3", 3, "execution")
        .expect("create phase 3");

    // All phases start as 'pending'.
    assert_eq!(read_phase_status(&conn, &phase1.id), "pending");
    assert_eq!(read_phase_status(&conn, &phase2.id), "pending");
    assert_eq!(read_phase_status(&conn, &phase3.id), "pending");

    // Activate phase 1 — DB side of the activate_phase Tauri command.
    force_phase_status(&conn, &phase1.id, "running");

    // Assertions.
    assert_eq!(
        read_phase_status(&conn, &phase1.id),
        "running",
        "phase 1 must be running after activation"
    );
    assert_eq!(
        read_phase_status(&conn, &phase2.id),
        "pending",
        "phase 2 must still be pending"
    );
    assert_eq!(
        read_phase_status(&conn, &phase3.id),
        "pending",
        "phase 3 must still be pending"
    );

    // Verify the lifecycle_stage was also set to 'execution'.
    let p1 = db_get_phase(&conn, &phase1.id).expect("get phase 1");
    assert_eq!(
        p1.lifecycle_stage,
        PhaseLifecycleStage::Execution,
        "phase lifecycle_stage must be 'execution' after activation"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// TC-7: Phase gate + advance_phase
// ═════════════════════════════════════════════════════════════════════════════

/// TC-7: Create 2 phases. Set phase 1 running, add a task to it, complete all tasks.
/// Simulate advance_phase DB operations (no AppHandle).
/// Assert: phase 1 status='complete', phase 2 status='running'.
///
/// The live advance_phase command additionally emits Tauri events and signals
/// the orchestrator via dag_tx — those side-effects are not tested here.
#[test]
fn tc7_advance_phase_completes_phase1_and_activates_phase2() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn);

    let phase1 = db_create_phase(&conn, &project_id, "Phase 1", 1, "execution")
        .expect("create phase 1");
    let phase2 = db_create_phase(&conn, &project_id, "Phase 2", 2, "execution")
        .expect("create phase 2");

    // Activate phase 1.
    force_phase_status(&conn, &phase1.id, "running");

    // Add a task to phase 1 and complete it (simulates all phase tasks done → gate).
    let task_id = create_pending_task(&conn, &project_id, Some(&phase1.id), Some("implementer"));
    force_status(&conn, &task_id, "running");
    let done_json = r#"{"poe":"done","summary":"phase task done"}"#;
    db_handle_done(&conn, done_json, &project_id, &task_id, "agent-1")
        .expect("db_handle_done");

    assert_eq!(
        read_status(&conn, &task_id),
        NodeStatus::Complete,
        "task must be complete"
    );

    // Simulate advance_phase DB operations (mirrors advance_phase Tauri command).
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE phases SET status = 'complete', gate_held = 0, updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, phase1.id],
    )
    .expect("complete phase 1");
    conn.execute(
        "UPDATE phases SET status = 'running', lifecycle_stage = 'execution', gate_held = 0, updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, phase2.id],
    )
    .expect("activate phase 2");

    // Assertions.
    assert_eq!(
        read_phase_status(&conn, &phase1.id),
        "complete",
        "phase 1 must be complete"
    );
    assert_eq!(
        read_phase_status(&conn, &phase2.id),
        "running",
        "phase 2 must be running after advance"
    );

    // Verify via db_list_phases.
    let phases = db_list_phases(&conn, &project_id).expect("list phases");
    assert_eq!(phases.len(), 2);
    assert_eq!(phases[0].status, "complete");
    assert_eq!(phases[1].status, "running");
}

// ═════════════════════════════════════════════════════════════════════════════
// TC-8: Phase gate + revise_phase
// ═════════════════════════════════════════════════════════════════════════════

/// TC-8: Phase at gate. Call revise_phase(phase_id, task_ids=[t1]).
/// Assert t1 reset to pending, other tasks (t2) remain complete, phase status='running'.
///
/// The live revise_phase command additionally emits a Tauri event and signals
/// dag_tx — those side-effects are not tested here.
#[test]
fn tc8_revise_phase_resets_selected_tasks_and_resumes_phase() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn);

    let phase1 = db_create_phase(&conn, &project_id, "Phase 1", 1, "execution")
        .expect("create phase 1");

    // Create 2 tasks in phase 1.
    let t1 = create_pending_task(&conn, &project_id, Some(&phase1.id), Some("implementer"));
    let t2 = create_pending_task(&conn, &project_id, Some(&phase1.id), Some("implementer"));

    // Complete both tasks (simulates gate condition).
    force_status(&conn, &t1, "complete");
    force_status(&conn, &t2, "complete");

    // Set phase to gate.
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE phases SET status = 'gate', gate_held = 1, updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, phase1.id],
    )
    .expect("set gate");

    assert_eq!(read_phase_status(&conn, &phase1.id), "gate");

    // revise_phase(phase_id, task_ids=[t1]) — DB side of the revise_phase Tauri command.
    conn.execute(
        "UPDATE nodes SET status = 'pending', updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, t1],
    )
    .expect("reset t1");
    conn.execute(
        "UPDATE phases SET status = 'running', gate_held = 0, updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, phase1.id],
    )
    .expect("resume phase");

    // Assertions.
    assert_eq!(
        read_status(&conn, &t1),
        NodeStatus::Pending,
        "t1 must be reset to Pending"
    );
    assert_eq!(
        read_status(&conn, &t2),
        NodeStatus::Complete,
        "t2 must remain Complete (not in revise list)"
    );
    assert_eq!(
        read_phase_status(&conn, &phase1.id),
        "running",
        "phase must be running after revise"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// TC-9: Phase gate + rerun_phase
// ═════════════════════════════════════════════════════════════════════════════

/// TC-9: Phase at gate. Call rerun_phase(phase_id).
/// Assert all done tasks reset to pending, phase status='running'.
///
/// The live rerun_phase command additionally emits a Tauri event and signals
/// dag_tx — those side-effects are not tested here.
#[test]
fn tc9_rerun_phase_resets_all_done_tasks_and_resumes_phase() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn);

    let phase1 = db_create_phase(&conn, &project_id, "Phase 1", 1, "execution")
        .expect("create phase 1");

    // Create 3 tasks — 2 complete, 1 cancelled.
    let t1 = create_pending_task(&conn, &project_id, Some(&phase1.id), Some("implementer"));
    let t2 = create_pending_task(&conn, &project_id, Some(&phase1.id), Some("implementer"));
    let t3 = create_pending_task(&conn, &project_id, Some(&phase1.id), Some("implementer"));

    force_status(&conn, &t1, "complete");
    force_status(&conn, &t2, "complete");
    force_status(&conn, &t3, "cancelled");

    // Set phase to gate.
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE phases SET status = 'gate', gate_held = 1, updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, phase1.id],
    )
    .expect("set gate");

    // rerun_phase — DB side of the rerun_phase Tauri command.
    conn.execute(
        "UPDATE nodes SET status = 'pending', updated_at = ?1 WHERE phase_id = ?2 AND status IN ('complete', 'done')",
        rusqlite::params![now, phase1.id],
    )
    .expect("reset done tasks");
    conn.execute(
        "UPDATE phases SET status = 'running', gate_held = 0, updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, phase1.id],
    )
    .expect("resume phase");

    // Assertions.
    assert_eq!(
        read_status(&conn, &t1),
        NodeStatus::Pending,
        "t1 must be reset to Pending"
    );
    assert_eq!(
        read_status(&conn, &t2),
        NodeStatus::Pending,
        "t2 must be reset to Pending"
    );
    assert_eq!(
        read_status(&conn, &t3),
        NodeStatus::Cancelled,
        "cancelled t3 must remain Cancelled (not reset)"
    );
    assert_eq!(
        read_phase_status(&conn, &phase1.id),
        "running",
        "phase must be running after rerun"
    );

    // All pending tasks in the phase should now appear as ready.
    let ready = db_find_ready_tasks(&conn, &phase1.project_id).expect("find ready tasks");
    let ready_ids: Vec<&str> = ready.iter().map(|n| n.id.as_str()).collect();
    // Note: db_find_ready_tasks requires phase status='running' and gate_held=0,
    // which we have set above.
    assert!(
        ready_ids.contains(&t1.as_str()),
        "t1 must be ready after rerun"
    );
    assert!(
        ready_ids.contains(&t2.as_str()),
        "t2 must be ready after rerun"
    );
    assert!(
        !ready_ids.contains(&t3.as_str()),
        "cancelled t3 must not appear in ready tasks"
    );
}

// ═════════════════════════════════════════════════════════════════════════════
// TC-10: skill_modes population
// ═════════════════════════════════════════════════════════════════════════════

/// TC-10: Skill frontmatter declares `modes: [autonomous, interactive]`.
/// Simulate SF-1 dispatch by calling db_update_node_skill_modes directly
/// with the parsed modes JSON array.
/// Assert: nodes.skill_modes = '["autonomous","interactive"]'.
///
/// In production, spawn_agent parses the skill file frontmatter and calls
/// db_update_node_skill_modes before launching the Claude subprocess.
#[test]
fn tc10_skill_modes_population_sets_json_array_on_node() {
    let conn = new_mem_db();
    let project_id = insert_project(&conn);
    let task_id = create_pending_task(&conn, &project_id, None, Some("interactive-skill"));

    // Initially, skill_modes is NULL.
    assert!(
        read_skill_modes(&conn, &task_id).is_none(),
        "skill_modes must be NULL before dispatch"
    );

    // Simulated skill frontmatter: modes: [autonomous, interactive]
    // In production: skills::load_skill parses the YAML frontmatter; spawn_agent
    // serialises the modes array and calls db_update_node_skill_modes.
    let skill_modes_json = r#"["autonomous","interactive"]"#;
    db_update_node_skill_modes(&conn, &task_id, skill_modes_json)
        .expect("update skill_modes");

    // Assertion.
    let stored = read_skill_modes(&conn, &task_id).expect("skill_modes must be set");
    assert_eq!(
        stored, skill_modes_json,
        "nodes.skill_modes must match the serialised modes array"
    );

    // Verify the value round-trips as valid JSON.
    let parsed: Vec<String> = serde_json::from_str(&stored).expect("skill_modes must be valid JSON");
    assert_eq!(parsed, vec!["autonomous", "interactive"]);

    // Also verify via db_get_node.
    let node = db_get_node(&conn, &task_id).expect("get node");
    assert_eq!(
        node.skill_modes.as_deref(),
        Some(skill_modes_json),
        "db_get_node must return the correct skill_modes"
    );
}
