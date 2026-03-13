pub mod commands;

use crate::agent_lifecycle::{self, AgentMap, SpawnRequest};
use crate::dag_store::{self, CreateNodeInput, EdgeType, Node, NodeStatus, NodeType, ProjectRegistry};
use crate::event_ingester::DagChanged;
use crate::event_sink::EventSink;
use crate::skills;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tokio::sync::mpsc;

// ── Reviewer watchdog configuration ───────────────────────────────────────────

/// Default timeout in seconds before the watchdog fires for a reviewer task.
const REVIEWER_TIMEOUT_SECS: u64 = 300; // 5 minutes

/// Maximum number of retries before the reviewer is marked cancelled.
const REVIEWER_MAX_RETRY: u32 = 2;

// ── Concurrency limits ────────────────────────────────────────────────────────

pub struct ConcurrencyLimits {
    pub per_project: Mutex<HashMap<String, usize>>,
    pub global_limit: AtomicUsize,
    pub default_per_project: usize,
}

impl ConcurrencyLimits {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            per_project: Mutex::new(HashMap::new()),
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
pub async fn start(
    sink: Arc<dyn EventSink>,
    registry: ProjectRegistry,
    limits: Arc<ConcurrencyLimits>,
    agent_map: AgentMap,
    dag_tx: mpsc::UnboundedSender<DagChanged>,
    mut dag_rx: mpsc::UnboundedReceiver<DagChanged>,
) {
    eprintln!("[orchestrator] start: loop running");

    // u7s.3: Start periodic ghost-agent integrity loop — fires every 5 minutes,
    // cross-references agents table against AgentMap, cleans up any divergence.
    spawn_ghost_agent_integrity_loop(registry.clone(), agent_map.clone());

    loop {
        let signal = match dag_rx.recv().await {
            Some(s) => s,
            None => break,
        };
        eprintln!("[orchestrator] start: received signal {:?}", signal);

        // Drain all pending signals — categorise into buckets
        let mut resume_requests: Vec<(String, String, String, String, String, String)> = Vec::new(); // (project_id, task_id, session_id, resolution, item_id, turn_type)
        let mut opened_projects: Vec<String> = Vec::new(); // projects needing ghost-agent recovery
        let mut node_status_changed: Vec<(String, String)> = Vec::new(); // (project_id, node_id) for NodeStatusChanged
        let mut project_ids = std::collections::HashSet::new();

        // Process a single signal inline (closure not used due to borrow rules)
        categorise_signal(signal, &mut resume_requests, &mut opened_projects, &mut node_status_changed, &mut project_ids);

        while let Ok(s) = dag_rx.try_recv() {
            categorise_signal(s, &mut resume_requests, &mut opened_projects, &mut node_status_changed, &mut project_ids);
        }

        // Recover ghost agents from previously-interrupted sessions before scheduling.
        // Recovery runs concurrently per project so it does not block signal processing.
        for project_id in opened_projects.iter().cloned() {
            eprintln!("[orchestrator] project opened — spawning concurrent ghost-agent recovery for {}", project_id);
            let registry_c = registry.clone();
            let dag_tx_c = dag_tx.clone();
            let agent_map_c = agent_map.clone();
            let sink_c = Arc::clone(&sink);
            tokio::spawn(async move {
                recover_interrupted(&project_id, &registry_c, &dag_tx_c, &agent_map_c, sink_c).await;
                // Re-signal the orchestrator so run_loop resumes normal scheduling after recovery.
                let _ = dag_tx_c.send(DagChanged::DagStructureChanged { project_id });
            });
        }

        // Handle resume continuations for resolved decisions and chat responses
        for (project_id, task_id, session_id, resolution, item_id, turn_type) in resume_requests {
            resume_waiting_agent(&project_id, &task_id, &session_id, &resolution, &item_id, &turn_type, &registry, &dag_tx, &agent_map, Arc::clone(&sink)).await;
        }

        // SF-4: handle NodeStatusChanged signals — yield-handling and completion checks
        for (project_id, node_id) in node_status_changed {
            handle_node_status_changed(&project_id, &node_id, &registry, &dag_tx, &agent_map, Arc::clone(&sink)).await;
        }

        // Run normal scheduling loop for DAG changes
        for project_id in project_ids {
            run_loop(&project_id, &registry, &limits, &dag_tx, &agent_map, Arc::clone(&sink)).await;
        }
    }
}

/// Route a single DagChanged signal into the appropriate processing bucket.
fn categorise_signal(
    signal: DagChanged,
    resume_requests: &mut Vec<(String, String, String, String, String, String)>,
    opened_projects: &mut Vec<String>,
    node_status_changed: &mut Vec<(String, String)>,
    project_ids: &mut std::collections::HashSet<String>,
) {
    match signal {
        DagChanged::QueueItemResolved { project_id, task_id, session_id, resolution, item_id, turn_type } => {
            resume_requests.push((project_id, task_id, session_id, resolution, item_id, turn_type));
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

/// Resume an agent that was waiting for a human decision or a chat response.
///
/// Routes directly via `turn_type` ("decision", "chat", or "advisor") carried in
/// QueueItemResolved — no DB COUNT(*) probes required.
async fn resume_waiting_agent(
    project_id: &str,
    task_id: &str,
    session_id: &str,
    resolution: &str,
    item_id: &str,
    turn_type: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    agent_map: &AgentMap,
    sink: Arc<dyn EventSink>,
) {
    match turn_type {
        "chat" => {
            resume_chat_agent(project_id, task_id, session_id, item_id, registry, dag_tx, agent_map, Arc::clone(&sink)).await;
        }
        "advisor" => {
            resume_advisor_agent(project_id, task_id, session_id, item_id, registry, dag_tx, agent_map, Arc::clone(&sink)).await;
        }
        _ => {
            // "decision" or any unrecognised value — fall through to decision arm
            resume_decision_agent(project_id, task_id, session_id, resolution, registry, dag_tx, agent_map, Arc::clone(&sink)).await;
        }
    }
}
/// SF-4 decision arm: resume an agent that was waiting for a human decision.
/// Spawns a new stream-json session with --resume and the human's resolution as the bundle.
async fn resume_decision_agent(
    project_id: &str,
    task_id: &str,
    session_id: &str,
    resolution: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    agent_map: &AgentMap,
    sink: Arc<dyn EventSink>,
) {
    let (project_path, skill_id, model) = {
        let reg = registry.lock().unwrap();
        let db = match reg.get(project_id) {
            Some(db) => db.clone(),
            None => {
                eprintln!("[orchestrator] resume_decision_agent: project not open {}", project_id);
                return;
            }
        };
        drop(reg);
        let conn = db.conn.lock().unwrap();
        let node = match dag_store::db_get_node(&conn, task_id) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("[orchestrator] resume_decision_agent: task not found {}: {}", task_id, e);
                return;
            }
        };
        let skill_id = node.skill_id.unwrap_or_else(|| "implementer".to_owned());
        let path = std::path::PathBuf::from(&db.project.path);
        let resource_dir = sink.resource_dir();
        let model = skills::load_skill(&skill_id, &path, &resource_dir)
            .ok()
            .and_then(|s| s.model.clone());
        (path, skill_id, model)
    };

    // bp6-5c5: Atomic claim — transition waiting → resuming before spawning.
    {
        let reg = registry.lock().unwrap();
        let db = match reg.get(project_id) {
            Some(db) => db.clone(),
            None => return,
        };
        drop(reg);
        let conn = db.conn.lock().unwrap();
        match dag_store::db_claim_node_resuming(&conn, task_id) {
            Ok(true) => {}
            Ok(false) => {
                eprintln!(
                    "[orchestrator] resume_decision_agent: claim lost for task={} — already resuming, skipping",
                    task_id
                );
                return;
            }
            Err(e) => {
                eprintln!(
                    "[orchestrator] resume_decision_agent: claim failed for task={}: {}",
                    task_id, e
                );
                return;
            }
        }
    }

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

    eprintln!("[orchestrator] resume_decision_agent: resuming task={} session={}", task_id, session_id);
    if let Err(e) = agent_lifecycle::spawn_agent(spawn_req, registry, dag_tx, agent_map, sink).await {
        eprintln!("[orchestrator] resume_decision_agent: failed to resume agent for task {}: {}", task_id, e);
    }
}

/// SF-4 chat arm: resume an agent that yielded with yield_reason='chat' after a human responds.
///
/// Looks up the chat_turn by `turn_id`, verifies `responded_at IS NOT NULL` (guard against
/// stale signals), then assembles `"Human: {response}"` and spawns --resume.
async fn resume_chat_agent(
    project_id: &str,
    task_id: &str,
    session_id: &str,
    turn_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    agent_map: &AgentMap,
    sink: Arc<dyn EventSink>,
) {
    eprintln!(
        "[orchestrator] resume_chat_agent: task={} session={} turn_id={}",
        task_id, session_id, turn_id
    );

    // Guard: verify responded_at IS NOT NULL — race condition / stale signal check.
    let (response_text, project_path, skill_id, model) = {
        let reg = registry.lock().unwrap();
        let db = match reg.get(project_id) {
            Some(db) => db.clone(),
            None => {
                eprintln!("[orchestrator] resume_chat_agent: project not open {}", project_id);
                return;
            }
        };
        drop(reg);
        let conn = db.conn.lock().unwrap();

        // Query the chat turn — must have responded_at set.
        // Both response and responded_at are nullable columns → Option<String>.
        let turn_row: Option<(Option<String>, Option<String>)> = conn
            .query_row(
                "SELECT response, responded_at FROM chat_turns WHERE id = ?1",
                rusqlite::params![turn_id],
                |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .ok();

        let (response_opt, responded_at) = match turn_row {
            Some(r) => r,
            None => {
                eprintln!(
                    "[orchestrator] resume_chat_agent: chat turn not found turn_id={} — aborting",
                    turn_id
                );
                return;
            }
        };

        if responded_at.is_none() {
            eprintln!(
                "[orchestrator] resume_chat_agent: turn_id={} has no responded_at — stale signal, aborting",
                turn_id
            );
            return;
        }

        let response_text = match response_opt {
            Some(r) => r,
            None => {
                eprintln!(
                    "[orchestrator] resume_chat_agent: turn_id={} responded_at set but response is NULL — aborting",
                    turn_id
                );
                return;
            }
        };

        // Load skill info for the task.
        let node = match dag_store::db_get_node(&conn, task_id) {
            Ok(n) => n,
            Err(e) => {
                eprintln!(
                    "[orchestrator] resume_chat_agent: task not found {}: {}",
                    task_id, e
                );
                return;
            }
        };
        let skill_id = node.skill_id.unwrap_or_else(|| "implementer".to_owned());
        let path = std::path::PathBuf::from(&db.project.path);
        let resource_dir = sink.resource_dir();
        let model = skills::load_skill(&skill_id, &path, &resource_dir)
            .ok()
            .and_then(|s| s.model.clone());

        (response_text, path, skill_id, model)
    };

    // bp6-5c5: Atomic claim — transition waiting → resuming before spawning.
    // Guards against duplicate QueueItemResolved signals for the same turn.
    {
        let reg = registry.lock().unwrap();
        let db = match reg.get(project_id) {
            Some(db) => db.clone(),
            None => return,
        };
        drop(reg);
        let conn = db.conn.lock().unwrap();
        match dag_store::db_claim_node_resuming(&conn, task_id) {
            Ok(true) => {}
            Ok(false) => {
                eprintln!(
                    "[orchestrator] resume_chat_agent: claim lost for task={} — already resuming, skipping",
                    task_id
                );
                return;
            }
            Err(e) => {
                eprintln!(
                    "[orchestrator] resume_chat_agent: claim failed for task={}: {}",
                    task_id, e
                );
                return;
            }
        }
    }

    // Continuation bundle: identical format to decision resolution (Protocol.md §5).
    let input_bundle = format!("---\nHuman: {}\n", response_text);

    let spawn_req = agent_lifecycle::SpawnRequest {
        project_id: project_id.to_owned(),
        task_id: task_id.to_owned(),
        skill_id,
        project_path,
        input_bundle,
        resume_session_id: Some(session_id.to_owned()),
        model,
    };

    eprintln!(
        "[orchestrator] resume_chat_agent: spawning --resume for task={} session={}",
        task_id, session_id
    );
    if let Err(e) = agent_lifecycle::spawn_agent(spawn_req, registry, dag_tx, agent_map, sink).await {
        eprintln!(
            "[orchestrator] resume_chat_agent: failed to resume agent for task {}: {}",
            task_id, e
        );
    }
}

/// SF-4 advisor arm: resume an agent that yielded with yield_reason='advisor' after a human responds.
///
/// Structurally identical to resume_chat_agent but queries advisor_turns.
async fn resume_advisor_agent(
    project_id: &str,
    task_id: &str,
    session_id: &str,
    turn_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    agent_map: &AgentMap,
    sink: Arc<dyn EventSink>,
) {
    eprintln!(
        "[orchestrator] resume_advisor_agent: task={} session={} turn_id={}",
        task_id, session_id, turn_id
    );

    let (response_text, project_path, skill_id, model) = {
        let reg = registry.lock().unwrap();
        let db = match reg.get(project_id) {
            Some(db) => db.clone(),
            None => {
                eprintln!("[orchestrator] resume_advisor_agent: project not open {}", project_id);
                return;
            }
        };
        drop(reg);
        let conn = db.conn.lock().unwrap();

        let turn_row: Option<(Option<String>, Option<String>)> = conn
            .query_row(
                "SELECT response, responded_at FROM advisor_turns WHERE id = ?1",
                rusqlite::params![turn_id],
                |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .ok();

        let (response_opt, responded_at) = match turn_row {
            Some(r) => r,
            None => {
                eprintln!(
                    "[orchestrator] resume_advisor_agent: advisor turn not found turn_id={} — aborting",
                    turn_id
                );
                return;
            }
        };

        if responded_at.is_none() {
            eprintln!(
                "[orchestrator] resume_advisor_agent: turn_id={} has no responded_at — stale signal, aborting",
                turn_id
            );
            return;
        }

        let response_text = match response_opt {
            Some(r) => r,
            None => {
                eprintln!(
                    "[orchestrator] resume_advisor_agent: turn_id={} responded_at set but response is NULL — aborting",
                    turn_id
                );
                return;
            }
        };

        let node = match dag_store::db_get_node(&conn, task_id) {
            Ok(n) => n,
            Err(e) => {
                eprintln!(
                    "[orchestrator] resume_advisor_agent: task not found {}: {}",
                    task_id, e
                );
                return;
            }
        };
        let skill_id = node.skill_id.unwrap_or_else(|| "advisor".to_owned());
        let path = std::path::PathBuf::from(&db.project.path);
        let resource_dir = sink.resource_dir();
        let model = skills::load_skill(&skill_id, &path, &resource_dir)
            .ok()
            .and_then(|s| s.model.clone());

        (response_text, path, skill_id, model)
    };

    // bp6-5c5: Atomic claim — transition waiting → resuming before spawning.
    {
        let reg = registry.lock().unwrap();
        let db = match reg.get(project_id) {
            Some(db) => db.clone(),
            None => return,
        };
        drop(reg);
        let conn = db.conn.lock().unwrap();
        match dag_store::db_claim_node_resuming(&conn, task_id) {
            Ok(true) => {}
            Ok(false) => {
                eprintln!(
                    "[orchestrator] resume_advisor_agent: claim lost for task={} — already resuming, skipping",
                    task_id
                );
                return;
            }
            Err(e) => {
                eprintln!(
                    "[orchestrator] resume_advisor_agent: claim failed for task={}: {}",
                    task_id, e
                );
                return;
            }
        }
    }

    let input_bundle = format!("---\nHuman: {}\n", response_text);

    let spawn_req = agent_lifecycle::SpawnRequest {
        project_id: project_id.to_owned(),
        task_id: task_id.to_owned(),
        skill_id,
        project_path,
        input_bundle,
        resume_session_id: Some(session_id.to_owned()),
        model,
    };

    eprintln!(
        "[orchestrator] resume_advisor_agent: spawning --resume for task={} session={}",
        task_id, session_id
    );
    if let Err(e) = agent_lifecycle::spawn_agent(spawn_req, registry, dag_tx, agent_map, sink).await {
        eprintln!(
            "[orchestrator] resume_advisor_agent: failed to resume agent for task {}: {}",
            task_id, e
        );
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
    agent_map: &AgentMap,
    sink: Arc<dyn EventSink>,
) {
    let node = {
        let reg = registry.lock().unwrap();
        let db = match reg.get(project_id) {
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
                    handle_review_yield(&node, project_id, registry, dag_tx, agent_map, Arc::clone(&sink)).await;
                }
                Some("decision") => {
                    // No action — await human resolution via resolve_decision command
                    // (already handled by QueueItemResolved path)
                    eprintln!(
                        "[orchestrator] handle_node_status_changed: node={} waiting for decision — no action",
                        node.id
                    );
                }
                Some("chat") => {
                    // SF-3 chat arm: task has emitted a poe:chat event and is now waiting.
                    // Do NOT auto-dispatch anything and do NOT set a watchdog timer.
                    // Simply wait for respond_to_chat to fire DagChanged::QueueItemResolved,
                    // which will be routed to resume_chat_agent (SF-4 chat arm).
                    eprintln!(
                        "[orchestrator] handle_node_status_changed: node={} waiting for chat response — no action",
                        node.id
                    );
                }
                Some("advisor") => {
                    // SF-4 advisor arm: task has emitted a poe:advisor event and is now waiting.
                    // Wait for respond_to_advisor to fire DagChanged::QueueItemResolved,
                    // which will be routed to resume_advisor_agent.
                    eprintln!(
                        "[orchestrator] handle_node_status_changed: node={} waiting for advisor response — no action",
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
                check_review_completion(requesting_task_id, project_id, registry, dag_tx, agent_map, Arc::clone(&sink)).await;
            }

            // Phase completion detection: if this node belongs to a phase, check if all
            // tasks in the phase are done or cancelled. If so, set phase status='gate'.
            if let Some(ref phase_id) = node.phase_id {
                check_phase_completion(phase_id, project_id, registry, sink.as_ref()).await;
            }

            // u7s.5: Hierarchy sweep — walk parent_id upward, closing container nodes
            // (feature, epic, etc.) whose children are all terminal. This is the primary
            // close path for container nodes which are never dispatched by db_find_ready_tasks.
            // Only parent_id (organisational hierarchy) is walked — never edges (dependencies).
            close_completed_ancestors(&node.id, project_id, registry, sink.as_ref()).await;
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
    agent_map: &AgentMap,
    sink: Arc<dyn EventSink>,
) {
    eprintln!(
        "[orchestrator] handle_review_yield: task={} — querying poe:review events",
        waiting_task.id
    );

    // Query poe:review events for this task to get (review_id, reviewer_skill) pairs
    let review_events: Vec<(String, String)> = {
        let reg = registry.lock().unwrap();
        let db = match reg.get(project_id) {
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
            let db = match reg.get(project_id) {
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
                id: None,
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
                requires_manual_verification: None,
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
            Arc::clone(&sink),
            registry.clone(),
            dag_tx.clone(),
            agent_map.clone(),
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
        dispatch_reviewer_task(reviewer, project_id, waiting_task, registry, dag_tx, agent_map, Arc::clone(&sink)).await;
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
    agent_map: &AgentMap,
    sink: Arc<dyn EventSink>,
) {
    eprintln!(
        "[orchestrator] dispatch_reviewer_task: reviewer={} review_id={:?} skill={:?}",
        reviewer.id, reviewer.review_id, reviewer.skill_id
    );

    // bp6-5c5: Atomic claim — same pattern as dispatch_task.
    {
        let reg = registry.lock().unwrap();
        let db = match reg.get(project_id) {
            Some(db) => db.clone(),
            None => return,
        };
        drop(reg);
        let conn = db.conn.lock().unwrap();
        match dag_store::db_claim_node_running(&conn, &reviewer.id) {
            Ok(true) => {
                eprintln!("[orchestrator] dispatch_reviewer_task: claimed reviewer={}", reviewer.id);
            }
            Ok(false) => {
                eprintln!(
                    "[orchestrator] dispatch_reviewer_task: claim lost for reviewer={} — already claimed, skipping",
                    reviewer.id
                );
                return;
            }
            Err(e) => {
                eprintln!(
                    "[orchestrator] dispatch_reviewer_task: claim failed for reviewer={}: {}",
                    reviewer.id, e
                );
                return;
            }
        }
    }

    let bundle_data = {
        let reg = registry.lock().unwrap();
        let db = match reg.get(project_id) {
            Some(db) => db.clone(),
            None => return,
        };
        drop(reg);

        let conn = db.conn.lock().unwrap();
        let project_path = PathBuf::from(&db.project.path);
        let skill_id = reviewer.skill_id.clone().unwrap_or_else(|| "implementer".to_owned());
        let resource_dir = sink.resource_dir();
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

    // Build ReviewRequest bundle per Protocol.md §3, routed through assemble_input_bundle()
    // so the reviewer receives the autonomous mode execution protocol block.
    let review_id = reviewer.review_id.as_deref().unwrap_or("unknown");
    let review_content = reviewer.description.as_deref().unwrap_or("");

    // Compose the reviewer task description to pass as task_description to assemble_input_bundle.
    let reviewer_description = format!(
        "## Review Request\n\n**Requested by**: {requesting_id} ({requesting_title})\n**Review ID**: {review_id}\n\n{review_content}\n\n> **Naming convention**: The reviewer MUST emit its artifact as {{\"poe\":\"artifact\",\"name\":\"review-{review_id}.md\",...}} where `{review_id}` is the Review ID above.",
        requesting_id = requesting_task.id,
        requesting_title = requesting_task.title,
        review_id = review_id,
        review_content = review_content,
    );

    let knowledge_refs: Vec<(&str, &str)> = knowledge
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let artifact_refs: Vec<(&str, &str)> = artifacts
        .iter()
        .map(|(t, f)| (t.as_str(), f.as_str()))
        .collect();

    let reviewer_title = format!("Plan Review — {}", requesting_task.title);
    let input_bundle = skills::assemble_input_bundle(
        &skills::SpawnMode::Autonomous,
        &skill,
        &reviewer_title,
        Some(&reviewer_description),
        &[], // reviewer has no WBS ancestry to inject
        &knowledge_refs,
        &artifact_refs,
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

    if let Err(e) = agent_lifecycle::spawn_agent(spawn_req, registry, dag_tx, agent_map, sink).await {
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
    agent_map: &AgentMap,
    sink: Arc<dyn EventSink>,
) {
    let (requesting_task, expected_ids, answered_ids, project_path) = {
        let reg = registry.lock().unwrap();
        let db = match reg.get(project_id) {
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
    // bp6-7r2.3: collect artifact paths so the resumed PM has a direct pointer
    // to the approved baseline file(s) rather than re-deriving from scratch.
    let mut artifact_paths: Vec<String> = Vec::new();
    for review_id in &expected_sorted {
        // Find the reviewer node to get skill, status, and stored verdict (u7s.4)
        let (reviewer_skill, verdict) = {
            let reg = registry.lock().unwrap();
            let db = match reg.get(project_id) {
                Some(db) => db.clone(),
                None => continue,
            };
            drop(reg);
            let conn = db.conn.lock().unwrap();
            let result = conn.query_row(
                "SELECT id, skill_id, status, verdict FROM nodes WHERE requesting_task_id = ?1 AND review_id = ?2 LIMIT 1",
                rusqlite::params![requesting_task_id, review_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,        // id
                        row.get::<_, Option<String>>(1)?, // skill_id
                        row.get::<_, String>(2)?,         // status
                        row.get::<_, Option<String>>(3)?, // verdict
                    ))
                },
            );
            match result {
                Ok((reviewer_node_id, skill, status, stored_verdict)) => {
                    // u7s.4: derive verdict from stored poe:review-outcome value,
                    // falling back to FAILED for cancelled nodes, or BLOCKED with
                    // a warning when poe:review-outcome was not received.
                    let verdict = if status == "cancelled" {
                        // Watchdog cancelled: treat as FAILED regardless of stored verdict
                        "FAILED".to_owned()
                    } else if let Some(v) = stored_verdict {
                        v
                    } else {
                        // Reviewer completed (poe:done) but never emitted poe:review-outcome
                        // — this is a reviewer skill bug. Emit a warning and use BLOCKED.
                        eprintln!(
                            "[orchestrator] check_review_completion: reviewer node={} review_id={} has no verdict (poe:review-outcome missing) — defaulting to BLOCKED",
                            reviewer_node_id, review_id
                        );
                        // Emit poe-ingester-warning to the frontend so the operator sees it.
                        crate::event_sink::emit_event(
                            &*sink,
                            "poe-ingester-warning",
                            &serde_json::json!({
                                "taskId": requesting_task_id,
                                "agentId": reviewer_node_id,
                                "eventType": "poe:review-outcome",
                                "error": format!(
                                    "Reviewer task {} (review_id={}) completed without emitting poe:review-outcome; verdict defaulted to BLOCKED",
                                    reviewer_node_id, review_id
                                )
                            }),
                        );
                        "BLOCKED".to_owned()
                    };
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
        // bp6-7r2.3: record the path so the PM resume bundle can reference it
        artifact_paths.push(artifact_path.to_string_lossy().into_owned());
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

    // bp6-7r2.3: append reviewer artifact paths to the bundle so the resumed PM
    // agent can read the approved baseline directly instead of re-deriving it.
    if !artifact_paths.is_empty() {
        bundle.push_str(&format!(
            "Reviewer artifacts: {}\n",
            artifact_paths.join(", ")
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

    // u7s.1: DB-arbitrated single-dispatch claim. Transition waiting → resuming so
    // only the first concurrent caller wins. Concurrent callers see rows_changed==0
    // and abort, preventing duplicate resume spawns when reviewers complete near-simultaneously.
    let claimed = {
        let reg = registry.lock().unwrap();
        let db = match reg.get(project_id) {
            Some(db) => db.clone(),
            None => return,
        };
        drop(reg);
        let conn = db.conn.lock().unwrap();
        match dag_store::db_claim_node_resuming(&conn, requesting_task_id) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "[orchestrator] check_review_completion: claim failed for task={}: {}",
                    requesting_task_id, e
                );
                return;
            }
        }
    };

    if !claimed {
        eprintln!(
            "[orchestrator] check_review_completion: claim lost — another caller is already resuming task={}",
            requesting_task_id
        );
        return;
    }

    let (skill_id, model) = {
        let skill_id = requesting_task.skill_id.clone().unwrap_or_else(|| "implementer".to_owned());
        let resource_dir = sink.resource_dir();
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

    if let Err(e) = agent_lifecycle::spawn_agent(spawn_req, registry, dag_tx, agent_map, sink).await {
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
    sink: Arc<dyn EventSink>,
    registry: ProjectRegistry,
    dag_tx: mpsc::UnboundedSender<DagChanged>,
    agent_map: AgentMap,
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

        // Read current node state under a short-lived lock
        let (current_status, current_retry) = {
            let reg = registry.lock().unwrap();
            let db = match reg.get(&project_id) {
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
            // u7s.2: Retry path — use atomic DB claim to prevent double-retry
            // when both the watchdog and the exit handler fire near-simultaneously.
            // rows_changed==1 means this caller won; rows_changed==0 means stand down.
            let retry_claimed = {
                let reg = registry.lock().unwrap();
                let db = match reg.get(&project_id) {
                    Some(db) => db.clone(),
                    None => return,
                };
                drop(reg);
                let conn = db.conn.lock().unwrap();
                match dag_store::db_claim_node_retry(&conn, &reviewer_task_id) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("[orchestrator] watchdog: db_claim_node_retry failed: {}", e);
                        return;
                    }
                }
            };

            if !retry_claimed {
                eprintln!(
                    "[orchestrator] watchdog: reviewer={} retry claim lost — another path already handled it, standing down",
                    reviewer_task_id
                );
                return;
            }

            eprintln!(
                "[orchestrator] watchdog: reviewer={} retry claimed — requeueing to pending",
                reviewer_task_id
            );

            // Signal the run_loop to re-dispatch the pending reviewer (SF-1)
            let _ = dag_tx.send(DagChanged::DagStructureChanged {
                project_id: project_id.clone(),
            });

            // Spawn a fresh watchdog for the retry
            spawn_reviewer_watchdog(
                sink,
                registry,
                dag_tx,
                agent_map,
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
                let db = match reg.get(&project_id) {
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
            check_review_completion(&requesting_task_id, &project_id, &registry, &dag_tx, &agent_map, sink).await;
        }
    });
}

// ── Phase completion ──────────────────────────────────────────────────────────

/// Check if all tasks in a phase are done/cancelled. If so, set phase status='gate'.
async fn check_phase_completion(
    phase_id: &str,
    project_id: &str,
    registry: &ProjectRegistry,
    sink: &dyn EventSink,
) {
    let should_gate = {
        let reg = registry.lock().unwrap();
        let db = match reg.get(project_id) {
            Some(db) => db.clone(),
            None => return,
        };
        drop(reg);
        let conn = db.conn.lock().unwrap();

        // Get phase — only check if status='running'
        let phase = match dag_store::db_get_phase(&conn, phase_id) {
            Ok(p) => p,
            Err(_) => return,
        };
        if phase.status != "running" {
            return;
        }

        // Count pending/running/waiting tasks in this phase
        let active_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM nodes WHERE phase_id = ?1 AND status IN ('pending','running','waiting') AND node_type IN ('task','bug','chore','subtask','plan_review','advisor')",
            [phase_id],
            |row| row.get(0),
        ).unwrap_or(1); // default to 1 (not done) on error

        let total_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM nodes WHERE phase_id = ?1 AND node_type IN ('task','bug','chore','subtask','plan_review','advisor')",
            [phase_id],
            |row| row.get(0),
        ).unwrap_or(0);

        total_count > 0 && active_count == 0
    };

    if should_gate {
        eprintln!("[orchestrator] check_phase_completion: phase={} all tasks done — setting status=gate", phase_id);
        let reg = registry.lock().unwrap();
        let db = match reg.get(project_id) {
            Some(db) => db.clone(),
            None => return,
        };
        drop(reg);
        let conn = db.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        let _ = conn.execute(
            "UPDATE phases SET status = 'gate', updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, phase_id],
        );
        drop(conn);

        sink.emit("poe-phase-update", &serde_json::json!({
            "phaseId": phase_id,
            "status": "gate",
            "projectId": project_id,
        }));
    }
}

// ── u7s.5: Hierarchy sweep ────────────────────────────────────────────────────

/// Walk parent_id upward from `node_id`, closing container nodes (feature, epic,
/// project, etc.) whose children are all in terminal state (complete or cancelled).
///
/// Design invariants:
/// - Walks parent_id only (organisational hierarchy). NEVER walks edges (technical deps).
/// - Calls check_phase_completion at each level — cheap and idempotent.
/// - Only closes containers that have at least one child (empty containers stay open).
async fn close_completed_ancestors(
    node_id: &str,
    project_id: &str,
    registry: &ProjectRegistry,
    sink: &dyn EventSink,
) {
    let mut current_id = node_id.to_owned();

    loop {
        // Get the parent of the current node
        let parent_id_opt = {
            let reg = registry.lock().unwrap();
            let db = match reg.get(project_id) {
                Some(db) => db.clone(),
                None => return,
            };
            drop(reg);
            let conn = db.conn.lock().unwrap();
            match dag_store::db_get_node_parent(&conn, &current_id) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!(
                        "[orchestrator] close_completed_ancestors: db_get_node_parent failed for node={}: {}",
                        current_id, e
                    );
                    return;
                }
            }
        };

        let parent_id = match parent_id_opt {
            Some(p) => p,
            None => {
                // Reached top-level node (no parent) — sweep complete
                return;
            }
        };

        // Check if all children of the parent are terminal
        let all_terminal = {
            let reg = registry.lock().unwrap();
            let db = match reg.get(project_id) {
                Some(db) => db.clone(),
                None => return,
            };
            drop(reg);
            let conn = db.conn.lock().unwrap();
            match dag_store::db_all_children_terminal(&conn, &parent_id) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!(
                        "[orchestrator] close_completed_ancestors: db_all_children_terminal failed for parent={}: {}",
                        parent_id, e
                    );
                    return;
                }
            }
        };

        if !all_terminal {
            // Siblings are still pending/running/waiting — do not close this container yet
            return;
        }

        // Close the parent container
        eprintln!(
            "[orchestrator] close_completed_ancestors: all children of parent={} are terminal — closing container",
            parent_id
        );

        let parent_phase_id = {
            let reg = registry.lock().unwrap();
            let db = match reg.get(project_id) {
                Some(db) => db.clone(),
                None => return,
            };
            drop(reg);
            let conn = db.conn.lock().unwrap();
            let now = chrono::Utc::now().to_rfc3339();
            // Only close if not already terminal (idempotent)
            let _ = conn.execute(
                "UPDATE nodes SET status = 'complete', updated_at = ?1 WHERE id = ?2 AND status NOT IN ('complete', 'cancelled')",
                rusqlite::params![now, &parent_id],
            );
            // Return the phase_id for check_phase_completion
            match dag_store::db_get_node(&conn, &parent_id) {
                Ok(n) => n.phase_id,
                Err(_) => None,
            }
        };

        // Notify frontend of container closure via legacy poe-dag-node-status
        sink.emit("poe-dag-node-status", &serde_json::json!({
            "nodeId": parent_id,
            "status": "complete",
            "projectId": project_id,
        }));
        // bp6-7r2.5 (backend): also emit node-status-changed so the WBS view
        // can subscribe to a single canonical event for all node status updates.
        sink.emit("node-status-changed", &serde_json::json!({
            "nodeId": parent_id,
            "status": "complete",
            "projectId": project_id,
        }));

        // Check phase completion at this level (cheap, idempotent)
        if let Some(ref phase_id) = parent_phase_id {
            check_phase_completion(phase_id, project_id, registry, sink).await;
        }

        // Ascend to the next level
        current_id = parent_id;
    }
}

// ── Core loop ─────────────────────────────────────────────────────────────────

async fn run_loop(
    project_id: &str,
    registry: &ProjectRegistry,
    limits: &Arc<ConcurrencyLimits>,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    agent_map: &AgentMap,
    sink: Arc<dyn EventSink>,
) {
    eprintln!("[orchestrator] run_loop wakeup project={}", project_id);

    let ready_tasks = {
        let reg = registry.lock().unwrap();
        let db = match reg.get(project_id) {
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
        dispatch_task(task, project_id, registry, dag_tx, agent_map, Arc::clone(&sink)).await;
    }
}

async fn dispatch_task(
    task: Node,
    project_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    agent_map: &AgentMap,
    sink: Arc<dyn EventSink>,
) {
    eprintln!(
        "[orchestrator] dispatch_task: id={} title={:?} skill={:?}",
        task.id, task.title, task.skill_id
    );

    // bp6-5c5: Atomic claim — transition pending → running before any bundle assembly.
    // Closes the TOCTOU window: if a second DagStructureChanged arrives while bundle
    // assembly is in progress, the second dispatch_task call will see rows_affected=0
    // and return early. This is idempotent and race-free.
    {
        let reg = registry.lock().unwrap();
        let db = match reg.get(project_id) {
            Some(db) => db.clone(),
            None => return,
        };
        drop(reg);
        let conn = db.conn.lock().unwrap();
        match dag_store::db_claim_node_running(&conn, &task.id) {
            Ok(true) => {
                eprintln!("[orchestrator] dispatch_task: claimed task={}", task.id);
            }
            Ok(false) => {
                eprintln!(
                    "[orchestrator] dispatch_task: claim lost for task={} — already claimed by another dispatch, skipping",
                    task.id
                );
                return;
            }
            Err(e) => {
                eprintln!(
                    "[orchestrator] dispatch_task: claim failed for task={}: {}",
                    task.id, e
                );
                return;
            }
        }
    }

    let bundle_data = {
        let reg = registry.lock().unwrap();
        let db = match reg.get(project_id) {
            Some(db) => db.clone(),
            None => return,
        };
        drop(reg);

        let conn = db.conn.lock().unwrap();

        let project_path = PathBuf::from(&db.project.path);
        let skill_id = task.skill_id.clone().unwrap_or_else(|| "implementer".to_owned());

        let resource_dir = sink.resource_dir();

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
            eprintln!("[orchestrator] Failed to load skill '{}': {} — triggering self-healing skill-author task", skill_id, e);

            // Self-healing: synthesise a skill-author task instead of cancelling.
            // The failing task stays `pending`; it will be dispatched once the
            // skill-author task completes and the skill file exists.
            let synth_title = format!("Synthesize missing skill: {}", skill_id);

            let reg = registry.lock().unwrap();
            let db = match reg.get(project_id) {
                Some(db) => db.clone(),
                None => return,
            };
            drop(reg);

            let conn = db.conn.lock().unwrap();

            // ── Dedup check ───────────────────────────────────────────────────
            // 1. Look for an existing pending/running skill-author task for this skill.
            let existing_author: Option<String> = {
                let mut found = None;
                for status_str in &["pending", "running"] {
                    if let Ok(nodes) = dag_store::db_list_nodes_by_status(&conn, project_id, status_str) {
                        for n in nodes {
                            if n.skill_id.as_deref() == Some("skill-author") && n.title == synth_title {
                                found = Some(n.id.clone());
                                break;
                            }
                        }
                    }
                    if found.is_some() {
                        break;
                    }
                }
                found
            };

            // 2. If no active skill-author exists, check for a *completed* one whose
            //    skill file is still missing (i.e., the agent finished but wrote nothing).
            //    Retrying would loop forever — cancel the failing task instead.
            if existing_author.is_none() {
                let completed_author_exists = dag_store::db_list_nodes_by_status(&conn, project_id, "complete")
                    .unwrap_or_default()
                    .into_iter()
                    .any(|n| n.skill_id.as_deref() == Some("skill-author") && n.title == synth_title);

                if completed_author_exists {
                    eprintln!(
                        "[orchestrator] dispatch_task: skill-author already completed for '{}' but skill file is still missing — \
                         agent produced no output. Cancelling task '{}' to prevent infinite retry loop.",
                        skill_id, task.id
                    );
                    let cancel_input = dag_store::UpdateNodeInput {
                        title: None,
                        description: Some(format!(
                            "Skill synthesis failed: skill-author completed without producing '{}'. Manual intervention required.",
                            skill_id
                        )),
                        status: Some(NodeStatus::Cancelled),
                        skill_id: None,
                        assignee: None,
                        yield_reason: None,
                        session_id: None,
                    };
                    if let Err(ue) = dag_store::db_update_node(&conn, &task.id, &cancel_input) {
                        eprintln!(
                            "[orchestrator] dispatch_task: failed to cancel task '{}': {}",
                            task.id, ue
                        );
                    }
                    drop(conn);
                    let _ = dag_tx.send(DagChanged::DagStructureChanged {
                        project_id: project_id.to_string(),
                    });
                    return;
                }
            }

            let author_id = if let Some(id) = existing_author {
                eprintln!(
                    "[orchestrator] dispatch_task: existing skill-author task {} found for skill '{}', reusing",
                    id, skill_id
                );
                id
            } else {
                // ── Gather sibling tasks that also need this skill ────────────
                let description = {
                    let mut desc = format!("Synthesize the '{}' skill file.\n\nBlocked tasks:\n", skill_id);
                    if let Ok(pending) = dag_store::db_list_nodes_by_status(&conn, project_id, "pending") {
                        for n in &pending {
                            if n.skill_id.as_deref() == Some(skill_id.as_str()) {
                                desc.push_str(&format!("- {}\n", n.title));
                            }
                        }
                    }
                    desc
                };

                // ── Create the skill-author node ──────────────────────────────
                let input = CreateNodeInput {
                    project_id: project_id.to_string(),
                    id: None,
                    phase_id: None, // no-phase tasks are always eligible in db_find_ready_tasks
                    parent_id: None,
                    node_type: NodeType::Task,
                    title: synth_title.clone(),
                    description: Some(description),
                    skill_id: Some("skill-author".to_string()),
                    initial_status: Some(NodeStatus::Pending),
                    requesting_task_id: None,
                    review_id: None,
                    retry_count: None,
                    requires_manual_verification: None,
                };
                match dag_store::db_create_node(&conn, &input) {
                    Ok(n) => {
                        eprintln!(
                            "[orchestrator] dispatch_task: created skill-author task {} for skill '{}'",
                            n.id, skill_id
                        );
                        n.id
                    }
                    Err(ce) => {
                        eprintln!(
                            "[orchestrator] dispatch_task: failed to create skill-author task for skill '{}': {}",
                            skill_id, ce
                        );
                        return;
                    }
                }
            };

            // ── Add depends_on edges: author node → failing task (and siblings) ──
            // Edge semantics: db_find_ready_tasks blocks `to_id` until `from_id` is complete.
            // So (from=author, to=task) means task waits for skill-author to finish.
            if let Err(ee) = dag_store::db_create_edge(&conn, &author_id, &task.id, EdgeType::DependsOn) {
                eprintln!(
                    "[orchestrator] dispatch_task: failed to add edge {} → {}: {}",
                    author_id, task.id, ee
                );
            }
            // Also wire any other pending tasks with the same skill_id.
            if let Ok(pending) = dag_store::db_list_nodes_by_status(&conn, project_id, "pending") {
                for n in &pending {
                    if n.id != task.id
                        && n.skill_id.as_deref() == Some(skill_id.as_str())
                    {
                        if let Err(ee) = dag_store::db_create_edge(&conn, &author_id, &n.id, EdgeType::DependsOn) {
                            eprintln!(
                                "[orchestrator] dispatch_task: edge {} → {} failed: {}",
                                author_id, n.id, ee
                            );
                        }
                    }
                }
            }

            drop(conn);

            // Wake the scheduler so the new skill-author task is dispatched immediately.
            let _ = dag_tx.send(DagChanged::DagStructureChanged {
                project_id: project_id.to_string(),
            });

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

    // Choose spawn mode: interactive when the skill declares it, autonomous otherwise.
    let spawn_mode = if skill.modes.contains(&"interactive".to_string()) {
        skills::SpawnMode::Interactive
    } else {
        skills::SpawnMode::Autonomous
    };

    // For skill-author tasks, use a specialised bundle that injects skill-authoring context.
    let input_bundle = if skill_id == "skill-author" {
        // The task title encodes the missing skill name: "Synthesize missing skill: {name}"
        let missing_skill_name = task
            .title
            .strip_prefix("Synthesize missing skill: ")
            .unwrap_or(task.title.as_str())
            .to_owned();

        // Gather pending tasks that need the missing skill.
        let failing_tasks_owned: Vec<(String, String)> = {
            let reg = registry.lock().unwrap();
            if let Some(db) = reg.get(project_id) {
                let conn = db.conn.lock().unwrap();
                dag_store::db_list_nodes_by_status(&conn, project_id, "pending")
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|n| n.skill_id.as_deref() == Some(missing_skill_name.as_str()))
                    .map(|n| (n.title, n.description.unwrap_or_default()))
                    .collect()
            } else {
                vec![]
            }
        };
        let failing_refs: Vec<(&str, &str)> = failing_tasks_owned
            .iter()
            .map(|(t, d)| (t.as_str(), d.as_str()))
            .collect();

        // Collect existing skill names from the same search paths load_skill() uses.
        let existing_skills_owned: Vec<String> = {
            let mut names: Vec<String> = Vec::new();
            let mut seen = std::collections::HashSet::new();
            let mut scan_dir = |dir: PathBuf| {
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|e| e.to_str()) == Some("md") {
                            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                                if stem != "SKILL_GUIDE" && seen.insert(stem.to_owned()) {
                                    names.push(stem.to_owned());
                                }
                            }
                        }
                    }
                }
            };

            #[cfg(debug_assertions)]
            scan_dir(
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("skills"),
            );
            scan_dir(resource_dir.join("skills"));
            if let Some(home) = dirs::home_dir() {
                scan_dir(home.join(".poe").join("skills"));
            }
            scan_dir(project_path.join(".poe").join("skills"));

            names
        };
        let existing_refs: Vec<&str> = existing_skills_owned.iter().map(|s| s.as_str()).collect();

        skills::assemble_skill_author_bundle(
            &spawn_mode,
            &skill,
            &missing_skill_name,
            &failing_refs,
            &existing_refs,
            &knowledge_refs,
            &artifact_refs,
        )
    } else {
        skills::assemble_input_bundle(
            &spawn_mode,
            &skill,
            &task.title,
            task.description.as_deref(),
            &ancestry_refs,
            &knowledge_refs,
            &artifact_refs,
        )
    };

    let spawn_req = SpawnRequest {
        project_id: project_id.to_owned(),
        task_id: task.id.clone(),
        skill_id,
        project_path,
        input_bundle,
        resume_session_id: None,
        model: skill.model.clone(),
    };

    if let Err(e) = agent_lifecycle::spawn_agent(spawn_req, registry, dag_tx, agent_map, sink).await {
        eprintln!("[orchestrator] Failed to spawn agent for task {}: {}", task.id, e);
    }
}

// ── Recovery on app start ─────────────────────────────────────────────────────

/// u7s.3 — Cross-reference agents table against AgentMap and close any ghost rows.
///
/// A ghost agent is an `agents` row with `status='running'` whose `agent_id` is NOT
/// present in the in-memory AgentMap (i.e. there is no corresponding live process).
/// Ghost agents inflate `db_count_running_agents` and permanently consume concurrency
/// slots until they are cleaned up.
///
/// For each ghost agent found:
/// 1. Mark agents row as `failed` (status='failed', ended_at=now).
/// 2. If the associated node is still `running` (not `resuming`, `waiting`, etc.),
///    atomically claim it for retry via `db_claim_node_retry` (running → pending).
///    This queues it for fresh dispatch by the normal scheduler.
async fn sweep_ghost_agents(
    project_id: &str,
    registry: &ProjectRegistry,
    agent_map: &AgentMap,
) {
    // Collect the set of live agent IDs while holding AgentMap lock as briefly as possible.
    let live_ids: std::collections::HashSet<String> = {
        agent_map.lock().unwrap().keys().cloned().collect()
    };

    let ghost_agents = {
        let reg = registry.lock().unwrap();
        let db = match reg.get(project_id) {
            Some(db) => db.clone(),
            None => return,
        };
        drop(reg);
        let conn = db.conn.lock().unwrap();
        dag_store::db_list_ghost_agents(&conn, project_id, &live_ids).unwrap_or_default()
    };

    if ghost_agents.is_empty() {
        return;
    }

    eprintln!(
        "[orchestrator] sweep_ghost_agents: found {} ghost agent(s) for project={}",
        ghost_agents.len(),
        project_id
    );

    for ghost in ghost_agents {
        eprintln!(
            "[orchestrator] sweep_ghost_agents: closing ghost agent={} task={}",
            ghost.id, ghost.task_id
        );

        let reg = registry.lock().unwrap();
        let db = match reg.get(project_id) {
            Some(db) => db.clone(),
            None => continue,
        };
        drop(reg);
        let conn = db.conn.lock().unwrap();

        // Mark the agent row as failed.
        if let Err(e) = dag_store::db_end_agent(&conn, &ghost.id, "failed") {
            eprintln!(
                "[orchestrator] sweep_ghost_agents: failed to end agent={}: {}",
                ghost.id, e
            );
        }

        // If the node is still running, atomically claim it for retry (running → pending).
        // db_claim_node_retry returns Ok(false) if the node is no longer running — safe to ignore.
        match dag_store::db_claim_node_retry(&conn, &ghost.task_id) {
            Ok(true) => {
                eprintln!(
                    "[orchestrator] sweep_ghost_agents: ghost agent={} task={} reset to pending",
                    ghost.id, ghost.task_id
                );
            }
            Ok(false) => {
                // Node already moved to a non-running status (e.g. complete, cancelled, waiting).
                // No action needed — the existing status is authoritative.
            }
            Err(e) => {
                eprintln!(
                    "[orchestrator] sweep_ghost_agents: db_claim_node_retry failed for task={}: {}",
                    ghost.task_id, e
                );
            }
        }
    }
}

/// u7s.3 — Spawn a periodic ghost-agent integrity check that fires every 5 minutes.
///
/// This is a mid-session safeguard: if a live agent crashes without removing itself
/// from the agents table (e.g. Tokio runtime crash), the ghost row would block
/// future scheduling indefinitely. The periodic sweep detects and cleans these up.
///
/// The loop runs until the registry is empty for all projects on a given check, at
/// which point it stops (app is closing). In practice it runs for the entire app
/// lifetime because there is always at least one open project.
pub fn spawn_ghost_agent_integrity_loop(
    registry: ProjectRegistry,
    agent_map: AgentMap,
) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(300)).await; // 5 minutes

            // Collect currently open project IDs.
            let project_ids: Vec<String> = {
                registry.lock().unwrap().keys().cloned().collect()
            };

            if project_ids.is_empty() {
                // No projects open — nothing to sweep. Keep looping in case a project
                // is opened later in the session.
                continue;
            }

            eprintln!(
                "[orchestrator] ghost_integrity_loop: periodic sweep across {} project(s)",
                project_ids.len()
            );

            for project_id in project_ids {
                sweep_ghost_agents(&project_id, &registry, &agent_map).await;
            }
        }
    });
}

pub async fn recover_interrupted(
    project_id: &str,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    agent_map: &AgentMap,
    sink: Arc<dyn EventSink>,
) {
    // u7s.3: Ghost-agent sweep — mark any agents table rows with status='running'
    // that have no corresponding live process (not in AgentMap) as failed, and
    // reset their associated node to pending so it can be re-dispatched.
    // This runs BEFORE the normal interrupted-agent recovery so that ghost rows
    // do not artificially inflate db_count_running_agents.
    sweep_ghost_agents(project_id, registry, agent_map).await;

    let (interrupted, waiting_nodes, project_path) = {
        let reg = registry.lock().unwrap();
        let db = match reg.get(project_id) {
            Some(db) => db.clone(),
            None => return,
        };
        drop(reg);
        let conn = db.conn.lock().unwrap();
        let agents = dag_store::db_list_agents_by_status(&conn, project_id, "running")
            .unwrap_or_default();

        // u7s.1: Ghost-claim recovery — reset any nodes stuck in `resuming` back to
        // `waiting` so they can re-enter the SF-3 resume path on the next completion
        // signal. These are nodes where the resume spawn was claimed but the app
        // crashed before spawn_agent completed.
        let now = chrono::Utc::now().to_rfc3339();
        let ghost_count = conn.execute(
            "UPDATE nodes SET status = 'waiting', updated_at = ?1 WHERE project_id = ?2 AND status = 'resuming'",
            rusqlite::params![now, project_id],
        ).unwrap_or(0);
        if ghost_count > 0 {
            eprintln!(
                "[orchestrator] recover_interrupted: reset {} ghost-claim resuming node(s) back to waiting for project={}",
                ghost_count, project_id
            );
        }

        let waiting = dag_store::db_list_nodes_by_status(&conn, project_id, "waiting")
            .unwrap_or_default();
        let path = PathBuf::from(&db.project.path);
        (agents, waiting, path)
    };

    let resource_dir = sink.resource_dir();

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

            match agent_lifecycle::spawn_agent(spawn_req, registry, dag_tx, agent_map, Arc::clone(&sink)).await {
                Ok(_) => {
                    eprintln!("[orchestrator] Resumed agent for task {}", agent.task_id);
                }
                Err(e) => {
                    eprintln!(
                        "[orchestrator] Resume failed for task {}: {} — requeueing",
                        agent.task_id, e
                    );
                    let reg = registry.lock().unwrap();
                    if let Some(db) = reg.get(project_id) {
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
            if let Some(db) = reg.get(project_id) {
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
                recover_waiting_review(&node, project_id, registry, dag_tx, agent_map, Arc::clone(&sink)).await;
            }
            Some("decision") => {
                // Decision record persists in queue_items — human resolves when ready.
                // No action needed; QueueItemResolved path handles it when resolved.
                eprintln!(
                    "[orchestrator] recover_interrupted: waiting node={} yield_reason=decision — no action (human must resolve)",
                    node.id
                );
            }
            Some("chat") => {
                // Chat turn persists in chat_turns — human responds when ready.
                // No action needed; respond_to_chat fires QueueItemResolved when the
                // human replies, which routes to resume_chat_agent (SF-4 chat arm).
                eprintln!(
                    "[orchestrator] recover_interrupted: waiting node={} yield_reason=chat — no action (awaiting human chat response)",
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
    agent_map: &AgentMap,
    sink: Arc<dyn EventSink>,
) {
    eprintln!(
        "[orchestrator] recover_waiting_review: checking reviewer completion for task={}",
        node.id
    );

    // Query expected vs. answered reviewer nodes.
    let (expected_ids, answered_ids) = {
        let reg = registry.lock().unwrap();
        let db = match reg.get(project_id) {
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
        handle_review_yield(node, project_id, registry, dag_tx, agent_map, Arc::clone(&sink)).await;
        return;
    }

    if expected_sorted == answered_sorted {
        // All reviewers finished before the restart — trigger SF-3 immediately.
        eprintln!(
            "[orchestrator] recover_waiting_review: task={} — all {} reviewers already done, resuming immediately",
            node.id,
            expected_sorted.len()
        );
        check_review_completion(&node.id, project_id, registry, dag_tx, agent_map, sink).await;
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
