use super::{interrupt_agent_process, write_to_agent_stdin, AgentMap};
use crate::dag_store::{AgentRecord, ProjectRegistry};
use tauri::State;

#[tauri::command]
pub async fn write_to_agent(
    agent_id: String,
    content: String,
    agent_map: State<'_, AgentMap>,
) -> Result<(), String> {
    write_to_agent_stdin(&agent_map, &agent_id, &content).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn interrupt_agent(
    agent_id: String,
    agent_map: State<'_, AgentMap>,
) -> Result<(), String> {
    interrupt_agent_process(&agent_map, &agent_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_agents(
    project_id: String,
    status: Option<String>,
    registry: State<'_, ProjectRegistry>,
) -> Result<Vec<AgentRecord>, String> {
    let reg = registry.lock().unwrap();
    let db = reg
        .get(&project_id)
        .ok_or_else(|| format!("Project not open: {}", project_id))?
        .clone();
    drop(reg);

    let conn = db.conn.lock().unwrap();
    let s = status.as_deref().unwrap_or("running");
    crate::dag_store::db_list_agents_by_status(&conn, &project_id, s).map_err(|e| e.to_string())
}
