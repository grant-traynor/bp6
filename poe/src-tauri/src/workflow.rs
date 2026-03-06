// bp6-3d1.1 + bp6-3d1.4 + bp6-3d1.5: V-lifecycle workflows, sub-workflow orchestration, work assignment

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::agents::{spawn_agent_internal, AgentState, SpawnAgentParams, WorkflowStatusEvent};
use crate::dag::{NewWorkflow, WorkflowRecord};
use crate::project::{NodeUpsertedEvent, ProjectState};
use crate::skills::build_skills_prompt;

pub const WORKFLOW_TYPES: &[&str] = &[
    "RequirementsWorkflow",
    "DesignWorkflow",
    "ImplementationWorkflow",
    "ValidationWorkflow",
    "DeploymentWorkflow",
];

fn infer_workflow_type(node_type: &str) -> &'static str {
    match node_type {
        "Feature" | "Task" => "ImplementationWorkflow",
        "Epic" => "DesignWorkflow",
        "Review" => "ValidationWorkflow",
        _ => "ImplementationWorkflow",
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkflowParams {
    pub node_id: String,
    pub workflow_type: Option<String>,
    pub cmd: String,
    pub args: Vec<String>,
    pub env: Option<HashMap<String, String>>,
    pub fanout: Option<bool>,
    /// Skill IDs to prepend as context. The last arg in `args` is treated as the task prompt.
    pub skill_ids: Option<Vec<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorkflowResult {
    pub workflow: WorkflowRecord,
    pub agent_id: String,
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
    parent_workflow_id: Option<String>,
) -> Result<(WorkflowRecord, String), String> {
    // Get node context
    let node = project_state.with_active(|store, _| store.get_node(node_id))?;

    // Generate session ID before creating the workflow record so it is persisted
    // before the agent starts — a crash immediately after spawn can still resume.
    let session_id = Uuid::new_v4().to_string();

    // Create workflow record
    let workflow = project_state.with_active(|store, _| {
        store.create_workflow_record(NewWorkflow {
            project_id: project_id.to_string(),
            node_id: node_id.to_string(),
            agent_id: None,
            workflow_type: workflow_type.to_string(),
            config: serde_json::json!({ "cmd": cmd, "args": args }),
            parent_workflow_id: parent_workflow_id.clone(),
            session_id: Some(session_id.clone()),
        })
    })?;

    // Build env with node context
    let mut agent_env = env.unwrap_or_default();
    agent_env.insert("POE_WORKFLOW_ID".to_string(), workflow.id.clone());
    agent_env.insert("POE_NODE_ID".to_string(), node_id.to_string());
    agent_env.insert("POE_WORKFLOW_TYPE".to_string(), workflow_type.to_string());
    agent_env.insert("POE_NODE_TYPE".to_string(), node.node_type.clone());
    if let Ok(data_str) = serde_json::to_string(&node.data) {
        agent_env.insert("POE_NODE_DATA".to_string(), data_str);
    }

    // Spawn agent
    let spawned = spawn_agent_internal(
        SpawnAgentParams {
            cmd: cmd.to_string(),
            args: args.to_vec(),
            env: Some(agent_env),
            agent_id: None,
            workflow_id: Some(workflow.id.clone()),
            node_id: Some(node_id.to_string()),
            workflow_type: Some(workflow_type.to_string()),
            session_id: Some(session_id),
            resume: false,
        },
        app,
        agent_state,
    )?;

    // Update workflow with agent_id + running
    let workflow = project_state.with_active(|store, _| {
        store.update_workflow_status(&workflow.id, "running", Some(&spawned.agent_id), None)
    })?;

    // Update DAG node: in_progress + workflowId
    let updated_node = project_state.with_active(|store, _| {
        let mut data = node.data.clone();
        data["status"] = serde_json::json!("in_progress");
        data["workflowId"] = serde_json::json!(workflow.id);
        data["workflowType"] = serde_json::json!(workflow_type);
        store.update_node(node_id, data)
    })?;

    app.emit("dag:node:upserted", NodeUpsertedEvent { node: updated_node })
        .map_err(|e| format!("Failed to emit dag:node:upserted: {}", e))?;

    Ok((workflow, spawned.agent_id))
}

// ── Prompt enrichment ─────────────────────────────────────────────────────────

/// Prepend skill context blocks to the task prompt (last element of args).
/// Returns a new args vec with the enriched prompt in place of the last arg.
fn build_enriched_args(
    app: &AppHandle,
    project_state: &ProjectState,
    args: &[String],
    skill_ids: &Option<Vec<String>>,
) -> Vec<String> {
    let ids = match skill_ids {
        Some(ids) if !ids.is_empty() => ids,
        _ => return args.to_vec(),
    };

    let project_dir = project_state
        .active_dir_str()
        .map(std::path::PathBuf::from);

    let skills_block = build_skills_prompt(ids, app, project_dir.as_deref());
    if skills_block.is_empty() {
        return args.to_vec();
    }

    let mut new_args = args.to_vec();
    if let Some(last) = new_args.last_mut() {
        *last = format!("{}\n\n{}", skills_block, last);
    }
    new_args
}

// ── Tauri commands ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn create_workflow(
    params: CreateWorkflowParams,
    app: AppHandle,
    project_state: State<'_, ProjectState>,
    agent_state: State<'_, AgentState>,
) -> Result<CreateWorkflowResult, String> {
    let (project_id, node_type) = project_state.with_active(|store, project_id| {
        let node = store.get_node(&params.node_id)?;
        Ok((project_id.to_string(), node.node_type))
    })?;

    let workflow_type = params
        .workflow_type
        .as_deref()
        .unwrap_or_else(|| infer_workflow_type(&node_type))
        .to_string();

    let enriched_args = build_enriched_args(&app, &project_state, &params.args, &params.skill_ids);

    let (workflow, agent_id) = create_one_workflow(
        &app,
        &project_state,
        &agent_state,
        &project_id,
        &params.node_id,
        &workflow_type,
        &params.cmd,
        &enriched_args,
        params.env.clone(),
        None,
    )?;

    // Fan-out to child Task/Feature nodes
    let mut child_workflow_ids = Vec::new();
    if params.fanout.unwrap_or(false) {
        let child_node_ids = project_state.with_active(|store, pid| {
            let snapshot = store.snapshot(pid)?;
            Ok(snapshot
                .edges
                .iter()
                .filter(|e| {
                    e.from_id == params.node_id
                        && (e.edge_type == "implements" || e.edge_type == "blocks")
                })
                .map(|e| e.to_id.clone())
                .collect::<Vec<_>>())
        })?;

        for child_node_id in child_node_ids {
            let child_node_type = project_state.with_active(|store, _| {
                Ok(store.get_node(&child_node_id)?.node_type)
            })?;
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
                &enriched_args,
                params.env.clone(),
                Some(workflow.id.clone()),
            ) {
                Ok((child_wf, _)) => {
                    // Register this child on the parent so fan-in can gate completion.
                    if let Err(e) = project_state.with_active(|store, _| {
                        store.add_child_workflow(&workflow.id, &child_wf.id)
                    }) {
                        eprintln!("[workflow] Failed to register child workflow '{}' on parent '{}': {}", child_wf.id, workflow.id, e);
                    }
                    child_workflow_ids.push(child_wf.id);
                }
                Err(e) => eprintln!("[workflow] Fan-out failed for '{}': {}", child_node_id, e),
            }
        }
    }

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

    Ok(CreateWorkflowResult { workflow, agent_id, child_workflow_ids })
}

#[tauri::command]
pub fn list_workflows(
    params: ListWorkflowsParams,
    project_state: State<'_, ProjectState>,
) -> Result<Vec<WorkflowRecord>, String> {
    let mut records = project_state.with_active(|store, project_id| {
        store.list_workflows(project_id)
    })?;
    if let Some(filter) = params.status_filter {
        records.retain(|r| r.status == filter);
    }
    Ok(records)
}

#[tauri::command]
pub fn cancel_workflow(
    params: CancelWorkflowParams,
    project_state: State<'_, ProjectState>,
    agent_state: State<'_, AgentState>,
) -> Result<WorkflowRecord, String> {
    agent_state.stop_workflow_agent_graceful(&params.workflow_id);
    project_state.with_active(|store, _| {
        store.update_workflow_status(&params.workflow_id, "cancelled", None, params.reason.as_deref())
    })
}
