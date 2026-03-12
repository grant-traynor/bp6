pub mod commands;

use crate::agent::text_extractor::TextBufExtractor;
use crate::agent::transport::{JsonStreamTransport, StreamCallbacks};
use crate::dag_store::{self, ProjectRegistry, UpdateNodeInput};
use crate::event_ingester::{self, DagChanged};
use crate::event_sink::EventSink;
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
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
    agent_map: &AgentMap,
    sink: Arc<dyn EventSink>,
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
            ..Default::default()
        };
        dag_store::db_update_node(&conn, &req.task_id, &update)?;

        // Write skill_modes to nodes.skill_modes at SF-1 dispatch time (Phase 3).
        // Load the skill to get its declared modes, then serialize as JSON array.
        let resource_dir = sink.resource_dir();
        if let Ok(skill) = crate::skills::load_skill(&req.skill_id, &req.project_path, &resource_dir) {
            if let Ok(modes_json) = serde_json::to_string(&skill.modes) {
                let _ = dag_store::db_update_node_skill_modes(&conn, &req.task_id, &modes_json);
            }
        }

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
    sink.emit(
        "poe-agent-started",
        &serde_json::json!({
            "agentId": agent_id,
            "taskId": req.task_id,
            "projectId": req.project_id,
            "skillId": req.skill_id,
            "model": req.model,
        }),
    );

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
    let sink_clone = sink.clone();
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

        // Persistent yield_reason tracker — tracks the last substantive poe: event
        // emitted before poe:yield across all lines of the agent's output.
        // Must survive across on_text_chunk calls (each call may carry only part of
        // the output; poe:chat and poe:yield arrive in different chunks).
        let last_substantive: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let last_sub_for_chunk = last_substantive.clone();
        let last_sub_for_complete = last_substantive.clone();

        // Clone string keys for each closure independently.
        let project_id_for_session = project_id.clone();
        let project_id_for_chunk = project_id.clone();
        let project_id_for_raw = project_id.clone();
        let project_id_for_complete = project_id.clone();

        let task_id_for_chunk = task_id.clone();
        let task_id_for_raw = task_id.clone();
        let task_id_for_complete = task_id.clone();

        let agent_id_for_pid = agent_id_clone.clone();
        let agent_id_for_session = agent_id_clone.clone();
        let agent_id_for_chunk = agent_id_clone.clone();
        let agent_id_for_raw = agent_id_clone.clone();
        let agent_id_for_complete = agent_id_clone.clone();

        let agent_map_for_pid = agent_map_clone.clone();
        let agent_map_for_session = agent_map_clone.clone();

        let registry_for_session = registry_clone.clone();
        let registry_for_chunk = registry_clone.clone();
        let registry_for_complete = registry_clone.clone();

        let dag_tx_for_chunk = dag_tx_clone.clone();
        let dag_tx_for_complete = dag_tx_clone.clone();

        let sink_for_chunk = sink_clone.clone();
        let sink_for_raw = sink_clone.clone();
        let sink_for_complete = sink_clone.clone();

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
                    // Extract Arc<ProjectDb> from the registry, then drop the registry
                    // lock before acquiring db.conn — avoids registry → conn nested-lock
                    // deadlock potential.
                    let db = {
                        let reg = registry_for_session.lock().unwrap();
                        reg.values()
                            .find(|db| db.project.id == project_id_for_session)
                            .cloned()
                    };
                    if let Some(db) = db {
                        let conn = db.conn.lock().unwrap();
                        // Write to agents.session_id (legacy).
                        if let Err(e) = dag_store::db_update_agent_session(
                            &conn,
                            &agent_id_for_session,
                            &sid,
                        ) {
                            eprintln!(
                                "[agent_lifecycle] agent={} failed to store agents.session_id: {}",
                                agent_id_for_session, e
                            );
                        }
                        // Write to nodes.session_id (canonical per Protocol.md §1).
                        let task_id_for_node_session = {
                            let map = agent_map_for_session.lock().unwrap();
                            map.get(&agent_id_for_session)
                                .map(|a| a.task_id.clone())
                        };
                        if let Some(ref tid) = task_id_for_node_session {
                            if let Err(e) = dag_store::db_update_node_session(&conn, tid, &sid) {
                                eprintln!(
                                    "[agent_lifecycle] agent={} failed to store nodes.session_id: {}",
                                    agent_id_for_session, e
                                );
                            }
                        }
                    }
                }),
                on_text_chunk: Box::new(move |text| {
                    let mut ex = extractor_for_chunk.lock().unwrap();
                    let lines = ex.push(&text);
                    drop(ex); // release lock before ingesting
                    let mut last_sub = last_sub_for_chunk.lock().unwrap();
                    for line in lines {
                        event_ingester::ingest_line_with_tracker(
                            &line,
                            &project_id_for_chunk,
                            &task_id_for_chunk,
                            &agent_id_for_chunk,
                            &registry_for_chunk,
                            &dag_tx_for_chunk,
                            sink_for_chunk.as_ref(),
                            &project_path_for_chunk,
                            &mut *last_sub,
                        );
                    }
                }),
                on_raw_json: Box::new(move |json| {
                    eprintln!("[transport] raw: {}", json);
                    sink_for_raw.emit(
                        "poe-agent-stream",
                        &serde_json::json!({
                            "agentId": agent_id_for_raw,
                            "taskId": task_id_for_raw,
                            "projectId": project_id_for_raw,
                            "event": json,
                        }),
                    );
                }),
                on_complete: Box::new(move || {
                    // Flush TextBufExtractor tail — last poe: event may lack a trailing newline.
                    let tail = extractor_for_complete.lock().unwrap().flush();
                    if let Some(tail) = tail {
                        if !tail.trim().is_empty() {
                            let mut last_sub = last_sub_for_complete.lock().unwrap();
                            event_ingester::ingest_line_with_tracker(
                                &tail,
                                &project_id_for_complete,
                                &task_id_for_complete,
                                &agent_id_for_complete,
                                &registry_for_complete,
                                &dag_tx_for_complete,
                                sink_for_complete.as_ref(),
                                &project_path_for_complete,
                                &mut *last_sub,
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
        // Waiting (yielded) is also a clean exit — the agent paused intentionally.
        let task_complete = {
            let reg = registry_clone.lock().unwrap();
            if let Some(db) = reg.get(&project_id) {
                let conn = db.conn.lock().unwrap();
                if let Ok(node) = dag_store::db_get_node(&conn, &task_id) {
                    let agent_status = match node.status {
                        dag_store::NodeStatus::Complete => "complete",
                        dag_store::NodeStatus::Waiting => "yielded",
                        dag_store::NodeStatus::Running => {
                            // Crashed — poe:done and poe:yield never received.
                            // Re-queue to Pending so the orchestrator can retry.
                            let update = UpdateNodeInput {
                                status: Some(dag_store::NodeStatus::Pending),
                                title: None,
                                description: None,
                                skill_id: None,
                                assignee: None,
                                ..Default::default()
                            };
                            let _ = dag_store::db_update_node(&conn, &task_id, &update);
                            "failed"
                        }
                        _ => "failed",
                    };
                    let _ = dag_store::db_end_agent(&conn, &agent_id_clone, agent_status);
                    // Success = completed normally OR paused via poe:yield.
                    matches!(agent_status, "complete" | "yielded")
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

        sink_clone.emit(
            "poe-agent-exited",
            &serde_json::json!({
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
///
/// Uses `nix::sys::signal::kill` directly — no `kill` subprocess, no PID-reuse
/// race, errors are surfaced and logged.
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
            let nix_pid = nix::unistd::Pid::from_raw(pid as i32);
            if let Err(e) = nix::sys::signal::kill(nix_pid, nix::sys::signal::Signal::SIGTERM) {
                let msg = format!(
                    "interrupt_agent_process: kill(pid={}, SIGTERM) failed: {}",
                    pid, e
                );
                eprintln!("[agent_lifecycle] {}", msg);
                return Err(anyhow::anyhow!(msg));
            }
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

    // Wrap in Arc<Mutex<>> so both on_text_chunk and on_complete can call the observer.
    let on_line: Arc<Mutex<Option<Box<dyn Fn(String) + Send + 'static>>>> =
        Arc::new(Mutex::new(on_line));
    let on_line_for_complete = on_line.clone();

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
                    if let Some(ref cb) = *on_line.lock().unwrap() {
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
                        // Also fire on_line for the tail so observers see it.
                        if let Some(ref cb) = *on_line_for_complete.lock().unwrap() {
                            cb(t.clone());
                        }
                        lines_for_complete.lock().unwrap().push(t);
                    }
                }
            }),
        },
    )?;

    let result = lines.lock().unwrap().clone();
    Ok(result)
}
