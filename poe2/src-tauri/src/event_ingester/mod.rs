use crate::dag_store::{
    self, CreateArtifactInput, CreateKnowledgeInput, CreateNodeInput, EdgeType, NodeStatus,
    NodeType, ProjectRegistry, UpdateNodeInput,
};
use anyhow::Result;
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
    artifact_type: String,
    content: String,
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
pub fn ingest_line(
    line: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
    project_path: &Path,
) {
    let trimmed = line.trim();
    let Some((poe_event, _)) = parse_poe_event(trimmed) else {
        return; // not a poe: event — raw PTY output, caller handles it
    };
    let poe_event = poe_event.as_str();

    let result = match poe_event {
        "task" => handle_task(trimmed, project_id, task_id, agent_id, registry, dag_tx, app),
        "task:update" => handle_task_update(trimmed, project_id, task_id, agent_id, registry, dag_tx, app),
        "task:cancel" => handle_task_cancel(trimmed, project_id, task_id, agent_id, registry, dag_tx, app),
        "edge" => handle_edge(trimmed, project_id, task_id, agent_id, registry, dag_tx, app),
        "edge:remove" => handle_edge_remove(trimmed, project_id, task_id, agent_id, registry, dag_tx, app),
        "artifact" => handle_artifact(trimmed, project_id, task_id, agent_id, registry, app, project_path),
        "skill" => handle_skill(trimmed, project_id, task_id, agent_id, registry, app, project_path),
        "knowledge" => handle_knowledge(trimmed, project_id, task_id, agent_id, registry, app),
        "brief" => handle_log_only("poe:brief", trimmed, project_id, task_id, agent_id, registry, app),
        "step" => handle_log_only("poe:step", trimmed, project_id, task_id, agent_id, registry, app),
        "decision" => handle_decision(trimmed, project_id, task_id, agent_id, registry, app),
        "done" => handle_done(trimmed, project_id, task_id, agent_id, registry, dag_tx, app),
        "review" => handle_review(trimmed, project_id, task_id, agent_id, registry, dag_tx, app),
        other => {
            eprintln!("[event_ingester] Unknown poe: event type: {}", other);
            Ok(None)
        }
    };

    if let Err(e) = result {
        eprintln!("[event_ingester] Error processing poe:{}: {}", poe_event, e);
    }
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
    project_id: &str,
    task_id: &str,
    agent_id: &str,
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
        project_id: project_id.to_string(),
        phase_id: None,
        parent_id: payload.parent_id.clone(),
        node_type,
        title: payload.title.clone(),
        description: payload.description.clone(),
        skill_id: payload.skill.clone(),
    };

    with_project_conn(registry, project_id, |conn| {
        let node = dag_store::db_create_node(conn, &input)?;
        // Create edges for depends_on entries: from=node.id, to=dep
        if let Some(ref deps) = payload.depends_on {
            for dep_id in deps {
                dag_store::db_create_edge(conn, &node.id, dep_id, EdgeType::DependsOn)?;
            }
        }
        dag_store::db_log_event(conn, project_id, Some(agent_id), Some(task_id), "poe:task", json)?;
        emit_tauri_event(app, "poe-task-created", &node);
        let change = DagChanged::DagStructureChanged {
            project_id: project_id.to_string(),
        };
        let _ = dag_tx.send(change.clone());
        Ok(Some(change))
    })
}

fn handle_task_update(
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) -> Result<Option<DagChanged>> {
    let payload: PoeTaskUpdate = serde_json::from_str(json)?;
    let input = UpdateNodeInput {
        title: payload.title,
        description: payload.description,
        status: None,
        skill_id: payload.skill,
        assignee: None,
    };

    with_project_conn(registry, project_id, |conn| {
        let node = dag_store::db_update_node(conn, &payload.id, &input)?;
        dag_store::db_log_event(conn, project_id, Some(agent_id), Some(task_id), "poe:task:update", json)?;
        emit_tauri_event(app, "poe-node-updated", &node);
        let change = DagChanged::DagStructureChanged {
            project_id: project_id.to_string(),
        };
        let _ = dag_tx.send(change.clone());
        Ok(Some(change))
    })
}

fn handle_task_cancel(
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) -> Result<Option<DagChanged>> {
    let payload: PoeTaskCancel = serde_json::from_str(json)?;

    with_project_conn(registry, project_id, |conn| {
        let node = dag_store::db_cancel_node(conn, &payload.id)?;
        dag_store::db_log_event(conn, project_id, Some(agent_id), Some(task_id), "poe:task:cancel", json)?;
        emit_tauri_event(app, "poe-node-updated", &node);
        let change = DagChanged::DagStructureChanged {
            project_id: project_id.to_string(),
        };
        let _ = dag_tx.send(change.clone());
        Ok(Some(change))
    })
}

fn handle_edge(
    json: &str,
    project_id: &str,
    _task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) -> Result<Option<DagChanged>> {
    let payload: PoeEdge = serde_json::from_str(json)?;

    with_project_conn(registry, project_id, |conn| {
        let edge = dag_store::db_create_edge(conn, &payload.from, &payload.to, EdgeType::DependsOn)?;
        dag_store::db_log_event(conn, project_id, Some(agent_id), None, "poe:edge", json)?;
        emit_tauri_event(app, "poe-edge-created", &edge);
        let change = DagChanged::DagStructureChanged {
            project_id: project_id.to_string(),
        };
        let _ = dag_tx.send(change.clone());
        Ok(Some(change))
    })
}

fn handle_edge_remove(
    json: &str,
    project_id: &str,
    _task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) -> Result<Option<DagChanged>> {
    let payload: PoeEdgeRemove = serde_json::from_str(json)?;

    with_project_conn(registry, project_id, |conn| {
        dag_store::db_remove_edge(conn, &payload.from, &payload.to)?;
        dag_store::db_log_event(conn, project_id, Some(agent_id), None, "poe:edge:remove", json)?;
        emit_tauri_event(app, "poe-edge-removed", &serde_json::json!({
            "fromId": payload.from,
            "toId": payload.to,
        }));
        let change = DagChanged::DagStructureChanged {
            project_id: project_id.to_string(),
        };
        let _ = dag_tx.send(change.clone());
        Ok(Some(change))
    })
}

fn handle_artifact(
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    app: &AppHandle,
    project_path: &Path,
) -> Result<Option<DagChanged>> {
    let payload: PoeArtifact = serde_json::from_str(json)?;

    // Write content to {project}/docs/<name>
    let docs_dir = project_path.join("docs");
    std::fs::create_dir_all(&docs_dir)?;
    let artifact_path = docs_dir.join(&payload.name);
    std::fs::write(&artifact_path, &payload.content)
        .map_err(|e| anyhow::anyhow!("Failed to write artifact {:?}: {}", artifact_path, e))?;

    let input = CreateArtifactInput {
        project_id: project_id.to_string(),
        phase_id: None,
        artifact_type: payload.artifact_type,
        filename: payload.name,
        produced_by_stage: None,
    };

    with_project_conn(registry, project_id, |conn| {
        let artifact = dag_store::db_upsert_artifact(conn, &input)?;
        dag_store::db_log_event(conn, project_id, Some(agent_id), Some(task_id), "poe:artifact", json)?;
        emit_tauri_event(app, "poe-artifact-created", &artifact);
        Ok(None) // artifacts don't trigger orchestrator
    })
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
    app: &AppHandle,
    project_path: &Path,
) -> Result<Option<DagChanged>> {
    let payload: PoeSkill = serde_json::from_str(json)?;

    write_project_skill(project_path, &payload.name, &payload.content)?;

    with_project_conn(registry, project_id, |conn| {
        dag_store::db_log_event(conn, project_id, Some(agent_id), Some(task_id), "poe:skill", json)?;
        emit_tauri_event(app, "poe-event", &serde_json::json!({
            "eventType": "poe:skill",
            "projectId": project_id,
            "agentId": agent_id,
            "taskId": task_id,
            "payload": json,
        }));
        Ok(None) // skill writes don't trigger orchestrator
    })
}

fn handle_knowledge(
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    app: &AppHandle,
) -> Result<Option<DagChanged>> {
    let payload: PoeKnowledge = serde_json::from_str(json)?;

    let input = CreateKnowledgeInput {
        project_id: project_id.to_string(),
        key: payload.key,
        value: payload.content, // wire: "content" → DB: "value"
        source: None,
        supersedes_id: payload.supersedes,
    };

    with_project_conn(registry, project_id, |conn| {
        let entry = dag_store::db_create_knowledge(conn, &input)?;
        dag_store::db_log_event(conn, project_id, Some(agent_id), Some(task_id), "poe:knowledge", json)?;
        emit_tauri_event(app, "poe-knowledge-created", &entry);
        Ok(None) // knowledge writes don't trigger orchestrator
    })
}

fn handle_decision(
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    app: &AppHandle,
) -> Result<Option<DagChanged>> {
    let payload: PoeDecision = serde_json::from_str(json)?;
    let options_str = payload
        .options
        .map(|opts| serde_json::to_string(&opts).unwrap_or_default());

    with_project_conn(registry, project_id, |conn| {
        let item = dag_store::db_create_queue_item(
            conn,
            project_id,
            Some(agent_id),
            Some(task_id),
            &payload.question,
            options_str.as_deref(),
        )?;
        dag_store::db_log_event(conn, project_id, Some(agent_id), Some(task_id), "poe:decision", json)?;
        emit_tauri_event(app, "poe-decision-queued", &item);
        Ok(None) // decisions don't trigger orchestrator
    })
}

fn handle_done(
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) -> Result<Option<DagChanged>> {
    let _payload: PoeDone = serde_json::from_str(json)?;

    let update = UpdateNodeInput {
        title: None,
        description: None,
        status: Some(NodeStatus::Complete),
        skill_id: None,
        assignee: None,
    };

    with_project_conn(registry, project_id, |conn| {
        let node = dag_store::db_update_node(conn, task_id, &update)?;
        dag_store::db_log_event(conn, project_id, Some(agent_id), Some(task_id), "poe:done", json)?;
        emit_tauri_event(app, "poe-task-done", &node);
        let change = DagChanged::NodeStatusChanged {
            project_id: project_id.to_string(),
            node_id: task_id.to_string(),
        };
        let _ = dag_tx.send(change.clone());
        Ok(Some(change))
    })
}

fn handle_log_only(
    event_type: &str,
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    app: &AppHandle,
) -> Result<Option<DagChanged>> {
    with_project_conn(registry, project_id, |conn| {
        dag_store::db_log_event(
            conn,
            project_id,
            Some(agent_id),
            Some(task_id),
            event_type,
            json,
        )?;
        emit_tauri_event(app, "poe-event", &serde_json::json!({
            "eventType": event_type,
            "projectId": project_id,
            "agentId": agent_id,
            "taskId": task_id,
            "payload": json,
        }));
        Ok(None)
    })
}

/// Handle `poe:review` — creates a review task, blocks the requesting task,
/// and wires the dependency so the orchestrator unblocks the original when the review completes.
fn handle_review(
    json: &str,
    project_id: &str,
    task_id: &str,
    agent_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) -> Result<Option<DagChanged>> {
    let payload: PoeReview = serde_json::from_str(json)?;

    with_project_conn(registry, project_id, |conn| {
        // Get the requesting task to inherit phase/parent context
        let requesting_task = dag_store::db_get_node(conn, task_id)?;

        // Create review task node
        let review_input = CreateNodeInput {
            project_id: project_id.to_string(),
            phase_id: requesting_task.phase_id.clone(),
            parent_id: requesting_task.parent_id.clone(),
            node_type: NodeType::Task,
            title: format!("Review: {}", requesting_task.title),
            description: payload.content,
            skill_id: Some(payload.reviewer_skill.clone()),
        };
        let review_task = dag_store::db_create_node(conn, &review_input)?;

        // Block the requesting task
        let block_update = UpdateNodeInput {
            status: Some(NodeStatus::Blocked),
            title: None, description: None, skill_id: None, assignee: None,
        };
        dag_store::db_update_node(conn, task_id, &block_update)?;

        // Wire dependency: requesting_task depends_on review_task
        // (when review_task completes, orchestrator will unblock requesting_task)
        dag_store::db_create_edge(
            conn,
            &review_task.id,
            task_id,
            EdgeType::DependsOn,
        )?;

        dag_store::db_log_event(
            conn,
            project_id,
            Some(agent_id),
            Some(task_id),
            "poe:review",
            json,
        )?;

        emit_tauri_event(app, "poe-review-requested", &serde_json::json!({
            "requestingTaskId": task_id,
            "reviewTaskId": review_task.id,
            "reviewerSkillId": payload.reviewer_skill,
        }));

        let change = DagChanged::DagStructureChanged {
            project_id: project_id.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

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
        use crate::agent::text_extractor::TextBufExtractor;
        let mut ex = TextBufExtractor::new();
        let chunks = [
            "{\"poe\":\"artifact\",",
            "\"name\":\"out.md\",",
            "\"artifact_type\":\"doc\",\"content\":\"hello\"}",
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
