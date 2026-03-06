// bp6-coj: Project Advisor — session management and context assembly

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::agents::{AgentState, SpawnAgentParams, spawn_agent_internal};
use crate::project::ProjectState;
use crate::skills::build_skills_prompt;

// ── Types ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AdvisorStartResult {
    pub agent_id: String,
    pub session_id: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AdvisorStateResult {
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    pub is_active: bool,
}

// ── Context assembler (bp6-coj.2) ─────────────────────────────────────────────

/// Build a structured markdown block describing current project state.
/// bp6-coj.2: STUBBED — real data sources (lifecycle engine, artefact manifest)
/// not yet built. Returns a placeholder until those features land.
pub fn build_advisor_context(_project_id: &str) -> String {
    let ts = chrono::Utc::now().to_rfc3339();
    format!(
        "## Project State — {ts}\n\n\
         _(Project state context assembly is pending — lifecycle engine and artefact sources not yet integrated.)_\n\n"
    )
}

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Load the project-advisor skill body (falls back to empty string gracefully).
fn load_advisor_skill(app: &AppHandle, project_dir: &str) -> String {
    let dir = PathBuf::from(project_dir);
    build_skills_prompt(
        &["project-advisor".to_string()],
        app,
        Some(&dir),
    )
}

/// Write a message to the advisor PTY, prefixed with fresh project state context.
fn write_advisor_turn(
    agent_state: &State<'_, AgentState>,
    agent_id: &str,
    project_id: &str,
    message: &str,
) -> Result<(), String> {
    let context = build_advisor_context(project_id);
    let text = format!("{context}{message}\n");

    let handles = agent_state.handles.lock().map_err(|e| e.to_string())?;
    let handle = handles
        .get(agent_id)
        .ok_or_else(|| format!("Advisor agent '{}' not found", agent_id))?;
    handle.write_text(&text)
}

// ── Tauri commands ─────────────────────────────────────────────────────────────

/// Start a new advisor query (or resume existing session).
/// Spawns the advisor PTY if not already active, then sends context + query.
/// Returns agent_id so the UI can subscribe to agent:stdout events.
#[tauri::command]
pub async fn start_advisor_query(
    project_dir: String,
    query: String,
    app: AppHandle,
    project_state: State<'_, ProjectState>,
    agent_state: State<'_, AgentState>,
) -> Result<AdvisorStartResult, String> {
    // Get project_id for this project_dir
    let project_id = {
        let projects = project_state.projects.lock().unwrap();
        projects
            .get(&project_dir)
            .map(|p| p.project_id.clone())
            .ok_or("Project not open")?
    };

    // Load current advisor state
    let (stored_session_id, stored_agent_id) = {
        let projects = project_state.projects.lock().unwrap();
        let proj = projects.get(&project_dir).ok_or("Project not open")?;
        proj.store.get_advisor_state(&project_id)?
    };

    // Check if advisor PTY is already alive
    let existing_active = stored_agent_id.as_deref().and_then(|aid| {
        let handles = agent_state.handles.lock().ok()?;
        if handles.contains_key(aid) { Some(aid.to_string()) } else { None }
    });

    let (agent_id, session_id) = if let Some(aid) = existing_active {
        // PTY is already running — just write to it
        let sid = stored_session_id.unwrap_or_else(|| aid.clone());
        write_advisor_turn(&agent_state, &aid, &project_id, &query)?;
        (aid, sid)
    } else {
        // Need to spawn a new PTY
        let new_agent_id = Uuid::new_v4().to_string();
        let (resume, session_id) = match stored_session_id {
            Some(ref sid) => (true, sid.clone()),
            None => (false, Uuid::new_v4().to_string()),
        };

        let spawned = spawn_agent_internal(
            SpawnAgentParams {
                cmd: "claude".to_string(),
                args: vec![],
                env: None,
                agent_id: Some(new_agent_id.clone()),
                workflow_id: Some(new_agent_id.clone()), // key for agent:stdout events
                node_id: None,
                workflow_type: Some("advisor".to_string()),
                session_id: Some(session_id.clone()),
                resume,
                cwd: Some(project_dir.clone()),
            },
            &app,
            &agent_state,
        )?;

        let aid = spawned.agent_id;

        // Persist new state
        {
            let projects = project_state.projects.lock().unwrap();
            if let Some(proj) = projects.get(&project_dir) {
                proj.store.set_advisor_state(&project_id, Some(&session_id), Some(&aid))?;
            }
        }

        // On fresh session, prepend the skill content to the first message
        let skill_prefix = if !resume {
            let s = load_advisor_skill(&app, &project_dir);
            if s.is_empty() { String::new() } else { format!("{s}\n\n") }
        } else {
            String::new()
        };

        let context = build_advisor_context(&project_id);
        let text = format!("{skill_prefix}{context}{query}\n");

        // Small delay to let PTY initialise before writing
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        {
            let handles = agent_state.handles.lock().map_err(|e| e.to_string())?;
            if let Some(handle) = handles.get(&aid) {
                handle.write_text(&text)?;
            }
        }

        (aid, session_id)
    };

    Ok(AdvisorStartResult { agent_id, session_id })
}

/// Send a follow-up message to an existing advisor session.
#[tauri::command]
pub fn send_advisor_message(
    agent_id: String,
    message: String,
    project_dir: String,
    project_state: State<'_, ProjectState>,
    agent_state: State<'_, AgentState>,
) -> Result<(), String> {
    let project_id = {
        let projects = project_state.projects.lock().unwrap();
        projects
            .get(&project_dir)
            .map(|p| p.project_id.clone())
            .ok_or("Project not open")?
    };
    write_advisor_turn(&agent_state, &agent_id, &project_id, &message)
}

/// Reset the advisor session: kill active PTY and clear stored state.
/// Next query will start a fresh Claude session.
#[tauri::command]
pub fn reset_advisor_session(
    project_dir: String,
    project_state: State<'_, ProjectState>,
    agent_state: State<'_, AgentState>,
) -> Result<(), String> {
    let project_id = {
        let projects = project_state.projects.lock().unwrap();
        projects
            .get(&project_dir)
            .map(|p| p.project_id.clone())
            .ok_or("Project not open")?
    };

    // Find and kill the active advisor agent
    let stored_agent_id = {
        let projects = project_state.projects.lock().unwrap();
        let proj = projects.get(&project_dir).ok_or("Project not open")?;
        proj.store.get_advisor_state(&project_id)?.1
    };

    if let Some(aid) = stored_agent_id {
        let mut handles = agent_state.handles.lock().map_err(|e| e.to_string())?;
        if let Some(handle) = handles.remove(&aid) {
            drop(handle); // PTY cleanup on drop (SIGTERM via AgentHandle Drop impl)
        }
    }

    // Clear stored state
    {
        let projects = project_state.projects.lock().unwrap();
        if let Some(proj) = projects.get(&project_dir) {
            proj.store.set_advisor_state(&project_id, None, None)?;
        }
    }

    Ok(())
}

/// Get current advisor state for the project.
#[tauri::command]
pub fn get_advisor_state(
    project_dir: String,
    project_state: State<'_, ProjectState>,
    agent_state: State<'_, AgentState>,
) -> Result<AdvisorStateResult, String> {
    let project_id = {
        let projects = project_state.projects.lock().unwrap();
        projects
            .get(&project_dir)
            .map(|p| p.project_id.clone())
            .ok_or("Project not open")?
    };

    let (session_id, agent_id) = {
        let projects = project_state.projects.lock().unwrap();
        let proj = projects.get(&project_dir).ok_or("Project not open")?;
        proj.store.get_advisor_state(&project_id)?
    };

    let is_active = agent_id.as_deref().map_or(false, |aid| {
        agent_state.handles.lock().map_or(false, |h| h.contains_key(aid))
    });

    Ok(AdvisorStateResult { session_id, agent_id, is_active })
}
