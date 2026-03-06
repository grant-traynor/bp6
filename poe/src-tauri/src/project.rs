use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::dag::{DagEdge, DagNode, DagSnapshot, DagStore, EdgeType, NewQueueItem, NodeType, ProbeData, QueueItem, QueueItemOption};
use crate::agents::{spawn_agent_internal, SpawnAgentParams};

// ── Open project entry ─────────────────────────────────────────────────────────

pub struct OpenProject {
    pub store: DagStore,
    pub project_id: String,
    pub name: String,
}

// ── Managed project state ──────────────────────────────────────────────────────

/// Holds all concurrently-open projects, keyed by their project directory string.
/// `active_dir` points to the one currently displayed in the UI.
pub struct ProjectState {
    pub projects: Mutex<HashMap<String, OpenProject>>,
    pub active_dir: Mutex<Option<String>>,
}

impl ProjectState {
    pub fn new() -> Self {
        ProjectState {
            projects: Mutex::new(HashMap::new()),
            active_dir: Mutex::new(None),
        }
    }

    pub fn active_dir_str(&self) -> Option<String> {
        self.active_dir.lock().unwrap().clone()
    }

    /// Execute a closure with the active project's store and project_id.
    /// The projects lock is held only for the duration of the closure.
    pub fn with_active<T, F>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce(&DagStore, &str) -> Result<T, String>,
    {
        let active_dir = self.active_dir_str().ok_or("No project open")?;
        let projects = self.projects.lock().unwrap();
        let proj = projects.get(&active_dir).ok_or("No project open")?;
        f(&proj.store, &proj.project_id)
    }
}

// ── Tauri event payloads ───────────────────────────────────────────────────────

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectOpenedEvent {
    pub info: ProjectInfo,
    pub snapshot: DagSnapshot,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSwitchedEvent {
    pub info: ProjectInfo,
    pub snapshot: DagSnapshot,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectClosedEvent {
    pub project_dir: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeUpsertedEvent {
    pub node: DagNode,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDeletedEvent {
    pub id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeUpsertedEvent {
    pub edge: DagEdge,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeDeletedEvent {
    pub id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItemAddedEvent {
    pub item: QueueItem,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItemResolvedEvent {
    pub item_id: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub project_id: String,
    pub project_dir: String,
    pub name: String,
}

// ── Project commands ───────────────────────────────────────────────────────────

/// Open a project directory. If already open, switches to it (emits project:switched).
/// Otherwise opens the DB, emits project:opened.
#[tauri::command]
pub async fn open_project(
    dir: String,
    app: AppHandle,
    state: State<'_, ProjectState>,
) -> Result<ProjectInfo, String> {
    let project_dir = PathBuf::from(&dir);
    if !project_dir.exists() {
        return Err(format!("Directory does not exist: {}", dir));
    }

    // Already open? Just switch.
    {
        let projects = state.projects.lock().unwrap();
        if let Some(proj) = projects.get(&dir) {
            let info = ProjectInfo {
                project_id: proj.project_id.clone(),
                project_dir: dir.clone(),
                name: proj.name.clone(),
            };
            let snapshot = proj.store.snapshot(&proj.project_id)?;
            drop(projects);
            *state.active_dir.lock().unwrap() = Some(dir);
            app.emit("project:switched", ProjectSwitchedEvent { info: info.clone(), snapshot })
                .map_err(|e| format!("Failed to emit project:switched: {}", e))?;
            return Ok(info);
        }
    }

    // New project: open DB.
    let db_path = project_dir.join(".poe").join("dag.db");
    let store = DagStore::open(&db_path)?;

    let name = project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unnamed")
        .to_string();

    let project_id = {
        let existing = store.list_nodes("__root__")?;
        if let Some(proj_node) = existing.iter().find(|n| n.node_type == "Project") {
            proj_node.id.clone()
        } else {
            store.upsert_node(
                &NodeType::Project,
                "__root__",
                serde_json::json!({ "name": name, "dir": dir }),
            )?.id
        }
    };

    // Bootstrap: auto-create a "Requirements" Epic on brand-new empty projects.
    {
        let children = store.list_nodes(&project_id)?;
        let has_assignable = children
            .iter()
            .any(|n| matches!(n.node_type.as_str(), "Epic" | "Feature" | "Task"));
        if !has_assignable {
            let epic = store.upsert_node(
                &NodeType::Epic,
                &project_id,
                serde_json::json!({ "title": "Requirements", "status": "open" }),
            )?;
            store.add_edge(&project_id, &epic.id, &EdgeType::Implements, serde_json::json!({}))?;
        }
    }

    let snapshot = store.snapshot(&project_id)?;
    let info = ProjectInfo {
        project_id: project_id.clone(),
        project_dir: dir.clone(),
        name: name.clone(),
    };

    {
        let mut projects = state.projects.lock().unwrap();
        projects.insert(dir.clone(), OpenProject { store, project_id, name });
    }
    *state.active_dir.lock().unwrap() = Some(dir);

    app.emit("project:opened", ProjectOpenedEvent { info: info.clone(), snapshot })
        .map_err(|e| format!("Failed to emit project:opened: {}", e))?;

    Ok(info)
}

/// Switch the active view to an already-open project.
#[tauri::command]
pub async fn switch_project(
    dir: String,
    app: AppHandle,
    state: State<'_, ProjectState>,
) -> Result<ProjectInfo, String> {
    let (info, snapshot) = {
        let projects = state.projects.lock().unwrap();
        let proj = projects.get(&dir)
            .ok_or_else(|| format!("Project '{}' is not open", dir))?;
        let info = ProjectInfo {
            project_id: proj.project_id.clone(),
            project_dir: dir.clone(),
            name: proj.name.clone(),
        };
        let snapshot = proj.store.snapshot(&proj.project_id)?;
        (info, snapshot)
    };
    *state.active_dir.lock().unwrap() = Some(dir);
    app.emit("project:switched", ProjectSwitchedEvent { info: info.clone(), snapshot })
        .map_err(|e| format!("Failed to emit project:switched: {}", e))?;
    Ok(info)
}

/// Close a project. If `dir` is None, closes the active project.
/// If the closed project was active, automatically switches to the next open one
/// (emits project:switched) or clears state if none remain.
#[tauri::command]
pub async fn close_project(
    dir: Option<String>,
    app: AppHandle,
    state: State<'_, ProjectState>,
) -> Result<(), String> {
    let close_dir = match dir {
        Some(d) => d,
        None => state.active_dir_str().ok_or("No project open")?,
    };

    let was_active = state.active_dir.lock().unwrap().as_deref() == Some(close_dir.as_str());

    {
        let mut projects = state.projects.lock().unwrap();
        projects.remove(&close_dir);
    }

    app.emit("project:closed", ProjectClosedEvent { project_dir: close_dir })
        .map_err(|e| format!("Failed to emit project:closed: {}", e))?;

    if was_active {
        let next = state.projects.lock().unwrap().keys().next().cloned();
        if let Some(next_dir) = next {
            let (info, snapshot) = {
                let projects = state.projects.lock().unwrap();
                let proj = projects.get(&next_dir).ok_or("Next project not found")?;
                let info = ProjectInfo {
                    project_id: proj.project_id.clone(),
                    project_dir: next_dir.clone(),
                    name: proj.name.clone(),
                };
                (info, proj.store.snapshot(&proj.project_id)?)
            };
            *state.active_dir.lock().unwrap() = Some(next_dir);
            app.emit("project:switched", ProjectSwitchedEvent { info, snapshot })
                .map_err(|e| format!("Failed to emit project:switched: {}", e))?;
        } else {
            *state.active_dir.lock().unwrap() = None;
        }
    }

    Ok(())
}

/// Returns ProjectInfo for all currently open projects.
#[tauri::command]
pub fn list_open_projects(state: State<'_, ProjectState>) -> Vec<ProjectInfo> {
    let projects = state.projects.lock().unwrap();
    projects
        .iter()
        .map(|(dir, proj)| ProjectInfo {
            project_id: proj.project_id.clone(),
            project_dir: dir.clone(),
            name: proj.name.clone(),
        })
        .collect()
}

// ── Node commands ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNodeParams {
    pub node_type: String,
    pub data: serde_json::Value,
}

#[tauri::command]
pub async fn create_node(
    params: CreateNodeParams,
    app: AppHandle,
    state: State<'_, ProjectState>,
) -> Result<DagNode, String> {
    let active_dir = state.active_dir_str().ok_or("No project open")?;
    let node = {
        let projects = state.projects.lock().unwrap();
        let proj = projects.get(&active_dir).ok_or("No project open")?;
        let node_type = NodeType::from_str(&params.node_type)?;
        proj.store.upsert_node(&node_type, &proj.project_id, params.data)?
    };
    app.emit("dag:node:upserted", NodeUpsertedEvent { node: node.clone() })
        .map_err(|e| format!("Failed to emit: {}", e))?;
    Ok(node)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNodeParams {
    pub id: String,
    pub data: serde_json::Value,
}

#[tauri::command]
pub async fn update_node(
    params: UpdateNodeParams,
    app: AppHandle,
    state: State<'_, ProjectState>,
) -> Result<DagNode, String> {
    let active_dir = state.active_dir_str().ok_or("No project open")?;
    let node = {
        let projects = state.projects.lock().unwrap();
        let proj = projects.get(&active_dir).ok_or("No project open")?;
        proj.store.update_node(&params.id, params.data)?
    };
    app.emit("dag:node:upserted", NodeUpsertedEvent { node: node.clone() })
        .map_err(|e| format!("Failed to emit: {}", e))?;
    Ok(node)
}

#[tauri::command]
pub async fn delete_node(
    id: String,
    app: AppHandle,
    state: State<'_, ProjectState>,
) -> Result<(), String> {
    let active_dir = state.active_dir_str().ok_or("No project open")?;
    {
        let projects = state.projects.lock().unwrap();
        let proj = projects.get(&active_dir).ok_or("No project open")?;
        proj.store.delete_node(&id)?;
    }
    app.emit("dag:node:deleted", NodeDeletedEvent { id })
        .map_err(|e| format!("Failed to emit: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn get_snapshot(state: State<'_, ProjectState>) -> Result<DagSnapshot, String> {
    let active_dir = state.active_dir_str().ok_or("No project open")?;
    let projects = state.projects.lock().unwrap();
    let proj = projects.get(&active_dir).ok_or("No project open")?;
    proj.store.snapshot(&proj.project_id)
}

// ── Edge commands ──────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEdgeParams {
    pub from_id: String,
    pub to_id: String,
    pub edge_type: String,
    pub data: Option<serde_json::Value>,
}

#[tauri::command]
pub async fn create_edge(
    params: CreateEdgeParams,
    app: AppHandle,
    state: State<'_, ProjectState>,
) -> Result<DagEdge, String> {
    let active_dir = state.active_dir_str().ok_or("No project open")?;
    let edge = {
        let projects = state.projects.lock().unwrap();
        let proj = projects.get(&active_dir).ok_or("No project open")?;
        let edge_type = EdgeType::from_str(&params.edge_type)?;
        proj.store.add_edge(
            &params.from_id,
            &params.to_id,
            &edge_type,
            params.data.unwrap_or(serde_json::json!({})),
        )?
    };
    app.emit("dag:edge:upserted", EdgeUpsertedEvent { edge: edge.clone() })
        .map_err(|e| format!("Failed to emit: {}", e))?;
    Ok(edge)
}

#[tauri::command]
pub async fn delete_edge(
    id: String,
    app: AppHandle,
    state: State<'_, ProjectState>,
) -> Result<(), String> {
    let active_dir = state.active_dir_str().ok_or("No project open")?;
    {
        let projects = state.projects.lock().unwrap();
        let proj = projects.get(&active_dir).ok_or("No project open")?;
        proj.store.delete_edge(&id)?;
    }
    app.emit("dag:edge:deleted", EdgeDeletedEvent { id })
        .map_err(|e| format!("Failed to emit: {}", e))?;
    Ok(())
}

// ── Queue commands ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateQueueItemParams {
    pub agent_id: String,
    pub workflow_id: Option<String>,
    pub awakeable_id: Option<String>,
    pub question: String,
    pub options: Vec<QueueItemOption>,
    pub context_snapshot: Option<serde_json::Value>,
    pub priority: Option<i32>,
}

#[tauri::command]
pub async fn create_queue_item(
    params: CreateQueueItemParams,
    app: AppHandle,
    state: State<'_, ProjectState>,
) -> Result<QueueItem, String> {
    let active_dir = state.active_dir_str().ok_or("No project open")?;
    let item = {
        let projects = state.projects.lock().unwrap();
        let proj = projects.get(&active_dir).ok_or("No project open")?;
        proj.store.create_queue_item(NewQueueItem {
            project_id: proj.project_id.clone(),
            agent_id: params.agent_id,
            workflow_id: params.workflow_id,
            awakeable_id: params.awakeable_id,
            question: params.question,
            options: params.options,
            context_snapshot: params.context_snapshot.unwrap_or(serde_json::json!({})),
            priority: params.priority.unwrap_or(2),
        })?
    };
    app.emit("queue:item:added", QueueItemAddedEvent { item: item.clone() })
        .map_err(|e| format!("Failed to emit: {}", e))?;
    Ok(item)
}

#[tauri::command]
pub async fn list_queue_items(state: State<'_, ProjectState>) -> Result<Vec<QueueItem>, String> {
    let active_dir = state.active_dir_str().ok_or("No project open")?;
    let projects = state.projects.lock().unwrap();
    let proj = projects.get(&active_dir).ok_or("No project open")?;
    proj.store.list_queue_items(&proj.project_id)
}

/// Resume an in-place workflow after a one-shot agent has exited due to a decision.
///
/// Instead of rebuilding the prompt and creating a new child record (which pollutes
/// the fan-in child list), we spawn `claude --resume <session_id>` under the *same*
/// workflow ID. The resolution context is written to the new agent's stdin by the
/// existing PTY-write path in resolve_queue_item immediately after this returns.
///
/// CANONICAL RESOLUTION PATH NOTE:
///   - UI path: resolve_queue_item (Tauri) calls this function then writes to PTY stdin.
///   - HTTP path: handle_resolve_item in queue_service.rs resolves awakeables and
///     writes to PTY stdin for interactive agents.
/// These paths are mutually exclusive: the HTTP path is called by Restate (not by the
/// UI directly), so both cannot fire for the same resolution event.
fn resume_workflow(
    app: &AppHandle,
    state: &State<'_, ProjectState>,
    agent_state: &State<'_, crate::agents::AgentState>,
    orig_wf: &crate::dag::WorkflowRecord,
    active_dir: &str,
) {
    let session_id = match &orig_wf.session_id {
        Some(id) => id.clone(),
        None => {
            eprintln!(
                "[project] resume_workflow: workflow '{}' has no session_id — cannot resume",
                orig_wf.id
            );
            return;
        }
    };

    let cmd = orig_wf.config["cmd"].as_str().unwrap_or("claude").to_string();

    let mut env = HashMap::new();
    env.insert("POE_WORKFLOW_ID".to_string(), orig_wf.id.clone());
    env.insert("POE_NODE_ID".to_string(), orig_wf.node_id.clone());
    env.insert("POE_WORKFLOW_TYPE".to_string(), orig_wf.workflow_type.clone());

    let spawned = match spawn_agent_internal(
        SpawnAgentParams {
            cmd,
            args: vec![],    // ignored when resume=true
            env: Some(env),
            agent_id: None,
            workflow_id: Some(orig_wf.id.clone()),
            node_id: Some(orig_wf.node_id.clone()),
            workflow_type: Some(orig_wf.workflow_type.clone()),
            session_id: Some(session_id),
            resume: true,
            cwd: Some(active_dir.to_string()),
        },
        app,
        agent_state,
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[project] resume_workflow: failed to spawn: {}", e);
            return;
        }
    };

    // Transition the existing workflow record back to running (clears completed_at/error).
    {
        let projects = state.projects.lock().unwrap();
        if let Some(proj) = projects.get(active_dir) {
            if let Err(e) = proj.store.resume_workflow_record(&orig_wf.id, &spawned.agent_id) {
                eprintln!("[project] resume_workflow: failed to update DB: {}", e);
            }
        }
    }

    // Update the DAG node back to in_progress (same workflow ID — no new record).
    {
        let projects = state.projects.lock().unwrap();
        if let Some(proj) = projects.get(active_dir) {
            if let Ok(node) = proj.store.get_node(&orig_wf.node_id) {
                let mut data = node.data.clone();
                data["status"] = serde_json::json!("in_progress");
                data["workflowId"] = serde_json::json!(&orig_wf.id);
                if let Ok(updated) = proj.store.update_node(&orig_wf.node_id, data) {
                    let _ = app.emit("dag:node:upserted", NodeUpsertedEvent { node: updated });
                }
            }
        }
    }

    eprintln!(
        "[project] resume_workflow: spawned --resume agent={} workflow={}",
        spawned.agent_id, orig_wf.id
    );
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveQueueItemParams {
    pub item_id: String,
    pub chosen_option_id: String,
    /// Optional free-text context supplied by the human alongside their choice.
    pub user_context: Option<String>,
}

#[tauri::command]
pub async fn resolve_queue_item(
    params: ResolveQueueItemParams,
    app: AppHandle,
    state: State<'_, ProjectState>,
    agent_state: State<'_, crate::agents::AgentState>,
) -> Result<(), String> {
    let (item, project_id, active_dir) = {
        let active_dir = state.active_dir_str().ok_or("No project open")?;
        let projects = state.projects.lock().unwrap();
        let proj = projects.get(&active_dir).ok_or("No project open")?;
        let item = proj.store.get_queue_item(&params.item_id)?;
        (item, proj.project_id.clone(), active_dir)
    };

    let chosen = item
        .options
        .iter()
        .find(|o| o.id == params.chosen_option_id)
        .ok_or_else(|| format!("Option '{}' not found", params.chosen_option_id))?
        .clone();

    let resolution = serde_json::json!({
        "queueItemId": item.id,
        "chosenOptionId": chosen.id,
        "chosenOptionLabel": chosen.label,
        "userContext": params.user_context,
    });

    let decision_node = {
        let projects = state.projects.lock().unwrap();
        let proj = projects.get(&active_dir).ok_or("No project open")?;
        proj.store.upsert_node(
            &NodeType::Decision,
            &project_id,
            serde_json::json!({
                "question": item.question,
                "chosenOptionId": chosen.id,
                "chosenOptionLabel": chosen.label,
                "userContext": params.user_context,
                "agentId": item.agent_id,
                "queueItemId": item.id,
            }),
        )?
    };

    if let Some(ref awakeable_id) = item.awakeable_id {
        let url = format!(
            "http://127.0.0.1:{}/restate/awakeables/{}/resolve",
            crate::restate::RESTATE_SERVICES_PORT,
            awakeable_id
        );
        reqwest::Client::new()
            .post(&url)
            .json(&resolution)
            .send()
            .await
            .map_err(|e| format!("Failed to call Restate: {}", e))?
            .error_for_status()
            .map_err(|e| format!("Restate error: {}", e))?;
    }

    {
        let projects = state.projects.lock().unwrap();
        let proj = projects.get(&active_dir).ok_or("No project open")?;
        proj.store.resolve_queue_item_in_db(&item.id, resolution)?;
    }

    // For one-shot agents (claude -p): if the workflow is already terminal,
    // auto-spawn a continuation with the decision resolution baked into the prompt.
    // Only continue when this is the LAST pending decision for the workflow —
    // if other queue items are still unresolved, wait for all of them first.
    if let Some(ref workflow_id) = item.workflow_id {
        let should_continue = {
            let projects = state.projects.lock().unwrap();
            projects.get(&active_dir).and_then(|proj| {
                let wf = proj.store.get_workflow(workflow_id).ok()?;
                if wf.status != "completed" && wf.status != "failed" {
                    return None; // still running — PTY write above handles it
                }
                // Count remaining pending queue items for this workflow
                // (the item we just resolved is already marked resolved in DB)
                let pending = proj.store.list_queue_items(&project_id).ok()?
                    .into_iter()
                    .filter(|qi| {
                        qi.workflow_id.as_deref() == Some(workflow_id)
                            && qi.status == "pending"
                            && qi.id != item.id
                    })
                    .count();
                if pending == 0 { Some(wf) } else { None }
            })
        };

        if let Some(orig_wf) = should_continue {
            resume_workflow(
                &app,
                &state,
                &agent_state,
                &orig_wf,
                &active_dir,
            );
        }
    }

    // Write resolution to PTY stdin for interactive agents.
    // Format as a clear human message so it reads naturally in any LLM conversation.
    {
        let mut msg = format!(
            "\n[DECISION RESOLVED]\nQuestion: {}\nChosen: {}\n",
            item.question, chosen.label
        );
        if let Some(ref ctx) = params.user_context {
            if !ctx.trim().is_empty() {
                msg.push_str(&format!("Context: {}\n", ctx.trim()));
            }
        }
        msg.push('\n');

        if let Err(e) = agent_state.write_to_workflow_agent(
            item.workflow_id.as_deref().unwrap_or(""),
            &msg,
        ) {
            // Non-fatal — agent may have already exited (e.g. claude -p one-shot mode)
            eprintln!("[project] PTY write for decision resolution (non-fatal): {}", e);
        }

        // On resume, the trust-folder prompt appears after the decision context has been
        // processed. Send a trailing \r so it lands in the stdin buffer at the right time.
        let _ = agent_state.write_to_workflow_agent(
            item.workflow_id.as_deref().unwrap_or(""),
            "\r",
        );
    }

    app.emit("dag:node:upserted", NodeUpsertedEvent { node: decision_node })
        .map_err(|e| format!("Failed to emit: {}", e))?;
    app.emit("queue:item:resolved", QueueItemResolvedEvent { item_id: item.id.clone() })
        .map_err(|e| format!("Failed to emit: {}", e))?;

    Ok(())
}

// ── Global app state (~/.poe/app-state.json) ──────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentProjectRecord {
    pub path: String,
    pub name: String,
    pub last_opened_at: String,
    pub is_favourite: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppStateData {
    pub recent_projects: Vec<RecentProjectRecord>,
}

fn app_state_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Cannot resolve home directory")?;
    Ok(home.join(".poe").join("app-state.json"))
}

#[tauri::command]
pub fn load_app_state() -> Result<AppStateData, String> {
    let path = app_state_path()?;
    if !path.exists() {
        return Ok(AppStateData::default());
    }
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read app state: {}", e))?;
    serde_json::from_str(&raw)
        .map_err(|e| format!("Failed to parse app state: {}", e))
}

#[tauri::command]
pub fn save_app_state(state: AppStateData) -> Result<(), String> {
    let path = app_state_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create ~/.poe: {}", e))?;
    }
    let json = serde_json::to_string_pretty(&state)
        .map_err(|e| format!("Failed to serialise app state: {}", e))?;
    std::fs::write(&path, json)
        .map_err(|e| format!("Failed to write app state: {}", e))
}

// ── Mission Control commands (bp6-80q) ─────────────────────────────────────────

#[tauri::command]
pub async fn probe_node(
    node_id: String,
    state: State<'_, ProjectState>,
) -> Result<ProbeData, String> {
    let active_dir = state.active_dir_str().ok_or("No project open")?;
    let projects = state.projects.lock().unwrap();
    let proj = projects.get(&active_dir).ok_or("No project open")?;
    proj.store.probe_node(&node_id)
}

#[tauri::command]
pub async fn get_provenance(
    node_id: String,
    depth: u32,
    state: State<'_, ProjectState>,
) -> Result<DagSnapshot, String> {
    let active_dir = state.active_dir_str().ok_or("No project open")?;
    let projects = state.projects.lock().unwrap();
    let proj = projects.get(&active_dir).ok_or("No project open")?;
    proj.store.get_provenance(&node_id, &proj.project_id, depth)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StopWorkflowParams {
    pub workflow_id: String,
    pub node_id: String,
    pub reason: Option<String>,
}

#[tauri::command]
pub async fn stop_workflow(
    params: StopWorkflowParams,
    app: AppHandle,
    state: State<'_, ProjectState>,
    agent_state: State<'_, crate::agents::AgentState>,
) -> Result<DagNode, String> {
    let (active_dir, project_id) = {
        let active_dir = state.active_dir_str().ok_or("No project open")?;
        let projects = state.projects.lock().unwrap();
        let proj = projects.get(&active_dir).ok_or("No project open")?;
        (active_dir, proj.project_id.clone())
    };

    agent_state.stop_workflow_agent_graceful(&params.workflow_id);

    let _ = reqwest::Client::new()
        .post(format!(
            "http://127.0.0.1:{}/invocations/{}/cancel",
            crate::restate::RESTATE_ADMIN_PORT,
            params.workflow_id
        ))
        .send()
        .await;

    let updated_node = {
        let projects = state.projects.lock().unwrap();
        let proj = projects.get(&active_dir).ok_or("No project open")?;
        let node = proj.store.get_node(&params.node_id)?;
        let mut data = node.data.clone();
        data["status"] = serde_json::json!("cancelled");
        proj.store.update_node(&params.node_id, data)?
    };

    {
        let projects = state.projects.lock().unwrap();
        if let Some(proj) = projects.get(&active_dir) {
            let reason = params.reason.as_deref().unwrap_or("Stopped by user");
            let _ = proj.store.update_workflow_status(&params.workflow_id, "cancelled", None, Some(reason));
        }
    }

    let decision_node = {
        let projects = state.projects.lock().unwrap();
        let proj = projects.get(&active_dir).ok_or("No project open")?;
        proj.store.upsert_node(
            &NodeType::Decision,
            &project_id,
            serde_json::json!({
                "action": "stop",
                "workflowId": params.workflow_id,
                "targetNodeId": params.node_id,
                "reason": params.reason.unwrap_or_default(),
                "status": "stop-recorded",
            }),
        )?
    };

    app.emit("dag:node:upserted", NodeUpsertedEvent { node: updated_node.clone() })
        .map_err(|e| format!("Failed to emit: {}", e))?;
    app.emit("dag:node:upserted", NodeUpsertedEvent { node: decision_node })
        .map_err(|e| format!("Failed to emit: {}", e))?;
    Ok(updated_node)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedirectWorkflowParams {
    pub workflow_id: String,
    pub node_id: String,
    pub instruction: String,
}

#[tauri::command]
pub async fn redirect_workflow(
    params: RedirectWorkflowParams,
    app: AppHandle,
    state: State<'_, ProjectState>,
    agent_state: State<'_, crate::agents::AgentState>,
) -> Result<DagNode, String> {
    let (active_dir, project_id) = {
        let active_dir = state.active_dir_str().ok_or("No project open")?;
        let projects = state.projects.lock().unwrap();
        let proj = projects.get(&active_dir).ok_or("No project open")?;
        (active_dir, proj.project_id.clone())
    };

    let decision_node = {
        let projects = state.projects.lock().unwrap();
        let proj = projects.get(&active_dir).ok_or("No project open")?;
        proj.store.upsert_node(
            &NodeType::Decision,
            &project_id,
            serde_json::json!({
                "action": "redirect",
                "workflowId": params.workflow_id,
                "targetNodeId": params.node_id,
                "instruction": params.instruction,
                "status": "redirect-recorded",
            }),
        )?
    };

    app.emit("dag:node:upserted", NodeUpsertedEvent { node: decision_node.clone() })
        .map_err(|e| format!("Failed to emit: {}", e))?;

    if let Err(e) = agent_state.write_to_workflow_agent(
        &params.workflow_id,
        &format!("\n[REDIRECT]: {}\n", params.instruction),
    ) {
        eprintln!("[project] redirect PTY write failed (non-fatal): {}", e);
    }

    Ok(decision_node)
}
