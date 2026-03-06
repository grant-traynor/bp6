// bp6-3d1.1 + bp6-3d1.4 + bp6-3d1.5: V-lifecycle workflows, sub-workflow orchestration, work assignment
//
// V-lifecycle workflow types: Requirements, Design, Implementation, Validation, Deployment.
// Each workflow is tracked in SQLite with durable state. Agents report progress via poe: events.
// Fan-out: Feature/Epic nodes spawn child workflows for their Task descendants.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::agents::{spawn_agent_internal, AgentState, SpawnAgentParams, WorkflowStatusEvent};
use crate::dag::{NewWorkflow, WorkflowRecord};
use crate::project::{NodeUpsertedEvent, ProjectState};

// ── V-lifecycle workflow types ─────────────────────────────────────────────────

/// Canonical workflow type names.
pub const WORKFLOW_TYPES: &[&str] = &[
    "RequirementsWorkflow",
    "DesignWorkflow",
    "ImplementationWorkflow",
    "ValidationWorkflow",
    "DeploymentWorkflow",
];

/// Infer a sensible default workflow type from a DAG node type.
fn infer_workflow_type(node_type: &str) -> &'static str {
    match node_type {
        "Feature" | "Task" => "ImplementationWorkflow",
        "Epic" => "DesignWorkflow",
        "Review" => "ValidationWorkflow",
        _ => "ImplementationWorkflow",
    }
}

// ── Tauri command types ────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkflowParams {
    /// DAG node to assign the workflow to.
    pub node_id: String,
    /// V-lifecycle workflow type. If None, inferred from node type.
    pub workflow_type: Option<String>,
    /// Agent command to run (e.g., "claude").
    pub cmd: String,
    /// Arguments for the agent.
    pub args: Vec<String>,
    /// Extra environment variables for the agent.
    pub env: Option<HashMap<String, String>>,
    /// If true, also spawn child workflows for Task nodes under this node.
    pub fanout: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkflowResult {
    pub workflow: WorkflowRecord,
    pub agent_id: String,
    /// Child workflow IDs created by fan-out (may be empty).
    pub child_workflow_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListWorkflowsParams {
    pub status_filter: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelWorkflowParams {
    pub workflow_id: String,
    pub reason: Option<String>,
}

// ── Helper ─────────────────────────────────────────────────────────────────────

/// Core workflow creation: create SQLite record, spawn agent, update node status.
/// Returns the workflow record and the agent_id.
fn create_one_workflow(
    app: &AppHandle,
    project_state: &ProjectState,
    agent_state: &AgentState,
    project_id: &str,
    node_id: &str,
    workflow_type: &str,
    cmd: &str,
    args: &[String],
    env: Option<HashMap<String, String>>,
) -> Result<(WorkflowRecord, String), String> {
    // ── Get node for context ───────────────────────────────────────────────────
    let node = {
        let lock = project_state
            .store
            .lock()
            .map_err(|e| format!("Store lock: {}", e))?;
        let store = lock.as_ref().ok_or("No project open")?;
        store.get_node(node_id)?
    };

    // ── Create workflow record ─────────────────────────────────────────────────
    let workflow = {
        let lock = project_state
            .store
            .lock()
            .map_err(|e| format!("Store lock: {}", e))?;
        let store = lock.as_ref().ok_or("No project open")?;
        store.create_workflow_record(NewWorkflow {
            project_id: project_id.to_string(),
            node_id: node_id.to_string(),
            agent_id: None,
            workflow_type: workflow_type.to_string(),
            config: serde_json::json!({
                "cmd": cmd,
                "args": args,
            }),
        })?
    };

    // ── Build env for agent: inject node context ───────────────────────────────
    let mut agent_env = env.unwrap_or_default();
    agent_env.insert("POE_WORKFLOW_ID".to_string(), workflow.id.clone());
    agent_env.insert("POE_NODE_ID".to_string(), node_id.to_string());
    agent_env.insert("POE_WORKFLOW_TYPE".to_string(), workflow_type.to_string());
    agent_env.insert(
        "POE_NODE_TYPE".to_string(),
        node.node_type.clone(),
    );
    if let Ok(data_str) = serde_json::to_string(&node.data) {
        agent_env.insert("POE_NODE_DATA".to_string(), data_str);
    }

    // ── Spawn agent ────────────────────────────────────────────────────────────
    let spawn_params = SpawnAgentParams {
        cmd: cmd.to_string(),
        args: args.to_vec(),
        env: Some(agent_env),
        agent_id: None,
        workflow_id: Some(workflow.id.clone()),
        node_id: Some(node_id.to_string()),
        workflow_type: Some(workflow_type.to_string()),
    };

    let spawned = spawn_agent_internal(spawn_params, app, agent_state)?;

    // ── Update workflow with agent_id ──────────────────────────────────────────
    let workflow = {
        let lock = project_state
            .store
            .lock()
            .map_err(|e| format!("Store lock: {}", e))?;
        let store = lock.as_ref().ok_or("No project open")?;
        store.update_workflow_status(&workflow.id, "running", Some(&spawned.agent_id), None)?
    };

    // ── Update DAG node: status = in_progress, workflowId attached ────────────
    let updated_node = {
        let lock = project_state
            .store
            .lock()
            .map_err(|e| format!("Store lock: {}", e))?;
        let store = lock.as_ref().ok_or("No project open")?;
        let mut data = node.data.clone();
        data["status"] = serde_json::json!("in_progress");
        data["workflowId"] = serde_json::json!(workflow.id);
        data["workflowType"] = serde_json::json!(workflow_type);
        store.update_node(node_id, data)?
    };

    app.emit(
        "dag:node:upserted",
        NodeUpsertedEvent {
            node: updated_node,
        },
    )
    .map_err(|e| format!("Failed to emit dag:node:upserted: {}", e))?;

    Ok((workflow, spawned.agent_id))
}

// ── Tauri commands ─────────────────────────────────────────────────────────────

/// Create a workflow: spawn agent, attach to DAG node, optionally fan out to children.
///
/// Flow:
/// 1. Infer workflow type from node type (if not provided)
/// 2. Create SQLite workflow record
/// 3. Spawn PTY agent with node context in env
/// 4. Update workflow agent_id + status = running
/// 5. Update DAG node status = in_progress
/// 6. If fanout=true, recursively create workflows for child Task nodes
#[tauri::command]
pub async fn create_workflow(
    params: CreateWorkflowParams,
    app: AppHandle,
    project_state: State<'_, ProjectState>,
    agent_state: State<'_, AgentState>,
) -> Result<CreateWorkflowResult, String> {
    let project_id = project_state
        .project_id
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or("No project open")?;

    // Infer workflow type
    let node_type = {
        let lock = project_state.store.lock().map_err(|e| e.to_string())?;
        let store = lock.as_ref().ok_or("No project open")?;
        store.get_node(&params.node_id)?.node_type
    };

    let workflow_type = params
        .workflow_type
        .as_deref()
        .unwrap_or_else(|| infer_workflow_type(&node_type))
        .to_string();

    let (workflow, agent_id) = create_one_workflow(
        &app,
        &project_state,
        &agent_state,
        &project_id,
        &params.node_id,
        &workflow_type,
        &params.cmd,
        &params.args,
        params.env.clone(),
    )?;

    // ── Fan-out to child Task/Feature nodes (bp6-3d1.4) ───────────────────────
    let mut child_workflow_ids = Vec::new();

    if params.fanout.unwrap_or(false) {
        // Find direct children: nodes connected via incoming "implements" or "blocks" edges
        let child_node_ids = {
            let lock = project_state.store.lock().map_err(|e| e.to_string())?;
            let store = lock.as_ref().ok_or("No project open")?;
            let snapshot = store.snapshot(&project_id)?;
            // Children are nodes that this node implements (i.e., outgoing "implements" edges)
            // or nodes that this node blocks (reversed: children depend on parent)
            snapshot
                .edges
                .iter()
                .filter(|e| {
                    e.from_id == params.node_id
                        && (e.edge_type == "implements" || e.edge_type == "blocks")
                })
                .map(|e| e.to_id.clone())
                .collect::<Vec<_>>()
        };

        for child_node_id in child_node_ids {
            let child_node_type = {
                let lock = project_state.store.lock().map_err(|e| e.to_string())?;
                let store = lock.as_ref().ok_or("No project open")?;
                store.get_node(&child_node_id)?.node_type
            };

            // Only fan out to Task and Feature nodes
            if child_node_type != "Task" && child_node_type != "Feature" {
                continue;
            }

            let child_wf_type = infer_workflow_type(&child_node_type).to_string();

            match create_one_workflow(
                &app,
                &project_state,
                &agent_state,
                &project_id,
                &child_node_id,
                &child_wf_type,
                &params.cmd,
                &params.args,
                params.env.clone(),
            ) {
                Ok((child_wf, _)) => {
                    child_workflow_ids.push(child_wf.id);
                }
                Err(e) => {
                    eprintln!("[workflow] Fan-out failed for node '{}': {}", child_node_id, e);
                }
            }
        }
    }

    // Emit workflow:status for the parent
    let _ = app.emit(
        "workflow:status",
        WorkflowStatusEvent {
            workflow_id: workflow.id.clone(),
            node_id: params.node_id.clone(),
            agent_id: agent_id.clone(),
            status: "running".to_string(),
            current_step: None,
            started_at: workflow.started_at.clone(),
        },
    );

    Ok(CreateWorkflowResult {
        workflow,
        agent_id,
        child_workflow_ids,
    })
}

/// List all workflows for the open project, optionally filtered by status.
#[tauri::command]
pub fn list_workflows(
    params: ListWorkflowsParams,
    project_state: State<'_, ProjectState>,
) -> Result<Vec<WorkflowRecord>, String> {
    let project_id = project_state
        .project_id
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or("No project open")?;

    let lock = project_state.store.lock().map_err(|e| e.to_string())?;
    let store = lock.as_ref().ok_or("No project open")?;
    let mut records = store.list_workflows(&project_id)?;

    if let Some(filter) = params.status_filter {
        records.retain(|r| r.status == filter);
    }

    Ok(records)
}

/// Cancel a running workflow: SIGTERM the agent, mark workflow cancelled.
#[tauri::command]
pub fn cancel_workflow(
    params: CancelWorkflowParams,
    project_state: State<'_, ProjectState>,
    agent_state: State<'_, AgentState>,
) -> Result<WorkflowRecord, String> {
    // Graceful stop: SIGTERM → SIGKILL after 3s
    agent_state.stop_workflow_agent_graceful(&params.workflow_id);

    // Mark workflow cancelled in SQLite
    let lock = project_state.store.lock().map_err(|e| e.to_string())?;
    let store = lock.as_ref().ok_or("No project open")?;
    store.update_workflow_status(
        &params.workflow_id,
        "cancelled",
        None,
        params.reason.as_deref(),
    )
}
