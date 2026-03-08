pub mod commands;

use crate::agent_lifecycle::{self, SpawnRequest};
use crate::dag_store::{self, Node, ProjectRegistry};
use crate::event_ingester::DagChanged;
use crate::skills;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;

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
    loop {
        let signal = match dag_rx.recv().await {
            Some(s) => s,
            None => break,
        };

        // Drain all pending signals — categorise into resume requests and DAG changes
        let mut resume_requests: Vec<(String, String, String, String)> = Vec::new(); // (project_id, task_id, session_id, resolution)
        let mut project_ids = std::collections::HashSet::new();

        // Process a single signal inline (closure not used due to borrow rules)
        match signal {
            DagChanged::QueueItemResolved { project_id, task_id, session_id, resolution, .. } => {
                resume_requests.push((project_id, task_id, session_id, resolution));
            }
            other => {
                project_ids.insert(signal_project_id(&other).to_owned());
            }
        }

        while let Ok(s) = dag_rx.try_recv() {
            match s {
                DagChanged::QueueItemResolved { project_id, task_id, session_id, resolution, .. } => {
                    resume_requests.push((project_id, task_id, session_id, resolution));
                }
                other => {
                    project_ids.insert(signal_project_id(&other).to_owned());
                }
            }
        }

        let registry = app.state::<ProjectRegistry>().inner().clone();
        let limits = app.state::<Arc<ConcurrencyLimits>>().inner().clone();
        let dag_tx = app.state::<mpsc::UnboundedSender<DagChanged>>().inner().clone();

        // Handle resume continuations for resolved decisions
        for (project_id, task_id, session_id, resolution) in resume_requests {
            resume_waiting_agent(&project_id, &task_id, &session_id, &resolution, &registry, &dag_tx, &app).await;
        }

        // Run normal scheduling loop for DAG changes
        for project_id in project_ids {
            run_loop(&project_id, &registry, &limits, &dag_tx, &app).await;
        }
    }
}

fn signal_project_id(signal: &DagChanged) -> &str {
    match signal {
        DagChanged::NodeStatusChanged { project_id, .. } => project_id,
        DagChanged::DagStructureChanged { project_id } => project_id,
        DagChanged::QueueItemResolved { project_id, .. } => project_id,
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

// ── Core loop ─────────────────────────────────────────────────────────────────

async fn run_loop(
    project_id: &str,
    registry: &ProjectRegistry,
    limits: &Arc<ConcurrencyLimits>,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) {
    let ready_tasks = {
        let reg = registry.lock().unwrap();
        let db = match reg.values().find(|db| db.project.id == project_id) {
            Some(db) => db.clone(),
            None => return,
        };
        drop(reg);

        let conn = db.conn.lock().unwrap();

        let global_running = dag_store::db_count_running_agents_global(&conn).unwrap_or(0);
        let global_limit = limits.global_limit.load(Ordering::Relaxed);
        if global_running >= global_limit {
            return;
        }

        let project_running = dag_store::db_count_running_agents(&conn, project_id).unwrap_or(0);
        let project_limit = limits.get_project_limit(project_id);
        if project_running >= project_limit {
            return;
        }

        let slots = (project_limit - project_running).min(global_limit - global_running);

        match dag_store::db_find_ready_tasks(&conn, project_id) {
            Ok(tasks) => tasks.into_iter().take(slots).collect::<Vec<_>>(),
            Err(e) => {
                eprintln!("[orchestrator] Failed to find ready tasks: {}", e);
                return;
            }
        }
    };

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
        Ok(s) => s,
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
    let (interrupted, project_path) = {
        let reg = registry.lock().unwrap();
        let db = match reg.values().find(|db| db.project.id == project_id) {
            Some(db) => db.clone(),
            None => return,
        };
        drop(reg);
        let conn = db.conn.lock().unwrap();
        let agents = dag_store::db_list_agents_by_status(&conn, project_id, "running")
            .unwrap_or_default();
        let path = PathBuf::from(&db.project.path);
        (agents, path)
    };

    let resource_dir = app
        .path()
        .resource_dir()
        .unwrap_or_else(|_| PathBuf::from("."));

    for agent in interrupted {
        if let Some(ref session_id) = agent.session_id {
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
                        };
                        let _ = dag_store::db_update_node(&conn, &agent.task_id, &update);
                        let _ = dag_store::db_end_agent(&conn, &agent.id, "failed");
                    }
                }
            }
        }
    }
}
