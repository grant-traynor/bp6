// bp6-coj + bp6-8nr: Project Advisor
//
// Each query spawns a fresh one-shot process:
//   claude -p "prompt" --output-format stream-json [--session-id | --resume] <uuid>
//
// This produces clean NDJSON on stdout (no ANSI noise). The frontend parses
// the JSON events to render text, tool activity, and diffs properly.
// Session continuity is preserved via --session-id (first turn) / --resume (subsequent).

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
    /// is_active = a session exists (session_id is stored). Per-query processes
    /// are ephemeral so there is no persistent agent_id to check.
    pub is_active: bool,
}

// ── Context assembler (bp6-coj.2) ─────────────────────────────────────────────

/// Build a structured markdown block describing current project state.
/// STUBBED — real data sources (lifecycle engine, artefact manifest) not yet built.
pub fn build_advisor_context(_project_id: &str) -> String {
    let ts = chrono::Utc::now().to_rfc3339();
    format!(
        "## Project State — {ts}\n\n\
         _(Project state context assembly is pending — lifecycle engine and artefact sources not yet integrated.)_\n\n"
    )
}

// ── Internal helpers ───────────────────────────────────────────────────────────

fn load_advisor_skill(app: &AppHandle, project_dir: &str) -> String {
    build_skills_prompt(
        &["project-advisor".to_string()],
        app,
        Some(&PathBuf::from(project_dir)),
    )
}

/// Spawn a one-shot advisor process. Returns agent_id (for agent:stdout subscription).
fn spawn_advisor_turn(
    project_dir: &str,
    full_prompt: String,
    session_id: &str,
    is_resume: bool,
    app: &AppHandle,
    agent_state: &State<'_, AgentState>,
) -> Result<String, String> {
    let agent_id = Uuid::new_v4().to_string();

    // Build: claude -p "<prompt>" --output-format stream-json [--resume | --session-id] <sid>
    let mut args = vec![
        "-p".to_string(),
        full_prompt,
        "--output-format".to_string(),
        "stream-json".to_string(),
    ];
    if is_resume {
        args.push("--resume".to_string());
    } else {
        args.push("--session-id".to_string());
    }
    args.push(session_id.to_string());

    spawn_agent_internal(
        SpawnAgentParams {
            cmd: "claude".to_string(),
            args,
            env: None,
            agent_id: Some(agent_id.clone()),
            workflow_id: Some(agent_id.clone()), // agent:stdout events keyed by this
            node_id: None,
            workflow_type: Some("advisor".to_string()),
            session_id: None, // handled manually in args
            resume: false,
            cwd: Some(project_dir.to_string()),
        },
        app,
        agent_state,
    )?;

    Ok(agent_id)
}

/// Core query logic shared by start_advisor_query and send_advisor_message.
async fn run_advisor_query(
    project_dir: String,
    message: String,
    app: AppHandle,
    project_state: State<'_, ProjectState>,
    agent_state: State<'_, AgentState>,
) -> Result<AdvisorStartResult, String> {
    let project_id = {
        let projects = project_state.projects.lock().unwrap();
        projects
            .get(&project_dir)
            .map(|p| p.project_id.clone())
            .ok_or("Project not open")?
    };

    let (stored_session_id, _) = {
        let projects = project_state.projects.lock().unwrap();
        let proj = projects.get(&project_dir).ok_or("Project not open")?;
        proj.store.get_advisor_state(&project_id)?
    };

    let is_resume = stored_session_id.is_some();
    let session_id = stored_session_id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let context = build_advisor_context(&project_id);

    // First turn: prepend skill prompt so it becomes the system context for this session.
    // Subsequent turns: skill is already in the session context via --resume.
    let full_prompt = if !is_resume {
        let skill = load_advisor_skill(&app, &project_dir);
        if skill.is_empty() {
            format!("{context}{message}")
        } else {
            format!("{skill}\n\n{context}{message}")
        }
    } else {
        format!("{context}{message}")
    };

    let agent_id = spawn_advisor_turn(
        &project_dir,
        full_prompt,
        &session_id,
        is_resume,
        &app,
        &agent_state,
    )?;

    // Persist session_id. Agent_id is ephemeral (new per turn) — not stored.
    {
        let projects = project_state.projects.lock().unwrap();
        if let Some(proj) = projects.get(&project_dir) {
            proj.store.set_advisor_state(&project_id, Some(&session_id), None)?;
        }
    }

    Ok(AdvisorStartResult { agent_id, session_id })
}

// ── Tauri commands ─────────────────────────────────────────────────────────────

/// Send a query to the advisor. Spawns a fresh one-shot process per call.
/// Returns agent_id so the UI can subscribe to that turn's agent:stdout events.
#[tauri::command]
pub async fn start_advisor_query(
    project_dir: String,
    query: String,
    app: AppHandle,
    project_state: State<'_, ProjectState>,
    agent_state: State<'_, AgentState>,
) -> Result<AdvisorStartResult, String> {
    run_advisor_query(project_dir, query, app, project_state, agent_state).await
}

/// Send a follow-up message. Identical to start_advisor_query — always resumes
/// the stored session. Kept as a separate command for semantic clarity in the UI.
#[tauri::command]
pub async fn send_advisor_message(
    project_dir: String,
    message: String,
    app: AppHandle,
    project_state: State<'_, ProjectState>,
    agent_state: State<'_, AgentState>,
) -> Result<AdvisorStartResult, String> {
    run_advisor_query(project_dir, message, app, project_state, agent_state).await
}

/// Reset the advisor session. Clears stored session_id so next query starts fresh.
/// No PTY to kill — per-query processes self-terminate.
#[tauri::command]
pub fn reset_advisor_session(
    project_dir: String,
    project_state: State<'_, ProjectState>,
) -> Result<(), String> {
    let project_id = {
        let projects = project_state.projects.lock().unwrap();
        projects
            .get(&project_dir)
            .map(|p| p.project_id.clone())
            .ok_or("Project not open")?
    };
    let projects = project_state.projects.lock().unwrap();
    if let Some(proj) = projects.get(&project_dir) {
        proj.store.set_advisor_state(&project_id, None, None)?;
    }
    Ok(())
}

/// Get current advisor state for the project.
#[tauri::command]
pub fn get_advisor_state(
    project_dir: String,
    project_state: State<'_, ProjectState>,
) -> Result<AdvisorStateResult, String> {
    let project_id = {
        let projects = project_state.projects.lock().unwrap();
        projects
            .get(&project_dir)
            .map(|p| p.project_id.clone())
            .ok_or("Project not open")?
    };
    let projects = project_state.projects.lock().unwrap();
    let proj = projects.get(&project_dir).ok_or("Project not open")?;
    let (session_id, _) = proj.store.get_advisor_state(&project_id)?;
    let is_active = session_id.is_some();
    Ok(AdvisorStateResult { session_id, is_active })
}
