use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::dag::{DagEdge, DagNode, DagSnapshot, DagStore, EdgeType, NewQueueItem, NodeType, ProbeData, QueueItem, QueueItemOption};

// ── Managed project state ──────────────────────────────────────────────────────

pub struct ProjectState {
    pub store: Mutex<Option<DagStore>>,
    pub project_id: Mutex<Option<String>>,
    pub project_dir: Mutex<Option<PathBuf>>,
}

impl ProjectState {
    pub fn new() -> Self {
        ProjectState {
            store: Mutex::new(None),
            project_id: Mutex::new(None),
            project_dir: Mutex::new(None),
        }
    }
}

// ── Tauri event payloads ───────────────────────────────────────────────────────

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

// ── Commands ───────────────────────────────────────────────────────────────────

/// Open a project directory. Creates or opens .poe/dag.db, runs migrations,
/// loads the project node, and emits the initial snapshot.
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

    let db_path = project_dir.join(".poe").join("dag.db");
    let store = DagStore::open(&db_path)?;

    // Derive project name from directory name
    let name = project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unnamed")
        .to_string();

    // Ensure the root Project node exists
    let project_id = {
        let existing = store.list_nodes("__root__")?;
        if let Some(proj_node) = existing.iter().find(|n| n.node_type == "Project") {
            proj_node.id.clone()
        } else {
            let node = store.upsert_node(
                &NodeType::Project,
                "__root__",
                serde_json::json!({ "name": name, "dir": dir }),
            )?;
            node.id
        }
    };

    // Load the initial snapshot and emit it
    let snapshot = store.snapshot(&project_id)?;

    *state.store.lock().unwrap() = Some(store);
    *state.project_id.lock().unwrap() = Some(project_id.clone());
    *state.project_dir.lock().unwrap() = Some(project_dir.clone());

    // Emit snapshot to frontend
    app.emit("project:opened", &snapshot)
        .map_err(|e| format!("Failed to emit project:opened: {}", e))?;

    Ok(ProjectInfo {
        project_id,
        project_dir: dir,
        name,
    })
}

/// Close the current project. Flushes writes, drops SQLite connection, clears state.
#[tauri::command]
pub async fn close_project(
    app: AppHandle,
    state: State<'_, ProjectState>,
) -> Result<(), String> {
    *state.store.lock().unwrap() = None;
    *state.project_id.lock().unwrap() = None;
    *state.project_dir.lock().unwrap() = None;

    app.emit("project:closed", ())
        .map_err(|e| format!("Failed to emit project:closed: {}", e))?;

    Ok(())
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
    let lock = state.store.lock().unwrap();
    let store = lock.as_ref().ok_or("No project open")?;
    let project_id = state.project_id.lock().unwrap().clone().ok_or("No project open")?;

    let node_type = NodeType::from_str(&params.node_type)?;
    let node = store.upsert_node(&node_type, &project_id, params.data)?;

    // Reactive bridge: emit delta
    app.emit("dag:node:upserted", NodeUpsertedEvent { node: node.clone() })
        .map_err(|e| format!("Failed to emit dag:node:upserted: {}", e))?;

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
    let lock = state.store.lock().unwrap();
    let store = lock.as_ref().ok_or("No project open")?;

    let node = store.update_node(&params.id, params.data)?;

    app.emit("dag:node:upserted", NodeUpsertedEvent { node: node.clone() })
        .map_err(|e| format!("Failed to emit dag:node:upserted: {}", e))?;

    Ok(node)
}

#[tauri::command]
pub async fn delete_node(
    id: String,
    app: AppHandle,
    state: State<'_, ProjectState>,
) -> Result<(), String> {
    let lock = state.store.lock().unwrap();
    let store = lock.as_ref().ok_or("No project open")?;

    store.delete_node(&id)?;

    app.emit("dag:node:deleted", NodeDeletedEvent { id })
        .map_err(|e| format!("Failed to emit dag:node:deleted: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn get_snapshot(state: State<'_, ProjectState>) -> Result<DagSnapshot, String> {
    let lock = state.store.lock().unwrap();
    let store = lock.as_ref().ok_or("No project open")?;
    let project_id = state.project_id.lock().unwrap().clone().ok_or("No project open")?;
    store.snapshot(&project_id)
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
    let lock = state.store.lock().unwrap();
    let store = lock.as_ref().ok_or("No project open")?;

    let edge_type = EdgeType::from_str(&params.edge_type)?;
    let edge = store.add_edge(
        &params.from_id,
        &params.to_id,
        &edge_type,
        params.data.unwrap_or(serde_json::json!({})),
    )?;

    app.emit("dag:edge:upserted", EdgeUpsertedEvent { edge: edge.clone() })
        .map_err(|e| format!("Failed to emit dag:edge:upserted: {}", e))?;

    Ok(edge)
}

#[tauri::command]
pub async fn delete_edge(
    id: String,
    app: AppHandle,
    state: State<'_, ProjectState>,
) -> Result<(), String> {
    let lock = state.store.lock().unwrap();
    let store = lock.as_ref().ok_or("No project open")?;

    store.delete_edge(&id)?;

    app.emit("dag:edge:deleted", EdgeDeletedEvent { id })
        .map_err(|e| format!("Failed to emit dag:edge:deleted: {}", e))?;

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
    let lock = state.store.lock().unwrap();
    let store = lock.as_ref().ok_or("No project open")?;
    let project_id = state.project_id.lock().unwrap().clone().ok_or("No project open")?;

    let item = store.create_queue_item(NewQueueItem {
        project_id,
        agent_id: params.agent_id,
        workflow_id: params.workflow_id,
        awakeable_id: params.awakeable_id,
        question: params.question,
        options: params.options,
        context_snapshot: params.context_snapshot.unwrap_or(serde_json::json!({})),
        priority: params.priority.unwrap_or(2),
    })?;

    app.emit("queue:item:added", QueueItemAddedEvent { item: item.clone() })
        .map_err(|e| format!("Failed to emit queue:item:added: {}", e))?;

    Ok(item)
}

#[tauri::command]
pub async fn list_queue_items(state: State<'_, ProjectState>) -> Result<Vec<QueueItem>, String> {
    let lock = state.store.lock().unwrap();
    let store = lock.as_ref().ok_or("No project open")?;
    let project_id = state.project_id.lock().unwrap().clone().ok_or("No project open")?;
    store.list_queue_items(&project_id)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveQueueItemParams {
    pub item_id: String,
    pub chosen_option_id: String,
}

/// End-to-end resolution flow (bp6-2a5.5):
/// 1. Create Decision DAG node linked to the queue item
/// 2. Call Restate to complete the awakeable (if present)
/// 3. Update queue_item status to resolved in SQLite
/// 4. Emit dag:node:upserted + queue:item:resolved events
/// 5. TODO(bp6-2a5.1): Send chosen option to agent stdin via PTY
#[tauri::command]
pub async fn resolve_queue_item(
    params: ResolveQueueItemParams,
    app: AppHandle,
    state: State<'_, ProjectState>,
) -> Result<(), String> {
    // Read item + project_id under lock, then release before async HTTP call
    let (item, project_id) = {
        let lock = state.store.lock().unwrap();
        let store = lock.as_ref().ok_or("No project open")?;
        let project_id = state.project_id.lock().unwrap().clone().ok_or("No project open")?;
        let item = store.get_queue_item(&params.item_id)?;
        (item, project_id)
    };

    let chosen = item
        .options
        .iter()
        .find(|o| o.id == params.chosen_option_id)
        .ok_or_else(|| format!("Option '{}' not found in queue item '{}'", params.chosen_option_id, item.id))?
        .clone();

    let resolution = serde_json::json!({
        "queueItemId": item.id,
        "chosenOptionId": chosen.id,
        "chosenOptionLabel": chosen.label,
    });

    // ── Step 1: Create Decision DAG node ──────────────────────────────────────
    let decision_node = {
        let lock = state.store.lock().unwrap();
        let store = lock.as_ref().ok_or("No project open")?;
        store.upsert_node(
            &NodeType::Decision,
            &project_id,
            serde_json::json!({
                "question": item.question,
                "chosenOptionId": chosen.id,
                "chosenOptionLabel": chosen.label,
                "agentId": item.agent_id,
                "queueItemId": item.id,
            }),
        )?
    };

    // ── Step 2: Call Restate to complete awakeable ────────────────────────────
    if let Some(ref awakeable_id) = item.awakeable_id {
        let url = format!(
            "http://127.0.0.1:{}/restate/awakeables/{}/resolve",
            crate::restate::RESTATE_SERVICES_PORT,
            awakeable_id
        );
        let client = reqwest::Client::new();
        client
            .post(&url)
            .json(&resolution)
            .send()
            .await
            .map_err(|e| format!("Failed to call Restate resolveItem: {}", e))?
            .error_for_status()
            .map_err(|e| format!("Restate resolveItem returned error: {}", e))?;
    }

    // ── Step 3: Mark resolved in SQLite ───────────────────────────────────────
    {
        let lock = state.store.lock().unwrap();
        let store = lock.as_ref().ok_or("No project open")?;
        store.resolve_queue_item_in_db(&item.id, resolution)?;
    }

    // ── Step 4: Emit events ───────────────────────────────────────────────────
    app.emit("dag:node:upserted", NodeUpsertedEvent { node: decision_node })
        .map_err(|e| format!("Failed to emit dag:node:upserted: {}", e))?;

    app.emit("queue:item:resolved", QueueItemResolvedEvent { item_id: item.id.clone() })
        .map_err(|e| format!("Failed to emit queue:item:resolved: {}", e))?;

    // ── Step 5: PTY stdin (bp6-2a5.1) ────────────────────────────────────────
    // TODO(bp6-2a5.1): Write resolution JSON to agent stdin via PTY when
    // agent process management is implemented.
    eprintln!("[PTY stub] Resolution for agent '{}': option '{}'", item.agent_id, chosen.id);

    Ok(())
}

// ── Mission Control commands (bp6-80q) ─────────────────────────────────────────

/// Probe a node: returns the node, its edges, and directly linked nodes.
/// Used by the probe/inspect panel (bp6-80q.3).
#[tauri::command]
pub async fn probe_node(
    node_id: String,
    state: State<'_, ProjectState>,
) -> Result<ProbeData, String> {
    let lock = state.store.lock().unwrap();
    let store = lock.as_ref().ok_or("No project open")?;
    store.probe_node(&node_id)
}

/// Returns the subgraph within `depth` hops of `node_id`.
/// Used by the provenance traversal view (bp6-80q.5).
#[tauri::command]
pub async fn get_provenance(
    node_id: String,
    depth: u32,
    state: State<'_, ProjectState>,
) -> Result<DagSnapshot, String> {
    let lock = state.store.lock().unwrap();
    let store = lock.as_ref().ok_or("No project open")?;
    let project_id = state.project_id.lock().unwrap().clone().ok_or("No project open")?;
    store.get_provenance(&node_id, &project_id, depth)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StopWorkflowParams {
    pub workflow_id: String,
    pub node_id: String,
    pub reason: Option<String>,
}

/// Stop a running workflow: cancel Restate invocation, mark node cancelled, create Decision record.
/// PTY SIGTERM is stubbed until bp6-3d1 (Phase 3 Workflow Engine) is implemented.
#[tauri::command]
pub async fn stop_workflow(
    params: StopWorkflowParams,
    app: AppHandle,
    state: State<'_, ProjectState>,
) -> Result<DagNode, String> {
    let project_id = state.project_id.lock().unwrap().clone().ok_or("No project open")?;

    // ── Cancel Restate invocation ──────────────────────────────────────────────
    let cancel_url = format!(
        "http://127.0.0.1:{}/invocations/{}/cancel",
        crate::restate::RESTATE_ADMIN_PORT,
        params.workflow_id
    );
    let client = reqwest::Client::new();
    // Best-effort — don't fail if Restate isn't running or invocation already done
    let _ = client.post(&cancel_url).send().await;

    // ── Update node status to cancelled ───────────────────────────────────────
    let updated_node = {
        let lock = state.store.lock().unwrap();
        let store = lock.as_ref().ok_or("No project open")?;
        let node = store.get_node(&params.node_id)?;
        let mut data = node.data.clone();
        data["status"] = serde_json::json!("cancelled");
        store.update_node(&params.node_id, data)?
    };

    // ── Create Decision node recording the stop ────────────────────────────────
    let decision_node = {
        let lock = state.store.lock().unwrap();
        let store = lock.as_ref().ok_or("No project open")?;
        store.upsert_node(
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
        .map_err(|e| format!("Failed to emit dag:node:upserted: {}", e))?;
    app.emit("dag:node:upserted", NodeUpsertedEvent { node: decision_node })
        .map_err(|e| format!("Failed to emit dag:node:upserted: {}", e))?;

    // TODO(bp6-3d1): SIGTERM the PTY process for this workflow
    eprintln!("[PTY stub] Stop workflow '{}'", params.workflow_id);

    Ok(updated_node)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedirectWorkflowParams {
    pub workflow_id: String,
    pub node_id: String,
    pub instruction: String,
}

/// Redirect a running workflow: record a Redirect Decision node, inject instruction via PTY.
/// PTY stdin injection is stubbed until bp6-3d1 (Phase 3 Workflow Engine) is implemented.
#[tauri::command]
pub async fn redirect_workflow(
    params: RedirectWorkflowParams,
    app: AppHandle,
    state: State<'_, ProjectState>,
) -> Result<DagNode, String> {
    let project_id = state.project_id.lock().unwrap().clone().ok_or("No project open")?;

    // ── Write Redirect Decision node to DAG ───────────────────────────────────
    let decision_node = {
        let lock = state.store.lock().unwrap();
        let store = lock.as_ref().ok_or("No project open")?;
        store.upsert_node(
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
        .map_err(|e| format!("Failed to emit dag:node:upserted: {}", e))?;

    // TODO(bp6-3d1): Inject instruction to agent stdin via PTY
    eprintln!("[PTY stub] Redirect workflow '{}': {}", params.workflow_id, params.instruction);

    Ok(decision_node)
}
