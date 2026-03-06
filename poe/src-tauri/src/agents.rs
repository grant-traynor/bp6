// bp6-3d1.2 + bp6-3d1.3: Agent spawn & lifecycle + Agent progress protocol
//
// Manages agent processes spawned via PTY. Handles:
//   - poe:decision   → queue item (Phase 2)
//   - poe:step       → update workflow current_step in SQLite (Phase 3)
//   - poe:artifact   → create AgentOutput DAG node (Phase 3)
//   - poe:done       → mark workflow completed (Phase 3)
// Exposes graceful stop (SIGTERM → SIGKILL after 3s).
//
// bp6-7fi.1: Agent heartbeat & liveness monitoring.
//   Watchdog checks every AGENT_WATCHDOG_INTERVAL_SECS seconds; if an agent
//   produces no output for AGENT_SILENCE_TIMEOUT_SECS it emits workflow:status
//   (status=blocked) and creates a queue item for human intervention.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use crate::dag::{EdgeType, NewQueueItem, NodeType, QueueItemOption};
use crate::project::{NodeUpsertedEvent, ProjectState, QueueItemAddedEvent};
use crate::restate::RESTATE_SERVICES_PORT;

// ── Watchdog constants ─────────────────────────────────────────────────────────

/// How long an agent may be silent before the watchdog raises an alert.
const AGENT_SILENCE_TIMEOUT_SECS: u64 = 120;

/// How often the watchdog polls all active agents.
const AGENT_WATCHDOG_INTERVAL_SECS: u64 = 30;

// ── Helpers ────────────────────────────────────────────────────────────────────

/// Strip ANSI escape sequences from PTY output (e.g. color codes, cursor movement).
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // ESC — consume until end-of-sequence (a letter or a few special terminators)
            match chars.peek() {
                Some(&'[') => {
                    chars.next(); // consume '['
                    // CSI sequence: consume until we hit a letter
                    for ch in chars.by_ref() {
                        if ch.is_ascii_alphabetic() { break; }
                    }
                }
                Some(&']') => {
                    chars.next(); // consume ']'
                    // OSC sequence: consume until BEL or ESC
                    for ch in chars.by_ref() {
                        if ch == '\x07' || ch == '\x1b' { break; }
                    }
                }
                _ => {
                    // Other: consume one char
                    chars.next();
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

// ── AgentHandle ────────────────────────────────────────────────────────────────

pub struct AgentHandle {
    /// Shared writer to the PTY master (agent's stdin).
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    /// OS PID of the child process (for SIGTERM/SIGKILL).
    pub pid: Option<u32>,
    /// Workflow this agent is executing (if any).
    pub workflow_id: Option<String>,
    /// DAG node this agent is working on.
    pub node_id: Option<String>,
    /// Workflow type (e.g., "ImplementationWorkflow").
    pub workflow_type: Option<String>,
    /// ISO-8601 start time, used for workflow:status events.
    pub started_at: String,
    /// Last time a PTY output byte was received (for silence watchdog).
    pub last_output_at: Arc<Mutex<std::time::Instant>>,
    /// True once the watchdog has emitted a silence alert; reset on new output.
    pub silence_alerted: Arc<Mutex<bool>>,
}

impl AgentHandle {
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

pub struct AgentState {
    pub handles: Mutex<HashMap<String, AgentHandle>>,
}

impl AgentState {
    pub fn new() -> Self {
        AgentState {
            handles: Mutex::new(HashMap::new()),
        }
    }

    /// Find an agent_id by workflow_id. Returns None if not found.
    pub fn find_agent_by_workflow(&self, workflow_id: &str) -> Option<String> {
        let map = self.handles.lock().ok()?;
        map.iter()
            .find(|(_, h)| h.workflow_id.as_deref() == Some(workflow_id))
            .map(|(id, _)| id.clone())
    }

    /// Write text to an agent's stdin identified by workflow_id.
    pub fn write_to_workflow_agent(&self, workflow_id: &str, text: &str) -> Result<(), String> {
        let map = self
            .handles
            .lock()
            .map_err(|e| format!("Failed to lock AgentState: {}", e))?;
        let handle = map
            .iter()
            .find(|(_, h)| h.workflow_id.as_deref() == Some(workflow_id))
            .map(|(_, h)| h)
            .ok_or_else(|| format!("No agent found for workflow '{}'", workflow_id))?;
        handle.write_text(text)
    }

    /// Send SIGTERM to the agent running a workflow; spawn background SIGKILL watchdog after 3s.
    pub fn stop_workflow_agent_graceful(&self, workflow_id: &str) {
        let pid = {
            let map = match self.handles.lock() {
                Ok(m) => m,
                Err(_) => return,
            };
            map.values()
                .find(|h| h.workflow_id.as_deref() == Some(workflow_id))
                .and_then(|h| h.pid)
        };

        if let Some(pid) = pid {
            #[cfg(unix)]
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGTERM);
            }
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(3));
                #[cfg(unix)]
                unsafe {
                    libc::kill(pid as libc::pid_t, libc::SIGKILL);
                }
            });
        }
    }
}

// ── Wire protocol structs ──────────────────────────────────────────────────────

/// Phase 2: Human decision point.
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

/// Phase 3: Workflow step transition.
#[derive(Debug, Deserialize)]
struct PoeStep {
    step: String,
    /// "started" | "completed" | "failed"
    status: String,
    detail: Option<String>,
}

/// Phase 3: Artifact produced by agent (legacy format).
#[derive(Debug, Deserialize)]
struct PoeArtifact {
    /// "code" | "doc" | "test" | "decision"
    kind: String,
    #[serde(rename = "nodeId")]
    node_id: Option<String>,
    content: String,
}

/// bp6-ims.3: Knowledge artefact emitted by agent (new format).
/// Produces a KnowledgeArtifact DAG node and writes a markdown file to
/// `<project_dir>/docs/<filename>` (or `.poe/skills/<filename>.md` for kind=skill).
#[derive(Debug, Deserialize)]
struct PoeKnowledgeArtifact {
    filename: String,
    title: String,
    content: String,
    step: u32,
    /// "skill" → writes to .poe/skills/; anything else or absent → writes to docs/
    #[serde(default)]
    kind: Option<String>,
}

/// Phase 3: Workflow step complete.
#[derive(Debug, Deserialize)]
struct PoeDone {
    summary: String,
}

// ── Tauri event payloads ───────────────────────────────────────────────────────

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentOutputEvent {
    agent_id: String,
    line: String,
}

/// Matches TypeScript `AgentStdoutLine` (bp6-80q).
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStdoutEvent {
    pub workflow_id: String,
    pub line: String,
    pub ts: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentExitedEvent {
    agent_id: String,
}

/// Matches the TypeScript `WorkflowInfo` interface.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowStatusEvent {
    pub workflow_id: String,
    pub node_id: String,
    pub agent_id: String,
    pub status: String,
    pub current_step: Option<String>,
    pub started_at: String,
}

/// Emitted when the watchdog detects an agent has been silent too long.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentSilenceEvent {
    workflow_id: String,
    agent_id: String,
    status: String,
    reason: String,
    silent_for_secs: u64,
}

// ── Tauri command input/output types ──────────────────────────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnAgentParams {
    pub cmd: String,
    pub args: Vec<String>,
    pub env: Option<HashMap<String, String>>,
    pub agent_id: Option<String>,
    /// Phase 3: attach workflow context
    pub workflow_id: Option<String>,
    pub node_id: Option<String>,
    pub workflow_type: Option<String>,
    /// Claude session UUID. If set and resume=false: passes --session-id <uuid> to claude.
    /// If set and resume=true: passes --resume <uuid> (ignores args entirely).
    #[serde(default)]
    pub session_id: Option<String>,
    /// When true, spawn claude --resume <session_id> instead of a fresh session.
    #[serde(default)]
    pub resume: bool,
    /// Working directory for the agent process. Determines the workspace Claude
    /// Code checks for trust — must be set to the project directory so trust
    /// is saved per-project rather than defaulting to the app's cwd.
    #[serde(default)]
    pub cwd: Option<String>,
    /// When false, spawn with piped stdout instead of a PTY.
    /// Use false for stream-json conversational agents; true (default) for
    /// interactive PTY sessions (Claude Code CLI execution agents).
    #[serde(default = "default_use_pty")]
    pub use_pty: bool,
}

fn default_use_pty() -> bool { true }

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
pub struct StopAgentGracefulParams {
    pub agent_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteToAgentParams {
    pub agent_id: String,
    pub text: String,
}

// ── Tauri commands ─────────────────────────────────────────────────────────────

/// Core spawn logic usable both as a Tauri command and from workflow.rs.
pub fn spawn_agent_internal(
    params: SpawnAgentParams,
    app: &AppHandle,
    agent_state: &AgentState,
) -> Result<SpawnedAgent, String> {
    if !params.use_pty {
        return spawn_pipe_agent(params, app, agent_state);
    }

    let agent_id = params
        .agent_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let started_at = chrono::Utc::now().to_rfc3339();

    let pty_system = NativePtySystem::default();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            // Wide enough to prevent line-wrapping for stream-json output.
            // Stream-json lines can be 10k+ chars (full artifact content).
            // u16::MAX (65535) prevents wrapping for all practical responses.
            cols: u16::MAX,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {}", e))?;

    // Build the effective argument list, injecting --session-id or --resume as needed.
    let effective_args: Vec<String> = if params.resume {
        // Resume mode: ignore original args entirely; claude resumes from its journal.
        let sid = params.session_id.as_deref().unwrap_or("");
        vec![
            "--resume".to_string(),
            sid.to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--dangerously-skip-permissions".to_string(),
        ]
    } else if let Some(ref sid) = params.session_id {
        // Fresh session: prepend --session-id <uuid> before the caller's args.
        let mut args = vec!["--session-id".to_string(), sid.clone()];
        args.extend(params.args.iter().cloned());
        args
    } else {
        params.args.clone()
    };

    let mut cmd = CommandBuilder::new(&params.cmd);
    for arg in &effective_args {
        cmd.arg(arg);
    }
    if let Some(ref cwd) = params.cwd {
        cmd.cwd(cwd);
    }
    // Seed with the parent process environment so PATH, HOME, etc. are available.
    // portable_pty's CommandBuilder starts with an empty env when any .env() call is made,
    // so we must explicitly inherit the parent env before adding our own vars.
    for (k, v) in std::env::vars() {
        cmd.env(k, v);
    }
    if let Some(env_map) = params.env {
        for (k, v) in env_map {
            cmd.env(k, v);
        }
    }

    eprintln!(
        "[agents] Spawning '{}' with args {:?} (workflow={:?})",
        params.cmd, params.args, params.workflow_id
    );

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn agent '{}': {}", params.cmd, e))?;

    let pid = child.process_id();
    eprintln!("[agents] Spawned pid={:?} agent_id={}", pid, agent_id);

    let master_reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("Failed to clone PTY reader: {}", e))?;

    let master_writer: Box<dyn Write + Send> = pair
        .master
        .take_writer()
        .map_err(|e| format!("Failed to take PTY writer: {}", e))?;

    let writer = Arc::new(Mutex::new(master_writer));

    // Pre-buffer a CR (Enter in raw PTY mode) so the Claude Code workspace-trust
    // prompt is auto-confirmed without waiting for output.  The character sits in
    // the TTY input queue and is consumed the instant the process reads stdin.
    // For non-interactive / print-mode (claude -p …) invocations this is harmless
    // because claude never reads from stdin in that mode.
    if let Ok(mut w) = writer.lock() {
        let _ = w.write_all(b"\r");
        let _ = w.flush();
    }

    // Watchdog liveness tracking — shared between PTY reader thread and watchdog task.
    let last_output_at = Arc::new(Mutex::new(std::time::Instant::now()));
    let silence_alerted = Arc::new(Mutex::new(false));

    let workflow_id_bg = params.workflow_id.clone();
    let node_id_bg = params.node_id.clone();
    let workflow_type_bg = params.workflow_type.clone();
    let started_at_bg = started_at.clone();

    {
        let agent_id_bg = agent_id.clone();
        let app_for_thread = app.clone();
        let last_output_at_bg = Arc::clone(&last_output_at);
        let silence_alerted_bg = Arc::clone(&silence_alerted);
        let writer_bg = Arc::clone(&writer);
        std::thread::spawn(move || {
            let reader = BufReader::new(master_reader);
            let mut done_received = false;
            // Track markdown code fences so we don't parse example JSON inside them as real events.
            let mut in_code_fence = false;

            for line_result in reader.lines() {
                let line = match line_result {
                    Ok(l) => l,
                    Err(_) => break,
                };

                let trimmed = strip_ansi(line.trim());
                if trimmed.is_empty() {
                    continue;
                }

                // Update heartbeat timestamp and clear any pending silence alert.
                if let Ok(mut ts) = last_output_at_bg.lock() {
                    *ts = std::time::Instant::now();
                }
                if let Ok(mut alerted) = silence_alerted_bg.lock() {
                    *alerted = false;
                }

                eprintln!("[agents:pty {}] {}", agent_id_bg, trimmed);

                // Auto-confirm the Claude Code workspace-trust prompt.
                // The TUI spaces characters via cursor-movement escape sequences.
                // After ANSI stripping those moves disappear, so the text arrives
                // WITHOUT spaces: "Entertoconfirm·Esctocancel" not "Enter to confirm".
                // Match on the space-free form; also strip spaces from trimmed so this
                // works whether or not spaces survive stripping in future versions.
                {
                    let spaceless = trimmed.replace(' ', "");
                    if spaceless.contains("Entertoconfirm") || spaceless.contains("Itrustthisfolder") {
                        eprintln!("[agents:pty {}] auto-confirming trust-folder prompt", agent_id_bg);
                        if let Ok(mut w) = writer_bg.lock() {
                            let _ = w.write_all(b"\r");
                            let _ = w.flush();
                        }
                        continue;
                    }
                }

                // Track code fence state — toggle on any ``` line.
                if trimmed.starts_with("```") {
                    in_code_fence = !in_code_fence;
                }

                if let Some(wf_id) = &workflow_id_bg {
                    let _ = app_for_thread.emit(
                        "agent:stdout",
                        AgentStdoutEvent {
                            workflow_id: wf_id.clone(),
                            line: trimmed.clone(),
                            ts: chrono::Utc::now().to_rfc3339(),
                        },
                    );
                }

                // Skip fence delimiter lines themselves (```json, ```, etc.) — never events.
                // Do NOT skip content inside fences: agents often wrap their poe: JSON
                // output in ```json blocks, and we must still parse those as real events.
                if trimmed.starts_with("```") {
                    continue;
                }

                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&trimmed) {
                    match val.get("type").and_then(|t| t.as_str()) {
                        Some("poe:decision") => {
                            if let Ok(decision) = serde_json::from_value::<PoeDecision>(val) {
                                handle_poe_decision(
                                    &app_for_thread,
                                    &agent_id_bg,
                                    &workflow_id_bg,
                                    decision,
                                );
                                continue;
                            }
                        }
                        Some("poe:step") => {
                            if let (Ok(step), Some(wf_id), Some(nid)) = (
                                serde_json::from_value::<PoeStep>(val),
                                &workflow_id_bg,
                                &node_id_bg,
                            ) {
                                handle_poe_step(
                                    &app_for_thread,
                                    &agent_id_bg,
                                    wf_id,
                                    nid,
                                    &started_at_bg,
                                    step,
                                );
                                continue;
                            }
                        }
                        Some("poe:artifact") => {
                            // Try the new KnowledgeArtifact format first (has `filename` field).
                            // Fall back to the legacy AgentOutput format if that fails.
                            if let Ok(ka) = serde_json::from_value::<PoeKnowledgeArtifact>(val.clone()) {
                                handle_poe_knowledge_artifact(&app_for_thread, ka);
                                continue;
                            }
                            if let (Ok(artifact), Some(nid)) = (
                                serde_json::from_value::<PoeArtifact>(val),
                                &node_id_bg,
                            ) {
                                handle_poe_artifact(&app_for_thread, nid, artifact);
                                continue;
                            }
                        }
                        Some("poe:done") => {
                            if let Ok(done_msg) = serde_json::from_value::<PoeDone>(val) {
                                done_received = true;
                                match (&workflow_id_bg, &node_id_bg) {
                                    (Some(wf_id), Some(nid)) => {
                                        handle_poe_done(
                                            &app_for_thread,
                                            &agent_id_bg,
                                            wf_id,
                                            nid,
                                            &started_at_bg,
                                            &workflow_type_bg,
                                            done_msg,
                                        );
                                    }
                                    (Some(wf_id), None) => {
                                        // Lifecycle step agent: no node_id, but has workflow_id
                                        // (workflow_id == project_id for lifecycle agents).
                                        handle_lifecycle_poe_done(
                                            &app_for_thread,
                                            &agent_id_bg,
                                            wf_id,
                                            done_msg,
                                        );
                                    }
                                    _ => {}
                                }
                                continue;
                            }
                        }
                        _ => {}
                    }
                }

                let _ = app_for_thread.emit(
                    "agent:output",
                    AgentOutputEvent {
                        agent_id: agent_id_bg.clone(),
                        line: trimmed,
                    },
                );
            }

            let _ = app_for_thread.emit(
                "agent:exited",
                AgentExitedEvent {
                    agent_id: agent_id_bg.clone(),
                },
            );

            eprintln!("[agents] PTY EOF for agent_id={} done_received={}", agent_id_bg, done_received);

            // Remove AgentHandle from registry so the watchdog's Arc::strong_count drops
            // to 1 and it knows to exit. Without this, the handle stays in the map and
            // the watchdog never detects that the agent has gone away. (bp6-7fi.4.1)
            let _ = app_for_thread
                .state::<AgentState>()
                .handles
                .lock()
                .ok()
                .map(|mut m| m.remove(&agent_id_bg));

            if !done_received {
                if let (Some(wf_id), Some(nid)) = (&workflow_id_bg, &node_id_bg) {
                    handle_agent_crash(&app_for_thread, &agent_id_bg, wf_id, nid, &started_at_bg);
                }
            }
        });
    }

    if let (Some(wf_id), Some(nid)) = (&params.workflow_id, &params.node_id) {
        let _ = app.emit(
            "workflow:status",
            WorkflowStatusEvent {
                workflow_id: wf_id.clone(),
                node_id: nid.clone(),
                agent_id: agent_id.clone(),
                status: "running".to_string(),
                current_step: None,
                started_at: started_at.clone(),
            },
        );
    }

    let handle = AgentHandle {
        writer,
        pid,
        workflow_id: params.workflow_id,
        node_id: params.node_id,
        workflow_type: params.workflow_type,
        started_at,
        last_output_at: Arc::clone(&last_output_at),
        silence_alerted: Arc::clone(&silence_alerted),
    };

    {
        let mut map = agent_state
            .handles
            .lock()
            .map_err(|e| format!("Failed to lock AgentState: {}", e))?;
        map.insert(agent_id.clone(), handle);
    }

    // ── Spawn watchdog task ────────────────────────────────────────────────────
    // Runs on the Tauri tokio runtime. Polls every AGENT_WATCHDOG_INTERVAL_SECS
    // and emits a silence alert if the agent has been quiet too long.
    //
    // Liveness detection: the watchdog holds an Arc<Mutex<bool>> (`liveness`)
    // that is also held by the AgentHandle (in the map) and the PTY reader
    // thread. When the agent EOF's the PTY thread exits and the handle is
    // removed — at that point strong_count drops to 1 (only the watchdog),
    // which is the exit signal.
    {
        let agent_id_wd = agent_id.clone();
        let app_wd = app.clone();
        // The watchdog's liveness beacon: same Arc as AgentHandle.silence_alerted
        // and silence_alerted_bg in the PTY thread.
        let liveness = Arc::clone(&silence_alerted);
        let last_output_wd = Arc::clone(&last_output_at);
        let workflow_id_wd = agent_state
            .handles
            .lock()
            .ok()
            .and_then(|m| m.get(&agent_id).and_then(|h| h.workflow_id.clone()));

        tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(
                Duration::from_secs(AGENT_WATCHDOG_INTERVAL_SECS),
            );
            // Skip the immediate first tick (fires at t=0).
            interval.tick().await;

            loop {
                interval.tick().await;

                // If the AgentHandle has been dropped (strong count == 1, only
                // this task holds a ref), the agent has exited — stop watching.
                if Arc::strong_count(&liveness) <= 1 {
                    eprintln!(
                        "[watchdog] agent_id={} no longer registered, exiting",
                        agent_id_wd
                    );
                    break;
                }

                let elapsed_secs = {
                    match last_output_wd.lock() {
                        Ok(ts) => ts.elapsed().as_secs(),
                        Err(_) => continue,
                    }
                };

                if elapsed_secs < AGENT_SILENCE_TIMEOUT_SECS {
                    continue;
                }

                // Check and set alerted flag atomically.
                let already_alerted = match liveness.lock() {
                    Ok(mut flag) => {
                        if *flag {
                            true
                        } else {
                            *flag = true;
                            false
                        }
                    }
                    Err(_) => continue,
                };

                if already_alerted {
                    continue;
                }

                let wf_id = match &workflow_id_wd {
                    Some(id) => id.clone(),
                    None => {
                        eprintln!(
                            "[watchdog] agent_id={} silent {}s but no workflow_id — skipping alert",
                            agent_id_wd, elapsed_secs
                        );
                        continue;
                    }
                };

                eprintln!(
                    "[watchdog] agent_id={} workflow_id={} silent for {}s — raising alert",
                    agent_id_wd, wf_id, elapsed_secs
                );

                // Emit workflow:status blocked event.
                let _ = app_wd.emit(
                    "workflow:status",
                    AgentSilenceEvent {
                        workflow_id: wf_id.clone(),
                        agent_id: agent_id_wd.clone(),
                        status: "blocked".to_string(),
                        reason: "agent_silent".to_string(),
                        silent_for_secs: elapsed_secs,
                    },
                );

                // Create a queue item so a human can decide what to do.
                let project_state = app_wd.state::<ProjectState>();
                let question = format!(
                    "Agent has been silent for {} seconds. What should we do?",
                    elapsed_secs
                );
                let options = vec![
                    QueueItemOption {
                        id: "wait".to_string(),
                        label: "Wait".to_string(),
                        description: Some("Give the agent more time to respond.".to_string()),
                    },
                    QueueItemOption {
                        id: "redirect".to_string(),
                        label: "Redirect".to_string(),
                        description: Some(
                            "Send a nudge message to the agent's stdin.".to_string(),
                        ),
                    },
                    QueueItemOption {
                        id: "kill".to_string(),
                        label: "Kill".to_string(),
                        description: Some("Terminate the agent process.".to_string()),
                    },
                ];
                let item_result: Result<crate::dag::QueueItem, String> =
                    project_state.with_active(|store, project_id| {
                        store.create_queue_item(NewQueueItem {
                            project_id: project_id.to_string(),
                            agent_id: agent_id_wd.clone(),
                            workflow_id: Some(wf_id.clone()),
                            awakeable_id: None,
                            question,
                            options,
                            context_snapshot: serde_json::json!({
                                "reason": "agent_silent",
                                "silentForSecs": elapsed_secs,
                            }),
                            priority: 1,
                        })
                    });
                match item_result {
                    Ok(item) => {
                        let _ = app_wd
                            .emit("queue:item:added", QueueItemAddedEvent { item });
                    }
                    Err(e) => {
                        eprintln!(
                            "[watchdog] Failed to create silence queue item: {}",
                            e
                        );
                    }
                }
            }
        });
    }

    Ok(SpawnedAgent { agent_id })
}

/// Spawn an agent using piped stdout instead of a PTY.
///
/// Used for `claude -p --output-format stream-json` conversational agents.
/// Pipe output is clean NDJSON — no ANSI escape codes, no line-wrapping corruption.
fn spawn_pipe_agent(
    params: SpawnAgentParams,
    app: &AppHandle,
    agent_state: &AgentState,
) -> Result<SpawnedAgent, String> {
    let agent_id = params
        .agent_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let started_at = chrono::Utc::now().to_rfc3339();

    // Build effective args (same logic as PTY path).
    let effective_args: Vec<String> = if params.resume {
        let sid = params.session_id.as_deref().unwrap_or("");
        vec![
            "--resume".to_string(),
            sid.to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
        ]
    } else if let Some(ref sid) = params.session_id {
        let mut args = vec!["--session-id".to_string(), sid.clone()];
        args.extend(params.args.iter().cloned());
        args
    } else {
        params.args.clone()
    };

    // Pipe agents cannot respond to interactive prompts (stdin is null).
    // Add --dangerously-skip-permissions so Claude never waits for trust/tool approval.
    let mut effective_args = effective_args;
    if !effective_args.contains(&"--dangerously-skip-permissions".to_string()) {
        effective_args.push("--dangerously-skip-permissions".to_string());
    }

    let mut cmd = std::process::Command::new(&params.cmd);
    cmd.args(&effective_args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped()); // capture stderr so errors appear in our logs
    cmd.stdin(Stdio::null());

    if let Some(ref cwd) = params.cwd {
        cmd.current_dir(cwd);
    }
    // Inherit parent environment so PATH, HOME, etc. are available.
    if let Some(env_map) = params.env {
        for (k, v) in env_map {
            cmd.env(k, v);
        }
    }

    eprintln!(
        "[agents:pipe] Spawning '{}' with args {:?} (workflow={:?})",
        params.cmd, effective_args, params.workflow_id
    );

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn pipe agent '{}': {}", params.cmd, e))?;

    let pid = child.id();
    eprintln!("[agents:pipe] Spawned pid={} agent_id={}", pid, agent_id);

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to take stdout from pipe agent".to_string())?;

    // Log stderr in a background thread so Claude errors surface in app logs.
    if let Some(stderr) = child.stderr.take() {
        let agent_id_err = agent_id.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                if !line.trim().is_empty() {
                    eprintln!("[agents:pipe:err {}] {}", agent_id_err, line);
                }
            }
        });
    }

    let last_output_at = Arc::new(Mutex::new(std::time::Instant::now()));
    let silence_alerted = Arc::new(Mutex::new(false));

    let workflow_id_bg = params.workflow_id.clone();
    let node_id_bg = params.node_id.clone();
    let workflow_type_bg = params.workflow_type.clone();
    let started_at_bg = started_at.clone();

    {
        let agent_id_bg = agent_id.clone();
        let app_for_thread = app.clone();
        let last_output_at_bg = Arc::clone(&last_output_at);
        let silence_alerted_bg = Arc::clone(&silence_alerted);

        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            let mut done_received = false;
            let mut in_code_fence = false;

            for line_result in reader.lines() {
                let line = match line_result {
                    Ok(l) => l,
                    Err(_) => break,
                };

                // Pipe output has no ANSI codes — just trim whitespace.
                let trimmed = line.trim().to_string();
                if trimmed.is_empty() {
                    continue;
                }

                if let Ok(mut ts) = last_output_at_bg.lock() {
                    *ts = std::time::Instant::now();
                }
                if let Ok(mut alerted) = silence_alerted_bg.lock() {
                    *alerted = false;
                }

                eprintln!(
                    "[agents:pipe {}] {}",
                    agent_id_bg,
                    &trimmed[..trimmed.len().min(200)]
                );

                if trimmed.starts_with("```") {
                    in_code_fence = !in_code_fence;
                }

                if let Some(wf_id) = &workflow_id_bg {
                    let _ = app_for_thread.emit(
                        "agent:stdout",
                        AgentStdoutEvent {
                            workflow_id: wf_id.clone(),
                            line: trimmed.clone(),
                            ts: chrono::Utc::now().to_rfc3339(),
                        },
                    );
                }

                if trimmed.starts_with("```") {
                    continue;
                }

                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&trimmed) {
                    match val.get("type").and_then(|t| t.as_str()) {
                        Some("poe:decision") => {
                            if let Ok(decision) = serde_json::from_value::<PoeDecision>(val) {
                                handle_poe_decision(
                                    &app_for_thread,
                                    &agent_id_bg,
                                    &workflow_id_bg,
                                    decision,
                                );
                                continue;
                            }
                        }
                        Some("poe:step") => {
                            if let (Ok(step), Some(wf_id), Some(nid)) = (
                                serde_json::from_value::<PoeStep>(val),
                                &workflow_id_bg,
                                &node_id_bg,
                            ) {
                                handle_poe_step(
                                    &app_for_thread,
                                    &agent_id_bg,
                                    wf_id,
                                    nid,
                                    &started_at_bg,
                                    step,
                                );
                                continue;
                            }
                        }
                        Some("poe:artifact") => {
                            if let Ok(ka) =
                                serde_json::from_value::<PoeKnowledgeArtifact>(val.clone())
                            {
                                handle_poe_knowledge_artifact(&app_for_thread, ka);
                                continue;
                            }
                            if let (Ok(artifact), Some(nid)) = (
                                serde_json::from_value::<PoeArtifact>(val),
                                &node_id_bg,
                            ) {
                                handle_poe_artifact(&app_for_thread, nid, artifact);
                                continue;
                            }
                        }
                        Some("poe:done") => {
                            if let Ok(done_msg) = serde_json::from_value::<PoeDone>(val) {
                                done_received = true;
                                match (&workflow_id_bg, &node_id_bg) {
                                    (Some(wf_id), Some(nid)) => {
                                        handle_poe_done(
                                            &app_for_thread,
                                            &agent_id_bg,
                                            wf_id,
                                            nid,
                                            &started_at_bg,
                                            &workflow_type_bg,
                                            done_msg,
                                        );
                                    }
                                    (Some(wf_id), None) => {
                                        handle_lifecycle_poe_done(
                                            &app_for_thread,
                                            &agent_id_bg,
                                            wf_id,
                                            done_msg,
                                        );
                                    }
                                    _ => {}
                                }
                                continue;
                            }
                        }
                        _ => {}
                    }
                }

                let _ = app_for_thread.emit(
                    "agent:output",
                    AgentOutputEvent {
                        agent_id: agent_id_bg.clone(),
                        line: trimmed,
                    },
                );
            }

            // Reap child process.
            let mut child = child;
            let _ = child.wait();

            let _ = app_for_thread.emit(
                "agent:exited",
                AgentExitedEvent {
                    agent_id: agent_id_bg.clone(),
                },
            );

            eprintln!(
                "[agents:pipe] EOF for agent_id={} done_received={}",
                agent_id_bg, done_received
            );

            let _ = app_for_thread
                .state::<AgentState>()
                .handles
                .lock()
                .ok()
                .map(|mut m| m.remove(&agent_id_bg));
        });
    }

    // Pipe agents don't need stdin — use a no-op writer.
    let writer: Box<dyn Write + Send> = Box::new(std::io::sink());
    let writer = Arc::new(Mutex::new(writer));

    let handle = AgentHandle {
        writer,
        pid: Some(pid),
        workflow_id: params.workflow_id,
        node_id: params.node_id,
        workflow_type: params.workflow_type,
        started_at,
        last_output_at: Arc::clone(&last_output_at),
        silence_alerted: Arc::clone(&silence_alerted),
    };

    {
        let mut map = agent_state
            .handles
            .lock()
            .map_err(|e| format!("Failed to lock AgentState: {}", e))?;
        map.insert(agent_id.clone(), handle);
    }

    Ok(SpawnedAgent { agent_id })
}

/// Spawn an agent process inside a PTY (Tauri command — delegates to spawn_agent_internal).
#[tauri::command]
pub fn spawn_agent(
    params: SpawnAgentParams,
    app: AppHandle,
    agent_state: State<'_, AgentState>,
) -> Result<SpawnedAgent, String> {
    spawn_agent_internal(params, &app, &agent_state)
}

/// Kill a running agent immediately (SIGHUP via PTY master close).
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
    // Dropping AgentHandle closes PTY master → SIGHUP child.
    Ok(())
}

/// Graceful stop: SIGTERM the agent; after 3 seconds force SIGKILL if still running.
#[tauri::command]
pub fn stop_agent_graceful(
    params: StopAgentGracefulParams,
    agent_state: State<'_, AgentState>,
) -> Result<(), String> {
    let map = agent_state
        .handles
        .lock()
        .map_err(|e| format!("Failed to lock AgentState: {}", e))?;

    let pid = map
        .get(&params.agent_id)
        .and_then(|h| h.pid)
        .ok_or_else(|| format!("Agent '{}' not found or has no PID", params.agent_id))?;

    drop(map);

    #[cfg(unix)]
    unsafe {
        libc::kill(pid as libc::pid_t, libc::SIGTERM);
    }

    // Watchdog: SIGKILL after 3 seconds if agent hasn't exited
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(3));
        #[cfg(unix)]
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGKILL);
        }
    });

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

// ── Internal event handlers ────────────────────────────────────────────────────

/// Attempt to create a Restate awakeable synchronously from a non-async context.
/// Returns None if Restate is unavailable.
fn create_awakeable_sync() -> Option<String> {
    let url = format!("http://127.0.0.1:{}/restate/awakeables", RESTATE_SERVICES_PORT);
    // Build a one-shot single-threaded runtime to run the async HTTP call.
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[agents] Failed to build tokio runtime for awakeable: {}", e);
            return None;
        }
    };
    rt.block_on(async move {
        let client = reqwest::Client::new();
        match client.post(&url).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    match resp.json::<serde_json::Value>().await {
                        Ok(body) => body
                            .get("id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        Err(e) => {
                            eprintln!("[agents] Failed to parse awakeable response: {}", e);
                            None
                        }
                    }
                } else {
                    eprintln!(
                        "[agents] Restate returned {} when creating awakeable",
                        resp.status()
                    );
                    None
                }
            }
            Err(e) => {
                eprintln!("[agents] Restate not available for awakeable creation: {}", e);
                None
            }
        }
    })
}

/// Phase 2: persist poe:decision → queue item.
fn handle_poe_decision(
    app: &AppHandle,
    agent_id: &str,
    workflow_id: &Option<String>,
    decision: PoeDecision,
) {
    // Create a Restate awakeable so resolution can unblock the agent.
    let awakeable_id = create_awakeable_sync();

    let project_state = app.state::<ProjectState>();

    let context_snapshot = decision.context.unwrap_or(serde_json::json!({}));
    let priority = decision.priority.unwrap_or(2);

    let item_result: Result<crate::dag::QueueItem, String> = project_state.with_active(|store, project_id| {
        store.create_queue_item(NewQueueItem {
            project_id: project_id.to_string(),
            agent_id: agent_id.to_string(),
            workflow_id: workflow_id.clone(),
            awakeable_id: awakeable_id.clone(),
            question: decision.question.clone(),
            options: decision.options.clone(),
            context_snapshot: context_snapshot.clone(),
            priority,
        })
    });
    let item_result = match item_result {
        Ok(item) => item,
        Err(e) => {
            eprintln!("[agents] No project open — dropping poe:decision: {}", e);
            return;
        }
    };

    let _ = app.emit("queue:item:added", QueueItemAddedEvent { item: item_result });
}

/// Phase 3: update workflow current_step in SQLite and emit workflow:status.
fn handle_poe_step(
    app: &AppHandle,
    agent_id: &str,
    workflow_id: &str,
    node_id: &str,
    started_at: &str,
    step: PoeStep,
) {
    let step_label = if step.status == "started" {
        step.step.clone()
    } else {
        format!("{}:{}", step.step, step.status)
    };

    if let Some(detail) = &step.detail {
        eprintln!("[agents] poe:step {} — {}", step_label, detail);
    }

    let project_state = app.state::<ProjectState>();
    let current_step = project_state
        .with_active(|store, _| store.update_workflow_step(workflow_id, &step_label))
        .ok()
        .and_then(|r| r.current_step);

    let _ = app.emit(
        "workflow:status",
        WorkflowStatusEvent {
            workflow_id: workflow_id.to_string(),
            node_id: node_id.to_string(),
            agent_id: agent_id.to_string(),
            status: "running".to_string(),
            current_step,
            started_at: started_at.to_string(),
        },
    );
}

/// Phase 3: create an AgentOutput DAG node and a generated-by edge from the work node.
fn handle_poe_artifact(app: &AppHandle, work_node_id: &str, artifact: PoeArtifact) {
    fn inner(
        app: &AppHandle,
        work_node_id: &str,
        artifact: PoeArtifact,
    ) -> Result<(), String> {
        let target_node_id = artifact
            .node_id
            .as_deref()
            .unwrap_or(work_node_id)
            .to_string();

        let project_state = app.state::<ProjectState>();
        let artifact_node = project_state.with_active(|store, project_id| {
            let node = store.upsert_node(
                &NodeType::AgentOutput,
                project_id,
                serde_json::json!({
                    "kind": artifact.kind,
                    "content": artifact.content,
                    "sourceNodeId": target_node_id,
                }),
            )?;
            store.add_edge(
                &target_node_id,
                &node.id,
                &EdgeType::GeneratedBy,
                serde_json::json!({}),
            )?;
            Ok(node)
        })?;

        let _ = app.emit("dag:node:upserted", NodeUpsertedEvent { node: artifact_node });
        Ok(())
    }

    if let Err(e) = inner(app, work_node_id, artifact) {
        eprintln!("[agents] Failed to create artifact node: {}", e);
    }
}

/// bp6-ims.3 + bp6-ims.11: create a KnowledgeArtifact DAG node and write the markdown
/// file to `<project_dir>/docs/<filename>`, or to `<project_dir>/.poe/skills/<filename>.md`
/// when `kind = "skill"`.
fn handle_poe_knowledge_artifact(app: &AppHandle, ka: PoeKnowledgeArtifact) {
    fn inner(app: &AppHandle, ka: PoeKnowledgeArtifact) -> Result<(), String> {
        let project_state = app.state::<ProjectState>();
        let is_skill = ka.kind.as_deref() == Some("skill");

        // Write the file to the appropriate directory.
        if let Some(active_dir) = project_state.active_dir_str() {
            let target_dir = if is_skill {
                std::path::Path::new(&active_dir).join(".poe").join("skills")
            } else {
                std::path::Path::new(&active_dir).join("docs")
            };
            std::fs::create_dir_all(&target_dir)
                .map_err(|e| format!("Failed to create target dir: {}", e))?;
            // Ensure skill filenames have a .md extension.
            let fname = if is_skill && !ka.filename.ends_with(".md") {
                format!("{}.md", ka.filename)
            } else {
                ka.filename.clone()
            };
            let file_path = target_dir.join(&fname);
            std::fs::write(&file_path, &ka.content)
                .map_err(|e| format!("Failed to write artefact file {}: {}", fname, e))?;
            eprintln!("[agents] poe:artifact written to {}", file_path.display());
        } else {
            eprintln!("[agents] poe:artifact: no active project — skipping file write for {}", ka.filename);
        }

        // Create a KnowledgeArtifact DAG node
        let node = project_state.with_active(|store, project_id| {
            store.upsert_node(
                &NodeType::KnowledgeArtifact,
                project_id,
                serde_json::json!({
                    "filename": ka.filename,
                    "title": ka.title,
                    "content": ka.content,
                    "step": ka.step,
                    "kind": ka.kind,
                }),
            )
        })?;

        let _ = app.emit("dag:node:upserted", NodeUpsertedEvent { node });
        Ok(())
    }

    if let Err(e) = inner(app, ka) {
        eprintln!("[agents] Failed to create KnowledgeArtifact node: {}", e);
    }
}

/// Phase 3: mark workflow completed, update DAG node status, emit events.
fn handle_poe_done(
    app: &AppHandle,
    agent_id: &str,
    workflow_id: &str,
    node_id: &str,
    started_at: &str,
    workflow_type: &Option<String>,
    done: PoeDone,
) {
    eprintln!(
        "[agents] poe:done workflow '{}': {}",
        workflow_id, done.summary
    );

    // Returns the parent_workflow_id if present (so we can trigger fan-in check).
    fn inner(
        app: &AppHandle,
        workflow_id: &str,
        node_id: &str,
        summary: &str,
    ) -> Result<Option<String>, String> {
        let project_state = app.state::<ProjectState>();
        let (updated_node, parent_workflow_id) = project_state.with_active(|store, _| {
            let wf = store.update_workflow_status(workflow_id, "completed", None, None)?;
            let node = store.get_node(node_id)?;
            let mut data = node.data.clone();
            data["status"] = serde_json::json!("completed");
            data["workflowSummary"] = serde_json::json!(summary);
            let updated = store.update_node(node_id, data)?;
            Ok((updated, wf.parent_workflow_id))
        })?;

        let _ = app.emit("dag:node:upserted", NodeUpsertedEvent { node: updated_node });
        Ok(parent_workflow_id)
    }

    let parent_workflow_id = match inner(app, workflow_id, node_id, &done.summary) {
        Ok(pid) => pid,
        Err(e) => {
            eprintln!("[agents] Failed to finalize workflow: {}", e);
            None
        }
    };

    let _ = app.emit(
        "workflow:status",
        WorkflowStatusEvent {
            workflow_id: workflow_id.to_string(),
            node_id: node_id.to_string(),
            agent_id: agent_id.to_string(),
            status: "completed".to_string(),
            current_step: workflow_type.as_deref().map(|t| format!("{t}:done")),
            started_at: started_at.to_string(),
        },
    );

    // Fan-in: if this workflow has a parent, check whether all siblings are done.
    if let Some(parent_id) = parent_workflow_id {
        check_parent_fan_in(app, &parent_id, workflow_id);
    }
}

/// bp6-1vf: Handle poe:done for a lifecycle step agent (no node_id).
///
/// With Restate owning lifecycle state, this function resolves the appropriate
/// Restate promise to unblock the workflow's run handler:
///
/// - Step 3 PM agent done → resolve "step-3-done"
/// - Step 4/5 task agent done → resolve "task-{task_id}-done" (routed by workflow_type)
/// - Step 5 rework-planning PM done → resolve "step-5-planning-done"
///
/// The Restate workflow run handler then decides what to do next (fan-out, etc.).
fn handle_lifecycle_poe_done(
    app: &AppHandle,
    agent_id: &str,
    project_id: &str,
    done: PoeDone,
) {
    eprintln!(
        "[lifecycle] poe:done for lifecycle agent_id={} project='{}': {}",
        agent_id, project_id, done.summary
    );

    // Determine which promise to resolve from the agent's workflow_type
    let promise_name: Option<String> = {
        let agent_state = app.state::<AgentState>();
        let guard = agent_state.handles.lock().unwrap_or_else(|e| e.into_inner());
        guard.get(agent_id).and_then(|handle| {
            handle.workflow_type.as_deref().map(|wt| {
                if wt.starts_with("lifecycle-step-4-task-") || wt.starts_with("lifecycle-step-5-task-") {
                    // Extract task_id from workflow_type: "lifecycle-step-4-task-{task_id}"
                    let task_id = wt
                        .trim_start_matches("lifecycle-step-4-task-")
                        .trim_start_matches("lifecycle-step-5-task-");
                    format!("task-{}-done", task_id)
                } else if wt == "lifecycle-product-manager-review" || wt.contains("pm-review") {
                    "step-5-planning-done".to_string()
                } else if wt.contains("lifecycle-step-3") || wt == "lifecycle-product-manager-review"
                    || wt == "lifecycle-3-review"
                {
                    "step-3-done".to_string()
                } else {
                    // Generic: resolve step-3-done (step 3 PM agent is the only PTY lifecycle agent)
                    "step-3-done".to_string()
                }
            })
        })
    };

    // Determine promise name from the agent handle's workflow_type
    // Fallback: use step-3-done for step 3 agents, task-{id}-done for task agents
    let resolved_promise = promise_name.unwrap_or_else(|| "step-3-done".to_string());

    eprintln!(
        "[lifecycle] Resolving Restate promise '{}' for project='{}' agent={}",
        resolved_promise, project_id, agent_id
    );

    let project_id_str = project_id.to_string();
    let agent_id_str = agent_id.to_string();
    let summary = done.summary.clone();
    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        let client = reqwest::Client::new();
        let url = format!(
            "{}/ProjectLifecycleWorkflow/{}/resolve_promise",
            crate::lifecycle::restate_ingress_url(),
            project_id_str
        );
        let payload = crate::lifecycle::ResolvePromisePayload {
            promise_name: resolved_promise.clone(),
            value: "done".to_string(),
        };
        match client.post(&url).json(&payload).send().await {
            Ok(_) => eprintln!(
                "[lifecycle] Resolved promise '{}' for project='{}'",
                resolved_promise, project_id_str
            ),
            Err(e) => eprintln!(
                "[lifecycle] Failed to resolve promise '{}' for project='{}': {}",
                resolved_promise, project_id_str, e
            ),
        }

        // Emit lifecycle:status so the frontend refreshes
        let _ = app_clone.emit(
            "lifecycle:status",
            serde_json::json!({
                "projectId": project_id_str,
                "agentId": agent_id_str,
                "status": "awaiting_approval",
                "summary": summary,
            }),
        );
    });
}

// bp6-1vf: handle_step4_task_done is removed — Restate workflow owns task tracking.
// Task completion is handled in handle_lifecycle_poe_done via promise resolution.

/// Fan-in gate: check all children of `parent_id` and complete/fail the parent
/// if all children are in a terminal state.
fn check_parent_fan_in(app: &AppHandle, parent_id: &str, completed_child_id: &str) {
    eprintln!(
        "[agents:fan-in] checking parent='{}' after child='{}' completed",
        parent_id, completed_child_id
    );

    fn inner(app: &AppHandle, parent_id: &str) -> Result<(), String> {
        let project_state = app.state::<ProjectState>();

        // Fetch parent to get its node_id (needed for the status event).
        let parent_wf = project_state.with_active(|store, _| store.get_workflow(parent_id))?;

        // Only proceed if parent is still running (not already completed/cancelled/failed).
        if parent_wf.status != "running" && parent_wf.status != "pending" {
            return Ok(());
        }

        let child_ids = project_state.with_active(|store, _| {
            store.get_child_workflow_ids(parent_id)
        })?;

        if child_ids.is_empty() {
            return Ok(());
        }

        // Collect statuses of all children.
        let mut all_completed = true;
        let mut failed_ids: Vec<String> = Vec::new();

        for child_id in &child_ids {
            let status = project_state.with_active(|store, _| {
                store.get_workflow_status(child_id)
            })?;
            match status.as_deref() {
                Some("completed") => {}
                Some("failed") => {
                    all_completed = false;
                    failed_ids.push(child_id.clone());
                }
                Some("cancelled") => {
                    // Treat cancelled same as failed for fan-in purposes.
                    all_completed = false;
                    failed_ids.push(child_id.clone());
                }
                _ => {
                    // Still running or pending — not ready yet.
                    all_completed = false;
                }
            }
        }

        // Any failed children? Raise a queue item for the human.
        if !failed_ids.is_empty() {
            for failed_id in &failed_ids {
                eprintln!(
                    "[agents:fan-in] child '{}' failed — creating queue item for parent '{}'",
                    failed_id, parent_id
                );
                let question = format!(
                    "Child workflow {} failed. How should the parent workflow proceed?",
                    failed_id
                );
                let options = vec![
                    crate::dag::QueueItemOption {
                        id: "retry".to_string(),
                        label: "Retry".to_string(),
                        description: Some("Re-spawn the failed child workflow.".to_string()),
                    },
                    crate::dag::QueueItemOption {
                        id: "skip".to_string(),
                        label: "Skip".to_string(),
                        description: Some(
                            "Mark the child as skipped and continue fan-in.".to_string(),
                        ),
                    },
                    crate::dag::QueueItemOption {
                        id: "abort".to_string(),
                        label: "Abort".to_string(),
                        description: Some("Fail the parent workflow entirely.".to_string()),
                    },
                ];
                let item_result: Result<crate::dag::QueueItem, String> =
                    project_state.with_active(|store, project_id| {
                        store.create_queue_item(crate::dag::NewQueueItem {
                            project_id: project_id.to_string(),
                            agent_id: String::new(),
                            workflow_id: Some(parent_id.to_string()),
                            awakeable_id: None,
                            question,
                            options,
                            context_snapshot: serde_json::json!({
                                "parentWorkflowId": parent_id,
                                "failedChildWorkflowId": failed_id,
                            }),
                            priority: 1,
                        })
                    });
                match item_result {
                    Ok(item) => {
                        let _ = app.emit(
                            "queue:item:added",
                            crate::project::QueueItemAddedEvent { item },
                        );
                    }
                    Err(e) => {
                        eprintln!("[agents:fan-in] Failed to create queue item: {}", e);
                    }
                }
            }
            return Ok(());
        }

        // All children completed — complete the parent.
        if all_completed {
            eprintln!(
                "[agents:fan-in] all children of parent '{}' completed — marking parent done",
                parent_id
            );
            project_state.with_active(|store, _| {
                store.update_workflow_status(parent_id, "completed", None, None)
            })?;

            let _ = app.emit(
                "workflow:status",
                WorkflowStatusEvent {
                    workflow_id: parent_id.to_string(),
                    node_id: parent_wf.node_id.clone(),
                    agent_id: parent_wf.agent_id.clone().unwrap_or_default(),
                    status: "completed".to_string(),
                    current_step: Some("fan-in:done".to_string()),
                    started_at: parent_wf.started_at.clone(),
                },
            );
        }

        Ok(())
    }

    if let Err(e) = inner(app, parent_id) {
        eprintln!("[agents:fan-in] Error during fan-in check for parent '{}': {}", parent_id, e);
    }
}

/// Phase 3: agent PTY EOF without poe:done → mark workflow as failed.
fn handle_agent_crash(
    app: &AppHandle,
    agent_id: &str,
    workflow_id: &str,
    node_id: &str,
    started_at: &str,
) {
    let project_state = app.state::<ProjectState>();

    let (failed, parent_workflow_id) = project_state
        .with_active(|store, _| {
            let wf = store.get_workflow(workflow_id)?;
            if wf.status == "running" || wf.status == "pending" {
                store.update_workflow_status(
                    workflow_id,
                    "failed",
                    None,
                    Some("Agent process exited unexpectedly"),
                )?;
                Ok((true, wf.parent_workflow_id))
            } else {
                Ok((false, wf.parent_workflow_id))
            }
        })
        .unwrap_or((false, None));

    if failed {
        let _ = app.emit(
            "workflow:status",
            WorkflowStatusEvent {
                workflow_id: workflow_id.to_string(),
                node_id: node_id.to_string(),
                agent_id: agent_id.to_string(),
                status: "failed".to_string(),
                current_step: None,
                started_at: started_at.to_string(),
            },
        );

        // Fan-in: notify parent that a child has failed.
        if let Some(parent_id) = parent_workflow_id {
            check_parent_fan_in(app, &parent_id, workflow_id);
        }
    }
}
