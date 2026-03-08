pub mod commands;

use crate::agent::text_extractor::TextBufExtractor;
use crate::agent::transport::{JsonStreamTransport, StreamCallbacks};
use crate::dag_store::{self, ProjectRegistry, UpdateNodeInput};
use crate::event_ingester::{self, DagChanged};
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;

// ── Active agent state ────────────────────────────────────────────────────────

pub struct ActiveAgent {
    pub agent_id: String,
    pub task_id: String,
    pub project_id: String,
    /// OS process id — used by interrupt_agent_process to send SIGTERM.
    pub pid: Mutex<Option<u32>>,
}

/// Global map of running agents: agent_id → ActiveAgent.
pub type AgentMap = Arc<Mutex<HashMap<String, Arc<ActiveAgent>>>>;

pub fn new_agent_map() -> AgentMap {
    Arc::new(Mutex::new(HashMap::new()))
}

// ── Spawn request ─────────────────────────────────────────────────────────────

pub struct SpawnRequest {
    pub project_id: String,
    pub task_id: String,
    pub skill_id: String,
    pub project_path: PathBuf,
    pub input_bundle: String,
    pub resume_session_id: Option<String>,
    /// Model override from skill frontmatter. When `Some`, passes `--model` to claude.
    /// When `None`, claude uses its configured default.
    pub model: Option<String>,
}

// ── Spawn ─────────────────────────────────────────────────────────────────────

/// Spawn a Claude Code agent for the given task via stream-json transport.
///
/// 1. Marks task as running in DB and creates an agent record.
/// 2. Registers the agent in AgentMap.
/// 3. Spawns a thread that calls JsonStreamTransport::run(), feeding output
///    through TextBufExtractor → event_ingester::ingest_line().
/// 4. On exit: marks task complete/failed, emits poe-agent-exited, notifies orchestrator.
pub async fn spawn_agent(
    req: SpawnRequest,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) -> Result<String> {
    // Mark task as running in DB and create agent record.
    let agent_id = {
        let reg = registry.lock().unwrap();
        let db = reg
            .values()
            .find(|db| db.project.id == req.project_id)
            .ok_or_else(|| anyhow::anyhow!("Project not open: {}", req.project_id))?
            .clone();
        drop(reg);

        let conn = db.conn.lock().unwrap();
        let update = UpdateNodeInput {
            status: Some(dag_store::NodeStatus::Running),
            title: None,
            description: None,
            skill_id: None,
            assignee: None,
        };
        dag_store::db_update_node(&conn, &req.task_id, &update)?;
        let record = dag_store::db_create_agent(
            &conn,
            &req.project_id,
            &req.skill_id,
            &req.task_id,
            None, // session_id populated via on_session_id callback
        )?;
        record.id
    };

    // Register in AgentMap.
    let agent_map = app.state::<AgentMap>().inner().clone();
    {
        let active = Arc::new(ActiveAgent {
            agent_id: agent_id.clone(),
            task_id: req.task_id.clone(),
            project_id: req.project_id.clone(),
            pid: Mutex::new(None),
        });
        agent_map.lock().unwrap().insert(agent_id.clone(), active);
    }

    // Emit agent started event.
    {
        use tauri::Emitter;
        let _ = app.emit(
            "poe-agent-started",
            serde_json::json!({
                "agentId": agent_id,
                "taskId": req.task_id,
                "projectId": req.project_id,
                "skillId": req.skill_id,
            }),
        );
    }

    eprintln!(
        "[agent_lifecycle] spawning agent={} task={} cwd={} resume={:?}",
        agent_id,
        req.task_id,
        req.project_path.display(),
        req.resume_session_id,
    );

    // Clone everything needed by the transport thread / callbacks.
    let registry_clone = registry.clone();
    let dag_tx_clone = dag_tx.clone();
    let app_clone = app.clone();
    let project_id = req.project_id.clone();
    let task_id = req.task_id.clone();
    let agent_id_clone = agent_id.clone();
    let project_path = req.project_path.clone();
    let input_bundle = req.input_bundle.clone();
    let resume_session_id = req.resume_session_id.clone();
    let model = req.model.clone();
    let agent_map_clone = agent_map.clone();

    std::thread::spawn(move || {
        eprintln!(
            "[agent_lifecycle] agent={} transport thread started",
            agent_id_clone
        );

        // Shared text accumulator — used by on_text_chunk and on_complete.
        let extractor = Arc::new(Mutex::new(TextBufExtractor::new()));
        let extractor_for_chunk = extractor.clone();
        let extractor_for_complete = extractor.clone();

        // Clone string keys for each closure independently.
        let project_id_for_session = project_id.clone();
        let project_id_for_chunk = project_id.clone();
        let project_id_for_complete = project_id.clone();

        let task_id_for_chunk = task_id.clone();
        let task_id_for_complete = task_id.clone();

        let agent_id_for_pid = agent_id_clone.clone();
        let agent_id_for_session = agent_id_clone.clone();
        let agent_id_for_chunk = agent_id_clone.clone();
        let agent_id_for_complete = agent_id_clone.clone();

        let agent_map_for_pid = agent_map_clone.clone();

        let registry_for_session = registry_clone.clone();
        let registry_for_chunk = registry_clone.clone();
        let registry_for_complete = registry_clone.clone();

        let dag_tx_for_chunk = dag_tx_clone.clone();
        let dag_tx_for_complete = dag_tx_clone.clone();

        let app_for_chunk = app_clone.clone();
        let app_for_complete = app_clone.clone();

        let project_path_for_chunk = project_path.clone();
        let project_path_for_complete = project_path.clone();

        let result = JsonStreamTransport::run(
            &input_bundle,
            resume_session_id.as_deref(),
            model.as_deref(),
            &project_path,
            StreamCallbacks {
                on_pid: Some(Box::new(move |pid| {
                    eprintln!(
                        "[agent_lifecycle] agent={} pid={}",
                        agent_id_for_pid, pid
                    );
                    let map = agent_map_for_pid.lock().unwrap();
                    if let Some(agent) = map.get(&agent_id_for_pid) {
                        *agent.pid.lock().unwrap() = Some(pid);
                    }
                })),
                on_session_id: Box::new(move |sid| {
                    eprintln!(
                        "[agent_lifecycle] agent={} session_id={}",
                        agent_id_for_session, sid
                    );
                    let reg = registry_for_session.lock().unwrap();
                    if let Some(db) = reg
                        .values()
                        .find(|db| db.project.id == project_id_for_session)
                    {
                        let conn = db.conn.lock().unwrap();
                        if let Err(e) = dag_store::db_update_agent_session(
                            &conn,
                            &agent_id_for_session,
                            &sid,
                        ) {
                            eprintln!(
                                "[agent_lifecycle] agent={} failed to store session_id: {}",
                                agent_id_for_session, e
                            );
                        }
                    }
                }),
                on_text_chunk: Box::new(move |text| {
                    let mut ex = extractor_for_chunk.lock().unwrap();
                    let lines = ex.push(&text);
                    drop(ex); // release lock before ingesting
                    for line in lines {
                        use tauri::Emitter;
                        let _ = app_for_chunk.emit(
                            "poe-pty-line",
                            serde_json::json!({
                                "agentId": agent_id_for_chunk,
                                "taskId": task_id_for_chunk,
                                "projectId": project_id_for_chunk,
                                "line": line,
                            }),
                        );
                        event_ingester::ingest_line(
                            &line,
                            &project_id_for_chunk,
                            &task_id_for_chunk,
                            &agent_id_for_chunk,
                            &registry_for_chunk,
                            &dag_tx_for_chunk,
                            &app_for_chunk,
                            &project_path_for_chunk,
                        );
                    }
                }),
                on_raw_json: Box::new(move |json| {
                    eprintln!("[transport] raw: {}", json);
                }),
                on_complete: Box::new(move || {
                    // Flush TextBufExtractor tail — last poe: event may lack a trailing newline.
                    let tail = extractor_for_complete.lock().unwrap().flush();
                    if let Some(tail) = tail {
                        if !tail.trim().is_empty() {
                            use tauri::Emitter;
                            let _ = app_for_complete.emit(
                                "poe-pty-line",
                                serde_json::json!({
                                    "agentId": agent_id_for_complete,
                                    "taskId": task_id_for_complete,
                                    "projectId": project_id_for_complete,
                                    "line": tail,
                                }),
                            );
                            event_ingester::ingest_line(
                                &tail,
                                &project_id_for_complete,
                                &task_id_for_complete,
                                &agent_id_for_complete,
                                &registry_for_complete,
                                &dag_tx_for_complete,
                                &app_for_complete,
                                &project_path_for_complete,
                            );
                        }
                    }
                }),
            },
        );

        if let Err(e) = result {
            eprintln!(
                "[agent_lifecycle] agent={} transport error: {}",
                agent_id_clone, e
            );
        }

        eprintln!(
            "[agent_lifecycle] agent={} process exited",
            agent_id_clone
        );

        // Remove from AgentMap.
        agent_map_clone.lock().unwrap().remove(&agent_id_clone);

        // Determine success by node status — poe:done sets it to Complete.
        // Claude's exit code is unreliable; SQLite node status is the authority.
        let task_complete = {
            let reg = registry_clone.lock().unwrap();
            if let Some(db) = reg.values().find(|db| db.project.id == project_id) {
                let conn = db.conn.lock().unwrap();
                let _ = dag_store::db_end_agent(
                    &conn,
                    &agent_id_clone,
                    if dag_store::db_get_node(&conn, &task_id)
                        .map(|n| n.status == dag_store::NodeStatus::Complete)
                        .unwrap_or(false)
                    {
                        "complete"
                    } else {
                        "failed"
                    },
                );
                if let Ok(node) = dag_store::db_get_node(&conn, &task_id) {
                    if node.status == dag_store::NodeStatus::Running {
                        // poe:done never received — re-queue to Pending.
                        let update = UpdateNodeInput {
                            status: Some(dag_store::NodeStatus::Pending),
                            title: None,
                            description: None,
                            skill_id: None,
                            assignee: None,
                        };
                        let _ = dag_store::db_update_node(&conn, &task_id, &update);
                        false
                    } else {
                        node.status == dag_store::NodeStatus::Complete
                    }
                } else {
                    false
                }
            } else {
                false
            }
        };

        eprintln!(
            "[agent_lifecycle] agent={} task_complete={}",
            agent_id_clone, task_complete
        );

        use tauri::Emitter;
        let _ = app_clone.emit(
            "poe-agent-exited",
            serde_json::json!({
                "agentId": agent_id_clone,
                "taskId": task_id,
                "projectId": project_id,
                "success": task_complete,
            }),
        );

        // Notify orchestrator — exit may unblock dependents.
        let _ = dag_tx_clone.send(DagChanged::NodeStatusChanged {
            project_id: project_id.clone(),
            node_id: task_id.clone(),
        });
    });

    Ok(agent_id)
}

// ── Agent control ─────────────────────────────────────────────────────────────

/// Write text to a running agent's stdin.
///
/// Not supported for stream-json agents — stdin is closed after bundle write.
/// Decision/review continuations use `spawn_agent` with `resume_session_id`.
pub fn write_to_agent_stdin(
    _agent_map: &AgentMap,
    agent_id: &str,
    _content: &str,
) -> Result<()> {
    Err(anyhow::anyhow!(
        "write_to_agent_stdin is not supported for stream-json agents ({}). \
         Use spawn_agent with resume_session_id for decision/review continuations.",
        agent_id
    ))
}

/// Interrupt a running agent via SIGTERM.
pub fn interrupt_agent_process(agent_map: &AgentMap, agent_id: &str) -> Result<()> {
    let pid_opt = {
        let map = agent_map.lock().unwrap();
        let agent = map
            .get(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent not found: {}", agent_id))?;
        let x = *agent.pid.lock().unwrap(); x
    };
    match pid_opt {
        Some(pid) => {
            let _ = std::process::Command::new("kill")
                .arg(pid.to_string())
                .status();
            Ok(())
        }
        None => Err(anyhow::anyhow!(
            "Agent {} has no pid yet (process not yet spawned)",
            agent_id
        )),
    }
}

// ── Protocol-level raw runner (used by integration tests) ─────────────────────

/// Spawn a Claude process, write a bundle to its stdin, and collect all
/// assistant text lines emitted before the process exits.
///
/// Uses JsonStreamTransport — no PTY, no trust dialog, no SIGTERM required
/// (stream-json exits cleanly after the result event).
///
/// `on_line` is called for each complete text line as it arrives.
/// Returns all complete lines extracted from assistant content.
///
/// CI-safe: returns `Err` if the `claude` binary is not found or spawning fails.
pub fn run_agent_capturing(
    bundle: &str,
    cwd: &Path,
    on_line: Option<Box<dyn Fn(String) + Send + 'static>>,
) -> Result<Vec<String>> {
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    let lines_for_chunk = lines.clone();
    let lines_for_complete = lines.clone();

    let extractor = Arc::new(Mutex::new(TextBufExtractor::new()));
    let extractor_for_chunk = extractor.clone();
    let extractor_for_complete = extractor.clone();

    JsonStreamTransport::run(
        bundle,
        None,
        None,
        cwd,
        StreamCallbacks {
            on_pid: None,
            on_session_id: Box::new(|_| {}),
            on_text_chunk: Box::new(move |text| {
                let mut ex = extractor_for_chunk.lock().unwrap();
                let complete = ex.push(&text);
                drop(ex);
                for line in complete {
                    if let Some(ref cb) = on_line {
                        cb(line.clone());
                    }
                    lines_for_chunk.lock().unwrap().push(line);
                }
            }),
            on_raw_json: Box::new(|_| {}),
            on_complete: Box::new(move || {
                let tail = extractor_for_complete.lock().unwrap().flush();
                if let Some(t) = tail {
                    if !t.is_empty() {
                        lines_for_complete.lock().unwrap().push(t);
                    }
                }
            }),
        },
    )?;

    let result = lines.lock().unwrap().clone();
    Ok(result)
}
