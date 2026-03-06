// bp6-ims.2: ProjectLifecycleWorkflow — top-level state machine
//
// Implementation strategy: SQLite-backed state machine (pragmatic fallback).
// The lifecycle state is persisted in the `lifecycle_state` table via DagStore.
// This achieves the same UX as a Restate workflow while being simpler and more
// robust given the partial Restate integration in this codebase.
//
// V-model lifecycle steps:
//   1 — Concept Development
//   2 — Guardrails Definition
//   3 — Stage Planning
//   4 — Autonomous Execution
//   5 — Rework (optional)
//   6 — Replanning & QA

use std::path::PathBuf;

use restate_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::agents::{spawn_agent_internal, AgentState, SpawnAgentParams};
use crate::dag::{DagStore, LifecycleStateRow};
use crate::project::ProjectState;
use crate::skills::build_skills_prompt;

pub const LIFECYCLE_SERVICE_PORT: u16 = 9083;

/// Maps lifecycle step to skill ID. Substeps use "step.substep" format.
/// Used by the agent dispatcher to select the correct specialist skill
/// when spawning an agent for a given lifecycle step.
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

// ── Public status DTO ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleStatus {
    pub step: u32,
    pub substep: Option<String>,
    /// One of: "idle" | "running" | "awaiting_approval" | "complete"
    pub status: String,
    pub pending_approval_id: Option<String>,
    pub project_id: String,
    /// The agent_id of the currently running lifecycle step agent (if any).
    /// The frontend uses this to route chat input to the right PTY agent.
    pub active_agent_id: Option<String>,
    /// Number of Task nodes queued for execution (populated after Step 3 completes).
    /// The frontend uses this to show "N tasks queued for execution".
    pub stage_task_count: Option<u32>,
}

impl From<LifecycleStateRow> for LifecycleStatus {
    fn from(row: LifecycleStateRow) -> Self {
        let stage_task_count = row
            .stage_task_ids
            .as_deref()
            .and_then(|s| serde_json::from_str::<Vec<serde_json::Value>>(s).ok())
            .map(|v| v.len() as u32);
        LifecycleStatus {
            step: row.step,
            substep: row.substep,
            status: row.status,
            pending_approval_id: row.pending_approval_id,
            project_id: row.project_id,
            active_agent_id: row.active_agent_id,
            stage_task_count,
        }
    }
}

// ── Step metadata ─────────────────────────────────────────────────────────────

const STEP_NAMES: [&str; 7] = [
    "idle",                  // 0
    "Concept Development",   // 1
    "Guardrails Definition", // 2
    "Stage Planning",        // 3
    "Autonomous Execution",  // 4
    "Rework",                // 5
    "Replanning & QA",       // 6
];

fn step_name(step: u32) -> &'static str {
    STEP_NAMES.get(step as usize).copied().unwrap_or("Unknown")
}

// ── Tauri commands ─────────────────────────────────────────────────────────────

/// Returns the current lifecycle status for a project.
/// If the lifecycle has not been started, returns step=0, status="idle".
#[tauri::command]
pub async fn get_lifecycle_status(
    project_id: String,
    state: State<'_, ProjectState>,
) -> Result<LifecycleStatus, String> {
    let row = state.with_active(|store, _| store.get_lifecycle_state(&project_id))?;
    Ok(LifecycleStatus::from(row))
}

/// Starts (or resumes) the lifecycle for a project.
/// Sets step=1 and status="running" if currently idle, then spawns the step 1 agent.
/// Idempotent: if already started, returns the current status.
#[tauri::command]
pub async fn start_lifecycle(
    project_id: String,
    state: State<'_, ProjectState>,
    agent_state: State<'_, AgentState>,
    app: AppHandle,
) -> Result<LifecycleStatus, String> {
    let current = state.with_active(|store, _| store.get_lifecycle_state(&project_id))?;

    if current.status != "idle" {
        // Already started — return current status
        eprintln!(
            "[lifecycle] start_lifecycle called for '{}' but already in status '{}' step {}",
            project_id, current.status, current.step
        );
        return Ok(LifecycleStatus::from(current));
    }

    // Persist step=1, status=running before spawning the agent
    let new_state = LifecycleStateRow {
        project_id: project_id.clone(),
        step: 1,
        substep: None,
        status: "running".to_string(),
        pending_approval_id: None,
        active_agent_id: None,
        stage_task_ids: None,
        completed_task_ids: None,
        active_task_agent_ids: None,
        stage_number: Some(1),
        rework_items: None,
    };
    state.with_active(|store, _| store.upsert_lifecycle_state(&new_state))?;

    eprintln!(
        "[lifecycle] Started lifecycle for '{}' — step 1 ({})",
        project_id,
        step_name(1)
    );

    // Spawn the step 1 agent (operational-analyst)
    let project_dir = state.active_dir_str();
    let agent_id = spawn_step_agent(
        &project_id,
        1,
        None,
        project_dir.as_deref(),
        &state,
        &agent_state,
        &app,
    )?;

    eprintln!(
        "[lifecycle] Spawned step 1 agent={} for project='{}'",
        agent_id, project_id
    );

    // Store the active_agent_id in the lifecycle state
    let updated_state = LifecycleStateRow {
        project_id: project_id.clone(),
        step: 1,
        substep: None,
        status: "running".to_string(),
        pending_approval_id: None,
        active_agent_id: Some(agent_id.clone()),
        stage_task_ids: None,
        completed_task_ids: None,
        active_task_agent_ids: None,
        stage_number: Some(1),
        rework_items: None,
    };
    state.with_active(|store, _| store.upsert_lifecycle_state(&updated_state))?;

    Ok(LifecycleStatus {
        step: 1,
        substep: None,
        status: "running".to_string(),
        pending_approval_id: None,
        project_id,
        active_agent_id: Some(agent_id),
        stage_task_count: None,
    })
}

/// Approves (or rejects) the current step, advancing the state machine.
///
/// `decision` should be "approve", "reject", or "revise".
/// On "approve": advances to the next substep or step.
/// On "reject"/"revise": keeps the agent in "running" status so the frontend
///   can send revision feedback via write_to_agent.
#[tauri::command]
pub async fn approve_lifecycle_step(
    project_id: String,
    step: u32,
    decision: String,
    state: State<'_, ProjectState>,
    agent_state: State<'_, AgentState>,
    app: AppHandle,
) -> Result<LifecycleStatus, String> {
    let current = state.with_active(|store, _| store.get_lifecycle_state(&project_id))?;

    if current.step != step {
        return Err(format!(
            "Step mismatch: project is at step {}, approval was for step {}",
            current.step, step
        ));
    }

    // Reject/revise path: set status back to running so the agent receives feedback.
    // The frontend calls write_to_agent to send the revision feedback.
    if decision == "reject" || decision == "revise" {
        eprintln!(
            "[lifecycle] '{}' {} step {}{} — returning agent to running",
            project_id,
            decision,
            step,
            current
                .substep
                .as_deref()
                .map(|s| format!(".{}", s))
                .unwrap_or_default()
        );
        let revised_state = LifecycleStateRow {
            status: "running".to_string(),
            ..current.clone()
        };
        state.with_active(|store, _| store.upsert_lifecycle_state(&revised_state))?;
        return Ok(LifecycleStatus::from(revised_state));
    }

    // ── Approve path ──────────────────────────────────────────────────────────

    // Step 2: GuardrailsWorkflow — substep progression
    // 2.1 (None/"1") → 2.2 → 2.3 → 2.4 → 2.review → step 3
    if step == 2 {
        let current_substep = current.substep.as_deref().unwrap_or("1");
        let next_substep: Option<&str> = match current_substep {
            "1" => Some("2"),
            "2" => Some("3"),
            "3" => Some("4"),
            "4" => Some("review"),
            "review" => None, // advance to step 3
            other => {
                return Err(format!("Unknown step 2 substep: {}", other));
            }
        };

        return if let Some(next_sub) = next_substep {
            // Advance to the next substep within step 2
            eprintln!(
                "[lifecycle] '{}' approved step 2.{} — advancing to step 2.{}",
                project_id, current_substep, next_sub
            );
            let next_state = LifecycleStateRow {
                project_id: project_id.clone(),
                step: 2,
                substep: Some(next_sub.to_string()),
                status: "running".to_string(),
                pending_approval_id: None,
                active_agent_id: None,
                stage_task_ids: None,
                completed_task_ids: None,
                active_task_agent_ids: None,
                stage_number: Some(1),
                rework_items: None,
            };
            state.with_active(|store, _| store.upsert_lifecycle_state(&next_state))?;

            let project_dir = state.active_dir_str();
            // For the EM review substep, include all step 2 artefacts by querying step < 3
            let artefact_step = if next_sub == "review" { 3 } else { 2 };
            match spawn_step_agent_with_artefact_step(
                &project_id,
                2,
                Some(next_sub),
                artefact_step,
                project_dir.as_deref(),
                &state,
                &agent_state,
                &app,
            ) {
                Ok(agent_id) => {
                    eprintln!(
                        "[lifecycle] Spawned step 2.{} agent={} for project='{}'",
                        next_sub, agent_id, project_id
                    );
                    let updated = LifecycleStateRow {
                        active_agent_id: Some(agent_id),
                        ..next_state.clone()
                    };
                    state.with_active(|store, _| store.upsert_lifecycle_state(&updated))?;
                    Ok(LifecycleStatus::from(updated))
                }
                Err(e) => {
                    eprintln!(
                        "[lifecycle] Failed to spawn step 2.{} agent for '{}': {}",
                        next_sub, project_id, e
                    );
                    Ok(LifecycleStatus::from(next_state))
                }
            }
        } else {
            // substep "review" approved — advance to step 3
            eprintln!(
                "[lifecycle] '{}' approved step 2.review — advancing to step 3 ({})",
                project_id,
                step_name(3)
            );
            let next_state = LifecycleStateRow {
                project_id: project_id.clone(),
                step: 3,
                substep: None,
                status: "running".to_string(),
                pending_approval_id: None,
                active_agent_id: None,
                stage_task_ids: None,
                completed_task_ids: None,
                active_task_agent_ids: None,
                stage_number: Some(1),
                rework_items: None,
            };
            state.with_active(|store, _| store.upsert_lifecycle_state(&next_state))?;

            let project_dir = state.active_dir_str();
            match spawn_step_agent(
                &project_id,
                3,
                None,
                project_dir.as_deref(),
                &state,
                &agent_state,
                &app,
            ) {
                Ok(agent_id) => {
                    eprintln!(
                        "[lifecycle] Spawned step 3 agent={} for project='{}'",
                        agent_id, project_id
                    );
                    let updated = LifecycleStateRow {
                        active_agent_id: Some(agent_id),
                        ..next_state.clone()
                    };
                    state.with_active(|store, _| store.upsert_lifecycle_state(&updated))?;
                    Ok(LifecycleStatus::from(updated))
                }
                Err(e) => {
                    eprintln!(
                        "[lifecycle] Failed to spawn step 3 agent for '{}': {}",
                        project_id, e
                    );
                    Ok(LifecycleStatus::from(next_state))
                }
            }
        };
    }

    // ── Generic step progression (steps 1, 3, 4, 5, 6) ───────────────────────

    // Step 3 → Step 4: fan-out execution — spawn parallel task agents.
    if step == 3 {
        eprintln!(
            "[lifecycle] '{}' approved step 3 — advancing to step 4 ({})",
            project_id,
            step_name(4)
        );

        // Read the task IDs collected at step 3 completion
        let task_ids_json = current.stage_task_ids.clone();

        let next_state = LifecycleStateRow {
            project_id: project_id.clone(),
            step: 4,
            substep: None,
            status: "running".to_string(),
            pending_approval_id: None,
            active_agent_id: None,
            stage_task_ids: task_ids_json.clone(),
            completed_task_ids: Some("[]".to_string()),
            active_task_agent_ids: Some("{}".to_string()),
            stage_number: Some(1),
            rework_items: None,
        };
        state.with_active(|store, _| store.upsert_lifecycle_state(&next_state))?;

        // bp6-ims.8: spawn parallel task execution fan-out
        let project_id_clone = project_id.clone();
        let task_ids_json_clone = task_ids_json.clone();
        let app_clone = app.clone();
        tauri::async_runtime::spawn(async move {
            let project_state = app_clone.state::<ProjectState>();
            let agent_state = app_clone.state::<AgentState>();
            if let Err(e) = spawn_execution_workflow(
                &project_id_clone,
                task_ids_json_clone.as_deref(),
                &project_state,
                &agent_state,
                &app_clone,
            ).await {
                eprintln!("[lifecycle] spawn_execution_workflow error for '{}': {}", project_id_clone, e);
            }
        });

        return Ok(LifecycleStatus::from(next_state));
    }

    // Step 4 approval (after PM review): deploy → step 6, rework → step 5
    if step == 4 {
        if decision == "deploy" {
            eprintln!("[lifecycle] '{}' step 4 approved (deploy) — advancing to step 6", project_id);
            let next_state = LifecycleStateRow {
                project_id: project_id.clone(),
                step: 6,
                substep: Some("validity".to_string()),
                status: "running".to_string(),
                pending_approval_id: None,
                active_agent_id: None,
                stage_task_ids: current.stage_task_ids.clone(),
                completed_task_ids: None,
                active_task_agent_ids: None,
                stage_number: current.stage_number,
                rework_items: None,
            };
            state.with_active(|store, _| store.upsert_lifecycle_state(&next_state))?;

            // bp6-ims.11: spawn validity-analyst for step 6
            let stage_num = current.stage_number.unwrap_or(1);
            let validity_prompt = format!(
                "Perform the validity check for Stage {}. Review the CONOPS and all guardrail documents against what was built this stage. Produce phase-{}-validity.md.",
                stage_num, stage_num
            );
            let project_dir = state.active_dir_str();
            match spawn_step_agent_with_prompt(
                &project_id,
                "validity-analyst",
                &validity_prompt,
                project_dir.as_deref(),
                &state,
                &agent_state,
                &app,
            ) {
                Ok(validity_agent_id) => {
                    eprintln!(
                        "[lifecycle] Spawned validity-analyst agent={} for project='{}' stage={}",
                        validity_agent_id, project_id, stage_num
                    );
                    let updated = LifecycleStateRow {
                        active_agent_id: Some(validity_agent_id),
                        ..next_state.clone()
                    };
                    state.with_active(|store, _| store.upsert_lifecycle_state(&updated))?;
                    return Ok(LifecycleStatus::from(updated));
                }
                Err(e) => {
                    eprintln!("[lifecycle] Failed to spawn validity-analyst for '{}': {}", project_id, e);
                }
            }
            return Ok(LifecycleStatus::from(next_state));
        } else {
            // rework → step 5: waiting for human to submit rework items via submit_rework_items
            eprintln!("[lifecycle] '{}' step 4 approved (rework) — advancing to step 5 (awaiting_rework)", project_id);
            let next_state = LifecycleStateRow {
                project_id: project_id.clone(),
                step: 5,
                substep: None,
                status: "awaiting_rework".to_string(),
                pending_approval_id: None,
                active_agent_id: None,
                stage_task_ids: current.stage_task_ids.clone(),
                completed_task_ids: current.completed_task_ids.clone(),
                active_task_agent_ids: None,
                stage_number: current.stage_number,
                rework_items: None,
            };
            state.with_active(|store, _| store.upsert_lifecycle_state(&next_state))?;
            // No agent spawned yet — human must call submit_rework_items first
            return Ok(LifecycleStatus::from(next_state));
        }
    }

    // Step 5 approval: "skip" → step 6
    if step == 5 && decision == "skip" {
        return handle_step5_skip(&project_id, current, &state, &agent_state, &app);
    }

    // Step 6: validity substep approved → spawn rca-analyst
    if step == 6 && current.substep.as_deref() == Some("validity") {
        eprintln!(
            "[lifecycle] '{}' approved step 6.validity — spawning rca-analyst",
            project_id
        );
        let stage_num = current.stage_number.unwrap_or(1);
        let rca_prompt = format!(
            "Perform root cause analysis for Stage {}. Analyse execution logs, failures, and interventions. Produce phase-{}-rca.md. Then update project-local skill files for any agents that showed systemic issues.",
            stage_num, stage_num
        );
        let rca_state = LifecycleStateRow {
            project_id: project_id.clone(),
            step: 6,
            substep: Some("rca".to_string()),
            status: "running".to_string(),
            pending_approval_id: None,
            active_agent_id: None,
            stage_task_ids: current.stage_task_ids.clone(),
            completed_task_ids: current.completed_task_ids.clone(),
            active_task_agent_ids: None,
            stage_number: current.stage_number,
            rework_items: None,
        };
        state.with_active(|store, _| store.upsert_lifecycle_state(&rca_state))?;
        let project_dir = state.active_dir_str();
        match spawn_step_agent_with_prompt(
            &project_id,
            "rca-analyst",
            &rca_prompt,
            project_dir.as_deref(),
            &state,
            &agent_state,
            &app,
        ) {
            Ok(rca_agent_id) => {
                eprintln!(
                    "[lifecycle] Spawned rca-analyst agent={} for project='{}' stage={}",
                    rca_agent_id, project_id, stage_num
                );
                let updated = LifecycleStateRow {
                    active_agent_id: Some(rca_agent_id),
                    ..rca_state.clone()
                };
                state.with_active(|store, _| store.upsert_lifecycle_state(&updated))?;
                return Ok(LifecycleStatus::from(updated));
            }
            Err(e) => {
                eprintln!("[lifecycle] Failed to spawn rca-analyst for '{}': {}", project_id, e);
            }
        }
        return Ok(LifecycleStatus::from(rca_state));
    }

    // Step 6: rca substep approved → increment stage, loop back to Step 3
    if step == 6 && current.substep.as_deref() == Some("rca") {
        eprintln!(
            "[lifecycle] '{}' approved step 6.rca — incrementing stage and looping to step 3",
            project_id
        );
        let next_stage = current.stage_number.unwrap_or(1) + 1;
        let loop_state = LifecycleStateRow {
            project_id: project_id.clone(),
            step: 3,
            substep: None,
            status: "running".to_string(),
            pending_approval_id: None,
            active_agent_id: None,
            stage_task_ids: None,
            completed_task_ids: None,
            active_task_agent_ids: None,
            stage_number: Some(next_stage),
            rework_items: None,
        };
        state.with_active(|store, _| store.upsert_lifecycle_state(&loop_state))?;
        // Spawn product-manager agent for next stage plan
        let pm_prompt = format!(
            "Plan Stage {} of the project. Review prior stages and artefacts, then decompose the next stage into a set of tasks for execution.",
            next_stage
        );
        let project_dir = state.active_dir_str();
        match spawn_step_agent_with_prompt(
            &project_id,
            "product-manager",
            &pm_prompt,
            project_dir.as_deref(),
            &state,
            &agent_state,
            &app,
        ) {
            Ok(pm_agent_id) => {
                eprintln!(
                    "[lifecycle] Spawned product-manager agent={} for project='{}' stage={}",
                    pm_agent_id, project_id, next_stage
                );
                let updated = LifecycleStateRow {
                    active_agent_id: Some(pm_agent_id),
                    ..loop_state.clone()
                };
                state.with_active(|store, _| store.upsert_lifecycle_state(&updated))?;
                return Ok(LifecycleStatus::from(updated));
            }
            Err(e) => {
                eprintln!("[lifecycle] Failed to spawn product-manager for stage {} of '{}': {}", next_stage, project_id, e);
            }
        }
        return Ok(LifecycleStatus::from(loop_state));
    }

    let next_step = step + 1;
    let new_state = if next_step > 6 {
        // All steps complete
        eprintln!("[lifecycle] '{}' lifecycle complete!", project_id);
        LifecycleStateRow {
            project_id: project_id.clone(),
            step,
            substep: None,
            status: "complete".to_string(),
            pending_approval_id: None,
            active_agent_id: None,
            stage_task_ids: current.stage_task_ids.clone(),
            completed_task_ids: current.completed_task_ids.clone(),
            active_task_agent_ids: None,
            stage_number: current.stage_number,
            rework_items: None,
        }
    } else {
        eprintln!(
            "[lifecycle] '{}' approved step {} — advancing to step {} ({})",
            project_id,
            step,
            next_step,
            step_name(next_step)
        );
        LifecycleStateRow {
            project_id: project_id.clone(),
            step: next_step,
            substep: None,
            status: "running".to_string(),
            pending_approval_id: None,
            active_agent_id: None,
            stage_task_ids: None,
            completed_task_ids: None,
            active_task_agent_ids: None,
            stage_number: Some(1),
            rework_items: None,
        }
    };

    state.with_active(|store, _| store.upsert_lifecycle_state(&new_state))?;

    // Spawn the next step agent when advancing and not complete
    let final_state = if new_state.status == "running" {
        let next_step = new_state.step;
        // For step 2, start with substep "1" (architecture-analyst).
        let substep = if next_step == 2 { Some("1") } else { None };

        let project_dir = state.active_dir_str();
        match spawn_step_agent(
            &project_id,
            next_step,
            substep,
            project_dir.as_deref(),
            &state,
            &agent_state,
            &app,
        ) {
            Ok(agent_id) => {
                eprintln!(
                    "[lifecycle] Spawned step {}{} agent={} for project='{}'",
                    next_step,
                    substep.map(|s| format!(".{}", s)).unwrap_or_default(),
                    agent_id,
                    project_id
                );
                let updated = LifecycleStateRow {
                    active_agent_id: Some(agent_id),
                    ..new_state.clone()
                };
                state.with_active(|store, _| store.upsert_lifecycle_state(&updated))?;
                updated
            }
            Err(e) => {
                eprintln!(
                    "[lifecycle] Failed to spawn step {} agent for '{}': {}",
                    next_step, project_id, e
                );
                new_state
            }
        }
    } else {
        new_state
    };

    Ok(LifecycleStatus::from(final_state))
}

/// Submit rework items from the human after a Step 4 PM review decision of "rework".
///
/// Transitions the lifecycle from step=5, status="awaiting_rework" to
/// step=5, substep="rework-planning", status="running" and spawns a
/// product-manager agent to create a minimal rework task plan.
///
/// If `rework_items` is empty, skips directly to step 6 (same as approve with "skip").
#[tauri::command]
pub fn submit_rework_items(
    project_id: String,
    rework_items: String,
    project_state: State<'_, ProjectState>,
    agent_state: State<'_, AgentState>,
    app_handle: AppHandle,
) -> Result<LifecycleStatus, String> {
    let current = project_state.with_active(|store, _| store.get_lifecycle_state(&project_id))?;

    if current.step != 5 {
        return Err(format!(
            "submit_rework_items: project '{}' is at step {}, expected step 5",
            project_id, current.step
        ));
    }
    if current.status != "awaiting_rework" && current.status != "running" {
        return Err(format!(
            "submit_rework_items: project '{}' is in status '{}', expected 'awaiting_rework'",
            project_id, current.status
        ));
    }

    // Empty rework_items → skip to step 6
    if rework_items.trim().is_empty() {
        eprintln!(
            "[lifecycle] submit_rework_items: empty items for '{}' — skipping to step 6",
            project_id
        );
        return handle_step5_skip(&project_id, current, &project_state, &agent_state, &app_handle);
    }

    // Store rework_items and transition to substep="rework-planning"
    let planning_state = LifecycleStateRow {
        step: 5,
        substep: Some("rework-planning".to_string()),
        status: "running".to_string(),
        pending_approval_id: None,
        active_agent_id: None,
        rework_items: Some(rework_items.clone()),
        ..current.clone()
    };
    project_state.with_active(|store, _| store.upsert_lifecycle_state(&planning_state))?;

    // Build PM rework-planning prompt
    let pm_prompt = format!(
        "Review these rework items and create a minimal task plan to address them.\n\n## Rework Items\n\n{}\n\nCreate only the tasks necessary to address the rework items. Do not re-do work already completed satisfactorily.",
        rework_items
    );

    let project_dir = project_state.active_dir_str();
    match spawn_step_agent_with_prompt(
        &project_id,
        "product-manager",
        &pm_prompt,
        project_dir.as_deref(),
        &project_state,
        &agent_state,
        &app_handle,
    ) {
        Ok(pm_agent_id) => {
            eprintln!(
                "[lifecycle] submit_rework_items: spawned PM rework-planning agent={} for project='{}'",
                pm_agent_id, project_id
            );
            let updated = LifecycleStateRow {
                active_agent_id: Some(pm_agent_id),
                ..planning_state
            };
            project_state.with_active(|store, _| store.upsert_lifecycle_state(&updated))?;
            Ok(LifecycleStatus::from(updated))
        }
        Err(e) => {
            eprintln!(
                "[lifecycle] submit_rework_items: failed to spawn PM agent for '{}': {}",
                project_id, e
            );
            Ok(LifecycleStatus::from(planning_state))
        }
    }
}

/// Approve step 5 with "skip" decision — skips rework and jumps to step 6.
/// Can be called when step=5, any status.
fn handle_step5_skip(
    project_id: &str,
    current: LifecycleStateRow,
    state: &ProjectState,
    agent_state: &crate::agents::AgentState,
    app: &AppHandle,
) -> Result<LifecycleStatus, String> {
    eprintln!(
        "[lifecycle] '{}' step 5 skip — advancing to step 6",
        project_id
    );
    let stage_num = current.stage_number.unwrap_or(1);
    let next_state = LifecycleStateRow {
        project_id: project_id.to_string(),
        step: 6,
        substep: Some("validity".to_string()),
        status: "running".to_string(),
        pending_approval_id: None,
        active_agent_id: None,
        stage_task_ids: current.stage_task_ids.clone(),
        completed_task_ids: None,
        active_task_agent_ids: None,
        stage_number: current.stage_number,
        rework_items: None,
    };
    state.with_active(|store, _| store.upsert_lifecycle_state(&next_state))?;

    // bp6-ims.11: spawn validity-analyst
    let validity_prompt = format!(
        "Perform the validity check for Stage {}. Review the CONOPS and all guardrail documents against what was built this stage. Produce phase-{}-validity.md.",
        stage_num, stage_num
    );
    let project_dir = state.active_dir_str();
    match spawn_step_agent_with_prompt(
        project_id,
        "validity-analyst",
        &validity_prompt,
        project_dir.as_deref(),
        state,
        agent_state,
        app,
    ) {
        Ok(validity_agent_id) => {
            eprintln!(
                "[lifecycle] (step5-skip) Spawned validity-analyst agent={} for project='{}' stage={}",
                validity_agent_id, project_id, stage_num
            );
            let updated = LifecycleStateRow {
                active_agent_id: Some(validity_agent_id),
                ..next_state.clone()
            };
            state.with_active(|store, _| store.upsert_lifecycle_state(&updated))?;
            return Ok(LifecycleStatus::from(updated));
        }
        Err(e) => {
            eprintln!("[lifecycle] Failed to spawn validity-analyst for '{}': {}", project_id, e);
        }
    }
    Ok(LifecycleStatus::from(next_state))
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Step 4 execution fan-out (bp6-ims.8).
///
/// Parses `task_ids_json` into a list of task node IDs, resolves which ones
/// are immediately ready (no unmet dependencies), and spawns a PTY task agent
/// for each. Updates `active_task_agent_ids` in lifecycle state.
pub async fn spawn_execution_workflow(
    project_id: &str,
    task_ids_json: Option<&str>,
    project_state: &ProjectState,
    agent_state: &AgentState,
    app_handle: &AppHandle,
) -> Result<(), String> {
    // 1. Parse task_ids from JSON
    let all_task_ids: Vec<String> = match task_ids_json {
        Some(json) => serde_json::from_str(json)
            .map_err(|e| format!("Failed to parse task_ids_json: {}", e))?,
        None => {
            eprintln!("[lifecycle] ExecutionWorkflow: no task IDs — nothing to execute for '{}'", project_id);
            return Ok(());
        }
    };

    eprintln!(
        "[lifecycle] ExecutionWorkflow: {} tasks queued for project='{}'",
        all_task_ids.len(), project_id
    );

    if all_task_ids.is_empty() {
        return Ok(());
    }

    // 2. Resolve which tasks are ready (no unmet dependencies)
    let ready_tasks: Vec<crate::dag::DagNode> = project_state.with_active(|store, _| {
        store.get_ready_tasks(&all_task_ids, &[])
    })?;

    eprintln!(
        "[lifecycle] ExecutionWorkflow: {} tasks immediately ready for '{}'",
        ready_tasks.len(), project_id
    );

    // 3. Spawn a PTY agent for each ready task; build active_task_agent_ids map
    let mut active_task_map: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();

    let project_dir = project_state.active_dir_str();

    // Get artefact context once (from all prior steps < 4)
    let artefacts = project_state.with_active(|store, pid| {
        store.get_artefacts_for_step(pid, 4)
    })?;
    let artefact_context = DagStore::build_artefact_context(&artefacts);

    for task in &ready_tasks {
        let task_title = task.data["title"].as_str().unwrap_or("Untitled Task");
        let task_description = task.data["description"].as_str().unwrap_or("");
        let skill_id = task.data["skill_id"].as_str().unwrap_or("poe-implementation");

        // Build skill prompt
        let project_dir_buf = project_dir.as_deref().map(PathBuf::from);
        let skill_prompt = build_skills_prompt(
            &[skill_id.to_string()],
            app_handle,
            project_dir_buf.as_deref(),
        );

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
                active_task_map.insert(spawned.agent_id, task.id.clone());
            }
            Err(e) => {
                eprintln!(
                    "[lifecycle] Failed to spawn task agent for task='{}' project='{}': {}",
                    task.id, project_id, e
                );
            }
        }
    }

    // 4. Persist the active_task_agent_ids map into lifecycle state
    let active_task_json = serde_json::to_string(&active_task_map)
        .map_err(|e| format!("Failed to serialize active_task_agent_ids: {}", e))?;

    project_state.with_active(|store, _| {
        let current = store.get_lifecycle_state(project_id)?;
        let updated = crate::dag::LifecycleStateRow {
            active_task_agent_ids: Some(active_task_json.clone()),
            status: "running".to_string(),
            ..current
        };
        store.upsert_lifecycle_state(&updated)
    })?;

    eprintln!(
        "[lifecycle] ExecutionWorkflow: spawned {} task agents for '{}'",
        active_task_map.len(), project_id
    );

    Ok(())
}

/// Spawn the appropriate PTY agent for a lifecycle step.
///
/// Looks up the skill from STEP_SPECIALIST_MAP, builds an artefact context
/// from any knowledge artefacts already produced for this project, prepends
/// the skill prompt, and calls spawn_agent_internal.
///
/// Returns the spawned agent_id on success.
fn spawn_step_agent(
    project_id: &str,
    step: u32,
    substep: Option<&str>,
    project_dir: Option<&str>,
    project_state: &ProjectState,
    agent_state: &AgentState,
    app: &AppHandle,
) -> Result<String, String> {
    // 1. Look up skill_id from STEP_SPECIALIST_MAP
    let step_key = match substep {
        Some(sub) => format!("{}.{}", step, sub),
        None => step.to_string(),
    };
    let skill_id = STEP_SPECIALIST_MAP
        .iter()
        .find(|(k, _)| *k == step_key.as_str())
        .map(|(_, v)| *v)
        .ok_or_else(|| format!("No skill mapped for step {}", step_key))?;

    // 2. Get artefacts produced so far (steps < current step)
    let artefacts = project_state.with_active(|store, pid| {
        store.get_artefacts_for_step(pid, step)
    })?;

    // 3. Build artefact context string
    let artefact_context = DagStore::build_artefact_context(&artefacts);

    // 4. Build skill prompt
    let project_dir_buf = project_dir.map(PathBuf::from);
    let skill_prompt = build_skills_prompt(
        &[skill_id.to_string()],
        app,
        project_dir_buf.as_deref(),
    );

    // 5. Compose agent prompt
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

    // 6. Spawn the agent
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

/// Like `spawn_step_agent` but allows specifying an explicit `artefact_step`
/// for the `get_artefacts_for_step` query. This is needed for step 2 substep
/// "review" where the EM needs artefacts from steps 1 AND 2 (so artefact_step=3).
fn spawn_step_agent_with_artefact_step(
    project_id: &str,
    step: u32,
    substep: Option<&str>,
    artefact_step: u32,
    project_dir: Option<&str>,
    project_state: &ProjectState,
    agent_state: &AgentState,
    app: &AppHandle,
) -> Result<String, String> {
    // 1. Look up skill_id from STEP_SPECIALIST_MAP
    let step_key = match substep {
        Some(sub) => format!("{}.{}", step, sub),
        None => step.to_string(),
    };
    let skill_id = STEP_SPECIALIST_MAP
        .iter()
        .find(|(k, _)| *k == step_key.as_str())
        .map(|(_, v)| *v)
        .ok_or_else(|| format!("No skill mapped for step {}", step_key))?;

    // 2. Get artefacts produced so far (steps < artefact_step)
    let artefacts = project_state.with_active(|store, pid| {
        store.get_artefacts_for_step(pid, artefact_step)
    })?;

    // 3. Build artefact context string
    let artefact_context = DagStore::build_artefact_context(&artefacts);

    // 4. Build skill prompt
    let project_dir_buf = project_dir.map(PathBuf::from);
    let skill_prompt = build_skills_prompt(
        &[skill_id.to_string()],
        app,
        project_dir_buf.as_deref(),
    );

    // 5. Compose agent prompt
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

    // 6. Spawn the agent
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
///
/// Used for dynamically-constructed prompts such as the PM review in Step 4.
/// Returns the spawned agent_id on success.
pub fn spawn_step_agent_with_prompt(
    project_id: &str,
    skill_id: &str,
    prompt_body: &str,
    project_dir: Option<&str>,
    project_state: &ProjectState,
    agent_state: &AgentState,
    app: &AppHandle,
) -> Result<String, String> {
    // Build the skill preamble
    let project_dir_buf = project_dir.map(PathBuf::from);
    let skill_prompt = build_skills_prompt(
        &[skill_id.to_string()],
        app,
        project_dir_buf.as_deref(),
    );

    // Get artefact context from all steps so far (step 99 = include everything)
    let artefacts = project_state.with_active(|store, pid| {
        store.get_artefacts_for_step(pid, 99)
    })?;
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

// ── Restate service (kept for SDK integration / future use) ───────────────────
//
// This thin Restate service exposes a `get_status` handler so that Restate-aware
// tooling can introspect lifecycle state. The real state lives in SQLite; this
// just bridges the query.

#[restate_sdk::service]
trait LifecycleService {
    /// Returns a JSON status stub. Real state is in SQLite (see Tauri commands above).
    async fn get_status(node_id: String) -> Result<String, HandlerError>;
}

struct LifecycleServiceImpl;

impl LifecycleService for LifecycleServiceImpl {
    async fn get_status(
        &self,
        _ctx: Context<'_>,
        _node_id: String,
    ) -> Result<String, HandlerError> {
        Ok(r#"{"step": 0, "status": "idle", "note": "query via Tauri get_lifecycle_status for live data"}"#.to_string())
    }
}

// ── Spawn ─────────────────────────────────────────────────────────────────────

/// Spawn the Restate lifecycle service HTTP endpoint on port 9083.
/// Called from Tauri setup; runs in its own Tokio task.
pub fn spawn_lifecycle_service() {
    std::thread::spawn(|| {
        let rt = tokio::runtime::Runtime::new()
            .expect("Failed to create Tokio runtime for lifecycle_service");
        rt.block_on(async {
            let addr = format!("0.0.0.0:{}", LIFECYCLE_SERVICE_PORT)
                .parse()
                .expect("Invalid lifecycle service address");

            HttpServer::new(
                Endpoint::builder()
                    .bind(LifecycleServiceImpl.serve())
                    .build(),
            )
            .listen_and_serve(addr)
            .await;
        });
    });

    eprintln!(
        "[lifecycle] Service endpoint listening on port {}",
        LIFECYCLE_SERVICE_PORT
    );
}
