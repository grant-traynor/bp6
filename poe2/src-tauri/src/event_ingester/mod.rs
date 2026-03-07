use crate::dag_store::{
    self, CreateArtifactInput, CreateKnowledgeInput, CreateNodeInput, EdgeType, NodeStatus,
    NodeType, ProjectRegistry, UpdateNodeInput,
};
use anyhow::Result;
use serde::Deserialize;
use std::path::Path;
use tauri::AppHandle;
use tokio::sync::mpsc;

/// Signal sent to the orchestrator when DAG state changes.
#[derive(Debug, Clone)]
pub enum DagChanged {
    NodeStatusChanged { project_id: String, node_id: String },
    DagStructureChanged { project_id: String },
    QueueItemResolved { project_id: String, item_id: String },
}

// ── poe: event payloads ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct PoeTask {
    task_id: Option<String>,
    project_id: String,
    phase_id: Option<String>,
    parent_id: Option<String>,
    #[serde(rename = "type")]
    node_type: Option<String>,
    title: String,
    description: Option<String>,
    skill_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PoeTaskUpdate {
    task_id: String,
    project_id: String,
    title: Option<String>,
    description: Option<String>,
    skill_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PoeTaskCancel {
    task_id: String,
    project_id: String,
}

#[derive(Debug, Deserialize)]
struct PoeEdge {
    from_id: String,
    to_id: String,
    project_id: String,
    #[serde(rename = "type")]
    edge_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PoeEdgeRemove {
    from_id: String,
    to_id: String,
    project_id: String,
}

#[derive(Debug, Deserialize)]
struct PoeArtifact {
    project_id: String,
    phase_id: Option<String>,
    #[serde(rename = "type")]
    artifact_type: String,
    filename: String,
    content: String,
    produced_by_stage: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PoeKnowledge {
    project_id: String,
    key: String,
    value: String,
    source: Option<String>,
    supersedes_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PoeDecision {
    project_id: String,
    agent_id: Option<String>,
    task_id: Option<String>,
    question: String,
    options: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct PoeDone {
    task_id: String,
    project_id: String,
}

#[derive(Debug, Deserialize)]
struct PoeBriefOrStep {
    project_id: String,
    agent_id: Option<String>,
    task_id: Option<String>,
    // All remaining fields captured as raw JSON for the event log
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct PoeReview {
    /// The task that is requesting a review (will be blocked).
    requesting_task_id: String,
    project_id: String,
    agent_id: Option<String>,
    /// Skill to assign to the review task.
    reviewer_skill_id: String,
    /// Optional description for the review task.
    description: Option<String>,
}

// ── Core ingestion ────────────────────────────────────────────────────────────

/// Process a single line of PTY output from an agent.
///
/// Lines not prefixed with `poe:` are ignored (passed to PTY buffer by the caller).
/// Returns `Some(DagChanged)` if the line mutated DAG structure or task status.
pub fn ingest_line(
    line: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
    project_path: &Path,
) -> Option<DagChanged> {
    let trimmed = line.trim();
    if !trimmed.starts_with("poe:") {
        return None;
    }

    // Find the JSON payload after the event type prefix
    let (event_type, json_str) = parse_poe_line(trimmed)?;

    let result = match event_type {
        "poe:task" => handle_task(json_str, registry, dag_tx, app),
        "poe:task:update" => handle_task_update(json_str, registry, dag_tx, app),
        "poe:task:cancel" => handle_task_cancel(json_str, registry, dag_tx, app),
        "poe:edge" => handle_edge(json_str, registry, dag_tx, app),
        "poe:edge:remove" => handle_edge_remove(json_str, registry, dag_tx, app),
        "poe:artifact" => handle_artifact(json_str, registry, app, project_path),
        "poe:knowledge" => handle_knowledge(json_str, registry, app),
        "poe:decision" => handle_decision(json_str, registry, app),
        "poe:done" => handle_done(json_str, registry, dag_tx, app),
        "poe:brief" | "poe:step" => handle_log_only(event_type, json_str, registry, app),
        "poe:review" => handle_review(json_str, registry, dag_tx, app),
        _ => {
            eprintln!("[event_ingester] Unknown poe: event type: {}", event_type);
            Ok(None)
        }
    };

    match result {
        Ok(change) => change,
        Err(e) => {
            eprintln!("[event_ingester] Error processing {}: {}", event_type, e);
            None
        }
    }
}

/// Split `poe:event:type {"json": "payload"}` into `("poe:event:type", "{json}")`.
fn parse_poe_line(line: &str) -> Option<(&str, &str)> {
    // Find the first `{` — everything before is the event type, everything from `{` is JSON
    let json_start = line.find('{')?;
    let event_type = line[..json_start].trim();
    let json_str = &line[json_start..];
    Some((event_type, json_str))
}

fn emit_tauri_event(app: &AppHandle, event: &str, payload: &impl serde::Serialize) {
    use tauri::Emitter;
    if let Ok(json) = serde_json::to_string(payload) {
        let _ = app.emit(event, json);
    }
}

// ── Handlers ──────────────────────────────────────────────────────────────────

fn handle_task(
    json: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) -> Result<Option<DagChanged>> {
    let payload: PoeTask = serde_json::from_str(json)?;
    let node_type = payload
        .node_type
        .as_deref()
        .unwrap_or("task")
        .parse::<NodeType>()
        .unwrap_or(NodeType::Task);

    let input = CreateNodeInput {
        project_id: payload.project_id.clone(),
        phase_id: payload.phase_id,
        parent_id: payload.parent_id,
        node_type,
        title: payload.title,
        description: payload.description,
        skill_id: payload.skill_id,
    };

    with_project_conn(registry, &payload.project_id, |conn| {
        let node = dag_store::db_create_node(conn, &input)?;
        dag_store::db_log_event(conn, &payload.project_id, None, Some(&node.id), "poe:task", json)?;
        emit_tauri_event(app, "poe-task-created", &node);
        let change = DagChanged::DagStructureChanged {
            project_id: payload.project_id.clone(),
        };
        let _ = dag_tx.send(change.clone());
        Ok(Some(change))
    })
}

fn handle_task_update(
    json: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) -> Result<Option<DagChanged>> {
    let payload: PoeTaskUpdate = serde_json::from_str(json)?;
    let input = UpdateNodeInput {
        title: payload.title,
        description: payload.description,
        status: None,
        skill_id: payload.skill_id,
        assignee: None,
    };

    with_project_conn(registry, &payload.project_id, |conn| {
        let node = dag_store::db_update_node(conn, &payload.task_id, &input)?;
        dag_store::db_log_event(conn, &payload.project_id, None, Some(&payload.task_id), "poe:task:update", json)?;
        emit_tauri_event(app, "poe-node-updated", &node);
        let change = DagChanged::DagStructureChanged {
            project_id: payload.project_id.clone(),
        };
        let _ = dag_tx.send(change.clone());
        Ok(Some(change))
    })
}

fn handle_task_cancel(
    json: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) -> Result<Option<DagChanged>> {
    let payload: PoeTaskCancel = serde_json::from_str(json)?;

    with_project_conn(registry, &payload.project_id, |conn| {
        let node = dag_store::db_cancel_node(conn, &payload.task_id)?;
        dag_store::db_log_event(conn, &payload.project_id, None, Some(&payload.task_id), "poe:task:cancel", json)?;
        emit_tauri_event(app, "poe-node-updated", &node);
        let change = DagChanged::DagStructureChanged {
            project_id: payload.project_id.clone(),
        };
        let _ = dag_tx.send(change.clone());
        Ok(Some(change))
    })
}

fn handle_edge(
    json: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) -> Result<Option<DagChanged>> {
    let payload: PoeEdge = serde_json::from_str(json)?;
    let et = payload
        .edge_type
        .as_deref()
        .unwrap_or("depends_on")
        .parse::<EdgeType>()
        .unwrap_or(EdgeType::DependsOn);

    with_project_conn(registry, &payload.project_id, |conn| {
        let edge = dag_store::db_create_edge(conn, &payload.from_id, &payload.to_id, et)?;
        dag_store::db_log_event(conn, &payload.project_id, None, None, "poe:edge", json)?;
        emit_tauri_event(app, "poe-edge-created", &edge);
        let change = DagChanged::DagStructureChanged {
            project_id: payload.project_id.clone(),
        };
        let _ = dag_tx.send(change.clone());
        Ok(Some(change))
    })
}

fn handle_edge_remove(
    json: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) -> Result<Option<DagChanged>> {
    let payload: PoeEdgeRemove = serde_json::from_str(json)?;

    with_project_conn(registry, &payload.project_id, |conn| {
        dag_store::db_remove_edge(conn, &payload.from_id, &payload.to_id)?;
        dag_store::db_log_event(conn, &payload.project_id, None, None, "poe:edge:remove", json)?;
        emit_tauri_event(app, "poe-edge-removed", &serde_json::json!({
            "fromId": payload.from_id,
            "toId": payload.to_id,
        }));
        let change = DagChanged::DagStructureChanged {
            project_id: payload.project_id.clone(),
        };
        let _ = dag_tx.send(change.clone());
        Ok(Some(change))
    })
}

fn handle_artifact(
    json: &str,
    registry: &ProjectRegistry,
    app: &AppHandle,
    project_path: &Path,
) -> Result<Option<DagChanged>> {
    let payload: PoeArtifact = serde_json::from_str(json)?;

    // Write content to {project}/docs/<filename>
    let docs_dir = project_path.join("docs");
    std::fs::create_dir_all(&docs_dir)?;
    let artifact_path = docs_dir.join(&payload.filename);
    std::fs::write(&artifact_path, &payload.content)
        .map_err(|e| anyhow::anyhow!("Failed to write artifact {:?}: {}", artifact_path, e))?;

    let input = CreateArtifactInput {
        project_id: payload.project_id.clone(),
        phase_id: payload.phase_id,
        artifact_type: payload.artifact_type,
        filename: payload.filename,
        produced_by_stage: payload.produced_by_stage,
    };

    with_project_conn(registry, &payload.project_id, |conn| {
        let artifact = dag_store::db_upsert_artifact(conn, &input)?;
        dag_store::db_log_event(conn, &payload.project_id, None, None, "poe:artifact", json)?;
        emit_tauri_event(app, "poe-artifact-created", &artifact);
        Ok(None) // artifacts don't trigger orchestrator
    })
}

fn handle_knowledge(
    json: &str,
    registry: &ProjectRegistry,
    app: &AppHandle,
) -> Result<Option<DagChanged>> {
    let payload: PoeKnowledge = serde_json::from_str(json)?;

    let input = CreateKnowledgeInput {
        project_id: payload.project_id.clone(),
        key: payload.key,
        value: payload.value,
        source: payload.source,
        supersedes_id: payload.supersedes_id,
    };

    with_project_conn(registry, &payload.project_id, |conn| {
        let entry = dag_store::db_create_knowledge(conn, &input)?;
        dag_store::db_log_event(conn, &payload.project_id, None, None, "poe:knowledge", json)?;
        emit_tauri_event(app, "poe-knowledge-created", &entry);
        Ok(None) // knowledge writes don't trigger orchestrator
    })
}

fn handle_decision(
    json: &str,
    registry: &ProjectRegistry,
    app: &AppHandle,
) -> Result<Option<DagChanged>> {
    let payload: PoeDecision = serde_json::from_str(json)?;
    let options_str = payload
        .options
        .map(|opts| serde_json::to_string(&opts).unwrap_or_default());

    with_project_conn(registry, &payload.project_id, |conn| {
        let item = dag_store::db_create_queue_item(
            conn,
            &payload.project_id,
            payload.agent_id.as_deref(),
            payload.task_id.as_deref(),
            &payload.question,
            options_str.as_deref(),
        )?;
        dag_store::db_log_event(conn, &payload.project_id, payload.agent_id.as_deref(), payload.task_id.as_deref(), "poe:decision", json)?;
        emit_tauri_event(app, "poe-decision-queued", &item);
        Ok(None) // decisions don't trigger orchestrator
    })
}

fn handle_done(
    json: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) -> Result<Option<DagChanged>> {
    let payload: PoeDone = serde_json::from_str(json)?;

    let update = UpdateNodeInput {
        title: None,
        description: None,
        status: Some(NodeStatus::Complete),
        skill_id: None,
        assignee: None,
    };

    with_project_conn(registry, &payload.project_id, |conn| {
        let node = dag_store::db_update_node(conn, &payload.task_id, &update)?;
        dag_store::db_log_event(conn, &payload.project_id, None, Some(&payload.task_id), "poe:done", json)?;
        emit_tauri_event(app, "poe-task-done", &node);
        let change = DagChanged::NodeStatusChanged {
            project_id: payload.project_id.clone(),
            node_id: payload.task_id.clone(),
        };
        let _ = dag_tx.send(change.clone());
        Ok(Some(change))
    })
}

fn handle_log_only(
    event_type: &str,
    json: &str,
    registry: &ProjectRegistry,
    app: &AppHandle,
) -> Result<Option<DagChanged>> {
    let payload: PoeBriefOrStep = serde_json::from_str(json)?;

    with_project_conn(registry, &payload.project_id, |conn| {
        dag_store::db_log_event(
            conn,
            &payload.project_id,
            payload.agent_id.as_deref(),
            payload.task_id.as_deref(),
            event_type,
            json,
        )?;
        emit_tauri_event(app, "poe-event", &serde_json::json!({
            "eventType": event_type,
            "projectId": payload.project_id,
            "agentId": payload.agent_id,
            "taskId": payload.task_id,
            "payload": json,
        }));
        Ok(None)
    })
}

/// Handle `poe:review` — creates a review task, blocks the requesting task,
/// and wires the dependency so the orchestrator unblocks the original when the review completes.
fn handle_review(
    json: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) -> Result<Option<DagChanged>> {
    let payload: PoeReview = serde_json::from_str(json)?;

    with_project_conn(registry, &payload.project_id, |conn| {
        // Get the requesting task to inherit phase/parent context
        let requesting_task = dag_store::db_get_node(conn, &payload.requesting_task_id)?;

        // Create review task node
        let review_input = CreateNodeInput {
            project_id: payload.project_id.clone(),
            phase_id: requesting_task.phase_id.clone(),
            parent_id: requesting_task.parent_id.clone(),
            node_type: NodeType::Task,
            title: format!("Review: {}", requesting_task.title),
            description: payload.description,
            skill_id: Some(payload.reviewer_skill_id.clone()),
        };
        let review_task = dag_store::db_create_node(conn, &review_input)?;

        // Block the requesting task
        let block_update = UpdateNodeInput {
            status: Some(NodeStatus::Blocked),
            title: None, description: None, skill_id: None, assignee: None,
        };
        dag_store::db_update_node(conn, &payload.requesting_task_id, &block_update)?;

        // Wire dependency: requesting_task depends_on review_task
        // (when review_task completes, orchestrator will unblock requesting_task)
        dag_store::db_create_edge(
            conn,
            &review_task.id,
            &payload.requesting_task_id,
            EdgeType::DependsOn,
        )?;

        dag_store::db_log_event(
            conn,
            &payload.project_id,
            payload.agent_id.as_deref(),
            Some(&payload.requesting_task_id),
            "poe:review",
            json,
        )?;

        emit_tauri_event(app, "poe-review-requested", &serde_json::json!({
            "requestingTaskId": payload.requesting_task_id,
            "reviewTaskId": review_task.id,
            "reviewerSkillId": payload.reviewer_skill_id,
        }));

        let change = DagChanged::DagStructureChanged {
            project_id: payload.project_id.clone(),
        };
        let _ = dag_tx.send(change.clone());
        Ok(Some(change))
    })
}

// ── Helper ────────────────────────────────────────────────────────────────────

fn with_project_conn<F, T>(registry: &ProjectRegistry, project_id: &str, f: F) -> Result<T>
where
    F: FnOnce(&rusqlite::Connection) -> Result<T>,
{
    let reg = registry.lock().unwrap();
    let db = reg
        .values()
        .find(|db| db.project.id == project_id)
        .ok_or_else(|| anyhow::anyhow!("Project not open: {}", project_id))?
        .clone();
    drop(reg);
    let conn = db.conn.lock().unwrap();
    f(&conn)
}
