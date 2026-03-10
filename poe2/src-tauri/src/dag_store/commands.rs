use super::*;
use tauri::{Manager, State};
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

// ── Phase 3 commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_edges(
    project_id: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<Edge>, String> {
    with_conn(&registry, &project_id, |conn| db_list_edges(conn, &project_id))
}

#[tauri::command]
pub async fn update_node_sort_order(
    node_id: String,
    sort_order: Option<i32>,
    project_id: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<(), String> {
    with_conn(&registry, &project_id, |conn| {
        db_update_node_sort_order(conn, &node_id, sort_order)
    })
}

#[tauri::command]
pub async fn create_phase(
    project_id: String,
    name: String,
    stage_type: String,
    number: i64,
    registry: State<'_, ProjectRegistry>,
) -> Result<Phase, String> {
    // Validate stage_type against catalogue
    const VALID_STAGE_TYPES: &[&str] = &[
        "conops", "guardrails", "increment_planning", "execution",
        "pm_review", "rework", "validity_analysis", "retrospective", "onboarding",
    ];
    if !VALID_STAGE_TYPES.contains(&stage_type.as_str()) {
        return Err(format!(
            "Invalid stage_type '{}'. Valid values: {}",
            stage_type,
            VALID_STAGE_TYPES.join(", ")
        ));
    }
    with_conn(&registry, &project_id, |conn| {
        db_create_phase(conn, &project_id, &name, number, &stage_type)
    })
}

#[tauri::command]
pub async fn activate_phase(
    project_id: String,
    phase_id: String,
    registry: State<'_, ProjectRegistry>,
    app: tauri::AppHandle,
    dag_tx: State<'_, mpsc::UnboundedSender<crate::event_ingester::DagChanged>>,
) -> Result<(), String> {
    use tauri::Emitter;
    use crate::event_ingester::DagChanged;

    with_conn(&registry, &project_id, |conn| {
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE phases SET status = 'running', lifecycle_stage = 'execution', gate_held = 0, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, phase_id],
        )
        .map_err(|e| anyhow::anyhow!("Failed to activate phase: {}", e))?;
        Ok(())
    })?;

    let _ = app.emit("poe-phase-update", serde_json::json!({
        "phaseId": phase_id,
        "status": "running",
        "projectId": project_id,
    }));
    let _ = dag_tx.send(DagChanged::DagStructureChanged { project_id });
    Ok(())
}

#[tauri::command]
pub async fn advance_phase(
    project_id: String,
    phase_id: String,
    registry: State<'_, ProjectRegistry>,
    app: tauri::AppHandle,
    dag_tx: State<'_, mpsc::UnboundedSender<crate::event_ingester::DagChanged>>,
) -> Result<(), String> {
    use tauri::Emitter;
    use crate::event_ingester::DagChanged;

    // Get next phase (ordered by number, with number > current phase's number)
    let next_phase_id = with_conn(&registry, &project_id, |conn| {
        // Mark current phase complete
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE phases SET status = 'complete', gate_held = 0, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, phase_id],
        )
        .map_err(|e| anyhow::anyhow!("Failed to complete phase: {}", e))?;

        // Find next phase by number
        let current_number: i64 = conn.query_row(
            "SELECT number FROM phases WHERE id = ?1",
            [&phase_id],
            |row| row.get(0),
        )
        .map_err(|e| anyhow::anyhow!("Phase not found: {}", e))?;

        let next_id: rusqlite::Result<String> = conn.query_row(
            "SELECT id FROM phases WHERE project_id = ?1 AND number > ?2 ORDER BY number LIMIT 1",
            rusqlite::params![project_id, current_number],
            |row| row.get(0),
        );
        match next_id {
            Ok(id) => {
                conn.execute(
                    "UPDATE phases SET status = 'running', lifecycle_stage = 'execution', gate_held = 0, updated_at = ?1 WHERE id = ?2",
                    rusqlite::params![now, id],
                )
                .map_err(|e| anyhow::anyhow!("Failed to activate next phase: {}", e))?;
                Ok(Some(id))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("Failed to find next phase: {}", e)),
        }
    })?;

    let _ = app.emit("poe-phase-update", serde_json::json!({
        "phaseId": phase_id,
        "status": "complete",
        "projectId": project_id,
    }));
    if let Some(next_id) = next_phase_id {
        let _ = app.emit("poe-phase-update", serde_json::json!({
            "phaseId": next_id,
            "status": "running",
            "projectId": project_id,
        }));
    }
    let _ = dag_tx.send(DagChanged::DagStructureChanged { project_id });
    Ok(())
}

#[tauri::command]
pub async fn revise_phase(
    project_id: String,
    phase_id: String,
    task_ids: Vec<String>,
    registry: State<'_, ProjectRegistry>,
    app: tauri::AppHandle,
    dag_tx: State<'_, mpsc::UnboundedSender<crate::event_ingester::DagChanged>>,
) -> Result<(), String> {
    use tauri::Emitter;
    use crate::event_ingester::DagChanged;

    with_conn(&registry, &project_id, |conn| {
        let now = chrono::Utc::now().to_rfc3339();
        for task_id in &task_ids {
            conn.execute(
                "UPDATE nodes SET status = 'pending', updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now, task_id],
            )
            .map_err(|e| anyhow::anyhow!("Failed to reset task {}: {}", task_id, e))?;
        }
        conn.execute(
            "UPDATE phases SET status = 'running', gate_held = 0, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, phase_id],
        )
        .map_err(|e| anyhow::anyhow!("Failed to update phase: {}", e))?;
        Ok(())
    })?;

    let _ = app.emit("poe-phase-update", serde_json::json!({
        "phaseId": phase_id,
        "status": "running",
        "projectId": project_id,
    }));
    let _ = dag_tx.send(DagChanged::DagStructureChanged { project_id });
    Ok(())
}

#[tauri::command]
pub async fn rerun_phase(
    project_id: String,
    phase_id: String,
    registry: State<'_, ProjectRegistry>,
    app: tauri::AppHandle,
    dag_tx: State<'_, mpsc::UnboundedSender<crate::event_ingester::DagChanged>>,
) -> Result<(), String> {
    use tauri::Emitter;
    use crate::event_ingester::DagChanged;

    with_conn(&registry, &project_id, |conn| {
        let now = chrono::Utc::now().to_rfc3339();
        // Reset all done/complete tasks in phase to pending
        conn.execute(
            "UPDATE nodes SET status = 'pending', updated_at = ?1 WHERE phase_id = ?2 AND status IN ('complete', 'done')",
            rusqlite::params![now, phase_id],
        )
        .map_err(|e| anyhow::anyhow!("Failed to reset tasks: {}", e))?;
        conn.execute(
            "UPDATE phases SET status = 'running', gate_held = 0, updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, phase_id],
        )
        .map_err(|e| anyhow::anyhow!("Failed to update phase: {}", e))?;
        Ok(())
    })?;

    let _ = app.emit("poe-phase-update", serde_json::json!({
        "phaseId": phase_id,
        "status": "running",
        "projectId": project_id,
    }));
    let _ = dag_tx.send(DagChanged::DagStructureChanged { project_id });
    Ok(())
}

#[tauri::command]
pub async fn update_knowledge(
    project_id: String,
    id: String,
    content: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<(), String> {
    with_conn(&registry, &project_id, |conn| {
        conn.execute(
            "UPDATE knowledge SET value = ?1 WHERE id = ?2",
            rusqlite::params![content, id],
        )
        .map_err(|e| anyhow::anyhow!("Failed to update knowledge: {}", e))?;
        Ok(())
    })
}

#[tauri::command]
pub async fn promote_knowledge(
    project_id: String,
    id: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<(), String> {
    let entry = with_conn(&registry, &project_id, |conn| {
        db_get_knowledge(conn, &id)
    })?;

    // Write to ~/.poe/knowledge/{key}.md
    let home = dirs::home_dir()
        .ok_or_else(|| "Cannot determine home directory".to_owned())?;
    let knowledge_dir = home.join(".poe").join("knowledge");
    std::fs::create_dir_all(&knowledge_dir)
        .map_err(|e| format!("Failed to create knowledge dir: {}", e))?;
    let file_path = knowledge_dir.join(format!("{}.md", entry.key));
    std::fs::write(&file_path, &entry.value)
        .map_err(|e| format!("Failed to write knowledge file: {}", e))?;

    // Log to event_log and set promoted flag
    with_conn(&registry, &project_id, |conn| {
        let now = chrono::Utc::now().to_rfc3339();
        let log_id = uuid::Uuid::new_v4().to_string();
        let payload = serde_json::json!({
            "id": id,
            "key": entry.key,
            "path": file_path.to_string_lossy(),
        }).to_string();
        conn.execute(
            "INSERT INTO events (id, project_id, agent_id, task_id, event_type, payload, created_at)
             VALUES (?1, ?2, NULL, NULL, 'knowledge:promoted', ?3, ?4)",
            rusqlite::params![log_id, project_id, payload, now],
        )
        .map_err(|e| anyhow::anyhow!("Failed to log promotion: {}", e))?;
        conn.execute(
            "UPDATE knowledge SET promoted = 1 WHERE id = ?1",
            [&id],
        )
        .map_err(|e| anyhow::anyhow!("Failed to set promoted flag: {}", e))?;
        Ok(())
    })
}

#[tauri::command]
pub async fn get_node_ancestry(
    project_id: String,
    node_id: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<Node>, String> {
    with_conn(&registry, &project_id, |conn| {
        db_get_node_ancestry(conn, &node_id)
    })
}

#[tauri::command]
pub async fn start_advisor_session(
    project_id: String,
    decision_id: Option<String>,
    registry: State<'_, ProjectRegistry>,
    app: tauri::AppHandle,
    dag_tx: State<'_, mpsc::UnboundedSender<crate::event_ingester::DagChanged>>,
) -> Result<(), String> {
    use crate::event_ingester::DagChanged;
    use crate::skills::{self, SpawnMode};
    use crate::agent_lifecycle::{self, SpawnRequest};

    // Build title based on whether we have a decision_id
    let title = if let Some(ref did) = decision_id {
        // Look up the decision question to build a meaningful title
        let question = with_conn(&registry, &project_id, |conn| {
            let q: rusqlite::Result<String> = conn.query_row(
                "SELECT question FROM queue_items WHERE id = ?1",
                rusqlite::params![did],
                |row| row.get(0),
            );
            match q {
                Ok(q) => Ok(q),
                Err(_) => Ok("decision".to_owned()),
            }
        }).unwrap_or_else(|_| "decision".to_owned());
        format!("Research for: {}", question)
    } else {
        "Advisor session".to_owned()
    };

    // Create advisor task node
    let task_node = with_conn(&registry, &project_id, |conn| {
        let input = CreateNodeInput {
            project_id: project_id.clone(),
            phase_id: None,
            parent_id: None,
            node_type: NodeType::Advisor,
            title: title.clone(),
            description: decision_id.as_ref().map(|d| format!("Advisor session for decision: {}", d)),
            skill_id: Some("advisor".to_owned()),
            initial_status: Some(NodeStatus::Pending),
            requesting_task_id: None,
            review_id: None,
            retry_count: Some(0),
        };
        db_create_node(conn, &input)
    })?;

    // Load advisor skill
    let (project_path, skill, knowledge, artifacts) = {
        let reg = registry.lock().unwrap();
        let db = reg
            .values()
            .find(|db| db.project.id == project_id)
            .ok_or_else(|| format!("Project not open: {}", project_id))?
            .clone();
        drop(reg);

        let project_path = std::path::PathBuf::from(&db.project.path);
        let resource_dir = app.path().resource_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

        let conn = db.conn.lock().unwrap();
        let knowledge: Vec<(String, String)> = db_list_knowledge(&conn, &project_id)
            .unwrap_or_default()
            .into_iter()
            .map(|k| (k.key, k.value))
            .collect();
        let artifacts: Vec<(String, String)> = db_list_artifacts(&conn, &project_id)
            .unwrap_or_default()
            .into_iter()
            .map(|a| (a.artifact_type, a.filename))
            .collect();
        drop(conn);

        let skill = skills::load_skill("advisor", &project_path, &resource_dir)
            .map_err(|e| format!("Failed to load advisor skill: {}", e))?;
        (project_path, skill, knowledge, artifacts)
    };

    let knowledge_refs: Vec<(&str, &str)> = knowledge.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let artifact_refs: Vec<(&str, &str)> = artifacts.iter().map(|(t, f)| (t.as_str(), f.as_str())).collect();

    let input_bundle = skills::assemble_input_bundle(
        &SpawnMode::Interactive,
        &skill,
        &title,
        None,
        &[],
        &knowledge_refs,
        &artifact_refs,
    );

    let spawn_req = SpawnRequest {
        project_id: project_id.clone(),
        task_id: task_node.id.clone(),
        skill_id: "advisor".to_owned(),
        project_path,
        input_bundle,
        resume_session_id: None,
        model: skill.model.clone(),
    };

    let _ = dag_tx.send(DagChanged::DagStructureChanged { project_id: project_id.clone() });

    agent_lifecycle::spawn_agent(spawn_req, &registry, &dag_tx, &app)
        .await
        .map_err(|e| format!("Failed to spawn advisor agent: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn respond_to_advisor(
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
            "UPDATE advisor_turns SET response = ?1, responded_at = datetime('now') WHERE id = ?2",
            rusqlite::params![response, turn_id],
        )
        .map_err(|e| anyhow::anyhow!("Failed to update advisor turn: {}", e))?;

        // Look up task_id from this advisor turn
        let tid: String = conn
            .query_row(
                "SELECT task_id FROM advisor_turns WHERE id = ?1",
                [&turn_id],
                |row| row.get(0),
            )
            .map_err(|e| anyhow::anyhow!("Advisor turn not found {}: {}", turn_id, e))?;

        // Look up session_id from nodes
        let sid = db_get_session_id_for_task(conn, &tid)?.unwrap_or_default();

        Ok((tid, sid))
    })?;

    // Emit frontend event
    let _ = app.emit("poe-advisor-responded", serde_json::json!({
        "turnId": turn_id,
        "projectId": project_id,
        "taskId": task_id,
    }));

    // Signal orchestrator to resume the waiting advisor agent (reuse QueueItemResolved)
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
pub async fn get_advisor_turns(
    project_id: String,
    task_id: String,
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<AdvisorTurn>, String> {
    with_conn(&registry, &project_id, |conn| {
        db_list_advisor_turns(conn, &task_id)
    })
}
