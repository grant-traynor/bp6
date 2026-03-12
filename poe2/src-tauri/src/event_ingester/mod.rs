use crate::dag_store::{
    self, CreateArtifactInput, CreateKnowledgeInput, CreateNodeInput, EdgeType, NodeStatus,
    NodeType, ProjectRegistry, UpdateNodeInput,
};
use crate::event_sink::EventSink;
use anyhow::Result;
use std::path::Path;
use tokio::sync::mpsc;

/// Signal sent to the orchestrator when DAG state changes.
#[derive(Debug, Clone)]
pub enum DagChanged {
    NodeStatusChanged { project_id: String, node_id: String },
    DagStructureChanged { project_id: String },
    /// Sent once when a project is first opened. Triggers ghost-agent recovery
    /// before the normal scheduling loop runs.
    ProjectOpened { project_id: String },
    QueueItemResolved {
        project_id: String,
        item_id: String,
        task_id: String,
        session_id: String,
        resolution: String,
        /// Which turn table owns `item_id`: "decision", "chat", or "advisor".
        /// Set at emit time to avoid DB COUNT(*) probes in resume_waiting_agent().
        turn_type: String,
    },
}

// ── poe: event payloads (Protocol.md §2 wire format) ─────────────────────────

#[derive(Debug, serde::Deserialize)]
struct PoeTask {
    id: String,
    title: String,
    description: Option<String>,
    skill: Option<String>,
    #[serde(rename = "type")]
    node_type: Option<String>,
    parent_id: Option<String>,
    depends_on: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize)]
struct PoeTaskUpdate {
    id: String,
    title: Option<String>,
    description: Option<String>,
    skill: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct PoeTaskCancel {
    id: String,
    reason: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct PoeEdge {
    from: String,
    to: String,
}

#[derive(Debug, serde::Deserialize)]
struct PoeEdgeRemove {
    from: String,
    to: String,
}

#[derive(Debug, serde::Deserialize)]
struct PoeArtifact {
    name: String,
    #[serde(default = "default_artifact_type")]
    artifact_type: String,
    // content field intentionally absent — agents write files directly using their own tools
    // (Write/Edit/Bash). If old agents send content it is silently dropped by serde.
}

fn default_artifact_type() -> String {
    "document".to_string()
}

#[derive(Debug, serde::Deserialize)]
struct PoeKnowledge {
    key: String,
    /// Wire field is `content`; DB field is `value`.
    content: String,
    supersedes: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct PoeDecision {
    question: String,
    options: Option<Vec<String>>,
}

#[derive(Debug, serde::Deserialize)]
struct PoeDone {
    summary: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct PoeYield {}

#[derive(Debug, serde::Deserialize)]
struct PoeChat {
    content: String,
    id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct PoeAdvisor {
    content: String,
    id: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct PoeReview {
    reviewer_skill: String,
    content: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct PoeSkill {
    name: String,
    content: String,
}

// ── Core ingestion ────────────────────────────────────────────────────────────

/// Detect and classify a single line as a poe: event.
///
/// Returns `Some((event_type, json_value))` if the line is valid JSON containing
/// a `"poe"` key. Returns `None` for non-JSON or non-poe lines.
///
/// Pure function — no Tauri, no SQLite, safe to call from tests.
pub fn parse_poe_event(line: &str) -> Option<(String, serde_json::Value)> {
    let json = serde_json::from_str::<serde_json::Value>(line.trim()).ok()?;
    let event_type = json.get("poe")?.as_str()?.to_owned();
    Some((event_type, json))
}

/// Process a single clean line from an agent.
///
/// Lines arrive pre-stripped (ANSI removal is handled upstream by TextBufExtractor).
/// A line is a poe: event iff it parses as valid JSON and contains a `"poe"` key.
/// All other lines are passed through — the caller handles those.
///
/// `last_substantive_event` tracks the most recent "substantive" poe: event
/// (decision | chat | review) emitted in this session so that `poe:yield` can
/// derive `yield_reason` without a wire field.
///
/// # Warning — test-only footgun
///
/// This function resets `last_substantive` to `None` on every call, silently
/// breaking `yield_reason` derivation for multi-line agent sessions. Production
/// code **must** use `ingest_line_with_tracker()` and manage the tracker across
/// calls. This function is compiled only in `#[cfg(test)]` builds to prevent
/// accidental use in production paths.
#[cfg(test)]
pub fn ingest_line(
    line: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    sink: &dyn EventSink,
    project_path: &Path,
) {
    // Per-call mutable tracker — callers that need cross-line tracking should
    // use ingest_line_with_tracker instead. This function resets to None each
    // call, preserving backward compatibility for single-line event dispatch.
    let mut last_substantive: Option<String> = None;
    ingest_line_with_tracker(
        line, project_id, task_id, agent_id, registry, dag_tx, sink, project_path,
        &mut last_substantive,
    );
}

/// Variant of `ingest_line` that accepts an external last-substantive-event tracker.
///
/// The caller owns the `last_substantive` value across multiple calls for the
/// same agent session, allowing `poe:yield` to derive `yield_reason` correctly
/// even when the substantive event and `poe:yield` arrive on separate lines.
pub fn ingest_line_with_tracker(
    line: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    sink: &dyn EventSink,
    project_path: &Path,
    last_substantive: &mut Option<String>,
) {
    let trimmed = line.trim();
    let Some((poe_event, _)) = parse_poe_event(trimmed) else {
        return; // not a poe: event — raw PTY output, caller handles it
    };
    let poe_event = poe_event.as_str();

    let result = match poe_event {
        "task" => handle_task(trimmed, project_id, task_id, agent_id, registry, dag_tx, sink),
        "task:update" => handle_task_update(trimmed, project_id, task_id, agent_id, registry, dag_tx, sink),
        "task:cancel" => handle_task_cancel(trimmed, project_id, task_id, agent_id, registry, dag_tx, sink),
        "edge" => handle_edge(trimmed, project_id, task_id, agent_id, registry, dag_tx, sink),
        "edge:remove" => handle_edge_remove(trimmed, project_id, task_id, agent_id, registry, dag_tx, sink),
        "artifact" => handle_artifact(trimmed, project_id, task_id, agent_id, registry, sink),
        "skill" => handle_skill(trimmed, project_id, task_id, agent_id, registry, sink, project_path),
        "knowledge" => handle_knowledge(trimmed, project_id, task_id, agent_id, registry, sink),
        "brief" => handle_log_only("poe:brief", trimmed, project_id, task_id, agent_id, registry, sink),
        "step" => handle_log_only("poe:step", trimmed, project_id, task_id, agent_id, registry, sink),
        "decision" => {
            *last_substantive = Some("decision".to_owned());
            handle_decision(trimmed, project_id, task_id, agent_id, registry, sink)
        }
        "chat" => {
            *last_substantive = Some("chat".to_owned());
            handle_chat(trimmed, project_id, task_id, agent_id, registry, dag_tx, sink)
        }
        "advisor" => {
            *last_substantive = Some("advisor".to_owned());
            handle_advisor(trimmed, project_id, task_id, agent_id, registry, dag_tx, sink)
        }
        "done" => handle_done(trimmed, project_id, task_id, agent_id, registry, dag_tx, sink),
        "yield" => {
            let derived_reason = last_substantive.clone();
            handle_yield(trimmed, project_id, task_id, agent_id, registry, dag_tx, sink, derived_reason)
        }
        "review" => {
            *last_substantive = Some("review".to_owned());
            handle_review(trimmed, project_id, task_id, agent_id, registry, dag_tx, sink)
        }
        other => {
            eprintln!("[event_ingester] Unknown poe: event type: {}", other);
            Ok(None)
        }
    };

    if let Err(e) = result {
        eprintln!("[event_ingester] Error processing poe:{}: {}", poe_event, e);
        // Emit a Tauri warning event for structured poe event failures so the
        // frontend can surface them in the UI (e.g. activity feed warnings).
        match poe_event {
            "task" | "edge" | "decision" | "skill" => {
                emit_tauri_event(
                    sink,
                    "poe-ingester-warning",
                    &serde_json::json!({
                        "taskId": task_id,
                        "agentId": agent_id,
                        "eventType": format!("poe:{}", poe_event),
                        "error": e.to_string()
                    }),
                );
            }
            _ => {}
        }
    }
}

fn emit_tauri_event(sink: &dyn EventSink, event: &str, payload: &impl serde::Serialize) {
    crate::event_sink::emit_event(sink, event, payload);
}

// ── Handlers ──────────────────────────────────────────────────────────────────

fn handle_task(
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    sink: &dyn EventSink,
) -> Result<Option<DagChanged>> {
    let payload: PoeTask = serde_json::from_str(json)?;
    let node_type = payload
        .node_type
        .as_deref()
        .unwrap_or("task")
        .parse::<NodeType>()
        .unwrap_or(NodeType::Task);

    let input = CreateNodeInput {
        project_id: project_id.to_string(),
        id: Some(payload.id.clone()),
        phase_id: None,
        parent_id: payload.parent_id.clone(),
        node_type,
        title: payload.title.clone(),
        description: payload.description.clone(),
        skill_id: payload.skill.clone(),
        initial_status: None,
        requesting_task_id: None,
        review_id: None,
        retry_count: None,
    };

    let mut pending_events: Vec<(String, serde_json::Value)> = Vec::new();
    let result = with_project_conn(registry, project_id, |conn| {
        // Node creation is fatal — propagate with `?`.
        let node = dag_store::db_create_node(conn, &input)?;
        // Finish-to-start edges: dep must finish before node can start.
        // from=dep, to=node — dep is the prerequisite, node is the dependent.
        // Edge wiring is best-effort: a missing dep_id should not prevent the
        // node from being registered or the DagChanged signal from firing.
        if let Some(ref deps) = payload.depends_on {
            for dep_id in deps {
                if let Err(e) = dag_store::db_create_edge(conn, dep_id, &node.id, EdgeType::DependsOn) {
                    eprintln!("[event_ingester] handle_task: dep edge {dep_id} → {} skipped: {e}", node.id);
                }
            }
        }
        dag_store::db_log_event(conn, project_id, Some(agent_id), Some(task_id), "poe:task", json)?;
        pending_events.push(("poe-task-created".to_string(), serde_json::to_value(&node)?));
        let change = DagChanged::DagStructureChanged {
            project_id: project_id.to_string(),
        };
        let _ = dag_tx.send(change.clone());
        Ok(Some(change))
    });
    for (event, payload) in pending_events {
        emit_tauri_event(sink, &event, &payload);
    }
    result
}

fn handle_task_update(
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    sink: &dyn EventSink,
) -> Result<Option<DagChanged>> {
    let payload: PoeTaskUpdate = serde_json::from_str(json)?;
    let input = UpdateNodeInput {
        title: payload.title,
        description: payload.description,
        status: None,
        skill_id: payload.skill,
        assignee: None,
        ..Default::default()
    };

    let mut pending_events: Vec<(String, serde_json::Value)> = Vec::new();
    let result = with_project_conn(registry, project_id, |conn| {
        let node = dag_store::db_update_node(conn, &payload.id, &input)?;
        dag_store::db_log_event(conn, project_id, Some(agent_id), Some(task_id), "poe:task:update", json)?;
        pending_events.push(("poe-node-updated".to_string(), serde_json::to_value(&node)?));
        let change = DagChanged::DagStructureChanged {
            project_id: project_id.to_string(),
        };
        let _ = dag_tx.send(change.clone());
        Ok(Some(change))
    });
    for (event, payload) in pending_events {
        emit_tauri_event(sink, &event, &payload);
    }
    result
}

fn handle_task_cancel(
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    sink: &dyn EventSink,
) -> Result<Option<DagChanged>> {
    let payload: PoeTaskCancel = serde_json::from_str(json)?;

    let mut pending_events: Vec<(String, serde_json::Value)> = Vec::new();
    let result = with_project_conn(registry, project_id, |conn| {
        let node = dag_store::db_cancel_node(conn, &payload.id)?;
        dag_store::db_log_event(conn, project_id, Some(agent_id), Some(task_id), "poe:task:cancel", json)?;
        pending_events.push(("poe-node-updated".to_string(), serde_json::to_value(&node)?));
        let change = DagChanged::DagStructureChanged {
            project_id: project_id.to_string(),
        };
        let _ = dag_tx.send(change.clone());
        Ok(Some(change))
    });
    for (event, payload) in pending_events {
        emit_tauri_event(sink, &event, &payload);
    }
    result
}

fn handle_edge(
    json: &str,
    project_id: &str,
    _task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    sink: &dyn EventSink,
) -> Result<Option<DagChanged>> {
    let payload: PoeEdge = serde_json::from_str(json)?;

    let mut pending_events: Vec<(String, serde_json::Value)> = Vec::new();
    let result = with_project_conn(registry, project_id, |conn| {
        let edge = dag_store::db_create_edge(conn, &payload.from, &payload.to, EdgeType::DependsOn)?;
        dag_store::db_log_event(conn, project_id, Some(agent_id), None, "poe:edge", json)?;
        pending_events.push(("poe-edge-created".to_string(), serde_json::to_value(&edge)?));
        let change = DagChanged::DagStructureChanged {
            project_id: project_id.to_string(),
        };
        let _ = dag_tx.send(change.clone());
        Ok(Some(change))
    });
    for (event, payload) in pending_events {
        emit_tauri_event(sink, &event, &payload);
    }
    result
}

fn handle_edge_remove(
    json: &str,
    project_id: &str,
    _task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    sink: &dyn EventSink,
) -> Result<Option<DagChanged>> {
    let payload: PoeEdgeRemove = serde_json::from_str(json)?;

    let mut pending_events: Vec<(String, serde_json::Value)> = Vec::new();
    let result = with_project_conn(registry, project_id, |conn| {
        dag_store::db_remove_edge(conn, &payload.from, &payload.to)?;
        dag_store::db_log_event(conn, project_id, Some(agent_id), None, "poe:edge:remove", json)?;
        pending_events.push(("poe-edge-removed".to_string(), serde_json::json!({
            "fromId": payload.from,
            "toId": payload.to,
        })));
        let change = DagChanged::DagStructureChanged {
            project_id: project_id.to_string(),
        };
        let _ = dag_tx.send(change.clone());
        Ok(Some(change))
    });
    for (event, payload) in pending_events {
        emit_tauri_event(sink, &event, &payload);
    }
    result
}

fn handle_artifact(
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    sink: &dyn EventSink,
) -> Result<Option<DagChanged>> {
    let payload: PoeArtifact = serde_json::from_str(json)?;

    // Agent has already written the file to {project}/docs/<name> using its own tools.
    // The orchestrator only indexes the path — it does not write file content.

    let input = CreateArtifactInput {
        project_id: project_id.to_string(),
        phase_id: None,
        artifact_type: payload.artifact_type,
        filename: payload.name,
        produced_by_stage: None,
    };

    let mut pending_events: Vec<(String, serde_json::Value)> = Vec::new();
    let result = with_project_conn(registry, project_id, |conn| {
        let artifact = dag_store::db_upsert_artifact(conn, &input)?;
        dag_store::db_log_event(conn, project_id, Some(agent_id), Some(task_id), "poe:artifact", json)?;
        pending_events.push(("poe-artifact-created".to_string(), serde_json::to_value(&artifact)?));
        Ok(None) // artifacts don't trigger orchestrator
    });
    for (event, payload) in pending_events {
        emit_tauri_event(sink, &event, &payload);
    }
    result
}

/// Write skill markdown to `{project_path}/.poe/skills/{name}.md`.
/// Creates the directory if it does not exist.
/// Public so integration tests can call it directly without a Tauri AppHandle.
pub fn write_project_skill(project_path: &Path, name: &str, content: &str) -> Result<()> {
    let skills_dir = project_path.join(".poe").join("skills");
    std::fs::create_dir_all(&skills_dir)?;
    let skill_path = skills_dir.join(format!("{}.md", name));
    std::fs::write(&skill_path, content)
        .map_err(|e| anyhow::anyhow!("Failed to write skill {:?}: {}", skill_path, e))?;
    Ok(())
}

fn handle_skill(
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    sink: &dyn EventSink,
    project_path: &Path,
) -> Result<Option<DagChanged>> {
    let payload: PoeSkill = serde_json::from_str(json)?;

    write_project_skill(project_path, &payload.name, &payload.content)?;

    let mut pending_events: Vec<(String, serde_json::Value)> = Vec::new();
    let result = with_project_conn(registry, project_id, |conn| {
        dag_store::db_log_event(conn, project_id, Some(agent_id), Some(task_id), "poe:skill", json)?;
        pending_events.push(("poe-event".to_string(), serde_json::json!({
            "eventType": "poe:skill",
            "projectId": project_id,
            "agentId": agent_id,
            "taskId": task_id,
            "payload": json,
        })));
        Ok(None) // skill writes don't trigger orchestrator
    });
    for (event, payload) in pending_events {
        emit_tauri_event(sink, &event, &payload);
    }
    result
}

fn handle_knowledge(
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    sink: &dyn EventSink,
) -> Result<Option<DagChanged>> {
    let payload: PoeKnowledge = serde_json::from_str(json)?;

    let input = CreateKnowledgeInput {
        project_id: project_id.to_string(),
        key: payload.key,
        value: payload.content, // wire: "content" → DB: "value"
        source: None,
        supersedes_id: payload.supersedes,
    };

    let mut pending_events: Vec<(String, serde_json::Value)> = Vec::new();
    let result = with_project_conn(registry, project_id, |conn| {
        let entry = dag_store::db_create_knowledge(conn, &input)?;
        dag_store::db_log_event(conn, project_id, Some(agent_id), Some(task_id), "poe:knowledge", json)?;
        pending_events.push(("poe-knowledge-created".to_string(), serde_json::to_value(&entry)?));
        Ok(None) // knowledge writes don't trigger orchestrator
    });
    for (event, payload) in pending_events {
        emit_tauri_event(sink, &event, &payload);
    }
    result
}

/// Pure DB mutation for `poe:decision`: set task Waiting, create queue_item.
///
/// Separated from the full handler so tests can call it without an AppHandle.
pub fn db_handle_decision(
    conn: &rusqlite::Connection,
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
) -> Result<dag_store::QueueItem> {
    let payload: PoeDecision = serde_json::from_str(json)?;
    let options_str = payload
        .options
        .map(|opts| serde_json::to_string(&opts).unwrap_or_default());

    let wait_update = UpdateNodeInput {
        status: Some(NodeStatus::Waiting),
        title: None, description: None, skill_id: None, assignee: None,
        ..Default::default()
    };
    dag_store::db_update_node(conn, task_id, &wait_update)?;

    let item = dag_store::db_create_queue_item(
        conn,
        project_id,
        Some(agent_id),
        Some(task_id),
        &payload.question,
        options_str.as_deref(),
    )?;
    dag_store::db_log_event(conn, project_id, Some(agent_id), Some(task_id), "poe:decision", json)?;
    Ok(item)
}

fn handle_decision(
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    sink: &dyn EventSink,
) -> Result<Option<DagChanged>> {
    let mut pending_events: Vec<(String, serde_json::Value)> = Vec::new();
    let result = with_project_conn(registry, project_id, |conn| {
        let item = db_handle_decision(conn, json, project_id, task_id, agent_id)?;
        pending_events.push(("poe-decision-queued".to_string(), serde_json::to_value(&item)?));
        Ok(None)
    });
    for (event, payload) in pending_events {
        emit_tauri_event(sink, &event, &payload);
    }
    result
}

/// Handle `poe:chat` — agent requests a human chat turn.
///
/// Inserts a `chat_turns` row with content and no response yet, logs to `event_log`,
/// and emits `poe://chat-turn` for the frontend.
fn handle_chat(
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    _dag_tx: &mpsc::UnboundedSender<DagChanged>,
    sink: &dyn EventSink,
) -> Result<Option<DagChanged>> {
    let payload: PoeChat = serde_json::from_str(json)?;
    let turn_id = payload
        .id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let mut pending_events: Vec<(String, serde_json::Value)> = Vec::new();
    let result = with_project_conn(registry, project_id, |conn| {
        let turn = dag_store::db_insert_chat_turn(conn, &turn_id, task_id, &payload.content)?;
        dag_store::db_log_event(conn, project_id, Some(agent_id), Some(task_id), "poe:chat", json)?;
        pending_events.push(("poe-chat-turn".to_string(), serde_json::json!({
            "turnId": turn.id,
            "taskId": task_id,
            "content": payload.content,
        })));
        Ok(None) // orchestrator reacts via respond_to_chat → QueueItemResolved
    });
    for (event, payload) in pending_events {
        emit_tauri_event(sink, &event, &payload);
    }
    result
}

/// Handle `poe:advisor` — agent sends a message to the human in the Pane 3 advisor panel.
///
/// Inserts an `advisor_turns` row with content and no response yet, logs to `event_log`,
/// and emits `poe://advisor-turn` for the frontend. Structurally identical to `poe:chat`
/// but routes to a different surface.
fn handle_advisor(
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    _dag_tx: &mpsc::UnboundedSender<DagChanged>,
    sink: &dyn EventSink,
) -> Result<Option<DagChanged>> {
    let payload: PoeAdvisor = serde_json::from_str(json)?;
    let turn_id = payload
        .id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let mut pending_events: Vec<(String, serde_json::Value)> = Vec::new();
    let result = with_project_conn(registry, project_id, |conn| {
        let turn = dag_store::db_insert_advisor_turn(conn, &turn_id, task_id, &payload.content)?;
        dag_store::db_log_event(conn, project_id, Some(agent_id), Some(task_id), "poe:advisor", json)?;
        pending_events.push(("poe-advisor-turn".to_string(), serde_json::json!({
            "turnId": turn.id,
            "taskId": task_id,
            "content": payload.content,
        })));
        Ok(None) // orchestrator reacts via respond_to_advisor → QueueItemResolved
    });
    for (event, payload) in pending_events {
        emit_tauri_event(sink, &event, &payload);
    }
    result
}

/// Pure DB mutation for `poe:done`: always marks the node Complete.
///
/// Returns the resulting `NodeStatus`. Separated from the full handler so tests can
/// call it without an AppHandle or dag_tx.
///
/// Checkpoint (staying Waiting) is handled exclusively by `poe:yield`. Once an agent
/// emits `poe:done`, the task is unconditionally complete.
pub fn db_handle_done(
    conn: &rusqlite::Connection,
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
) -> Result<NodeStatus> {
    let _payload: PoeDone = serde_json::from_str(json)?;

    let new_status = NodeStatus::Complete;

    let update = UpdateNodeInput {
        title: None,
        description: None,
        status: Some(new_status.clone()),
        skill_id: None,
        assignee: None,
        ..Default::default()
    };
    dag_store::db_update_node(conn, task_id, &update)?;
    dag_store::db_log_event(conn, project_id, Some(agent_id), Some(task_id), "poe:done", json)?;
    Ok(new_status)
}

fn handle_done(
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    sink: &dyn EventSink,
) -> Result<Option<DagChanged>> {
    let mut pending_events: Vec<(String, serde_json::Value)> = Vec::new();
    let result = with_project_conn(registry, project_id, |conn| {
        let _new_status = db_handle_done(conn, json, project_id, task_id, agent_id)?;
        let node = dag_store::db_get_node(conn, task_id)?;

        pending_events.push(("poe-task-done".to_string(), serde_json::to_value(&node)?));
        let change = DagChanged::NodeStatusChanged {
            project_id: project_id.to_string(),
            node_id: task_id.to_string(),
        };
        let _ = dag_tx.send(change.clone());
        Ok(Some(change))
    });
    for (event, payload) in pending_events {
        emit_tauri_event(sink, &event, &payload);
    }
    result
}

/// Handle `poe:yield` — sets task status=waiting, records yield_reason, signals orchestrator.
///
/// Under SF-4, this is the single point where a running task suspends. The
/// orchestrator receives NodeStatusChanged and is responsible for any follow-on
/// work (spawning reviewer tasks, etc.).
///
/// `derived_reason` is the last substantive poe: event emitted before this yield
/// (decision | chat | review | None), passed in by the caller's event tracker.
/// The wire format no longer carries a `reason` field.
fn handle_yield(
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    sink: &dyn EventSink,
    derived_reason: Option<String>,
) -> Result<Option<DagChanged>> {
    let _payload: PoeYield = serde_json::from_str(json)?;

    let mut pending_events: Vec<(String, serde_json::Value)> = Vec::new();
    let result = with_project_conn(registry, project_id, |conn| {
        let update = UpdateNodeInput {
            status: Some(NodeStatus::Waiting),
            yield_reason: derived_reason.clone(),
            title: None,
            description: None,
            skill_id: None,
            assignee: None,
            session_id: None,
        };
        let node = dag_store::db_update_node(conn, task_id, &update)?;
        dag_store::db_log_event(conn, project_id, Some(agent_id), Some(task_id), "poe:yield", json)?;

        // poe://task-update — frontend updates node status in the tree
        pending_events.push(("poe-node-updated".to_string(), serde_json::to_value(&node)?));

        // poe://event — activity feed entry
        let summary = match derived_reason.as_deref() {
            Some(r) => format!("Yielded — awaiting {}", r),
            None => "Yielded".to_owned(),
        };
        pending_events.push(("poe-event".to_string(), serde_json::json!({
            "eventType": "poe:yield",
            "projectId": project_id,
            "agentId": agent_id,
            "taskId": task_id,
            "summary": summary,
            "payload": json,
        })));

        let change = DagChanged::NodeStatusChanged {
            project_id: project_id.to_string(),
            node_id: task_id.to_string(),
        };
        let _ = dag_tx.send(change.clone());
        Ok(Some(change))
    });
    for (event, payload) in pending_events {
        emit_tauri_event(sink, &event, &payload);
    }
    result
}

fn handle_log_only(
    event_type: &str,
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    sink: &dyn EventSink,
) -> Result<Option<DagChanged>> {
    let mut pending_events: Vec<(String, serde_json::Value)> = Vec::new();
    let result = with_project_conn(registry, project_id, |conn| {
        dag_store::db_log_event(
            conn,
            project_id,
            Some(agent_id),
            Some(task_id),
            event_type,
            json,
        )?;
        pending_events.push(("poe-event".to_string(), serde_json::json!({
            "eventType": event_type,
            "projectId": project_id,
            "agentId": agent_id,
            "taskId": task_id,
            "payload": json,
        })));
        Ok(None)
    });
    for (event, payload) in pending_events {
        emit_tauri_event(sink, &event, &payload);
    }
    result
}

/// Handle `poe:review` — log-only. Records the event and emits activity feed entry.
///
/// Under SF-4, reviewer task creation, NodeStatus::Blocked, and edge wiring all
/// move to the orchestrator post-poe:yield. The ingester only persists the event
/// record here so the activity feed reflects the review request immediately.
fn handle_review(
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    _dag_tx: &mpsc::UnboundedSender<DagChanged>,
    sink: &dyn EventSink,
) -> Result<Option<DagChanged>> {
    let payload: PoeReview = serde_json::from_str(json)?;

    let mut pending_events: Vec<(String, serde_json::Value)> = Vec::new();
    let result = with_project_conn(registry, project_id, |conn| {
        dag_store::db_log_event(
            conn,
            project_id,
            Some(agent_id),
            Some(task_id),
            "poe:review",
            json,
        )?;

        pending_events.push(("poe-event".to_string(), serde_json::json!({
            "eventType": "poe:review",
            "projectId": project_id,
            "agentId": agent_id,
            "taskId": task_id,
            "summary": format!("Review requested — {}", payload.reviewer_skill),
            "payload": json,
        })));

        Ok(None) // orchestrator handles structural changes post-poe:yield
    });
    for (event, payload) in pending_events {
        emit_tauri_event(sink, &event, &payload);
    }
    result
}

// ── Helper ────────────────────────────────────────────────────────────────────

fn with_project_conn<F, T>(registry: &ProjectRegistry, project_id: &str, f: F) -> Result<T>
where
    F: FnOnce(&rusqlite::Connection) -> Result<T>,
{
    let reg = registry.lock().unwrap();
    let db = reg
        .get(project_id)
        .ok_or_else(|| anyhow::anyhow!("Project not open: {}", project_id))?
        .clone();
    drop(reg);
    let conn = db.conn.lock().unwrap();
    f(&conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Decision state machine helpers ────────────────────────────────────────

    /// Set up an in-memory SQLite connection with the full schema applied.
    fn make_test_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory DB");
        conn.execute_batch(crate::dag_store::schema::CREATE_TABLES)
            .expect("schema");
        conn
    }

    /// Insert a minimal project row and return its id.
    fn insert_project(conn: &rusqlite::Connection) -> String {
        let id = "proj-test".to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO projects (id, name, path, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, "Test Project", "/tmp/test-project", now, now],
        )
        .unwrap();
        id
    }

    /// Insert a minimal task node (status=pending) and return its id.
    fn insert_task(conn: &rusqlite::Connection, project_id: &str) -> String {
        let id = "task-test".to_string();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO nodes (id, project_id, node_type, title, status, created_at, updated_at)
             VALUES (?1, ?2, 'task', 'Test Task', 'pending', ?3, ?4)",
            rusqlite::params![id, project_id, now, now],
        )
        .unwrap();
        id
    }

    /// Read the current status of a node.
    fn read_status(conn: &rusqlite::Connection, node_id: &str) -> crate::dag_store::NodeStatus {
        let s: String = conn
            .query_row("SELECT status FROM nodes WHERE id = ?1", [node_id], |r| r.get(0))
            .unwrap();
        s.parse().unwrap()
    }

    /// Count queue_items for a task where resolved_at IS NULL.
    fn count_unresolved(conn: &rusqlite::Connection, task_id: &str) -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM queue_items WHERE task_id = ?1 AND resolved_at IS NULL",
            [task_id],
            |r| r.get(0),
        )
        .unwrap()
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    /// poe:decision → node becomes Waiting, one unresolved queue_item created.
    #[test]
    fn decision_sets_waiting_and_creates_queue_item() {
        let conn = make_test_conn();
        let project_id = insert_project(&conn);
        let task_id = insert_task(&conn, &project_id);

        let json = r#"{"poe":"decision","question":"Which approach?","options":["A","B"]}"#;
        let item = db_handle_decision(&conn, json, &project_id, &task_id, "agent-1").unwrap();

        // Node must now be Waiting
        assert_eq!(read_status(&conn, &task_id), NodeStatus::Waiting);

        // Exactly one unresolved queue_item
        assert_eq!(count_unresolved(&conn, &task_id), 1);

        // The returned item has no resolution
        assert!(item.resolved_at.is_none());
        assert_eq!(item.question, "Which approach?");
        assert_eq!(item.task_id.as_deref(), Some(task_id.as_str()));
    }

    /// poe:decision creates exactly one queue_item row (not duplicated).
    #[test]
    fn decision_creates_exactly_one_queue_item() {
        let conn = make_test_conn();
        let project_id = insert_project(&conn);
        let task_id = insert_task(&conn, &project_id);

        let json = r#"{"poe":"decision","question":"Deploy now?"}"#;
        db_handle_decision(&conn, json, &project_id, &task_id, "agent-1").unwrap();

        let total: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM queue_items WHERE task_id = ?1",
                [&task_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(total, 1, "expected exactly one queue_item row");
    }

    /// poe:done always marks Complete — checkpoint (staying Waiting) is poe:yield's job.
    ///
    /// Under the old protocol, poe:done conditionally stayed Waiting if unresolved
    /// queue_items existed. Under SF-4 / bp6-m2f.13, poe:done is unconditionally
    /// Complete. The agent must emit poe:yield before poe:done if it wants to wait.
    #[test]
    fn done_always_completes_regardless_of_queue_items() {
        let conn = make_test_conn();
        let project_id = insert_project(&conn);
        let task_id = insert_task(&conn, &project_id);

        // Put the task in Waiting with an open queue_item (simulates poe:decision)
        let decision_json = r#"{"poe":"decision","question":"Confirm before proceeding?"}"#;
        db_handle_decision(&conn, decision_json, &project_id, &task_id, "agent-1").unwrap();
        assert_eq!(count_unresolved(&conn, &task_id), 1);

        // poe:done must now always → Complete (queue_item presence is irrelevant)
        let done_json = r#"{"poe":"done","summary":"work finished"}"#;
        let resulting_status =
            db_handle_done(&conn, done_json, &project_id, &task_id, "agent-1").unwrap();

        assert_eq!(resulting_status, NodeStatus::Complete);
        assert_eq!(read_status(&conn, &task_id), NodeStatus::Complete);
    }

    /// poe:done with NO unresolved queue items → status=Complete immediately (happy path).
    #[test]
    fn done_with_no_queue_items_completes_immediately() {
        let conn = make_test_conn();
        let project_id = insert_project(&conn);
        let task_id = insert_task(&conn, &project_id);

        let done_json = r#"{"poe":"done","summary":"all done"}"#;
        let resulting_status =
            db_handle_done(&conn, done_json, &project_id, &task_id, "agent-1").unwrap();

        assert_eq!(resulting_status, NodeStatus::Complete);
        assert_eq!(read_status(&conn, &task_id), NodeStatus::Complete);
    }

    #[test]
    fn parse_poe_event_valid() {
        let line = r#"{"poe":"brief","content":"starting analysis"}"#;
        let result = parse_poe_event(line);
        assert!(result.is_some());
        let (event_type, json) = result.unwrap();
        assert_eq!(event_type, "brief");
        assert_eq!(json["content"], "starting analysis");
    }

    #[test]
    fn parse_poe_event_task() {
        let line = r#"{"poe":"task","id":"t-1","title":"Implement feature","skill":"senior-engineer"}"#;
        let (event_type, json) = parse_poe_event(line).unwrap();
        assert_eq!(event_type, "task");
        assert_eq!(json["id"], "t-1");
    }

    #[test]
    fn parse_poe_event_done() {
        let line = r#"{"poe":"done","summary":"all done"}"#;
        let (event_type, _) = parse_poe_event(line).unwrap();
        assert_eq!(event_type, "done");
    }

    #[test]
    fn parse_poe_event_not_json() {
        assert!(parse_poe_event("just some text").is_none());
        assert!(parse_poe_event("").is_none());
    }

    #[test]
    fn parse_poe_event_json_without_poe_key() {
        assert!(parse_poe_event(r#"{"type":"result","subtype":"success"}"#).is_none());
    }

    #[test]
    fn parse_poe_event_strips_leading_whitespace() {
        let line = r#"  {"poe":"step","name":"analyzing"}  "#;
        let (event_type, _) = parse_poe_event(line).unwrap();
        assert_eq!(event_type, "step");
    }

    // ── Full-stack: TextBufExtractor → parse_poe_event ────────────────────────

    #[test]
    fn full_stack_sequential_poe_events() {
        // Simulates what agent_lifecycle does: stream-json text chunks arrive,
        // TextBufExtractor splits them into lines, parse_poe_event classifies each.
        use crate::agent::text_extractor::TextBufExtractor;
        let mut ex = TextBufExtractor::new();
        let chunks = [
            "{\"poe\":\"brief\",\"content\":\"starting\"}\n",
            "{\"poe\":\"task\",\"id\":\"t-1\",\"title\":\"Build\"}\n",
            "{\"poe\":\"done\"}\n",
        ];
        let events: Vec<String> = chunks
            .iter()
            .flat_map(|chunk| ex.push(chunk))
            .filter_map(|line| parse_poe_event(&line).map(|(t, _)| t))
            .collect();
        assert_eq!(ex.flush(), None); // nothing left in buffer
        assert_eq!(events, vec!["brief", "task", "done"]);
    }

    #[test]
    fn full_stack_poe_json_split_across_chunks() {
        // JSON object arrives fragmented across multiple text deltas.
        // Note: no content field — agents write files directly using their own tools.
        use crate::agent::text_extractor::TextBufExtractor;
        let mut ex = TextBufExtractor::new();
        let chunks = [
            "{\"poe\":\"artifact\",",
            "\"name\":\"out.md\",",
            "\"artifact_type\":\"doc\"}",
            "\n",
        ];
        let events: Vec<String> = chunks
            .iter()
            .flat_map(|chunk| ex.push(chunk))
            .filter_map(|line| parse_poe_event(&line).map(|(t, _)| t))
            .collect();
        assert_eq!(events, vec!["artifact"]);
    }

    #[test]
    fn full_stack_prose_lines_between_events_are_discarded() {
        // Agent may emit prose commentary alongside poe: events.
        // Only lines that parse as poe: events are retained.
        use crate::agent::text_extractor::TextBufExtractor;
        let mut ex = TextBufExtractor::new();
        let chunks = [
            "I am now going to analyze the task.\n",
            "{\"poe\":\"brief\",\"content\":\"analysed\"}\n",
            "And here is my summary.\n",
            "{\"poe\":\"done\"}\n",
        ];
        let events: Vec<String> = chunks
            .iter()
            .flat_map(|chunk| ex.push(chunk))
            .filter_map(|line| parse_poe_event(&line).map(|(t, _)| t))
            .collect();
        assert_eq!(events, vec!["brief", "done"]);
    }

    /// poe:artifact without content field parses and upserts DB correctly.
    /// The ingester must NOT write any file to disk — that is the agent's job.
    #[test]
    fn handle_artifact_no_content_upserts_db() {
        let conn = make_test_conn();
        let project_id = insert_project(&conn);

        let input = crate::dag_store::CreateArtifactInput {
            project_id: project_id.clone(),
            phase_id: None,
            artifact_type: "conops".to_string(),
            filename: "conops.md".to_string(),
            produced_by_stage: None,
        };
        let artifact = crate::dag_store::db_upsert_artifact(&conn, &input).unwrap();
        assert_eq!(artifact.filename, "conops.md");
        assert_eq!(artifact.artifact_type, "conops");

        // Verify parse_poe_event handles a content-free artifact line correctly.
        let line = r#"{"poe":"artifact","name":"conops.md","artifact_type":"conops"}"#;
        let (event_type, _) = parse_poe_event(line).unwrap();
        assert_eq!(event_type, "artifact");
    }

    /// Old agents may still send a content field — it must be silently ignored (not written to disk).
    /// serde drops unknown fields by default, so this is a no-op at the struct level.
    #[test]
    fn handle_artifact_ignores_content_if_present() {
        let line = r#"{"poe":"artifact","name":"x.md","artifact_type":"doc","content":"should be ignored"}"#;
        let result = parse_poe_event(line);
        assert!(result.is_some(), "content-bearing artifact must still parse as a poe: event");
        let (event_type, _) = result.unwrap();
        assert_eq!(event_type, "artifact");
        // Struct deserialization drops unknown fields — content is silently discarded.
        let parsed: PoeArtifact = serde_json::from_str(line).unwrap();
        assert_eq!(parsed.name, "x.md");
        assert_eq!(parsed.artifact_type, "doc");
    }

    #[test]
    fn full_stack_tail_flushed_after_result_event() {
        // A poe:done event without a trailing newline is flushed via TextBufExtractor::flush().
        use crate::agent::text_extractor::TextBufExtractor;
        let mut ex = TextBufExtractor::new();
        // Push everything but no trailing newline on the last event.
        let lines_from_push: Vec<String> = ex.push("{\"poe\":\"brief\"}\n{\"poe\":\"done\"}");
        // brief comes out on push (had newline), done stays in buffer.
        assert_eq!(lines_from_push.len(), 1);
        assert_eq!(parse_poe_event(&lines_from_push[0]).map(|(t, _)| t).as_deref(), Some("brief"));
        // Flush retrieves the tail.
        let tail = ex.flush().unwrap();
        let (event_type, _) = parse_poe_event(&tail).unwrap();
        assert_eq!(event_type, "done");
    }
}
