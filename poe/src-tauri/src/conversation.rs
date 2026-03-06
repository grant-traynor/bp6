// bp6-ar2: Conversational lifecycle turns via claude CLI
//
// Mirrors the advisor pattern: each turn spawns a fresh one-shot process:
//   claude -p "prompt" --output-format stream-json [--session-id | --resume] <uuid>
//
// First turn:   skill content + artefact context + user message  → --session-id <new-uuid>
// Continuation: user message only                                → --resume <existing-session-id>
//
// The frontend subscribes to agent:stdout events keyed by the returned agent_id,
// parses the stream-json NDJSON, and extracts text + poe: JSON events.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::agents::{AgentState, SpawnAgentParams, spawn_agent_internal};
use crate::dag::DagStore;
use crate::project::ProjectState;
use crate::skills::build_skills_prompt;

// ── Types ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConversationTurnResult {
    pub agent_id: String,
    pub session_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartConversationParams {
    pub project_id: String,
    pub step: u32,
    pub substep: Option<String>,
    pub skill_id: String,
    pub initial_message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinueConversationParams {
    pub session_id: String,
    pub message: String,
    pub project_id: String,
}

// ── Internal spawn helper ──────────────────────────────────────────────────────

/// Spawn a one-shot conversation turn process. Returns agent_id.
/// Follows spawn_advisor_turn exactly; difference: wider tool set (Bash,Read,Write,Edit,Glob,Grep).
fn spawn_conversation_turn(
    project_dir: &str,
    full_prompt: String,
    session_id: &str,
    is_resume: bool,
    agent_state: &State<'_, AgentState>,
    app: &AppHandle,
) -> Result<String, String> {
    let agent_id = Uuid::new_v4().to_string();

    let mut args = vec![
        "-p".to_string(),
        full_prompt,
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--verbose".to_string(),
        "--allowedTools".to_string(),
        "Bash,Read,Write,Edit,Glob,Grep".to_string(),
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
            workflow_id: Some(agent_id.clone()),
            node_id: None,
            workflow_type: Some("conversation".to_string()),
            session_id: None, // handled manually in args
            resume: false,
            cwd: Some(project_dir.to_string()),
        },
        app,
        agent_state,
    )?;

    Ok(agent_id)
}

// ── Tauri commands ─────────────────────────────────────────────────────────────

/// Start a new conversational lifecycle turn. Spawns claude CLI with stream-json output.
/// Returns agent_id (for agent:stdout subscription) and session_id (for continuation).
#[tauri::command]
pub fn start_conversation_turn(
    params: StartConversationParams,
    project_state: State<'_, ProjectState>,
    agent_state: State<'_, AgentState>,
    app_handle: AppHandle,
) -> Result<ConversationTurnResult, String> {
    let active_dir = project_state.active_dir_str().ok_or("No project open")?;
    let project_dir_path = PathBuf::from(&active_dir);

    // 1. Load skill content via build_skills_prompt (same as advisor)
    let skill_content = build_skills_prompt(
        &[params.skill_id.clone()],
        &app_handle,
        Some(&project_dir_path),
    );

    // 2. Get artefact context for the current step
    let artefacts = {
        let projects = project_state.projects.lock().unwrap();
        let proj = projects.get(&active_dir).ok_or("No project open")?;
        proj.store.get_artefacts_for_step(&params.project_id, params.step)?
    };
    let artefact_context = DagStore::build_artefact_context(&artefacts);

    // 3. Build full prompt: skill + artefact context + user message
    let step_key = match &params.substep {
        Some(sub) => format!("{}.{}", params.step, sub),
        None => params.step.to_string(),
    };
    let mut prompt_parts: Vec<String> = Vec::new();
    if !skill_content.is_empty() {
        prompt_parts.push(skill_content);
    }
    if !artefact_context.is_empty() {
        prompt_parts.push(format!(
            "---\n## Prior Artefacts\n\n{}\n\n---\nProject ID: {}\nLifecycle Step: {}",
            artefact_context, params.project_id, step_key
        ));
    } else {
        prompt_parts.push(format!(
            "---\nProject ID: {}\nLifecycle Step: {}",
            params.project_id, step_key
        ));
    }
    prompt_parts.push(params.initial_message);
    let full_prompt = prompt_parts.join("\n\n");

    // 4. Generate a fresh session ID and spawn
    let session_id = Uuid::new_v4().to_string();
    let agent_id = spawn_conversation_turn(
        &active_dir,
        full_prompt,
        &session_id,
        false, // first turn: --session-id, not --resume
        &agent_state,
        &app_handle,
    )?;

    Ok(ConversationTurnResult { agent_id, session_id })
}

/// Continue an existing conversational turn using --resume <session_id>.
/// Returns a new agent_id (each spawn is a fresh process) and the same session_id.
#[tauri::command]
pub fn continue_conversation_turn(
    params: ContinueConversationParams,
    project_state: State<'_, ProjectState>,
    agent_state: State<'_, AgentState>,
    app_handle: AppHandle,
) -> Result<ConversationTurnResult, String> {
    let active_dir = project_state.active_dir_str().ok_or("No project open")?;

    let agent_id = spawn_conversation_turn(
        &active_dir,
        params.message,
        &params.session_id,
        true, // resume turn: --resume
        &agent_state,
        &app_handle,
    )?;

    Ok(ConversationTurnResult {
        agent_id,
        session_id: params.session_id,
    })
}
