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
use crate::dag::DagStore;
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

/// Lifecycle step names indexed by step number.
const STEP_NAMES: [&str; 7] = [
    "Idle",
    "Concept Development",
    "Guardrails Definition",
    "Stage Planning",
    "Autonomous Execution",
    "Rework",
    "Replanning & QA",
];

/// Build a structured markdown block describing current project state.
/// Injected as a context prefix on every advisor turn so the advisor always
/// has fresh information regardless of session age.
///
/// Token budget: target < 2000 tokens. Falls back gracefully if any source fails.
pub fn build_advisor_context(store: &DagStore, project_id: &str, agent_state: &AgentState) -> String {
    let ts = chrono::Utc::now().to_rfc3339();
    let mut out = format!("## Project State — {ts}\n\n");

    // ── Lifecycle ──────────────────────────────────────────────────────────────
    match store.get_lifecycle_state(project_id) {
        Ok(state) => {
            let name = STEP_NAMES.get(state.step as usize).copied().unwrap_or("Unknown");
            let substep = state
                .substep
                .as_deref()
                .map(|s| format!(" — {s}"))
                .unwrap_or_default();
            out.push_str(&format!(
                "**Lifecycle**: Step {} — {}{} ({})\n\n",
                state.step, name, substep, state.status
            ));
        }
        Err(_) => {
            out.push_str("**Lifecycle**: Not yet initialised\n\n");
        }
    }

    // ── DAG summary ────────────────────────────────────────────────────────────
    if let Ok(snapshot) = store.snapshot(project_id) {
        let mut epics = (0u32, 0u32, 0u32);    // (complete, in_progress, open)
        let mut features = (0u32, 0u32, 0u32);
        let mut tasks = (0u32, 0u32, 0u32);

        for node in &snapshot.nodes {
            let status = node.data.get("status").and_then(|v| v.as_str()).unwrap_or("open");
            let bucket = match node.node_type.as_str() {
                "Epic"    => &mut epics,
                "Feature" => &mut features,
                "Task"    => &mut tasks,
                _         => continue,
            };
            match status {
                "complete" | "completed" | "done" => bucket.0 += 1,
                "in_progress" | "running"          => bucket.1 += 1,
                _                                  => bucket.2 += 1,
            }
        }

        fn row(n: u32, label: &str, c: u32, ip: u32, o: u32) -> String {
            format!("- {n} {label} ({c} complete, {ip} in progress, {o} open)\n")
        }

        out.push_str("**DAG Summary**:\n");
        out.push_str(&row(epics.0 + epics.1 + epics.2, "Epics", epics.0, epics.1, epics.2));
        out.push_str(&row(features.0 + features.1 + features.2, "Features", features.0, features.1, features.2));
        out.push_str(&row(tasks.0 + tasks.1 + tasks.2, "Tasks", tasks.0, tasks.1, tasks.2));
        out.push('\n');
    }

    // ── Open queue items ───────────────────────────────────────────────────────
    if let Ok(items) = store.list_queue_items(project_id) {
        let open: Vec<_> = items.iter().filter(|i| i.status == "pending").collect();
        let total = open.len();
        out.push_str(&format!("**Open Queue Items** ({total}):\n"));
        if total == 0 {
            out.push_str("- None\n");
        } else {
            for item in open.iter().take(5) {
                let short_id = &item.id[..8.min(item.id.len())];
                let q = item.question.chars().take(80).collect::<String>();
                let ellipsis = if item.question.len() > 80 { "…" } else { "" };
                let date = &item.created_at[..10.min(item.created_at.len())];
                out.push_str(&format!("- [{short_id}] \"{q}{ellipsis}\" — open since {date}\n"));
            }
            if total > 5 {
                out.push_str(&format!("- … and {} more\n", total - 5));
            }
        }
        out.push('\n');
    }

    // ── Active agents ──────────────────────────────────────────────────────────
    let active_agents: Vec<(String, String)> = agent_state
        .handles
        .lock()
        .map(|handles| {
            handles
                .values()
                .filter(|h| h.workflow_type.as_deref() != Some("advisor"))
                .map(|h| {
                    let wf = h.workflow_type.clone().unwrap_or_else(|| "agent".to_string());
                    let node = h.node_id.clone().unwrap_or_else(|| "—".to_string());
                    (wf, node)
                })
                .collect()
        })
        .unwrap_or_default();

    out.push_str(&format!("**Active Agents** ({}):\n", active_agents.len()));
    if active_agents.is_empty() {
        out.push_str("- None\n");
    } else {
        for (wf_type, node_id) in &active_agents {
            out.push_str(&format!("- {wf_type} on node {node_id}\n"));
        }
    }
    out.push('\n');

    // ── Artefacts ──────────────────────────────────────────────────────────────
    // Use step=100 to retrieve all artefacts (no real step exceeds this).
    if let Ok(artefacts) = store.get_artefacts_for_step(project_id, 100) {
        out.push_str("**Artefacts**:\n");
        if artefacts.is_empty() {
            out.push_str("- None yet\n");
        } else {
            let total_content: usize = artefacts
                .iter()
                .filter_map(|n| n.data.get("content").and_then(|v| v.as_str()))
                .map(|s| s.len())
                .sum();
            let include_content = total_content < 3000;

            for node in &artefacts {
                let title    = node.data.get("title").and_then(|v| v.as_str()).unwrap_or("Untitled");
                let filename = node.data.get("filename").and_then(|v| v.as_str()).unwrap_or("");
                let step     = node.data.get("step").and_then(|v| v.as_u64()).unwrap_or(0);

                if include_content {
                    let content = node.data.get("content").and_then(|v| v.as_str()).unwrap_or("");
                    let preview: String = content.chars().take(300).collect();
                    let ellipsis = if content.len() > 300 { "…" } else { "" };
                    out.push_str(&format!(
                        "- {title} (Step {step}, {filename})\n  {preview}{ellipsis}\n"
                    ));
                } else {
                    out.push_str(&format!("- {title} (Step {step}, {filename})\n"));
                }
            }
        }
        out.push('\n');
    }

    out
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
    let context = {
        let projects = project_state.projects.lock().unwrap();
        match projects.get(&project_dir) {
            Some(proj) => build_advisor_context(&proj.store, &project_id, &agent_state),
            None => String::new(),
        }
    };

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
