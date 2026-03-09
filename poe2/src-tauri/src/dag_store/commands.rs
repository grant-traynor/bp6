use super::*;
use tauri::State;
use tokio::sync::mpsc;

pub type Registry = std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<ProjectDb>>>>;

#[tauri::command]
pub async fn open_project(
    path: String,
    registry: State<'_, ProjectRegistry>,
    dag_tx: State<'_, mpsc::UnboundedSender<crate::event_ingester::DagChanged>>,
) -> Result<Project, String> {
    let project_path = std::path::Path::new(&path);
    let (project, conn) = open_project_db(project_path).map_err(|e| e.to_string())?;

    let db = std::sync::Arc::new(ProjectDb {
        project: project.clone(),
        conn: Mutex::new(conn),
    });

    registry.lock().unwrap().insert(project.path.clone(), db);

    // Notify orchestrator: project just opened. This triggers ghost-agent recovery
    // (running tasks from a previous session whose process is no longer alive)
    // before the normal scheduling loop runs.
    let _ = dag_tx.send(crate::event_ingester::DagChanged::ProjectOpened {
        project_id: project.id.clone(),
    });

    Ok(project)
}

#[tauri::command]
pub async fn close_project(
    project_id: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<(), String> {
    let mut reg = registry.lock().unwrap();
    reg.retain(|_, db| db.project.id != project_id);
    Ok(())
}

#[tauri::command]
pub async fn list_projects(
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<Project>, String> {
    let reg = registry.lock().unwrap();
    Ok(reg.values().map(|db| db.project.clone()).collect())
}

fn with_conn<F, T>(registry: &State<'_, ProjectRegistry>, project_id: &str, f: F) -> Result<T, String>
where
    F: FnOnce(&Connection) -> anyhow::Result<T>,
{
    let reg = registry.lock().unwrap();
    let db = reg
        .values()
        .find(|db| db.project.id == project_id)
        .ok_or_else(|| format!("Project not open: {}", project_id))?
        .clone();
    drop(reg);
    let conn = db.conn.lock().unwrap();
    f(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_node(
    input: CreateNodeInput,
    registry: State<'_, ProjectRegistry>,
    dag_tx: State<'_, mpsc::UnboundedSender<crate::event_ingester::DagChanged>>,
) -> Result<Node, String> {
    let project_id = input.project_id.clone();
    let node = with_conn(&registry, &project_id, |conn| db_create_node(conn, &input))?;
    let _ = dag_tx.send(crate::event_ingester::DagChanged::DagStructureChanged { project_id });
    Ok(node)
}

#[tauri::command]
pub async fn update_node(
    node_id: String,
    project_id: String,
    input: UpdateNodeInput,
    registry: State<'_, ProjectRegistry>,
) -> Result<Node, String> {
    with_conn(&registry, &project_id, |conn| db_update_node(conn, &node_id, &input))
}

#[tauri::command]
pub async fn cancel_node(
    node_id: String,
    project_id: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<Node, String> {
    with_conn(&registry, &project_id, |conn| db_cancel_node(conn, &node_id))
}

#[tauri::command]
pub async fn get_node(
    node_id: String,
    project_id: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<Node, String> {
    with_conn(&registry, &project_id, |conn| db_get_node(conn, &node_id))
}

#[tauri::command]
pub async fn list_nodes(
    project_id: String,
    phase_id: Option<String>,
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<Node>, String> {
    with_conn(&registry, &project_id, |conn| {
        db_list_nodes(conn, &project_id, phase_id.as_deref())
    })
}

#[tauri::command]
pub async fn create_edge(
    from_id: String,
    to_id: String,
    project_id: String,
    edge_type: Option<String>,
    registry: State<'_, ProjectRegistry>,
) -> Result<Edge, String> {
    let et = edge_type
        .as_deref()
        .unwrap_or("depends_on")
        .parse::<EdgeType>()
        .map_err(|e| e.to_string())?;
    with_conn(&registry, &project_id, |conn| db_create_edge(conn, &from_id, &to_id, et))
}

#[tauri::command]
pub async fn remove_edge(
    from_id: String,
    to_id: String,
    project_id: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<(), String> {
    with_conn(&registry, &project_id, |conn| db_remove_edge(conn, &from_id, &to_id))
}

#[tauri::command]
pub async fn create_artifact(
    input: CreateArtifactInput,
    registry: State<'_, ProjectRegistry>,
) -> Result<Artifact, String> {
    let project_id = input.project_id.clone();
    with_conn(&registry, &project_id, |conn| db_upsert_artifact(conn, &input))
}

#[tauri::command]
pub async fn list_artifacts(
    project_id: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<Artifact>, String> {
    with_conn(&registry, &project_id, |conn| db_list_artifacts(conn, &project_id))
}

#[tauri::command]
pub async fn create_knowledge(
    input: CreateKnowledgeInput,
    registry: State<'_, ProjectRegistry>,
) -> Result<KnowledgeEntry, String> {
    let project_id = input.project_id.clone();
    with_conn(&registry, &project_id, |conn| db_create_knowledge(conn, &input))
}

#[tauri::command]
pub async fn list_knowledge(
    project_id: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<KnowledgeEntry>, String> {
    with_conn(&registry, &project_id, |conn| db_list_knowledge(conn, &project_id))
}

#[tauri::command]
pub async fn list_queue_items(
    project_id: String,
    unresolved_only: Option<bool>,
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<QueueItem>, String> {
    let unresolved = unresolved_only.unwrap_or(false);
    with_conn(&registry, &project_id, |conn| {
        db_list_queue_items(conn, &project_id, unresolved)
    })
}

#[tauri::command]
pub async fn resolve_queue_item(
    item_id: String,
    project_id: String,
    resolution: String,
    registry: State<'_, ProjectRegistry>,
    app: tauri::AppHandle,
    dag_tx: State<'_, mpsc::UnboundedSender<crate::event_ingester::DagChanged>>,
) -> Result<QueueItem, String> {
    use tauri::Emitter;
    use crate::event_ingester::DagChanged;

    let (item, task_id, session_id) = with_conn(&registry, &project_id, |conn| {
        let item = db_resolve_queue_item(conn, &item_id, &resolution)?;
        let tid = item.task_id.clone().unwrap_or_default();
        let sid = db_get_agent_session_for_task(conn, &tid)?.unwrap_or_default();
        Ok((item, tid, sid))
    })?;

    // Emit frontend event so queue panel removes the item
    let _ = app.emit("poe-decision-resolved", serde_json::json!({
        "itemId": item_id,
        "projectId": project_id,
        "taskId": task_id,
    }));

    // Signal orchestrator to resume the waiting agent
    if !session_id.is_empty() {
        let _ = dag_tx.send(DagChanged::QueueItemResolved {
            project_id: project_id.clone(),
            item_id: item_id.clone(),
            task_id: task_id.clone(),
            session_id: session_id.clone(),
            resolution: resolution.clone(),
        });
    }

    Ok(item)
}

#[tauri::command]
pub async fn read_artifact_content(
    artifact_id: String,
    project_id: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<String, String> {
    with_conn(&registry, &project_id, |conn| {
        let filename: String = conn.query_row(
            "SELECT filename FROM artifacts WHERE id = ?1",
            [&artifact_id],
            |row| row.get(0),
        ).map_err(|e| anyhow::anyhow!("Artifact not found {}: {}", artifact_id, e))?;

        // Get the project path from the registry (we're inside with_conn so registry is unlocked)
        // We need to look up project path separately — we have access to conn but not db.project.path here.
        // Use a raw SQL query against the projects table to get path.
        let project_path: String = conn.query_row(
            "SELECT path FROM projects WHERE id = ?1",
            [&project_id],
            |row| row.get(0),
        ).map_err(|e| anyhow::anyhow!("Project not found {}: {}", project_id, e))?;

        let path = std::path::Path::new(&project_path).join("docs").join(&filename);
        std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("Failed to read artifact {:?}: {}", path, e))
    })
}

#[tauri::command]
pub async fn flag_knowledge_for_promotion(
    id: String,
    project_id: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<(), String> {
    with_conn(&registry, &project_id, |conn| {
        conn.execute(
            "UPDATE knowledge SET promoted = 1 WHERE id = ?1",
            [&id],
        ).map_err(|e| anyhow::anyhow!("Failed to flag knowledge: {}", e))?;
        Ok(())
    })
}

#[tauri::command]
pub async fn list_phases(
    project_id: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<Phase>, String> {
    with_conn(&registry, &project_id, |conn| db_list_phases(conn, &project_id))
}

#[tauri::command]
pub async fn list_events(
    project_id: String,
    since: Option<String>,
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<EventRecord>, String> {
    with_conn(&registry, &project_id, |conn| {
        db_list_events(conn, &project_id, since.as_deref())
    })
}

#[tauri::command]
pub async fn respond_to_chat(
    project_id: String,
    turn_id: String,
    response: String,
    registry: State<'_, ProjectRegistry>,
    app: tauri::AppHandle,
    dag_tx: State<'_, mpsc::UnboundedSender<crate::event_ingester::DagChanged>>,
) -> Result<(), String> {
    use tauri::Emitter;
    use crate::event_ingester::DagChanged;

    let (task_id, session_id) = with_conn(&registry, &project_id, |conn| {
        // Update response and responded_at
        conn.execute(
            "UPDATE chat_turns SET response = ?1, responded_at = datetime('now') WHERE id = ?2",
            rusqlite::params![response, turn_id],
        )
        .map_err(|e| anyhow::anyhow!("Failed to update chat turn: {}", e))?;

        // Look up task_id from this chat turn
        let tid: String = conn
            .query_row(
                "SELECT task_id FROM chat_turns WHERE id = ?1",
                [&turn_id],
                |row| row.get(0),
            )
            .map_err(|e| anyhow::anyhow!("Chat turn not found {}: {}", turn_id, e))?;

        // Look up session_id from nodes
        let sid = db_get_session_id_for_task(conn, &tid)?.unwrap_or_default();

        Ok((tid, sid))
    })?;

    // Emit frontend event
    let _ = app.emit("poe-chat-responded", serde_json::json!({
        "turnId": turn_id,
        "projectId": project_id,
        "taskId": task_id,
    }));

    // Signal orchestrator to resume the waiting agent (reuse QueueItemResolved)
    if !session_id.is_empty() {
        let _ = dag_tx.send(DagChanged::QueueItemResolved {
            project_id: project_id.clone(),
            item_id: turn_id.clone(),
            task_id: task_id.clone(),
            session_id: session_id.clone(),
            resolution: response.clone(),
        });
    }

    Ok(())
}

#[tauri::command]
pub async fn get_chat_turns(
    project_id: String,
    task_id: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<ChatTurn>, String> {
    with_conn(&registry, &project_id, |conn| {
        db_list_chat_turns(conn, &task_id)
    })
}
