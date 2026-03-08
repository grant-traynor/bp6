pub mod commands;

use crate::agent_lifecycle::{self, SpawnRequest};
use crate::dag_store::{self, Node, NodeStatus, NodeType, ProjectRegistry};
use crate::event_ingester::DagChanged;
use crate::skills;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;

// ── Reviewer watchdog configuration ───────────────────────────────────────────

/// Default timeout in seconds before the watchdog fires for a reviewer task.
const REVIEWER_TIMEOUT_SECS: u64 = 300; // 5 minutes

/// Maximum number of retries before the reviewer is marked cancelled.
const REVIEWER_MAX_RETRY: u32 = 2;

// ── Concurrency limits ────────────────────────────────────────────────────────

pub struct ConcurrencyLimits {
    pub per_project: Mutex<HashMap<String, usize>>,
    pub global_running: AtomicUsize,
    pub global_limit: AtomicUsize,
    pub default_per_project: usize,
}

impl ConcurrencyLimits {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            per_project: Mutex::new(HashMap::new()),
            global_running: AtomicUsize::new(0),
            global_limit: AtomicUsize::new(15),
            default_per_project: 5,
        })
    }

    pub fn get_project_limit(&self, project_id: &str) -> usize {
        self.per_project
            .lock()
            .unwrap()
            .get(project_id)
            .copied()
            .unwrap_or(self.default_per_project)
    }

    pub fn set_project_limit(&self, project_id: &str, limit: usize) {
        self.per_project
            .lock()
            .unwrap()
            .insert(project_id.to_owned(), limit);
    }
}

// ── Orchestrator entry point ──────────────────────────────────────────────────

/// Start the orchestrator loop. Receives a channel receiver created in app setup.
pub async fn start(app: AppHandle, mut dag_rx: mpsc::UnboundedReceiver<DagChanged>) {
    eprintln!("[orchestrator] start: loop running");
    loop {
        let signal = match dag_rx.recv().await {
            Some(s) => s,
            None => break,
        };
        eprintln!("[orchestrator] start: received signal {:?}", signal);

        // Drain all pending signals — categorise into buckets
        let mut resume_requests: Vec<(String, String, String, String)> = Vec::new(); // (project_id, task_id, session_id, resolution)
        let mut opened_projects: Vec<String> = Vec::new(); // projects needing ghost-agent recovery
        let mut node_status_changed: Vec<(String, String)> = Vec::new(); // (project_id, node_id) for NodeStatusChanged
        let mut project_ids = std::collections::HashSet::new();

        // Process a single signal inline (closure not used due to borrow rules)
        categorise_signal(signal, &mut resume_requests, &mut opened_projects, &mut node_status_changed, &mut project_ids);

        while let Ok(s) = dag_rx.try_recv() {
            categorise_signal(s, &mut resume_requests, &mut opened_projects, &mut node_status_changed, &mut project_ids);
        }

        let registry = app.state::<ProjectRegistry>().inner().clone();
        let limits = app.state::<Arc<ConcurrencyLimits>>().inner().clone();
        let dag_tx = app.state::<mpsc::UnboundedSender<DagChanged>>().inner().clone();

        // Recover ghost agents from previously-interrupted sessions before scheduling.
        for project_id in &opened_projects {
            eprintln!("[orchestrator] project opened — running ghost-agent recovery for {}", project_id);
            recover_interrupted(project_id, &registry, &dag_tx, &app).await;
        }

        // Handle resume continuations for resolved decisions
        for (project_id, task_id, session_id, resolution) in resume_requests {
            resume_waiting_agent(&project_id, &task_id, &session_id, &resolution, &registry, &dag_tx, &app).await;
        }

        // SF-4: handle NodeStatusChanged signals — yield-handling and completion checks
        for (project_id, node_id) in node_status_changed {
            handle_node_status_changed(&project_id, &node_id, &registry, &dag_tx, &app).await;
        }

        // Run normal scheduling loop for DAG changes
        for project_id in project_ids {
            run_loop(&project_id, &registry, &limits, &dag_tx, &app).await;
        }
    }
}

/// Route a single DagChanged signal into the appropriate processing bucket.
fn categorise_signal(
    signal: DagChanged,
    resume_requests: &mut Vec<(String, String, String, String)>,
    opened_projects: &mut Vec<String>,
    node_status_changed: &mut Vec<(String, String)>,
    project_ids: &mut std::collections::HashSet<String>,
) {
    match signal {
        DagChanged::QueueItemResolved { project_id, task_id, session_id, resolution, .. } => {
            resume_requests.push((project_id, task_id, session_id, resolution));
        }
        DagChanged::ProjectOpened { project_id } => {
            opened_projects.push(project_id.clone());
            project_ids.insert(project_id);
        }
        DagChanged::NodeStatusChanged { project_id, node_id } => {
            project_ids.insert(project_id.clone());
            node_status_changed.push((project_id, node_id));
        }
        DagChanged::DagStructureChanged { project_id } => {
            project_ids.insert(project_id);
        }
    }
}

/// Resume an agent that was waiting for a human decision.
/// Spawns a new stream-json session with --resume and the human's resolution as the bundle.
async fn resume_waiting_agent(
    project_id: &str,
    task_id: &str,
    session_id: &str,
    resolution: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) {
    let (project_path, skill_id, model) = {
        let reg = registry.lock().unwrap();
        let db = match reg.values().find(|db| db.project.id == project_id) {
            Some(db) => db.clone(),
            None => {
                eprintln!("[orchestrator] resume_waiting_agent: project not open {}", project_id);
                return;
            }
        };
        drop(reg);
        let conn = db.conn.lock().unwrap();
        let node = match dag_store::db_get_node(&conn, task_id) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("[orchestrator] resume_waiting_agent: task not found {}: {}", task_id, e);
                return;
            }
        };
        let skill_id = node.skill_id.unwrap_or_else(|| "implementer".to_owned());
        let path = std::path::PathBuf::from(&db.project.path);
        let resource_dir = app.path().resource_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let model = skills::load_skill(&skill_id, &path, &resource_dir)
            .ok()
            .and_then(|s| s.model.clone());
        (path, skill_id, model)
    };

    // Resolution bundle format per Protocol.md §5
    let input_bundle = format!("---\nHuman: {}\n", resolution);

    let spawn_req = agent_lifecycle::SpawnRequest {
        project_id: project_id.to_owned(),
        task_id: task_id.to_owned(),
        skill_id,
        project_path,
        input_bundle,
        resume_session_id: Some(session_id.to_owned()),
        model,
    };

    eprintln!("[orchestrator] resuming waiting agent task={} session={}", task_id, session_id);
    if let Err(e) = agent_lifecycle::spawn_agent(spawn_req, registry, dag_tx, app).await {
        eprintln!("[orchestrator] Failed to resume agent for task {}: {}", task_id, e);
    }
}

// ── SF-4: Yield handling ──────────────────────────────────────────────────────

/// Called whenever a NodeStatusChanged signal is received.
///
/// This is the SF-4 entry point:
/// - If the node is waiting with yield_reason='review': spawn reviewer tasks and dispatch them.
/// - If the node is a reviewer (has requesting_task_id) and is now done/cancelled:
///   run the completion check and resume the requesting task if all reviewers are done.
/// - All other statuses: no special action (run_loop handles normal scheduling).
async fn handle_node_status_changed(
    project_id: &str,
    node_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) {
    let node = {
        let reg = registry.lock().unwrap();
        let db = match reg.values().find(|db| db.project.id == project_id) {
            Some(db) => db.clone(),
            None => return,
        };
        drop(reg);
        let conn = db.conn.lock().unwrap();
        match dag_store::db_get_node(&conn, node_id) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("[orchestrator] handle_node_status_changed: node not found {}: {}", node_id, e);
                return;
            }
        }
    };

    eprintln!(
        "[orchestrator] handle_node_status_changed: node={} status={} yield_reason={:?} requesting_task_id={:?}",
        node.id, node.status, node.yield_reason, node.requesting_task_id
    );

    match &node.status {
        NodeStatus::Waiting => {
            // SF-4 yield branch
            match node.yield_reason.as_deref() {
                Some("review") => {
                    handle_review_yield(&node, project_id, registry, dag_tx, app).await;
                }
                Some("decision") => {
                    // No action — await human resolution via resolve_decision command
                    // (already handled by QueueItemResolved path)
                    eprintln!(
                        "[orchestrator] handle_node_status_changed: node={} waiting for decision — no action",
                        node.id
                    );
                }
                other => {
                    eprintln!(
                        "[orchestrator] handle_node_status_changed: node={} unknown yield_reason={:?}",
                        node.id, other
                    );
                }
            }
        }
        NodeStatus::Complete | NodeStatus::Cancelled => {
            // Check if this is a reviewer node — if so, run completion check on the requesting task
            if let Some(ref requesting_task_id) = node.requesting_task_id {
                eprintln!(
                    "[orchestrator] handle_node_status_changed: reviewer node={} finished, checking completion for requesting_task={}",
                    node.id, requesting_task_id
                );
                check_review_completion(requesting_task_id, project_id, registry, dag_tx, app).await;
            }
        }
        _ => {
            // Running, Pending, Blocked — no SF-4 action needed
        }
    }
}

/// SF-4 review yield: spawn a reviewer node for each poe:review event logged for this task.
async fn handle_review_yield(
    waiting_task: &Node,
    project_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) {
    eprintln!(
        "[orchestrator] handle_review_yield: task={} — querying poe:review events",
        waiting_task.id
    );

    // Query poe:review events for this task to get (review_id, reviewer_skill) pairs
    let review_events: Vec<(String, String)> = {
        let reg = registry.lock().unwrap();
        let db = match reg.values().find(|db| db.project.id == project_id) {
            Some(db) => db.clone(),
            None => return,
        };
        drop(reg);
        let conn = db.conn.lock().unwrap();
        match dag_store::db_list_review_events_for_task(&conn, &waiting_task.id) {
            Ok(events) => events,
            Err(e) => {
                eprintln!("[orchestrator] handle_review_yield: failed to query review events: {}", e);
                return;
            }
        }
    };

    if review_events.is_empty() {
        eprintln!(
            "[orchestrator] handle_review_yield: task={} has no poe:review events — nothing to dispatch",
            waiting_task.id
        );
        return;
    }

    eprintln!(
        "[orchestrator] handle_review_yield: task={} dispatching {} reviewer(s)",
        waiting_task.id,
        review_events.len()
    );

    // For each poe:review event: create a reviewer node and dispatch it via SF-1
    let mut reviewer_nodes: Vec<Node> = Vec::new();
    for (review_id, reviewer_skill) in &review_events {
        let reviewer_node = {
            let reg = registry.lock().unwrap();
            let db = match reg.values().find(|db| db.project.id == project_id) {
                Some(db) => db.clone(),
                None => break,
            };
            drop(reg);
            let conn = db.conn.lock().unwrap();

            // Read the poe:review event payload to get the content for the reviewer bundle
            let content = {
                let mut stmt = match conn.prepare(
                    "SELECT payload FROM events WHERE task_id = ?1 AND event_type = 'poe:review' AND json_extract(payload, '$.id') = ?2 ORDER BY created_at LIMIT 1"
                ) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("[orchestrator] handle_review_yield: prepare failed: {}", e);
                        continue;
                    }
                };
                let payload_str: Option<String> = stmt
                    .query_row(rusqlite::params![&waiting_task.id, review_id], |row| row.get(0))
                    .ok();
                payload_str
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                    .and_then(|v| v.get("content").and_then(|c| c.as_str()).map(str::to_owned))
                    .unwrap_or_default()
            };

            let input = dag_store::CreateNodeInput {
                project_id: project_id.to_owned(),
                phase_id: waiting_task.phase_id.clone(),
                parent_id: None,
                node_type: NodeType::PlanReview,
                title: format!("Plan Review — {}", waiting_task.title),
                description: Some(content),
                skill_id: Some(reviewer_skill.clone()),
                initial_status: Some(NodeStatus::Pending),
                requesting_task_id: Some(waiting_task.id.clone()),
                review_id: Some(review_id.clone()),
                retry_count: Some(0),
            };

            match dag_store::db_create_node(&conn, &input) {
                Ok(n) => {
                    eprintln!(
                        "[orchestrator] handle_review_yield: created reviewer node={} review_id={} skill={}",
                        n.id, review_id, reviewer_skill
                    );
                    n
                }
                Err(e) => {
                    eprintln!(
                        "[orchestrator] handle_review_yield: failed to create reviewer node for review_id={}: {}",
                        review_id, e
                    );
                    continue;
                }
            }
        };

        // bp6-m2f.19: spawn per-reviewer watchdog timer
        spawn_reviewer_watchdog(
            app.clone(),
            project_id.to_owned(),
            reviewer_node.id.clone(),
            waiting_task.id.clone(),
            REVIEWER_TIMEOUT_SECS,
            REVIEWER_MAX_RETRY,
        );

        reviewer_nodes.push(reviewer_node);
    }

    // Dispatch each reviewer via SF-1 (same dispatch path as run_loop uses for ready tasks)
    for reviewer in reviewer_nodes {
        dispatch_reviewer_task(reviewer, project_id, waiting_task, registry, dag_tx, app).await;
    }
}

/// Dispatch a reviewer task. Similar to dispatch_task but builds a ReviewRequest bundle
/// per Protocol.md §3 "Reviewer stdin bundle".
async fn dispatch_reviewer_task(
    reviewer: Node,
    project_id: &str,
    requesting_task: &Node,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) {
    eprintln!(
        "[orchestrator] dispatch_reviewer_task: reviewer={} review_id={:?} skill={:?}",
        reviewer.id, reviewer.review_id, reviewer.skill_id
    );

    let bundle_data = {
        let reg = registry.lock().unwrap();
        let db = match reg.values().find(|db| db.project.id == project_id) {
            Some(db) => db.clone(),
            None => return,
        };
        drop(reg);

        let conn = db.conn.lock().unwrap();
        let project_path = PathBuf::from(&db.project.path);
        let skill_id = reviewer.skill_id.clone().unwrap_or_else(|| "implementer".to_owned());
        let resource_dir = app.path().resource_dir().unwrap_or_else(|_| PathBuf::from("."));
        let knowledge: Vec<(String, String)> = dag_store::db_list_knowledge(&conn, project_id)
            .unwrap_or_default()
            .into_iter()
            .map(|k| (k.key, k.value))
            .collect();
        let artifacts: Vec<(String, String)> = dag_store::db_list_artifacts(&conn, project_id)
            .unwrap_or_default()
            .into_iter()
            .map(|a| (a.artifact_type, a.filename))
            .collect();

        (project_path, skill_id, resource_dir, knowledge, artifacts)
    };

    let (project_path, skill_id, resource_dir, knowledge, artifacts) = bundle_data;

    let skill = match skills::load_skill(&skill_id, &project_path, &resource_dir) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[orchestrator] dispatch_reviewer_task: failed to load skill '{}': {}", skill_id, e);
            return;
        }
    };

    // Build ReviewRequest bundle per Protocol.md §3
    let review_id = reviewer.review_id.as_deref().unwrap_or("unknown");
    let review_content = reviewer.description.as_deref().unwrap_or("");

    let knowledge_section = if knowledge.is_empty() {
        String::new()
    } else {
        let entries: String = knowledge
            .iter()
            .map(|(k, v)| format!("## {}\n\n{}\n\n---\n", k, v))
            .collect();
        format!("# Knowledge Register\n\n{}", entries)
    };

    let artifacts_section = if artifacts.is_empty() {
        String::new()
    } else {
        let entries: String = artifacts
            .iter()
            .map(|(t, f)| format!("## {} ({})\n\n---\n", f, t))
            .collect();
        format!("# Artifacts\n\n{}", entries)
    };

    let input_bundle = format!(
        "# Task\n\n**ID**: {reviewer_id}\n**Title**: Plan Review — {requesting_title}\n**Type**: plan_review\n**Skill**: {skill_id}\n\n## Review Request\n\n**Requested by**: {requesting_id} ({requesting_title})\n**Review ID**: {review_id}\n\n{review_content}\n\n> **Naming convention**: The reviewer MUST emit its artifact as {{\"poe\":\"artifact\",\"name\":\"review-{review_id}.md\",...}} where `{review_id}` is the Review ID above.\n\n---\n\n# Skill\n\n{skill_prompt}\n\n---\n\n{artifacts_section}{knowledge_section}",
        reviewer_id = reviewer.id,
        requesting_title = requesting_task.title,
        skill_id = skill_id,
        requesting_id = requesting_task.id,
        review_id = review_id,
        review_content = review_content,
        skill_prompt = skill.prompt,
        artifacts_section = artifacts_section,
        knowledge_section = knowledge_section,
    );

    let spawn_req = SpawnRequest {
        project_id: project_id.to_owned(),
        task_id: reviewer.id.clone(),
        skill_id,
        project_path,
        input_bundle,
        resume_session_id: None,
        model: skill.model.clone(),
    };

    if let Err(e) = agent_lifecycle::spawn_agent(spawn_req, registry, dag_tx, app).await {
        eprintln!("[orchestrator] dispatch_reviewer_task: failed to spawn agent for reviewer {}: {}", reviewer.id, e);
    }
}

/// Completion check (SF-4): called when a reviewer node reaches done or cancelled.
/// If all expected reviewers have answered, resume the requesting task via SF-3.
async fn check_review_completion(
    requesting_task_id: &str,
    project_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) {
    let (requesting_task, expected_ids, answered_ids, project_path) = {
        let reg = registry.lock().unwrap();
        let db = match reg.values().find(|db| db.project.id == project_id) {
            Some(db) => db.clone(),
            None => return,
        };
        drop(reg);
        let conn = db.conn.lock().unwrap();
        let requesting_task = match dag_store::db_get_node(&conn, requesting_task_id) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("[orchestrator] check_review_completion: requesting task not found {}: {}", requesting_task_id, e);
                return;
            }
        };
        let (expected, answered) = match dag_store::db_reviewer_completion_status(&conn, requesting_task_id) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[orchestrator] check_review_completion: failed to query completion status: {}", e);
                return;
            }
        };
        let path = PathBuf::from(&db.project.path);
        (requesting_task, expected, answered, path)
    };

    // Sort for stable set comparison
    let mut expected_sorted = expected_ids.clone();
    expected_sorted.sort();
    let mut answered_sorted = answered_ids.clone();
    answered_sorted.sort();

    eprintln!(
        "[orchestrator] check_review_completion: requesting_task={} expected={:?} answered={:?}",
        requesting_task_id, expected_sorted, answered_sorted
    );

    if expected_sorted.is_empty() {
        eprintln!(
            "[orchestrator] check_review_completion: no reviewer nodes found for task={} — skipping",
            requesting_task_id
        );
        return;
    }

    if expected_sorted != answered_sorted {
        eprintln!(
            "[orchestrator] check_review_completion: not all reviewers done yet — waiting"
        );
        return;
    }

    eprintln!(
        "[orchestrator] check_review_completion: all {} reviewers done — building bundle and resuming task={}",
        expected_sorted.len(),
        requesting_task_id
    );

    // Build ReviewResult bundle per Protocol.md §5
    // Each reviewer's artifact is at {project.path}/docs/review-{review_id}.md
    let mut bundle = String::new();
    for review_id in &expected_sorted {
        // Find the reviewer node to get skill and status
        let (reviewer_skill, verdict) = {
            let reg = registry.lock().unwrap();
            let db = match reg.values().find(|db| db.project.id == project_id) {
                Some(db) => db.clone(),
                None => continue,
            };
            drop(reg);
            let conn = db.conn.lock().unwrap();
            let result = conn.query_row(
                "SELECT skill_id, status FROM nodes WHERE requesting_task_id = ?1 AND review_id = ?2 LIMIT 1",
                rusqlite::params![requesting_task_id, review_id],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, String>(1)?,
                    ))
                },
            );
            match result {
                Ok((skill, status)) => {
                    let verdict = if status == "cancelled" { "FAILED".to_owned() } else { "APPROVED".to_owned() };
                    (skill.unwrap_or_else(|| "unknown".to_owned()), verdict)
                }
                Err(e) => {
                    eprintln!("[orchestrator] check_review_completion: reviewer lookup failed: {}", e);
                    continue;
                }
            }
        };

        // Read artifact content from docs/review-{review_id}.md
        let artifact_path = project_path.join("docs").join(format!("review-{}.md", review_id));
        let artifact_content = std::fs::read_to_string(&artifact_path).unwrap_or_else(|e| {
            eprintln!(
                "[orchestrator] check_review_completion: failed to read artifact {:?}: {}",
                artifact_path, e
            );
            format!("[artifact not found: {}]", artifact_path.display())
        });

        bundle.push_str(&format!(
            "---\nReviewResult id={} skill={} verdict={}\n{}\n---\n",
            review_id, reviewer_skill, verdict, artifact_content
        ));
    }

    // Resume the requesting task via SF-3 (same as decision resolution path)
    let session_id = match &requesting_task.session_id {
        Some(sid) => sid.clone(),
        None => {
            eprintln!(
                "[orchestrator] check_review_completion: requesting task={} has no session_id — cannot resume",
                requesting_task_id
            );
            return;
        }
    };

    let (skill_id, model) = {
        let skill_id = requesting_task.skill_id.clone().unwrap_or_else(|| "implementer".to_owned());
        let resource_dir = app.path().resource_dir().unwrap_or_else(|_| PathBuf::from("."));
        let model = skills::load_skill(&skill_id, &project_path, &resource_dir)
            .ok()
            .and_then(|s| s.model.clone());
        (skill_id, model)
    };

    let spawn_req = SpawnRequest {
        project_id: project_id.to_owned(),
        task_id: requesting_task_id.to_owned(),
        skill_id,
        project_path,
        input_bundle: bundle,
        resume_session_id: Some(session_id.clone()),
        model,
    };

    eprintln!(
        "[orchestrator] check_review_completion: resuming task={} session={}",
        requesting_task_id, session_id
    );

    if let Err(e) = agent_lifecycle::spawn_agent(spawn_req, registry, dag_tx, app).await {
        eprintln!(
            "[orchestrator] check_review_completion: failed to resume task {}: {}",
            requesting_task_id, e
        );
    }
}

// ── Reviewer watchdog ─────────────────────────────────────────────────────────

/// Spawn a Tokio background task that wakes after `timeout_secs` and checks the
/// reviewer node's status.
///
/// - If the reviewer finished normally: no-op (normal completion beat the timer).
/// - If `retry_count < max_retry`: increment retry_count, reset node to pending
///   so the run_loop re-dispatches it (SF-1), and spawn a fresh watchdog.
/// - If `retry_count >= max_retry`: mark the node as cancelled, then call
///   `check_review_completion` — if all reviewers are now accounted for, SF-3
///   resumes the requesting task with a FAILED verdict for this reviewer.
fn spawn_reviewer_watchdog(
    app: AppHandle,
    project_id: String,
    reviewer_task_id: String,
    requesting_task_id: String,
    timeout_secs: u64,
    max_retry: u32,
) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(timeout_secs)).await;

        eprintln!(
            "[orchestrator] watchdog fired: reviewer={} requesting={} timeout={}s",
            reviewer_task_id, requesting_task_id, timeout_secs
        );

        let registry = app.state::<ProjectRegistry>().inner().clone();
        let dag_tx = app.state::<mpsc::UnboundedSender<DagChanged>>().inner().clone();

        // Read current node state under a short-lived lock
        let (current_status, current_retry) = {
            let reg = registry.lock().unwrap();
            let db = match reg.values().find(|db| db.project.id == project_id) {
                Some(db) => db.clone(),
                None => {
                    eprintln!("[orchestrator] watchdog: project not found {}", project_id);
                    return;
                }
            };
            drop(reg);
            let conn = db.conn.lock().unwrap();
            match dag_store::db_get_node(&conn, &reviewer_task_id) {
                Ok(n) => (n.status, n.retry_count),
                Err(e) => {
                    eprintln!(
                        "[orchestrator] watchdog: reviewer node not found {}: {}",
                        reviewer_task_id, e
                    );
                    return;
                }
            }
        };

        // If the reviewer already finished or was cancelled — normal completion won the race
        match current_status {
            NodeStatus::Complete | NodeStatus::Cancelled => {
                eprintln!(
                    "[orchestrator] watchdog: reviewer={} already terminal ({}) — no action",
                    reviewer_task_id, current_status
                );
                return;
            }
            _ => {}
        }

        if (current_retry as u32) < max_retry {
            // Retry path: increment retry_count, requeue to pending
            eprintln!(
                "[orchestrator] watchdog: reviewer={} retry {}/{} — requeueing to pending",
                reviewer_task_id, current_retry + 1, max_retry
            );

            let requeued = {
                let reg = registry.lock().unwrap();
                let db = match reg.values().find(|db| db.project.id == project_id) {
                    Some(db) => db.clone(),
                    None => return,
                };
                drop(reg);
                let conn = db.conn.lock().unwrap();
                let new_retry = match dag_store::db_increment_retry_count(&conn, &reviewer_task_id) {
                    Ok(n) => n,
                    Err(e) => {
                        eprintln!("[orchestrator] watchdog: increment retry failed: {}", e);
                        return;
                    }
                };
                let update = dag_store::UpdateNodeInput {
                    status: Some(NodeStatus::Pending),
                    title: None,
                    description: None,
                    skill_id: None,
                    assignee: None,
                    ..Default::default()
                };
                match dag_store::db_update_node(&conn, &reviewer_task_id, &update) {
                    Ok(n) => {
                        eprintln!(
                            "[orchestrator] watchdog: reviewer={} requeued, retry_count now {}",
                            reviewer_task_id, new_retry
                        );
                        n
                    }
                    Err(e) => {
                        eprintln!("[orchestrator] watchdog: requeue update failed: {}", e);
                        return;
                    }
                }
            };
            let _ = requeued; // status change will trigger run_loop via DagChanged

            // Signal the run_loop to re-dispatch the pending reviewer (SF-1)
            let _ = dag_tx.send(DagChanged::DagStructureChanged {
                project_id: project_id.clone(),
            });

            // Spawn a fresh watchdog for the retry
            spawn_reviewer_watchdog(
                app,
                project_id,
                reviewer_task_id,
                requesting_task_id,
                timeout_secs,
                max_retry,
            );
        } else {
            // Max retries exhausted — cancel the reviewer node
            eprintln!(
                "[orchestrator] watchdog: reviewer={} max retries ({}) exhausted — cancelling",
                reviewer_task_id, max_retry
            );

            {
                let reg = registry.lock().unwrap();
                let db = match reg.values().find(|db| db.project.id == project_id) {
                    Some(db) => db.clone(),
                    None => return,
                };
                drop(reg);
                let conn = db.conn.lock().unwrap();
                let update = dag_store::UpdateNodeInput {
                    status: Some(NodeStatus::Cancelled),
                    title: None,
                    description: None,
                    skill_id: None,
                    assignee: None,
                    ..Default::default()
                };
                if let Err(e) = dag_store::db_update_node(&conn, &reviewer_task_id, &update) {
                    eprintln!("[orchestrator] watchdog: cancel update failed: {}", e);
                    return;
                }
                eprintln!(
                    "[orchestrator] watchdog: reviewer={} marked cancelled",
                    reviewer_task_id
                );
            }

            // Run the completion check — if all reviewers are now accounted for,
            // this triggers SF-3 with a FAILED verdict for the cancelled node.
            check_review_completion(&requesting_task_id, &project_id, &registry, &dag_tx, &app).await;
        }
    });
}

// ── Core loop ─────────────────────────────────────────────────────────────────

async fn run_loop(
    project_id: &str,
    registry: &ProjectRegistry,
    limits: &Arc<ConcurrencyLimits>,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) {
    eprintln!("[orchestrator] run_loop wakeup project={}", project_id);

    let ready_tasks = {
        let reg = registry.lock().unwrap();
        let db = match reg.values().find(|db| db.project.id == project_id) {
            Some(db) => db.clone(),
            None => {
                eprintln!("[orchestrator] run_loop: project not open in registry, skipping");
                return;
            }
        };
        drop(reg);

        let conn = db.conn.lock().unwrap();

        let global_running = dag_store::db_count_running_agents_global(&conn).unwrap_or(0);
        let global_limit = limits.global_limit.load(Ordering::Relaxed);
        eprintln!(
            "[orchestrator] run_loop: global_running={} global_limit={}",
            global_running, global_limit
        );
        if global_running >= global_limit {
            eprintln!("[orchestrator] run_loop: global concurrency limit reached, skipping");
            return;
        }

        let project_running = dag_store::db_count_running_agents(&conn, project_id).unwrap_or(0);
        let project_limit = limits.get_project_limit(project_id);
        eprintln!(
            "[orchestrator] run_loop: project_running={} project_limit={}",
            project_running, project_limit
        );
        if project_running >= project_limit {
            eprintln!("[orchestrator] run_loop: project concurrency limit reached, skipping");
            return;
        }

        let slots = (project_limit - project_running).min(global_limit - global_running);

        match dag_store::db_find_ready_tasks(&conn, project_id) {
            Ok(tasks) => {
                eprintln!(
                    "[orchestrator] run_loop: found {} ready task(s), slots={}",
                    tasks.len(),
                    slots
                );
                for t in &tasks {
                    eprintln!(
                        "[orchestrator] run_loop: ready task id={} title={:?} skill={:?}",
                        t.id, t.title, t.skill_id
                    );
                }
                tasks.into_iter().take(slots).collect::<Vec<_>>()
            }
            Err(e) => {
                eprintln!("[orchestrator] Failed to find ready tasks: {}", e);
                return;
            }
        }
    };

    if ready_tasks.is_empty() {
        eprintln!("[orchestrator] run_loop: no ready tasks to dispatch");
    }

    for task in ready_tasks {
        dispatch_task(task, project_id, registry, dag_tx, app).await;
    }
}

async fn dispatch_task(
    task: Node,
    project_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) {
    eprintln!(
        "[orchestrator] dispatch_task: id={} title={:?} skill={:?}",
        task.id, task.title, task.skill_id
    );

    let bundle_data = {
        let reg = registry.lock().unwrap();
        let db = match reg.values().find(|db| db.project.id == project_id) {
            Some(db) => db.clone(),
            None => return,
        };
        drop(reg);

        let conn = db.conn.lock().unwrap();

        let project_path = PathBuf::from(&db.project.path);
        let skill_id = task.skill_id.clone().unwrap_or_else(|| "implementer".to_owned());

        let resource_dir = app
            .path()
            .resource_dir()
            .unwrap_or_else(|_| PathBuf::from("."));

        let knowledge: Vec<(String, String)> = dag_store::db_list_knowledge(&conn, project_id)
            .unwrap_or_default()
            .into_iter()
            .map(|k| (k.key, k.value))
            .collect();

        let ancestry: Vec<(String, String)> = dag_store::db_get_ancestry(&conn, &task.id)
            .unwrap_or_default()
            .into_iter()
            .map(|n| (n.node_type.to_string(), n.title))
            .collect();

        let artifacts: Vec<(String, String)> = dag_store::db_list_artifacts(&conn, project_id)
            .unwrap_or_default()
            .into_iter()
            .map(|a| (a.artifact_type, a.filename))
            .collect();

        (project_path, skill_id, resource_dir, knowledge, ancestry, artifacts)
    };

    let (project_path, skill_id, resource_dir, knowledge, ancestry, artifacts) = bundle_data;

    let skill = match skills::load_skill(&skill_id, &project_path, &resource_dir) {
        Ok(s) => {
            eprintln!(
                "[orchestrator] dispatch_task: skill loaded skill_id={} source={} model={:?}",
                s.skill_id, s.source, s.model
            );
            s
        }
        Err(e) => {
            eprintln!("[orchestrator] Failed to load skill '{}': {}", skill_id, e);
            return;
        }
    };

    let ancestry_refs: Vec<(&str, &str)> = ancestry
        .iter()
        .map(|(t, n)| (t.as_str(), n.as_str()))
        .collect();
    let knowledge_refs: Vec<(&str, &str)> = knowledge
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let artifact_refs: Vec<(&str, &str)> = artifacts
        .iter()
        .map(|(t, f)| (t.as_str(), f.as_str()))
        .collect();

    let input_bundle = skills::assemble_input_bundle(
        &skills::SpawnMode::Autonomous,
        &skill,
        &task.title,
        task.description.as_deref(),
        &ancestry_refs,
        &knowledge_refs,
        &artifact_refs,
    );

    let spawn_req = SpawnRequest {
        project_id: project_id.to_owned(),
        task_id: task.id.clone(),
        skill_id,
        project_path,
        input_bundle,
        resume_session_id: None,
        model: skill.model.clone(),
    };

    if let Err(e) = agent_lifecycle::spawn_agent(spawn_req, registry, dag_tx, app).await {
        eprintln!("[orchestrator] Failed to spawn agent for task {}: {}", task.id, e);
    }
}

// ── Recovery on app start ─────────────────────────────────────────────────────

pub async fn recover_interrupted(
    project_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) {
    let (interrupted, waiting_nodes, project_path) = {
        let reg = registry.lock().unwrap();
        let db = match reg.values().find(|db| db.project.id == project_id) {
            Some(db) => db.clone(),
            None => return,
        };
        drop(reg);
        let conn = db.conn.lock().unwrap();
        let agents = dag_store::db_list_agents_by_status(&conn, project_id, "running")
            .unwrap_or_default();
        let waiting = dag_store::db_list_nodes_by_status(&conn, project_id, "waiting")
            .unwrap_or_default();
        let path = PathBuf::from(&db.project.path);
        (agents, waiting, path)
    };

    let resource_dir = app
        .path()
        .resource_dir()
        .unwrap_or_else(|_| PathBuf::from("."));

    for agent in interrupted {
        eprintln!(
            "[orchestrator] recover_interrupted: agent={} task={} session={:?}",
            agent.id, agent.task_id, agent.session_id
        );

        if let Some(ref session_id) = agent.session_id {
            // Session was established — try to resume where we left off.
            let skill = skills::load_skill(&agent.skill_id, &project_path, &resource_dir).ok();
            let model = skill.as_ref().and_then(|s| s.model.clone());
            let input_bundle = skill
                .map(|s| s.prompt)
                .unwrap_or_else(|| "Resume previous task.".to_owned());

            let spawn_req = SpawnRequest {
                project_id: project_id.to_owned(),
                task_id: agent.task_id.clone(),
                skill_id: agent.skill_id.clone(),
                project_path: project_path.clone(),
                input_bundle,
                resume_session_id: Some(session_id.clone()),
                model,
            };

            match agent_lifecycle::spawn_agent(spawn_req, registry, dag_tx, app).await {
                Ok(_) => {
                    eprintln!("[orchestrator] Resumed agent for task {}", agent.task_id);
                }
                Err(e) => {
                    eprintln!(
                        "[orchestrator] Resume failed for task {}: {} — requeueing",
                        agent.task_id, e
                    );
                    let reg = registry.lock().unwrap();
                    if let Some(db) = reg.values().find(|db| db.project.id == project_id) {
                        let conn = db.conn.lock().unwrap();
                        let update = dag_store::UpdateNodeInput {
                            status: Some(dag_store::NodeStatus::Pending),
                            title: None,
                            description: None,
                            skill_id: None,
                            assignee: None,
                            ..Default::default()
                        };
                        let _ = dag_store::db_update_node(&conn, &agent.task_id, &update);
                        let _ = dag_store::db_end_agent(&conn, &agent.id, "failed");
                    }
                }
            }
        } else {
            // Session ID was never recorded (crash before on_session_id fired).
            // Cannot resume — requeue to pending so the orchestrator can restart fresh.
            eprintln!(
                "[orchestrator] recover_interrupted: no session_id for agent={}, requeueing task={} to pending",
                agent.id, agent.task_id
            );
            let reg = registry.lock().unwrap();
            if let Some(db) = reg.values().find(|db| db.project.id == project_id) {
                let conn = db.conn.lock().unwrap();
                let update = dag_store::UpdateNodeInput {
                    status: Some(dag_store::NodeStatus::Pending),
                    title: None,
                    description: None,
                    skill_id: None,
                    assignee: None,
                    ..Default::default()
                };
                let _ = dag_store::db_update_node(&conn, &agent.task_id, &update);
                let _ = dag_store::db_end_agent(&conn, &agent.id, "failed");
            }
        }
    }

    // ── Recover waiting nodes ─────────────────────────────────────────────────
    // Handle nodes that were in status=waiting when the app was shut down.
    for node in waiting_nodes {
        eprintln!(
            "[orchestrator] recover_interrupted: waiting node={} yield_reason={:?}",
            node.id, node.yield_reason
        );

        match node.yield_reason.as_deref() {
            Some("review") => {
                recover_waiting_review(&node, project_id, registry, dag_tx, app).await;
            }
            Some("decision") => {
                // Decision record persists in queue_items — human resolves when ready.
                // No action needed; QueueItemResolved path handles it when resolved.
                eprintln!(
                    "[orchestrator] recover_interrupted: waiting node={} yield_reason=decision — no action (human must resolve)",
                    node.id
                );
            }
            other => {
                eprintln!(
                    "[orchestrator] recover_interrupted: waiting node={} unknown yield_reason={:?} — no action",
                    node.id, other
                );
            }
        }
    }
}

/// Recovery path for a waiting node with yield_reason='review'.
///
/// Checks whether all reviewers already answered before the restart. If so,
/// immediately triggers SF-3 (resume requesting task with ReviewResult bundle).
/// Otherwise, any reviewer still running will re-enter the interrupted-agent
/// recovery path above and trigger SF-3 on completion via the normal SF-2 path.
/// Any pending (not-yet-started) reviewer nodes are re-dispatched via NodeStatusChanged.
async fn recover_waiting_review(
    node: &dag_store::Node,
    project_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) {
    eprintln!(
        "[orchestrator] recover_waiting_review: checking reviewer completion for task={}",
        node.id
    );

    // Query expected vs. answered reviewer nodes.
    let (expected_ids, answered_ids) = {
        let reg = registry.lock().unwrap();
        let db = match reg.values().find(|db| db.project.id == project_id) {
            Some(db) => db.clone(),
            None => return,
        };
        drop(reg);
        let conn = db.conn.lock().unwrap();
        match dag_store::db_reviewer_completion_status(&conn, &node.id) {
            Ok(r) => r,
            Err(e) => {
                eprintln!(
                    "[orchestrator] recover_waiting_review: failed to query completion status for task={}: {}",
                    node.id, e
                );
                return;
            }
        }
    };

    let mut expected_sorted = expected_ids.clone();
    expected_sorted.sort();
    let mut answered_sorted = answered_ids.clone();
    answered_sorted.sort();

    eprintln!(
        "[orchestrator] recover_waiting_review: task={} expected={:?} answered={:?}",
        node.id, expected_sorted, answered_sorted
    );

    if expected_sorted.is_empty() {
        // No reviewer nodes exist yet — they may have been lost before creation.
        // Re-dispatch review yield so reviewer nodes are (re-)created.
        eprintln!(
            "[orchestrator] recover_waiting_review: task={} — no reviewer nodes found, re-dispatching review yield",
            node.id
        );
        handle_review_yield(node, project_id, registry, dag_tx, app).await;
        return;
    }

    if expected_sorted == answered_sorted {
        // All reviewers finished before the restart — trigger SF-3 immediately.
        eprintln!(
            "[orchestrator] recover_waiting_review: task={} — all {} reviewers already done, resuming immediately",
            node.id,
            expected_sorted.len()
        );
        check_review_completion(&node.id, project_id, registry, dag_tx, app).await;
    } else {
        // Some reviewers are not yet done. Running reviewer agents re-enter the
        // interrupted-agent recovery path and will trigger SF-3 on completion.
        // Emit NodeStatusChanged so run_loop picks up any pending (not-yet-started)
        // reviewer nodes and dispatches them.
        eprintln!(
            "[orchestrator] recover_waiting_review: task={} — {} of {} reviewers pending/running, emitting NodeStatusChanged",
            node.id,
            expected_sorted.len().saturating_sub(answered_sorted.len()),
            expected_sorted.len()
        );
        let _ = dag_tx.send(DagChanged::NodeStatusChanged {
            project_id: project_id.to_owned(),
            node_id: node.id.clone(),
        });
    }
}
