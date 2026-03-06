// bp6-2a5.1: Agent decision protocol
//
// Manages agent processes spawned via PTY. Intercepts structured poe:decision
// JSON-lines from stdout, creates queue items in SQLite, and forwards
// resolutions to agent stdin.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use crate::dag::{NewQueueItem, QueueItemOption};
use crate::project::{ProjectState, QueueItemAddedEvent};

// ── AgentHandle ────────────────────────────────────────────────────────────────

/// A handle to a running agent process. Dropping this closes the PTY master
/// and terminates the child process.
pub struct AgentHandle {
    /// Shared writer to the PTY master (agent's stdin).
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl AgentHandle {
    /// Write text into the agent's stdin via the PTY master.
    pub fn write_text(&self, text: &str) -> Result<(), String> {
        let mut w = self
            .writer
            .lock()
            .map_err(|e| format!("Failed to lock PTY writer: {}", e))?;
        w.write_all(text.as_bytes())
            .map_err(|e| format!("Failed to write to PTY: {}", e))?;
        w.flush().map_err(|e| format!("Failed to flush PTY: {}", e))?;
        Ok(())
    }
}

// ── AgentState ─────────────────────────────────────────────────────────────────

/// Tauri-managed state holding all live agent handles.
pub struct AgentState {
    handles: Mutex<HashMap<String, AgentHandle>>,
}

impl AgentState {
    pub fn new() -> Self {
        AgentState {
            handles: Mutex::new(HashMap::new()),
        }
    }
}

// ── Wire protocol structs ──────────────────────────────────────────────────────

/// JSON-line emitted by an agent on stdout when it requires a human decision.
#[derive(Debug, Deserialize)]
struct PoeDecision {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    msg_type: String,
    question: String,
    options: Vec<QueueItemOption>,
    context: Option<serde_json::Value>,
    priority: Option<i32>,
}

// ── Tauri event payloads ───────────────────────────────────────────────────────

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentOutputEvent {
    agent_id: String,
    line: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentExitedEvent {
    agent_id: String,
}

// ── Tauri command input/output types ──────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnAgentParams {
    pub cmd: String,
    pub args: Vec<String>,
    pub env: Option<HashMap<String, String>>,
    pub agent_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnedAgent {
    pub agent_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KillAgentParams {
    pub agent_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteToAgentParams {
    pub agent_id: String,
    pub text: String,
}

// ── Tauri commands ─────────────────────────────────────────────────────────────

/// Spawn an agent process inside a PTY.
///
/// The background reader thread:
/// - Parses lines with `"type": "poe:decision"` → creates a queue item in
///   SQLite and emits `queue:item:added`.
/// - Forwards all other lines as `agent:output` events.
/// - On EOF emits `agent:exited`.
#[tauri::command]
pub fn spawn_agent(
    params: SpawnAgentParams,
    app: AppHandle,
    agent_state: State<'_, AgentState>,
) -> Result<SpawnedAgent, String> {
    let agent_id = params
        .agent_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // ── Build PTY ──────────────────────────────────────────────────────────────
    let pty_system = NativePtySystem::default();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {}", e))?;

    // ── Build command ──────────────────────────────────────────────────────────
    let mut cmd = CommandBuilder::new(&params.cmd);
    for arg in &params.args {
        cmd.arg(arg);
    }
    if let Some(env_map) = params.env {
        for (k, v) in env_map {
            cmd.env(k, v);
        }
    }

    // ── Spawn child ────────────────────────────────────────────────────────────
    let _child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn agent '{}': {}", params.cmd, e))?;

    // ── PTY master reader/writer ───────────────────────────────────────────────
    let master_reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("Failed to clone PTY reader: {}", e))?;

    let master_writer: Box<dyn Write + Send> = pair
        .master
        .take_writer()
        .map_err(|e| format!("Failed to take PTY writer: {}", e))?;

    let writer = Arc::new(Mutex::new(master_writer));

    // ── Background stdout reader thread ───────────────────────────────────────
    {
        let agent_id_bg = agent_id.clone();
        // Capture AppHandle (cloneable) and use it to re-acquire state in thread.
        let app_for_thread = app.clone();

        std::thread::spawn(move || {
            let reader = BufReader::new(master_reader);
            for line_result in reader.lines() {
                let line = match line_result {
                    Ok(l) => l,
                    Err(_) => break,
                };

                // Try to parse as poe:decision
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                    if val.get("type").and_then(|t| t.as_str()) == Some("poe:decision") {
                        if let Ok(decision) = serde_json::from_value::<PoeDecision>(val) {
                            handle_poe_decision(
                                &app_for_thread,
                                &agent_id_bg,
                                decision,
                            );
                            continue;
                        }
                    }
                }

                // Non-decision line → forward to UI
                let _ = app_for_thread.emit(
                    "agent:output",
                    AgentOutputEvent {
                        agent_id: agent_id_bg.clone(),
                        line,
                    },
                );
            }

            // EOF
            let _ = app_for_thread.emit(
                "agent:exited",
                AgentExitedEvent {
                    agent_id: agent_id_bg.clone(),
                },
            );
        });
    }

    // ── Store handle ───────────────────────────────────────────────────────────
    let handle = AgentHandle { writer };

    {
        let mut map = agent_state
            .handles
            .lock()
            .map_err(|e| format!("Failed to lock AgentState: {}", e))?;
        map.insert(agent_id.clone(), handle);
    }

    Ok(SpawnedAgent { agent_id })
}

/// Kill a running agent by closing its PTY master (terminates the child).
#[tauri::command]
pub fn kill_agent(
    params: KillAgentParams,
    agent_state: State<'_, AgentState>,
) -> Result<(), String> {
    let mut map = agent_state
        .handles
        .lock()
        .map_err(|e| format!("Failed to lock AgentState: {}", e))?;

    if map.remove(&params.agent_id).is_none() {
        return Err(format!("Agent '{}' not found", params.agent_id));
    }

    // Dropping the AgentHandle closes writer → closes PTY master → SIGHUP child.
    Ok(())
}

/// Write arbitrary text to an agent's PTY stdin (used to send decision resolutions).
#[tauri::command]
pub fn write_to_agent(
    params: WriteToAgentParams,
    agent_state: State<'_, AgentState>,
) -> Result<(), String> {
    let map = agent_state
        .handles
        .lock()
        .map_err(|e| format!("Failed to lock AgentState: {}", e))?;

    let handle = map
        .get(&params.agent_id)
        .ok_or_else(|| format!("Agent '{}' not found", params.agent_id))?;

    handle.write_text(&params.text)
}

// ── Internal helpers ───────────────────────────────────────────────────────────

/// Process a parsed `poe:decision` message: persist to SQLite and emit event.
fn handle_poe_decision(app: &AppHandle, agent_id: &str, decision: PoeDecision) {
    // Re-acquire ProjectState from the AppHandle
    let project_state = app.state::<ProjectState>();

    let project_id: String = {
        match project_state.project_id.lock() {
            Ok(guard) => match guard.as_ref() {
                Some(id) => id.clone(),
                None => {
                    eprintln!("[agents] No project open — dropping poe:decision");
                    return;
                }
            },
            Err(e) => {
                eprintln!("[agents] Failed to lock project_id: {}", e);
                return;
            }
        }
    };

    let context_snapshot = decision.context.unwrap_or(serde_json::json!({}));
    let priority = decision.priority.unwrap_or(2);

    let item_result: Result<crate::dag::QueueItem, String> = {
        match project_state.store.lock() {
            Ok(guard) => match guard.as_ref() {
                Some(store) => store.create_queue_item(NewQueueItem {
                    project_id,
                    agent_id: agent_id.to_string(),
                    workflow_id: None,
                    awakeable_id: None,
                    question: decision.question,
                    options: decision.options,
                    context_snapshot,
                    priority,
                }),
                None => {
                    eprintln!("[agents] Store not open — dropping poe:decision");
                    return;
                }
            },
            Err(e) => {
                eprintln!("[agents] Failed to lock store: {}", e);
                return;
            }
        }
    };

    match item_result {
        Ok(item) => {
            let _ = app.emit("queue:item:added", QueueItemAddedEvent { item });
        }
        Err(e) => {
            eprintln!("[agents] Failed to create queue item: {}", e);
        }
    }
}
