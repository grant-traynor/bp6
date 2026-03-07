use super::ConcurrencyLimits;
use crate::dag_store::{self, ProjectRegistry};
use crate::event_ingester::DagChanged;
use std::sync::Arc;
use tauri::State;
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
) -> Result<(), String> {
    let reg = registry.lock().unwrap();
    let db = reg
        .values()
        .find(|db| db.project.id == project_id)
        .ok_or_else(|| format!("Project not open: {}", project_id))?
        .clone();
    drop(reg);

    {
        let conn = db.conn.lock().unwrap();
        dag_store::db_advance_stage_gate(&conn, &phase_id).map_err(|e| e.to_string())?;
    }

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
