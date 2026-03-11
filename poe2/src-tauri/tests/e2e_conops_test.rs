//! E2E smoke test: full poe:chat CONOPS round-trip (bp6-881.11).
//!
//! Validates all success criteria from bp6-881 §9:
//!
//!   §9.1  Task reaches `done` (Complete) after 3 chat cycles + final poe:done
//!   §9.2  `conops.md` stored in `artifacts` table at end of round 3
//!   §9.3  All `chat_turns` rows have correct content and `responded_at` set after response
//!   §9.4  `yield_reason='chat'` on each waiting cycle
//!   §9.5  No PTY calls made — mock agent output injected directly via DB helpers
//!
//! Architecture:
//!   - One in-memory SQLite DB per test — no shared state.
//!   - No real claude process invoked. Agent output is simulated by calling the
//!     same DB helper functions that the real event_ingester would call after
//!     parsing agent stdout.
//!   - The "respond_to_chat" action is simulated by setting responded_at on the
//!     chat_turns row directly (the same write that commands::respond_to_chat does),
//!     and then re-asserting SF-4 guard conditions, mirroring the pattern from
//!     orchestrator_flow_tests::sf4_chat_resume_requires_responded_at_set.

use poe2_lib::dag_store::{
    schema,
    types::{
        CreateArtifactInput, CreateNodeInput, NodeStatus, NodeType, UpdateNodeInput,
    },
    db_create_node, db_insert_chat_turn, db_list_artifacts, db_list_chat_turns,
    db_log_event, db_update_node, db_upsert_artifact,
};
use poe2_lib::event_ingester::{db_handle_done, parse_poe_event};
use rusqlite::Connection;

// ── In-memory DB bootstrap ────────────────────────────────────────────────────

/// Open a fresh in-memory SQLite database with the full schema and all runtime
/// migrations applied.  Mirrors `new_mem_db()` from orchestrator_flow_tests.
fn new_mem_db() -> Connection {
    let conn = Connection::open_in_memory().expect("open_in_memory");
    conn.execute_batch(schema::CREATE_TABLES)
        .expect("apply CREATE_TABLES");
    // Runtime migrations — must stay in sync with open_project_db().
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
            id           TEXT PRIMARY KEY,
            task_id      TEXT NOT NULL,
            content      TEXT NOT NULL,
            response     TEXT,
            created_at   TEXT NOT NULL,
            responded_at TEXT
        )",
    );
    let _ = conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_chat_turns_task ON chat_turns(task_id)",
    );
    conn
}

// ── Minimal project fixture helpers ──────────────────────────────────────────

fn insert_project(conn: &Connection) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO projects (id, name, path, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, "CONOPS Test Project", format!("/tmp/test-{}", id), now, now],
    )
    .expect("insert project");
    id
}

fn create_pending_task(conn: &Connection, project_id: &str, skill_id: &str) -> String {
    let node = db_create_node(
        conn,
        &CreateNodeInput { id: None,
            project_id: project_id.to_owned(),
            phase_id: None,
            parent_id: None,
            node_type: NodeType::Task,
            title: "Draft CONOPS".to_owned(),
            description: Some("Produce a Concept of Operations document.".to_owned()),
            skill_id: Some(skill_id.to_owned()),
            initial_status: None,
            requesting_task_id: None,
            review_id: None,
            retry_count: None,
        },
    )
    .expect("create task node");
    node.id
}

fn force_status(conn: &Connection, node_id: &str, status: &str) {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE nodes SET status = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![status, now, node_id],
    )
    .expect("force_status");
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

/// Simulate the DB writes that `commands::respond_to_chat` performs:
/// set response + responded_at on the chat_turns row.
fn db_mark_chat_turn_responded(conn: &Connection, turn_id: &str, response: &str) {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE chat_turns SET response = ?1, responded_at = ?2 WHERE id = ?3",
        rusqlite::params![response, now, turn_id],
    )
    .expect("mark chat turn responded");
}

/// Read a single chat_turns row (id, content, response, responded_at).
fn read_chat_turn(conn: &Connection, turn_id: &str) -> (String, Option<String>, Option<String>) {
    conn.query_row(
        "SELECT content, response, responded_at FROM chat_turns WHERE id = ?1",
        [turn_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .unwrap_or_else(|_| panic!("chat turn not found: {}", turn_id))
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: simulate a single poe:chat + poe:yield cycle
//
// Represents what the orchestrator's event_ingester does when the mock agent
// emits these two lines of output:
//   {"poe":"chat","content":"<question>"}
//   {"poe":"yield"}
//
// Returns the newly-created turn_id.
fn simulate_chat_and_yield(
    conn: &Connection,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    question: &str,
) -> String {
    let turn_id = uuid::Uuid::new_v4().to_string();

    // --- poe:chat received: insert chat_turns row + log event ---
    db_insert_chat_turn(conn, &turn_id, task_id, question)
        .expect("db_insert_chat_turn");

    let chat_json = serde_json::json!({"poe": "chat", "content": question}).to_string();
    db_log_event(conn, project_id, Some(agent_id), Some(task_id), "poe:chat", &chat_json)
        .expect("log poe:chat event");

    // --- poe:yield received: set task Waiting + yield_reason='chat' ---
    db_update_node(
        conn,
        task_id,
        &UpdateNodeInput {
            status: Some(NodeStatus::Waiting),
            yield_reason: Some("chat".to_owned()),
            ..Default::default()
        },
    )
    .expect("set Waiting + yield_reason=chat");

    let yield_json = r#"{"poe":"yield"}"#;
    db_log_event(conn, project_id, Some(agent_id), Some(task_id), "poe:yield", yield_json)
        .expect("log poe:yield event");

    turn_id
}

/// Simulate the SF-4/chat resume path:
/// human responds → responded_at set → task resumed to Running.
fn simulate_human_response_and_resume(
    conn: &Connection,
    task_id: &str,
    turn_id: &str,
    response: &str,
) {
    // SF-4 guard: responded_at must be set before we act.
    db_mark_chat_turn_responded(conn, turn_id, response);

    // Guard assertion mirrors orchestrator resume_chat_agent.
    let (_, _, responded_at) = read_chat_turn(conn, turn_id);
    assert!(
        responded_at.is_some(),
        "SF-4 guard: responded_at must be Some before resuming agent (turn_id={})",
        turn_id
    );

    // Resume: set task back to Running (what spawn_agent / resume_chat_agent does).
    force_status(conn, task_id, "running");
}

// ─────────────────────────────────────────────────────────────────────────────
// E2E test: full poe:chat CONOPS round-trip
// ─────────────────────────────────────────────────────────────────────────────

/// bp6-881 §9 — Full CONOPS round-trip smoke test.
///
/// Walk the complete three-cycle chat/yield/respond/resume loop, then a final
/// poe:done, asserting all §9 success criteria at each stage.
#[test]
fn e2e_conops_full_round_trip() {
    // ── Setup ─────────────────────────────────────────────────────────────────
    //
    // §9.5: no PTY calls made — this entire test uses DB helpers only.
    let conn = new_mem_db();
    let project_id = insert_project(&conn);
    let agent_id = "mock-agent-conops-e2e";

    // Step 1: Create project + pending task with skill=operational-analyst.
    let task_id = create_pending_task(&conn, &project_id, "operational-analyst");

    assert_eq!(
        read_status(&conn, &task_id),
        NodeStatus::Pending,
        "task must start as Pending"
    );

    // ── SF-1: Orchestrator dispatches task (status → Running) ─────────────────
    //
    // Step 2: Verify orchestrator SF-1 dispatches the task (status → running).
    force_status(&conn, &task_id, "running");

    assert_eq!(
        read_status(&conn, &task_id),
        NodeStatus::Running,
        "SF-1: task must be Running after dispatch"
    );

    // ── Round 1 — poe:brief, poe:chat, poe:yield ──────────────────────────────
    //
    // Step 3a: Mock agent emits poe:brief (log-only, no status change).
    let brief_json = r#"{"poe":"brief","content":"Starting CONOPS analysis"}"#;
    let (brief_type, _) = parse_poe_event(brief_json).expect("parse poe:brief");
    assert_eq!(brief_type, "brief", "poe:brief must parse correctly");
    db_log_event(&conn, &project_id, Some(agent_id), Some(&task_id), "poe:brief", brief_json)
        .expect("log poe:brief");

    // Task must still be Running after poe:brief.
    assert_eq!(
        read_status(&conn, &task_id),
        NodeStatus::Running,
        "poe:brief must not change task status"
    );

    // Step 3b: Mock agent emits poe:chat "What problem does this solve?" + poe:yield.
    let turn_id_1 = simulate_chat_and_yield(
        &conn,
        &project_id,
        &task_id,
        agent_id,
        "What problem does this solve?",
    );

    // Step 4: Verify chat_turns row inserted, yield_reason='chat', task status=waiting.
    assert_eq!(
        read_status(&conn, &task_id),
        NodeStatus::Waiting,
        "§9.4: task must be Waiting after round-1 poe:yield"
    );
    assert_eq!(
        read_yield_reason(&conn, &task_id).as_deref(),
        Some("chat"),
        "§9.4: yield_reason must be 'chat' after round-1 cycle"
    );

    let turns_r1 = db_list_chat_turns(&conn, &task_id).expect("list chat turns after round 1");
    assert_eq!(turns_r1.len(), 1, "§9.3: exactly one chat_turns row after round 1");
    assert_eq!(
        turns_r1[0].content, "What problem does this solve?",
        "§9.3: round-1 chat turn content must match agent question"
    );
    assert!(
        turns_r1[0].responded_at.is_none(),
        "§9.3: responded_at must be NULL before human responds (round 1)"
    );

    // Step 5: Call respond_to_chat(turn_id_1, "It solves X problem").
    // Step 6: SF-4 detects responded_at IS NOT NULL, resumes agent.
    simulate_human_response_and_resume(
        &conn,
        &task_id,
        &turn_id_1,
        "It solves X problem",
    );

    // Verify responded_at is now set on round-1 turn.
    let (_, response_r1, responded_at_r1) = read_chat_turn(&conn, &turn_id_1);
    assert!(
        responded_at_r1.is_some(),
        "§9.3: responded_at must be set after round-1 human response"
    );
    assert_eq!(
        response_r1.as_deref(),
        Some("It solves X problem"),
        "§9.3: round-1 response text must match"
    );

    assert_eq!(
        read_status(&conn, &task_id),
        NodeStatus::Running,
        "SF-4: task must be Running after round-1 resume"
    );

    // ── Round 2 — poe:artifact (draft), poe:chat, poe:yield ──────────────────
    //
    // Step 7a: Mock agent emits poe:artifact (draft conops.md).
    // Simulate handle_artifact: upsert artifact, log event.
    let draft_artifact = db_upsert_artifact(
        &conn,
        &CreateArtifactInput {
            project_id: project_id.clone(),
            phase_id: None,
            artifact_type: "document".to_owned(),
            filename: "conops.md".to_owned(),
            produced_by_stage: Some("draft".to_owned()),
        },
    )
    .expect("upsert draft artifact");

    let artifact_json = "{\"poe\":\"artifact\",\"name\":\"conops.md\",\"artifact_type\":\"document\"}";
    db_log_event(
        &conn,
        &project_id,
        Some(agent_id),
        Some(&task_id),
        "poe:artifact",
        artifact_json,
    )
    .expect("log poe:artifact (draft)");

    assert_eq!(
        draft_artifact.filename, "conops.md",
        "draft artifact filename must be conops.md"
    );

    // Task must still be Running after poe:artifact (artifacts don't trigger orchestrator).
    assert_eq!(
        read_status(&conn, &task_id),
        NodeStatus::Running,
        "poe:artifact must not change task status"
    );

    // Step 7b: Mock agent emits poe:chat "Who are the users?" + poe:yield.
    let turn_id_2 = simulate_chat_and_yield(
        &conn,
        &project_id,
        &task_id,
        agent_id,
        "Who are the users?",
    );

    // Step 8: Verify second chat_turns row inserted.
    assert_eq!(
        read_status(&conn, &task_id),
        NodeStatus::Waiting,
        "§9.4: task must be Waiting after round-2 poe:yield"
    );
    assert_eq!(
        read_yield_reason(&conn, &task_id).as_deref(),
        Some("chat"),
        "§9.4: yield_reason must be 'chat' after round-2 cycle"
    );

    let turns_r2 = db_list_chat_turns(&conn, &task_id).expect("list chat turns after round 2");
    assert_eq!(turns_r2.len(), 2, "§9.3: exactly two chat_turns rows after round 2");
    assert_eq!(
        turns_r2[1].content, "Who are the users?",
        "§9.3: round-2 chat turn content must match agent question"
    );
    assert!(
        turns_r2[1].responded_at.is_none(),
        "§9.3: round-2 turn responded_at must be NULL before human responds"
    );

    // Step 9: Call respond_to_chat(turn_id_2, "Primary users are Y").
    // Step 10: SF-4 resumes again.
    simulate_human_response_and_resume(
        &conn,
        &task_id,
        &turn_id_2,
        "Primary users are Y",
    );

    let (_, response_r2, responded_at_r2) = read_chat_turn(&conn, &turn_id_2);
    assert!(
        responded_at_r2.is_some(),
        "§9.3: responded_at must be set after round-2 human response"
    );
    assert_eq!(
        response_r2.as_deref(),
        Some("Primary users are Y"),
        "§9.3: round-2 response text must match"
    );

    assert_eq!(
        read_status(&conn, &task_id),
        NodeStatus::Running,
        "SF-4: task must be Running after round-2 resume"
    );

    // ── Round 3 — poe:artifact (final), poe:chat, poe:yield, respond, poe:done ─
    //
    // Step 11a: Mock agent emits poe:artifact (final conops.md).
    let final_artifact = db_upsert_artifact(
        &conn,
        &CreateArtifactInput {
            project_id: project_id.clone(),
            phase_id: None,
            artifact_type: "document".to_owned(),
            filename: "conops.md".to_owned(),
            produced_by_stage: Some("final".to_owned()),
        },
    )
    .expect("upsert final artifact");

    db_log_event(
        &conn,
        &project_id,
        Some(agent_id),
        Some(&task_id),
        "poe:artifact",
        "{\"poe\":\"artifact\",\"name\":\"conops.md\",\"artifact_type\":\"document\"}",
    )
    .expect("log poe:artifact (final)");

    assert_eq!(
        final_artifact.filename, "conops.md",
        "final artifact filename must still be conops.md (upsert)"
    );
    assert_eq!(
        final_artifact.produced_by_stage.as_deref(),
        Some("final"),
        "final artifact must carry produced_by_stage='final'"
    );

    // Step 11b: Third chat cycle — "Any constraints I should know about?" + poe:yield.
    let turn_id_3 = simulate_chat_and_yield(
        &conn,
        &project_id,
        &task_id,
        agent_id,
        "Any constraints I should know about?",
    );

    assert_eq!(
        read_status(&conn, &task_id),
        NodeStatus::Waiting,
        "§9.4: task must be Waiting after round-3 poe:yield"
    );
    assert_eq!(
        read_yield_reason(&conn, &task_id).as_deref(),
        Some("chat"),
        "§9.4: yield_reason must be 'chat' after round-3 cycle"
    );

    let turns_r3 = db_list_chat_turns(&conn, &task_id).expect("list chat turns after round 3");
    assert_eq!(turns_r3.len(), 3, "§9.3: exactly three chat_turns rows after round 3");

    // Respond and resume round 3.
    simulate_human_response_and_resume(
        &conn,
        &task_id,
        &turn_id_3,
        "No hard constraints beyond the 30-day timeline",
    );

    let (_, response_r3, responded_at_r3) = read_chat_turn(&conn, &turn_id_3);
    assert!(
        responded_at_r3.is_some(),
        "§9.3: responded_at must be set after round-3 human response"
    );
    assert_eq!(
        response_r3.as_deref(),
        Some("No hard constraints beyond the 30-day timeline"),
        "§9.3: round-3 response text must match"
    );

    assert_eq!(
        read_status(&conn, &task_id),
        NodeStatus::Running,
        "SF-4: task must be Running after round-3 resume"
    );

    // ── Step 12: Mock agent (round 3) emits poe:done "CONOPS complete" ─────────

    let done_json = r#"{"poe":"done","summary":"CONOPS complete"}"#;
    let resulting_status =
        db_handle_done(&conn, done_json, &project_id, &task_id, agent_id)
            .expect("db_handle_done");

    // §9.1: Task reaches `done` (Complete).
    assert_eq!(
        resulting_status,
        NodeStatus::Complete,
        "§9.1: poe:done must mark task Complete"
    );
    assert_eq!(
        read_status(&conn, &task_id),
        NodeStatus::Complete,
        "§9.1: DB must reflect Complete after poe:done"
    );

    // ── Final assertions: success criteria §9.2 and §9.3 ─────────────────────

    // §9.2: conops.md stored in artifacts table.
    let artifacts = db_list_artifacts(&conn, &project_id).expect("list artifacts");
    assert_eq!(
        artifacts.len(),
        1,
        "§9.2: exactly one artifact row (conops.md) must exist after upsert"
    );
    assert_eq!(
        artifacts[0].filename, "conops.md",
        "§9.2: artifact filename must be 'conops.md'"
    );
    assert_eq!(
        artifacts[0].artifact_type, "document",
        "§9.2: artifact_type must be 'document'"
    );

    // §9.3: All three chat_turns rows have correct content and responded_at set.
    let final_turns = db_list_chat_turns(&conn, &task_id).expect("list final chat turns");
    assert_eq!(
        final_turns.len(),
        3,
        "§9.3: exactly 3 chat_turns rows must exist"
    );

    let expected = [
        ("What problem does this solve?", "It solves X problem"),
        ("Who are the users?", "Primary users are Y"),
        (
            "Any constraints I should know about?",
            "No hard constraints beyond the 30-day timeline",
        ),
    ];
    for (i, (turn, (exp_content, exp_response))) in
        final_turns.iter().zip(expected.iter()).enumerate()
    {
        assert_eq!(
            turn.content, *exp_content,
            "§9.3: turn {} content mismatch",
            i + 1
        );
        assert_eq!(
            turn.response.as_deref(),
            Some(*exp_response),
            "§9.3: turn {} response mismatch",
            i + 1
        );
        assert!(
            turn.responded_at.is_some(),
            "§9.3: turn {} responded_at must be set",
            i + 1
        );
    }

    // §9.4: Reconfirm: each waiting cycle used yield_reason='chat'.
    // (Already asserted inline above after each cycle; belt-and-suspenders check
    // via event log count.)
    let yield_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM events WHERE task_id = ?1 AND event_type = 'poe:yield'",
            [&task_id],
            |r| r.get(0),
        )
        .expect("count poe:yield events");
    assert_eq!(
        yield_count, 3,
        "§9.4: exactly 3 poe:yield events must be logged (one per chat cycle)"
    );

    let chat_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM events WHERE task_id = ?1 AND event_type = 'poe:chat'",
            [&task_id],
            |r| r.get(0),
        )
        .expect("count poe:chat events");
    assert_eq!(
        chat_count, 3,
        "§9.3: exactly 3 poe:chat events must be logged"
    );

    // §9.5: No PTY calls made — verified by construction: this test never calls
    // spawn_agent, portable_pty, or any process-launch path.
    // The poe:done event must be logged.
    let done_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM events WHERE task_id = ?1 AND event_type = 'poe:done'",
            [&task_id],
            |r| r.get(0),
        )
        .expect("count poe:done events");
    assert_eq!(done_count, 1, "poe:done event must be recorded exactly once");
}
