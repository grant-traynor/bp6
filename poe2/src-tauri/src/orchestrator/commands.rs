use super::ConcurrencyLimits;
use crate::agent_lifecycle::{AgentMap, interrupt_agent_process};
use crate::dag_store::{self, NodeStatus, ProjectRegistry, UpdateNodeInput};
use crate::event_ingester::DagChanged;
use std::sync::Arc;
use tauri::{Emitter, State};
use tokio::sync::mpsc;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConcurrencyStatus {
    pub project_id: String,
    pub running: usize,
    pub project_limit: usize,
    pub global_running: usize,
    pub global_limit: usize,
}

#[tauri::command]
pub async fn advance_stage_gate(
    phase_id: String,
    project_id: String,
    registry: State<'_, ProjectRegistry>,
    dag_tx: State<'_, mpsc::UnboundedSender<DagChanged>>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let reg = registry.lock().unwrap();
    let db = reg
        .get(&project_id)
        .ok_or_else(|| format!("Project not open: {}", project_id))?
        .clone();
    drop(reg);

    {
        let conn = db.conn.lock().unwrap();
        dag_store::db_advance_stage_gate(&conn, &phase_id).map_err(|e| e.to_string())?;
    }

    // Emit phase update event
    app.emit("poe-phase-update", serde_json::json!({
        "phaseId": phase_id,
        "status": "gate",
        "projectId": project_id,
    })).ok();

    // Notify orchestrator — gate advancement may unlock pending tasks
    let _ = dag_tx.send(DagChanged::DagStructureChanged { project_id });
    Ok(())
}

#[tauri::command]
pub async fn set_concurrency_limit(
    project_id: Option<String>,
    limit: usize,
    limits: State<'_, Arc<ConcurrencyLimits>>,
) -> Result<(), String> {
    if let Some(pid) = project_id {
        limits.set_project_limit(&pid, limit);
    } else {
        limits
            .global_limit
            .store(limit, std::sync::atomic::Ordering::Relaxed);
    }
    Ok(())
}

/// Cancel a single task and SIGTERM its running agent.
#[tauri::command]
pub async fn cancel_task(
    node_id: String,
    project_id: String,
    registry: State<'_, ProjectRegistry>,
    agent_map: State<'_, AgentMap>,
    dag_tx: State<'_, mpsc::UnboundedSender<DagChanged>>,
) -> Result<dag_store::Node, String> {
    // SIGTERM any running agent for this task
    {
        let map = agent_map.lock().unwrap();
        for agent in map.values() {
            if agent.task_id == node_id {
                let _ = interrupt_agent_process(&agent_map, &agent.agent_id);
                break;
            }
        }
    }
    // Cancel in DB
    let reg = registry.lock().unwrap();
    let db = reg.get(&project_id)
        .ok_or_else(|| format!("Project not open: {}", project_id))?.clone();
    drop(reg);
    let node = {
        let conn = db.conn.lock().unwrap();
        dag_store::db_cancel_node(&conn, &node_id).map_err(|e| e.to_string())?
    };
    let _ = dag_tx.send(DagChanged::DagStructureChanged { project_id });
    Ok(node)
}

/// SIGTERM all running agents in a phase; re-queue their tasks to Pending; hold the gate.
#[tauri::command]
pub async fn pause_stage(
    phase_id: String,
    project_id: String,
    registry: State<'_, ProjectRegistry>,
    agent_map: State<'_, AgentMap>,
    dag_tx: State<'_, mpsc::UnboundedSender<DagChanged>>,
) -> Result<(), String> {
    // Collect task_ids running in this phase
    let task_ids_in_phase: Vec<String> = {
        let reg = registry.lock().unwrap();
        let db = reg.get(&project_id)
            .ok_or_else(|| format!("Project not open: {}", project_id))?.clone();
        drop(reg);
        let conn = db.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id FROM nodes WHERE phase_id = ?1 AND status = 'running'"
        ).map_err(|e| e.to_string())?;
        let ids: Vec<String> = stmt.query_map([&phase_id], |row| row.get(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        ids
    };

    // SIGTERM agents for those tasks
    {
        let map = agent_map.lock().unwrap();
        for agent in map.values() {
            if task_ids_in_phase.contains(&agent.task_id) {
                let _ = interrupt_agent_process(&agent_map, &agent.agent_id);
            }
        }
    }

    // Re-queue tasks to Pending and hold gate
    {
        let reg = registry.lock().unwrap();
        let db = reg.get(&project_id)
            .ok_or_else(|| format!("Project not open: {}", project_id))?.clone();
        drop(reg);
        let conn = db.conn.lock().unwrap();
        for task_id in &task_ids_in_phase {
            let update = UpdateNodeInput {
                status: Some(NodeStatus::Pending),
                title: None, description: None, skill_id: None, assignee: None,
                ..Default::default()
            };
            let _ = dag_store::db_update_node(&conn, task_id, &update);
        }
        let now = chrono::Utc::now().to_rfc3339();
        let _ = conn.execute(
            "UPDATE phases SET status = 'paused', updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, phase_id],
        );
    }

    let _ = dag_tx.send(DagChanged::DagStructureChanged { project_id });
    Ok(())
}

/// SIGTERM all running agents across the project; re-queue their tasks to Pending.
#[tauri::command]
pub async fn abort_project(
    project_id: String,
    registry: State<'_, ProjectRegistry>,
    agent_map: State<'_, AgentMap>,
    dag_tx: State<'_, mpsc::UnboundedSender<DagChanged>>,
) -> Result<(), String> {
    // Collect all agent_ids for this project
    let agent_ids: Vec<String> = {
        let map = agent_map.lock().unwrap();
        map.values()
            .filter(|a| a.project_id == project_id)
            .map(|a| a.agent_id.clone())
            .collect()
    };

    for agent_id in &agent_ids {
        let _ = interrupt_agent_process(&agent_map, agent_id);
    }

    // Re-queue all running tasks for this project to Pending; mark project paused
    {
        let reg = registry.lock().unwrap();
        let db = reg.get(&project_id)
            .ok_or_else(|| format!("Project not open: {}", project_id))?.clone();
        drop(reg);
        let conn = db.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        let _ = conn.execute(
            "UPDATE nodes SET status = 'pending', updated_at = ?1 WHERE project_id = ?2 AND status = 'running'",
            rusqlite::params![now, project_id],
        );
        let _ = conn.execute(
            "UPDATE projects SET status = 'paused', updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, project_id],
        );
    }

    let _ = dag_tx.send(DagChanged::DagStructureChanged { project_id });
    Ok(())
}

/// Reset a phase back to Planning stage (allows plan changes before re-running).
#[tauri::command]
pub async fn revise_stage(
    phase_id: String,
    project_id: String,
    registry: State<'_, ProjectRegistry>,
    dag_tx: State<'_, mpsc::UnboundedSender<DagChanged>>,
) -> Result<(), String> {
    let reg = registry.lock().unwrap();
    let db = reg.get(&project_id)
        .ok_or_else(|| format!("Project not open: {}", project_id))?.clone();
    drop(reg);
    {
        let conn = db.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE phases SET status = 'pending', updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, phase_id],
        ).map_err(|e| e.to_string())?;
    }
    let _ = dag_tx.send(DagChanged::DagStructureChanged { project_id });
    Ok(())
}

/// Re-queue all tasks in a phase to Pending and release the gate so the orchestrator picks them up.
#[tauri::command]
pub async fn rerun_stage(
    phase_id: String,
    project_id: String,
    registry: State<'_, ProjectRegistry>,
    dag_tx: State<'_, mpsc::UnboundedSender<DagChanged>>,
) -> Result<(), String> {
    let reg = registry.lock().unwrap();
    let db = reg.get(&project_id)
        .ok_or_else(|| format!("Project not open: {}", project_id))?.clone();
    drop(reg);
    {
        let conn = db.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        // Re-queue all non-cancelled tasks in this phase
        conn.execute(
            "UPDATE nodes SET status = 'pending', updated_at = ?1 WHERE phase_id = ?2 AND status != 'cancelled'",
            rusqlite::params![now, phase_id],
        ).map_err(|e| e.to_string())?;
        // Set phase to running
        conn.execute(
            "UPDATE phases SET status = 'running', updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, phase_id],
        ).map_err(|e| e.to_string())?;
    }
    let _ = dag_tx.send(DagChanged::DagStructureChanged { project_id });
    Ok(())
}

/// Resume a paused stage — sets status back to 'running' and emits poe-phase-update.
#[tauri::command]
pub async fn resume_stage(
    phase_id: String,
    project_id: String,
    registry: State<'_, ProjectRegistry>,
    dag_tx: State<'_, mpsc::UnboundedSender<DagChanged>>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let reg = registry.lock().unwrap();
    let db = reg
        .get(&project_id)
        .ok_or_else(|| format!("Project not open: {}", project_id))?
        .clone();
    drop(reg);
    {
        let conn = db.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE phases SET status = 'running', updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, phase_id],
        )
        .map_err(|e| e.to_string())?;
    }
    app.emit("poe-phase-update", serde_json::json!({
        "phaseId": phase_id,
        "status": "running",
        "projectId": project_id,
    })).ok();
    let _ = dag_tx.send(DagChanged::DagStructureChanged { project_id });
    Ok(())
}

/// Resume a paused/aborted project — sets projects.status back to 'active' and resumes dispatch.
#[tauri::command]
pub async fn resume_project(
    project_id: String,
    registry: State<'_, ProjectRegistry>,
    dag_tx: State<'_, mpsc::UnboundedSender<DagChanged>>,
) -> Result<(), String> {
    let reg = registry.lock().unwrap();
    let db = reg
        .get(&project_id)
        .ok_or_else(|| format!("Project not open: {}", project_id))?
        .clone();
    drop(reg);
    {
        let conn = db.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE projects SET status = 'active', updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, project_id],
        )
        .map_err(|e| e.to_string())?;
    }
    let _ = dag_tx.send(DagChanged::DagStructureChanged { project_id });
    Ok(())
}
