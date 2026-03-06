// bp6-1vf: ProjectLifecycleWorkflow — Restate durable workflow
//
// Replaces the SQLite-backed state machine (bp6-ims.2) with a proper Restate
// durable workflow. State lives in Restate; promises gate each human-approval
// checkpoint; ctx.run wraps all side effects.
//
// V-model lifecycle steps:
//   1 — Concept Development        (API-driven, no PTY)
//   2 — Guardrails Definition      (API-driven, substeps 2.1→2.4→2.review)
//   3 — Stage Planning             (PTY: product-manager)
//   4 — Autonomous Execution       (PTY fan-out: task agents)
//   5 — Rework (optional)          (PTY: product-manager rework-planning)
//   6 — Replanning & QA            (API-driven: validity + rca substeps)

use std::path::PathBuf;

use restate_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::agents::{spawn_agent_internal, AgentState, SpawnAgentParams};
use crate::dag::DagStore;
use crate::project::ProjectState;
use crate::skills::build_skills_prompt;

pub const LIFECYCLE_SERVICE_PORT: u16 = 9083;
pub const RESTATE_INGRESS_URL: &str = "http://localhost:9080";

/// Maps lifecycle step to skill ID. Substeps use "step.substep" format.
pub const STEP_SPECIALIST_MAP: &[(&str, &str)] = &[
    ("1",          "operational-analyst"),
    ("2.1",        "architecture-analyst"),
    ("2.2",        "design-system-analyst"),
    ("2.3",        "user-analyst"),
    ("2.4",        "must-not-analyst"),
    ("2.review",   "engineering-manager"),
    ("3",          "product-manager"),
    ("6.validity", "validity-analyst"),
    ("6.rca",      "rca-analyst"),
];

// ── DTOs ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowInput {
    pub project_id: String,
    pub stage_number: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleStatus {
    pub step: u32,
    pub substep: Option<String>,
    /// One of: "idle" | "running" | "awaiting_approval" | "complete"
    pub status: String,
    pub pending_approval_id: Option<String>,
    pub project_id: String,
    pub active_agent_id: Option<String>,
    pub stage_task_count: Option<u32>,
    pub stage_number: Option<u32>,
}

// ── Restate workflow definition ───────────────────────────────────────────────

#[restate_sdk::workflow]
trait ProjectLifecycleWorkflow {
    /// Main durable run handler — executes the full V-model lifecycle loop.
    async fn run(input: Json<WorkflowInput>) -> Result<(), HandlerError>;

    /// Advance past the current human-approval gate.
    /// `decision` can be "approve", "reject", "revise", "deploy", "skip", or
    /// a substep-specific value. The run handler decides what to do with it.
    #[shared]
    async fn approve_step(decision: Json<serde_json::Value>) -> Result<(), HandlerError>;

    /// Resolve a named workflow promise from outside (used by task_done, etc.)
    #[shared]
    async fn resolve_promise(payload: Json<ResolvePromisePayload>) -> Result<(), HandlerError>;

    /// Read current workflow state without blocking.
    #[shared]
    async fn get_status() -> Result<Json<LifecycleStatus>, HandlerError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvePromisePayload {
    pub promise_name: String,
    pub value: String,
}

// ── Implementation ────────────────────────────────────────────────────────────

pub struct ProjectLifecycleWorkflowImpl {
    pub app_handle: AppHandle,
}

impl ProjectLifecycleWorkflow for ProjectLifecycleWorkflowImpl {
    // ── run ────────────────────────────────────────────────────────────────────

    async fn run(
        &self,
        ctx: WorkflowContext<'_>,
        input: Json<WorkflowInput>,
    ) -> Result<(), HandlerError> {
        let project_id = ctx.key().to_string();
        let initial_stage = input.0.stage_number;
        let app = self.app_handle.clone();

        // ── Step 1: Concept Development (API-driven) ──────────────────────────
        ctx.set("step", 1u32);
        ctx.set("substep", Option::<String>::None);
        ctx.set("status", "running".to_string());
        ctx.set("stage_number", initial_stage);

        emit_status_event(&app, &project_id, 1, None, "running");

        ctx.set("status", "awaiting_approval".to_string());
        emit_status_event(&app, &project_id, 1, None, "awaiting_approval");

        let _decision1: Json<serde_json::Value> =
            ctx.promise::<Json<serde_json::Value>>("step-1-approval").await?;

        // ── Step 2: Guardrails Definition (API-driven, 5 substeps) ────────────
        let step2_substeps = ["1", "2", "3", "4", "review"];
        for substep in &step2_substeps {
            ctx.set("step", 2u32);
            ctx.set("substep", Some(substep.to_string()));
            ctx.set("status", "running".to_string());
            emit_status_event(&app, &project_id, 2, Some(substep), "running");

            ctx.set("status", "awaiting_approval".to_string());
            emit_status_event(&app, &project_id, 2, Some(substep), "awaiting_approval");

            let _dec: Json<serde_json::Value> = ctx
                .promise::<Json<serde_json::Value>>(&format!("step-2-{}-approval", substep))
                .await?;
        }

        // ── Step 3+: Stage loop (may repeat for multi-stage projects) ─────────
        let mut stage_number = initial_stage;

        loop {
            ctx.set("stage_number", stage_number);

            // ── Step 3: Stage Planning (PTY: product-manager) ─────────────────
            ctx.set("step", 3u32);
            ctx.set("substep", Option::<String>::None);
            ctx.set("status", "running".to_string());
            emit_status_event(&app, &project_id, 3, None, "running");

            // Durable side effect: spawn product-manager agent
            let pm_prompt = if stage_number == 1 {
                format!(
                    "Plan Stage {} of the project. Decompose it into a set of tasks for autonomous execution.",
                    stage_number
                )
            } else {
                format!(
                    "Plan Stage {} of the project. Review prior stages and artefacts, then decompose the next stage into a set of tasks for execution.",
                    stage_number
                )
            };

            let app_c1 = app.clone();
            let pm_prompt_clone = pm_prompt.clone();
            let pid_c1 = project_id.clone();

            let pm_agent_id: String = ctx
                .run(|| async move {
                    let ps = app_c1.state::<ProjectState>();
                    let as_ = app_c1.state::<AgentState>();
                    let project_dir = ps.active_dir_str();
                    spawn_step_agent_with_prompt(
                        &pid_c1,
                        "product-manager",
                        &pm_prompt_clone,
                        project_dir.as_deref(),
                        &ps,
                        &as_,
                        &app_c1,
                    )
                    .map_err(|e| HandlerError::from(TerminalError::new(e)))
                })
                .name(&format!("spawn-pm-agent-stage-{}", stage_number))
                .await?;

            ctx.set("active_agent_id", Some(pm_agent_id.clone()));
            ctx.set("status", "awaiting_approval".to_string());
            emit_status_event(&app, &project_id, 3, None, "awaiting_approval");

            // Wait for PM agent to complete (agents.rs resolves "step-3-done" via resolve_promise)
            let _pm_done: String =
                ctx.promise::<String>("step-3-done").await?;

            // Collect task IDs from DAG
            let app_c2 = app.clone();
            let task_ids: Vec<String> = ctx
                .run(|| async move {
                    let ps = app_c2.state::<ProjectState>();
                    ps.with_active(|store, pid| store.get_task_nodes_for_project(pid))
                        .map(|nodes| Json(nodes.into_iter().map(|n| n.id).collect::<Vec<_>>()))
                        .map_err(|e| HandlerError::from(TerminalError::new(e)))
                })
                .name(&format!("collect-tasks-stage-{}", stage_number))
                .await?
                .0;

            let task_ids_json = serde_json::to_string(&task_ids).unwrap_or_default();
            ctx.set("stage_task_ids", task_ids_json.clone());
            ctx.set("stage_task_count", task_ids.len() as u32);
            ctx.set("active_agent_id", Option::<String>::None);

            // Approval gate: user approves step 3 plan before fan-out
            ctx.set("status", "awaiting_approval".to_string());
            emit_status_event(&app, &project_id, 3, None, "awaiting_approval");

            let _step3_approval: Json<serde_json::Value> =
                ctx.promise::<Json<serde_json::Value>>("step-3-approval").await?;

            // ── Step 4: Autonomous Execution (fan-out) ────────────────────────
            ctx.set("step", 4u32);
            ctx.set("substep", Option::<String>::None);
            ctx.set("status", "running".to_string());
            emit_status_event(&app, &project_id, 4, None, "running");

            // Fan-out: spawn ready task agents (durable side effect)
            let app_c3 = app.clone();
            let task_ids_clone = task_ids.clone();
            let pid_c3 = project_id.clone();

            ctx.run(|| async move {
                let ps = app_c3.state::<ProjectState>();
                let as_ = app_c3.state::<AgentState>();
                spawn_execution_workflow_sync(
                    &pid_c3,
                    &task_ids_clone,
                    &ps,
                    &as_,
                    &app_c3,
                )
                .await
                .map_err(|e| HandlerError::from(TerminalError::new(e)))
            })
            .name(&format!("fan-out-tasks-stage-{}", stage_number))
            .await?;

            // Wait for all tasks to complete (each resolved by agents.rs via resolve_promise)
            for task_id in &task_ids {
                let _done: String = ctx
                    .promise::<String>(&format!("task-{}-done", task_id))
                    .await?;
            }

            // All tasks done — wait for PM review approval (deploy/rework)
            ctx.set("substep", Some("pm-review".to_string()));
            ctx.set("status", "awaiting_approval".to_string());
            emit_status_event(&app, &project_id, 4, Some("pm-review"), "awaiting_approval");

            let step4_decision: Json<serde_json::Value> =
                ctx.promise::<Json<serde_json::Value>>("step-4-approval").await?;

            let decision_str = step4_decision
                .0.as_str()
                .unwrap_or("")
                .to_string();

            if decision_str == "rework" {
                // ── Step 5: Rework ────────────────────────────────────────────
                ctx.set("step", 5u32);
                ctx.set("substep", Option::<String>::None);
                ctx.set("status", "awaiting_rework".to_string());
                emit_status_event(&app, &project_id, 5, None, "awaiting_rework");

                // Wait for human to submit rework items
                let rework_items: String =
                    ctx.promise::<String>("step-5-rework-items").await?;

                if !rework_items.trim().is_empty() {
                    ctx.set("rework_items", rework_items.clone());
                    ctx.set("substep", Some("rework-planning".to_string()));
                    ctx.set("status", "running".to_string());
                    emit_status_event(&app, &project_id, 5, Some("rework-planning"), "running");

                    // Spawn PM rework-planning agent
                    let pm_rework_prompt = format!(
                        "Review these rework items and create a minimal task plan to address them.\n\n## Rework Items\n\n{}\n\nCreate only the tasks necessary to address the rework items. Do not re-do work already completed satisfactorily.",
                        rework_items
                    );

                    let app_rw = app.clone();
                    let pid_rw = project_id.clone();
                    let prompt_rw = pm_rework_prompt.clone();

                    let _rw_agent_id: String = ctx
                        .run(|| async move {
                            let ps = app_rw.state::<ProjectState>();
                            let as_ = app_rw.state::<AgentState>();
                            let project_dir = ps.active_dir_str();
                            spawn_step_agent_with_prompt(
                                &pid_rw,
                                "product-manager",
                                &prompt_rw,
                                project_dir.as_deref(),
                                &ps,
                                &as_,
                                &app_rw,
                            )
                            .map_err(|e| HandlerError::from(TerminalError::new(e)))
                        })
                        .name(&format!("spawn-rework-pm-stage-{}", stage_number))
                        .await?;

                    // Wait for rework PM to complete
                    let _rw_done: String = ctx.promise::<String>("step-5-planning-done").await?;

                    // Collect new rework task IDs
                    let app_rw2 = app.clone();
                    let rework_task_ids: Vec<String> = ctx
                        .run(|| async move {
                            let ps = app_rw2.state::<ProjectState>();
                            ps.with_active(|store, pid| store.get_task_nodes_for_project(pid))
                                .map(|nodes| Json(nodes.into_iter().map(|n| n.id).collect::<Vec<_>>()))
                                .map_err(|e| HandlerError::from(TerminalError::new(e)))
                        })
                        .name(&format!("collect-rework-tasks-stage-{}", stage_number))
                        .await?
                        .0;

                    let rework_task_ids_json = serde_json::to_string(&rework_task_ids).unwrap_or_default();
                    ctx.set("stage_task_ids", rework_task_ids_json);
                    ctx.set("active_agent_id", Option::<String>::None);
                    ctx.set("substep", Some("execution".to_string()));
                    ctx.set("status", "running".to_string());
                    emit_status_event(&app, &project_id, 5, Some("execution"), "running");

                    // Fan-out rework tasks
                    let app_rw3 = app.clone();
                    let task_ids_rw = rework_task_ids.clone();
                    let pid_rw3 = project_id.clone();

                    ctx.run(|| async move {
                        let ps = app_rw3.state::<ProjectState>();
                        let as_ = app_rw3.state::<AgentState>();
                        spawn_execution_workflow_sync(
                            &pid_rw3,
                            &task_ids_rw,
                            &ps,
                            &as_,
                            &app_rw3,
                        )
                        .await
                        .map_err(|e| HandlerError::from(TerminalError::new(e)))
                    })
                    .name(&format!("fan-out-rework-stage-{}", stage_number))
                    .await?;

                    // Wait for all rework tasks
                    for task_id in &rework_task_ids {
                        let _done: String = ctx
                            .promise::<String>(&format!("task-{}-done", task_id))
                            .await?;
                    }
                }
                // (if rework_items is empty, skip directly to step 6)
            }

            // ── Step 6: Replanning & QA (API-driven) ──────────────────────────
            ctx.set("step", 6u32);
            ctx.set("substep", Some("validity".to_string()));
            ctx.set("status", "running".to_string());
            ctx.set("active_agent_id", Option::<String>::None);
            emit_status_event(&app, &project_id, 6, Some("validity"), "running");

            ctx.set("status", "awaiting_approval".to_string());
            emit_status_event(&app, &project_id, 6, Some("validity"), "awaiting_approval");

            let _validity_approval: Json<serde_json::Value> =
                ctx.promise::<Json<serde_json::Value>>("step-6-validity-approval").await?;

            ctx.set("substep", Some("rca".to_string()));
            ctx.set("status", "running".to_string());
            emit_status_event(&app, &project_id, 6, Some("rca"), "running");

            ctx.set("status", "awaiting_approval".to_string());
            emit_status_event(&app, &project_id, 6, Some("rca"), "awaiting_approval");

            let rca_decision: Json<serde_json::Value> =
                ctx.promise::<Json<serde_json::Value>>("step-6-rca-approval").await?;

            let rca_str = rca_decision.0.as_str().unwrap_or("next-stage").to_string();

            if rca_str == "complete" {
                // Lifecycle complete
                break;
            }

            // Loop to next stage
            stage_number += 1;
            ctx.set("stage_number", stage_number);

            // Reset per-stage tracking
            ctx.set("step", 3u32);
            ctx.set("substep", Option::<String>::None);
            ctx.set("status", "running".to_string());
            ctx.set("stage_task_ids", String::new());
            ctx.set("stage_task_count", 0u32);
            ctx.set("active_agent_id", Option::<String>::None);
        }

        ctx.set("step", 6u32);
        ctx.set("status", "complete".to_string());
        emit_status_event(&app, &project_id, 6, None, "complete");

        Ok(())
    }

    // ── approve_step ──────────────────────────────────────────────────────────

    async fn approve_step(
        &self,
        ctx: SharedWorkflowContext<'_>,
        decision: Json<serde_json::Value>,
    ) -> Result<(), HandlerError> {
        let step: u32 = ctx.get("step").await?.unwrap_or(0);
        let substep: Option<String> = ctx.get("substep").await?;

        // Map current step/substep to the correct promise name
        let promise_name = match (step, substep.as_deref()) {
            (1, _) => "step-1-approval".to_string(),
            (2, Some(sub)) => format!("step-2-{}-approval", sub),
            (2, None) => "step-2-1-approval".to_string(),
            (3, Some("approval")) | (3, None) => {
                // Could be the plan-approval gate (post-PM-done) or the PM-done acknowledgment
                // Use "step-3-approval" for the plan gate
                "step-3-approval".to_string()
            }
            (4, _) => "step-4-approval".to_string(),
            (5, Some("awaiting_rework")) | (5, None) => "step-5-rework-items".to_string(),
            (6, Some("validity")) => "step-6-validity-approval".to_string(),
            (6, Some("rca")) => "step-6-rca-approval".to_string(),
            _ => format!("step-{}-approval", step),
        };

        ctx.resolve_promise::<Json<serde_json::Value>>(&promise_name, decision);
        Ok(())
    }

    // ── resolve_promise ────────────────────────────────────────────────────────

    async fn resolve_promise(
        &self,
        ctx: SharedWorkflowContext<'_>,
        payload: Json<ResolvePromisePayload>,
    ) -> Result<(), HandlerError> {
        ctx.resolve_promise::<String>(&payload.0.promise_name, payload.0.value.clone());
        Ok(())
    }

    // ── get_status ────────────────────────────────────────────────────────────

    async fn get_status(
        &self,
        ctx: SharedWorkflowContext<'_>,
    ) -> Result<Json<LifecycleStatus>, HandlerError> {
        let step: u32 = ctx.get("step").await?.unwrap_or(0);
        let substep: Option<String> = ctx.get("substep").await?;
        let status: String = ctx
            .get("status")
            .await?
            .unwrap_or_else(|| "idle".to_string());
        let stage_number: Option<u32> = ctx.get("stage_number").await?;
        let stage_task_count: Option<u32> = ctx.get("stage_task_count").await?;
        let active_agent_id: Option<String> = ctx.get("active_agent_id").await?;
        let project_id = ctx.key().to_string();

        Ok(Json(LifecycleStatus {
            step,
            substep,
            status,
            pending_approval_id: None,
            project_id,
            active_agent_id,
            stage_task_count,
            stage_number,
        }))
    }
}

// ── Tauri commands ─────────────────────────────────────────────────────────────

/// Returns the current lifecycle status for a project via Restate get_status.
/// Falls back to idle if the workflow has not been started.
#[tauri::command]
pub async fn get_lifecycle_status(project_id: String) -> Result<LifecycleStatus, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/ProjectLifecycleWorkflow/{}/get_status",
        RESTATE_INGRESS_URL, project_id
    );
    match client.get(&url).send().await {
        Ok(resp) => {
            let text = resp.text().await.map_err(|e| e.to_string())?;
            serde_json::from_str::<LifecycleStatus>(&text).map_err(|_| {
                // Workflow not yet started — return idle
                text.clone()
            })
        }
        Err(_) => Ok(LifecycleStatus {
            step: 0,
            substep: None,
            status: "idle".to_string(),
            pending_approval_id: None,
            project_id: project_id.clone(),
            active_agent_id: None,
            stage_task_count: None,
            stage_number: None,
        }),
    }
    .or_else(|_| {
        Ok::<LifecycleStatus, String>(LifecycleStatus {
            step: 0,
            substep: None,
            status: "idle".to_string(),
            pending_approval_id: None,
            project_id,
            active_agent_id: None,
            stage_task_count: None,
            stage_number: None,
        })
    })
}

/// Starts the lifecycle for a project by POSTing to the Restate workflow run endpoint.
/// Idempotent: Restate deduplicates concurrent starts for the same workflow key.
#[tauri::command]
pub async fn start_lifecycle(project_id: String) -> Result<LifecycleStatus, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/ProjectLifecycleWorkflow/{}/run",
        RESTATE_INGRESS_URL, project_id
    );
    let body = serde_json::json!({
        "projectId": project_id,
        "stageNumber": 1
    });
    client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Failed to start lifecycle: {}", e))?;

    Ok(LifecycleStatus {
        step: 1,
        substep: None,
        status: "running".to_string(),
        pending_approval_id: None,
        project_id,
        active_agent_id: None,
        stage_task_count: None,
        stage_number: Some(1),
    })
}

/// Approves (or provides a decision for) the current lifecycle step.
/// Calls the Restate approve_step shared handler.
#[tauri::command]
pub async fn approve_lifecycle_step(
    project_id: String,
    _step: u32,
    decision: String,
) -> Result<LifecycleStatus, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/ProjectLifecycleWorkflow/{}/approve_step",
        RESTATE_INGRESS_URL, project_id
    );
    client
        .post(&url)
        .json(&serde_json::Value::String(decision.clone()))
        .send()
        .await
        .map_err(|e| format!("Failed to approve lifecycle step: {}", e))?;

    // Return current status from Restate
    get_lifecycle_status(project_id).await
}

/// Submit rework items after a Step 4 PM review decision of "rework".
/// Resolves the "step-5-rework-items" promise, unblocking the run handler.
#[tauri::command]
pub async fn submit_rework_items(
    project_id: String,
    rework_items: String,
) -> Result<LifecycleStatus, String> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/ProjectLifecycleWorkflow/{}/resolve_promise",
        RESTATE_INGRESS_URL, project_id
    );
    let payload = ResolvePromisePayload {
        promise_name: "step-5-rework-items".to_string(),
        value: rework_items,
    };
    client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Failed to submit rework items: {}", e))?;

    get_lifecycle_status(project_id).await
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Emit a `lifecycle:status` Tauri event to the frontend.
fn emit_status_event(
    app: &AppHandle,
    project_id: &str,
    step: u32,
    substep: Option<&str>,
    status: &str,
) {
    let _ = app.emit(
        "lifecycle:status",
        serde_json::json!({
            "projectId": project_id,
            "step": step,
            "substep": substep,
            "status": status,
        }),
    );
}

/// Spawn execution workflow — wraps the async fan-out in a sync-compatible form.
/// Called from inside ctx.run() closures (which require owned values).
async fn spawn_execution_workflow_sync(
    project_id: &str,
    task_ids: &[String],
    project_state: &tauri::State<'_, ProjectState>,
    agent_state: &tauri::State<'_, AgentState>,
    app_handle: &AppHandle,
) -> Result<(), String> {
    let task_ids_json = serde_json::to_string(task_ids).unwrap_or_default();
    spawn_execution_workflow(project_id, Some(&task_ids_json), project_state, agent_state, app_handle).await
}

/// Step 4 execution fan-out.
///
/// Parses task IDs, resolves which ones are immediately ready (no unmet
/// dependencies), and spawns a PTY task agent for each.
pub async fn spawn_execution_workflow(
    project_id: &str,
    task_ids_json: Option<&str>,
    project_state: &tauri::State<'_, ProjectState>,
    agent_state: &tauri::State<'_, AgentState>,
    app_handle: &AppHandle,
) -> Result<(), String> {
    let all_task_ids: Vec<String> = match task_ids_json {
        Some(json) => serde_json::from_str(json)
            .map_err(|e| format!("Failed to parse task_ids_json: {}", e))?,
        None => {
            eprintln!(
                "[lifecycle] ExecutionWorkflow: no task IDs — nothing to execute for '{}'",
                project_id
            );
            return Ok(());
        }
    };

    if all_task_ids.is_empty() {
        return Ok(());
    }

    eprintln!(
        "[lifecycle] ExecutionWorkflow: {} tasks queued for project='{}'",
        all_task_ids.len(),
        project_id
    );

    let ready_tasks: Vec<crate::dag::DagNode> =
        project_state.with_active(|store, _| store.get_ready_tasks(&all_task_ids, &[]))?;

    eprintln!(
        "[lifecycle] ExecutionWorkflow: {} tasks immediately ready for '{}'",
        ready_tasks.len(),
        project_id
    );

    let project_dir = project_state.active_dir_str();

    let artefacts = project_state.with_active(|store, pid| store.get_artefacts_for_step(pid, 4))?;
    let artefact_context = DagStore::build_artefact_context(&artefacts);

    for task in &ready_tasks {
        let task_title = task.data["title"].as_str().unwrap_or("Untitled Task");
        let task_description = task.data["description"].as_str().unwrap_or("");
        let skill_id = task.data["skill_id"]
            .as_str()
            .unwrap_or("poe-implementation");

        let project_dir_buf = project_dir.as_deref().map(PathBuf::from);
        let skill_prompt =
            build_skills_prompt(&[skill_id.to_string()], app_handle, project_dir_buf.as_deref());

        let prompt = if artefact_context.is_empty() {
            format!(
                "{}\n\n---\n## Task\n\n**{}**\n\n{}\n\nProject ID: {}\n\nExecute this task.",
                skill_prompt, task_title, task_description, project_id
            )
        } else {
            format!(
                "{}\n\n---\n## Prior Artefacts\n\n{}\n\n---\n## Task\n\n**{}**\n\n{}\n\nProject ID: {}\n\nUsing the prior artefacts as context, execute this task.",
                skill_prompt, artefact_context, task_title, task_description, project_id
            )
        };

        match spawn_agent_internal(
            SpawnAgentParams {
                cmd: "claude".to_string(),
                args: vec![prompt],
                env: None,
                agent_id: None,
                workflow_id: Some(project_id.to_string()),
                node_id: None,
                workflow_type: Some(format!("lifecycle-step-4-task-{}", task.id)),
                session_id: None,
                resume: false,
                cwd: project_dir.clone(),
            },
            app_handle,
            agent_state,
        ) {
            Ok(spawned) => {
                eprintln!(
                    "[lifecycle] Spawned task agent={} for task='{}' project='{}'",
                    spawned.agent_id, task.id, project_id
                );
            }
            Err(e) => {
                eprintln!(
                    "[lifecycle] Failed to spawn task agent for task='{}' project='{}': {}",
                    task.id, project_id, e
                );
            }
        }
    }

    Ok(())
}

/// Spawn the appropriate PTY agent for a lifecycle step using STEP_SPECIALIST_MAP.
pub fn spawn_step_agent(
    project_id: &str,
    step: u32,
    substep: Option<&str>,
    project_dir: Option<&str>,
    project_state: &tauri::State<'_, ProjectState>,
    agent_state: &tauri::State<'_, AgentState>,
    app: &AppHandle,
) -> Result<String, String> {
    let step_key = match substep {
        Some(sub) => format!("{}.{}", step, sub),
        None => step.to_string(),
    };
    let skill_id = STEP_SPECIALIST_MAP
        .iter()
        .find(|(k, _)| *k == step_key.as_str())
        .map(|(_, v)| *v)
        .ok_or_else(|| format!("No skill mapped for step {}", step_key))?;

    let artefacts = project_state.with_active(|store, pid| store.get_artefacts_for_step(pid, step))?;
    let artefact_context = DagStore::build_artefact_context(&artefacts);

    let project_dir_buf = project_dir.map(PathBuf::from);
    let skill_prompt = build_skills_prompt(&[skill_id.to_string()], app, project_dir_buf.as_deref());

    let prompt = if artefact_context.is_empty() {
        format!(
            "{}\n\n---\nProject ID: {}\nLifecycle Step: {}\n\nBegin your analysis.",
            skill_prompt, project_id, step_key
        )
    } else {
        format!(
            "{}\n\n---\n## Prior Artefacts\n\n{}\n\n---\nProject ID: {}\nLifecycle Step: {}\n\nUsing the prior artefacts as context, begin your analysis.",
            skill_prompt, artefact_context, project_id, step_key
        )
    };

    let spawned = spawn_agent_internal(
        SpawnAgentParams {
            cmd: "claude".to_string(),
            args: vec![prompt],
            env: None,
            agent_id: None,
            workflow_id: Some(project_id.to_string()),
            node_id: None,
            workflow_type: Some(format!("lifecycle-step-{}", step_key)),
            session_id: None,
            resume: false,
            cwd: project_dir.map(|s| s.to_string()),
        },
        app,
        agent_state,
    )?;

    Ok(spawned.agent_id)
}

/// Spawn a lifecycle step agent with a pre-built prompt, bypassing STEP_SPECIALIST_MAP.
pub fn spawn_step_agent_with_prompt(
    project_id: &str,
    skill_id: &str,
    prompt_body: &str,
    project_dir: Option<&str>,
    project_state: &tauri::State<'_, ProjectState>,
    agent_state: &tauri::State<'_, AgentState>,
    app: &AppHandle,
) -> Result<String, String> {
    let project_dir_buf = project_dir.map(PathBuf::from);
    let skill_prompt = build_skills_prompt(&[skill_id.to_string()], app, project_dir_buf.as_deref());

    let artefacts = project_state.with_active(|store, pid| store.get_artefacts_for_step(pid, 99))?;
    let artefact_context = DagStore::build_artefact_context(&artefacts);

    let full_prompt = if artefact_context.is_empty() {
        format!("{}\n\n---\nProject ID: {}\n\n{}", skill_prompt, project_id, prompt_body)
    } else {
        format!(
            "{}\n\n---\n## Prior Artefacts\n\n{}\n\n---\nProject ID: {}\n\n{}",
            skill_prompt, artefact_context, project_id, prompt_body
        )
    };

    let spawned = spawn_agent_internal(
        SpawnAgentParams {
            cmd: "claude".to_string(),
            args: vec![full_prompt],
            env: None,
            agent_id: None,
            workflow_id: Some(project_id.to_string()),
            node_id: None,
            workflow_type: Some(format!("lifecycle-{}-review", skill_id)),
            session_id: None,
            resume: false,
            cwd: project_dir.map(|s| s.to_string()),
        },
        app,
        agent_state,
    )?;

    Ok(spawned.agent_id)
}

// ── Spawn ─────────────────────────────────────────────────────────────────────

/// Spawn the Restate lifecycle workflow HTTP endpoint on port 9083.
/// Called from Tauri setup; runs in its own Tokio task.
pub fn spawn_lifecycle_service(app_handle: AppHandle) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new()
            .expect("Failed to create Tokio runtime for lifecycle_service");
        rt.block_on(async {
            let addr = format!("0.0.0.0:{}", LIFECYCLE_SERVICE_PORT)
                .parse()
                .expect("Invalid lifecycle service address");

            let impl_ = ProjectLifecycleWorkflowImpl { app_handle };

            HttpServer::new(
                Endpoint::builder()
                    .bind(impl_.serve())
                    .build(),
            )
            .listen_and_serve(addr)
            .await;
        });
    });

    eprintln!(
        "[lifecycle] Workflow endpoint listening on port {}",
        LIFECYCLE_SERVICE_PORT
    );
}
