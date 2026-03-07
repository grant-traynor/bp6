pub mod commands;

use crate::dag_store::{self, ProjectRegistry, UpdateNodeInput};
use crate::event_ingester::{self, DagChanged};
use anyhow::{Context, Result};
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::sync::Mutex;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::mpsc;

// ── Active agent state ────────────────────────────────────────────────────────

pub struct ActiveAgent {
    pub agent_id: String,
    pub task_id: String,
    pub project_id: String,
    /// PTY writer for sending text to the agent's stdin.
    pub writer: Mutex<Box<dyn Write + Send>>,
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
}

// ── Spawn ─────────────────────────────────────────────────────────────────────

/// Spawn a Claude Code agent for the given task.
///
/// 1. Creates PTY process (claude code or claude --resume <session-id>)
/// 2. Writes the input bundle to its stdin
/// 3. Starts a watchdog thread that reads PTY output, feeds the event ingester,
///    and marks the task complete/failed when the agent exits
pub async fn spawn_agent(
    req: SpawnRequest,
    registry: &ProjectRegistry,
    dag_tx: &mpsc::UnboundedSender<DagChanged>,
    app: &AppHandle,
) -> Result<String> {
    // Mark task as running in DB and create agent record
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
            title: None, description: None, skill_id: None, assignee: None,
        };
        dag_store::db_update_node(&conn, &req.task_id, &update)?;
        let record = dag_store::db_create_agent(
            &conn,
            &req.project_id,
            &req.skill_id,
            &req.task_id,
            None, // session_id populated after spawn
        )?;
        record.id
    };

    // Build PTY command
    let pty_system = NativePtySystem::default();
    let pair = pty_system
        .openpty(PtySize {
            rows: 50,
            cols: 220,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("Failed to open PTY")?;

    let mut cmd = CommandBuilder::new("claude");
    if let Some(ref session_id) = req.resume_session_id {
        cmd.arg("--resume");
        cmd.arg(session_id);
    }
    // Protocol.md §5: no -p, no CLI bundle arg. Bundle is written to stdin after spawn.
    cmd.arg("--dangerously-skip-permissions");
    cmd.cwd(&req.project_path);

    eprintln!(
        "[agent_lifecycle] spawning agent={} task={} cwd={} resume={:?}",
        agent_id,
        req.task_id,
        req.project_path.display(),
        req.resume_session_id,
    );

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .context("Failed to spawn claude process")?;

    eprintln!("[agent_lifecycle] agent={} process spawned, writing bundle ({} bytes)", agent_id, req.input_bundle.len());

    let writer = pair.master.take_writer().context("Failed to get PTY writer")?;
    let reader = pair.master.try_clone_reader().context("Failed to get PTY reader")?;

    // Register in AgentMap before writing the bundle. The writer must be in AgentMap
    // (and the watchdog must be reading) before we write — otherwise the PTY output
    // buffer fills with Claude's startup banner and write_all deadlocks.
    let agent_map = app.state::<AgentMap>().inner().clone();
    {
        let active = Arc::new(ActiveAgent {
            agent_id: agent_id.clone(),
            task_id: req.task_id.clone(),
            project_id: req.project_id.clone(),
            writer: Mutex::new(writer),
        });
        agent_map.lock().unwrap().insert(agent_id.clone(), active);
    }

    // Emit agent started event
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

    // Clone handles for the watchdog thread
    let registry_clone = registry.clone();
    let dag_tx_clone = dag_tx.clone();
    let app_clone = app.clone();
    let project_id = req.project_id.clone();
    let task_id = req.task_id.clone();
    let agent_id_clone = agent_id.clone();
    let project_path = req.project_path.clone();

    // Spawn the watchdog BEFORE writing the bundle. The watchdog drains PTY stdout;
    // without it running first, write_all will block once the PTY buffer fills.
    std::thread::spawn(move || {
        eprintln!("[agent_lifecycle] agent={} watchdog thread started", agent_id_clone);
        let buf_reader = BufReader::new(reader);
        let mut session_captured = false;
        for line in buf_reader.lines() {
            match line {
                Ok(line) => {
                    eprintln!("[agent_lifecycle] agent={} PTY> {}", agent_id_clone, line);

                    // Capture session ID from Claude's startup banner (Protocol.md §5)
                    if !session_captured {
                        if let Some(sid) = line.strip_prefix("Session ID: ") {
                            let sid = sid.trim().to_owned();
                            eprintln!("[agent_lifecycle] agent={} session_id captured: {}", agent_id_clone, sid);
                            let reg = registry_clone.lock().unwrap();
                            if let Some(db) = reg.values().find(|db| db.project.id == project_id) {
                                let conn = db.conn.lock().unwrap();
                                if let Err(e) = dag_store::db_update_agent_session(&conn, &agent_id_clone, &sid) {
                                    eprintln!("[agent_lifecycle] agent={} failed to store session_id: {}", agent_id_clone, e);
                                }
                            }
                            session_captured = true;
                        }
                    }
                    // Feed to event ingester
                    event_ingester::ingest_line(
                        &line,
                        &project_id,
                        &task_id,
                        &agent_id_clone,
                        &registry_clone,
                        &dag_tx_clone,
                        &app_clone,
                        &project_path,
                    );
                    // Also emit raw PTY line for drill-down view
                    use tauri::Emitter;
                    let _ = app_clone.emit(
                        "poe-pty-line",
                        serde_json::json!({
                            "agentId": agent_id_clone,
                            "taskId": task_id,
                            "projectId": project_id,
                            "line": line,
                        }),
                    );
                }
                Err(e) => {
                    eprintln!("[agent_lifecycle] PTY read error for agent {}: {}", agent_id_clone, e);
                    break;
                }
            }
        }

        eprintln!("[agent_lifecycle] agent={} PTY reader EOF — waiting for process exit", agent_id_clone);
        if !session_captured {
            eprintln!("[agent_lifecycle] agent={} WARNING: session_id never captured (no 'Session ID: ' line seen)", agent_id_clone);
        }

        // Agent exited — wait for process
        let exit_status = child.wait();
        eprintln!("[agent_lifecycle] agent={} process exited: {:?}", agent_id_clone, exit_status);

        // Remove from AgentMap (also drops the writer, which is now safe since the process exited)
        {
            let agent_map = app_clone.state::<AgentMap>().inner().clone();
            agent_map.lock().unwrap().remove(&agent_id_clone);
        }

        // Determine success by node status — poe:done sets it to Complete regardless of exit code.
        // Claude's exit code is unreliable; use SQLite node status as the authority (Protocol.md §5).
        let task_complete = {
            let reg = registry_clone.lock().unwrap();
            if let Some(db) = reg.values().find(|db| db.project.id == project_id) {
                let conn = db.conn.lock().unwrap();
                let _ = dag_store::db_end_agent(&conn, &agent_id_clone,
                    if dag_store::db_get_node(&conn, &task_id)
                        .map(|n| n.status == dag_store::NodeStatus::Complete)
                        .unwrap_or(false) { "complete" } else { "failed" });
                // Re-queue if still running (poe:done was never emitted)
                if let Ok(node) = dag_store::db_get_node(&conn, &task_id) {
                    if node.status == dag_store::NodeStatus::Running {
                        let update = UpdateNodeInput {
                            status: Some(dag_store::NodeStatus::Pending),
                            title: None, description: None, skill_id: None, assignee: None,
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

        eprintln!("[agent_lifecycle] agent={} task_complete={}", agent_id_clone, task_complete);

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

        // Notify orchestrator — agent exit may unblock dependents
        let _ = dag_tx_clone.send(DagChanged::NodeStatusChanged {
            project_id: project_id.clone(),
            node_id: task_id.clone(),
        });
    });

    // Write T+S+K bundle AFTER the watchdog thread is running, so the reader is
    // already draining PTY stdout before we fill PTY stdin. (Protocol.md §5)
    {
        let map = agent_map.lock().unwrap();
        let active = map.get(&agent_id).ok_or_else(|| anyhow::anyhow!("Agent vanished from map immediately after insert"))?;
        let mut writer = active.writer.lock().unwrap();
        writer
            .write_all(req.input_bundle.as_bytes())
            .context("Failed to write input bundle to agent stdin")?;
        // PTY raw mode: Enter = \r (carriage return), not \n. \n creates a newline
        // inside the input box; \r submits the message to Claude.
        writer
            .write_all(b"\r")
            .context("Failed to write submit keystroke to agent stdin")?;
        writer.flush().context("Failed to flush input bundle")?;
    }
    eprintln!("[agent_lifecycle] agent={} bundle flushed to stdin", agent_id);

    Ok(agent_id)
}

// ── Write to agent (context injection) ───────────────────────────────────────

/// Write text into a running agent's PTY stdin.
/// Used by the orchestrator to inject reviewer artifacts into blocked agents.
pub fn write_to_agent_stdin(agent_map: &AgentMap, agent_id: &str, content: &str) -> Result<()> {
    let map = agent_map.lock().unwrap();
    let agent = map
        .get(agent_id)
        .ok_or_else(|| anyhow::anyhow!("Agent not found: {}", agent_id))?;
    let mut writer = agent.writer.lock().unwrap();
    writer.write_all(content.as_bytes()).context("Failed to write to agent stdin")?;
    writer.write_all(b"\n").context("Failed to write newline to agent stdin")?;
    writer.flush().context("Failed to flush agent stdin")?;
    Ok(())
}

/// Interrupt (kill) a running agent process.
pub fn interrupt_agent_process(agent_map: &AgentMap, agent_id: &str) -> Result<()> {
    // Sending SIGTERM via PTY: write Ctrl-C to stdin
    let map = agent_map.lock().unwrap();
    let agent = map
        .get(agent_id)
        .ok_or_else(|| anyhow::anyhow!("Agent not found: {}", agent_id))?;
    let mut writer = agent.writer.lock().unwrap();
    writer.write_all(&[3u8]).context("Failed to send interrupt to agent")?; // ASCII ETX = Ctrl-C
    writer.flush().context("Failed to flush interrupt")?;
    Ok(())
}
