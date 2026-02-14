use std::collections::HashMap;
use std::process::{Command, Stdio, Child};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::time::SystemTime;
use std::path::PathBuf;
use std::fs::{self, File};
use tauri::{AppHandle, Emitter, State};
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Status of an agent session
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Running,
    Stopped,
    Error,
}

/// Internal session state tracking a running agent process
#[derive(Debug)]
pub struct SessionState {
    /// The running CLI process handle
    pub process: Child,
    /// The bead/issue ID this session is working on (if any)
    pub bead_id: Option<String>,
    /// The persona/role for this session (specialist, product-manager, qa-engineer)
    pub persona: String,
    /// The CLI backend being used (Gemini, ClaudeCode)
    pub backend_id: crate::agent::plugin::BackendId,
    /// Current status of the session
    pub status: SessionStatus,
    /// When this session was created
    pub created_at: SystemTime,
    /// The CLI-provided session ID for resume capability (if available)
    pub cli_session_id: Option<String>,
}

/// Serializable session information for UI display (excludes process handle)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    /// Unique session identifier (UUID v4)
    pub session_id: String,
    /// The bead/issue ID this session is working on (if any)
    pub bead_id: Option<String>,
    /// The persona/role for this session
    pub persona: String,
    /// The CLI backend being used
    pub backend_id: crate::agent::plugin::BackendId,
    /// Current status of the session
    pub status: SessionStatus,
    /// When this session was created (seconds since UNIX epoch)
    pub created_at: u64,
    /// The CLI-provided session ID for resume capability (if available)
    pub cli_session_id: Option<String>,
}

/// Type of log event
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogEventType {
    SessionStart,
    Message,
    Chunk,
    SessionEnd,
}

/// Log event for conversation logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvent {
    pub timestamp: String,
    pub session_id: String,
    pub bead_id: Option<String>,
    pub persona: String,
    pub backend: String,
    pub event_type: LogEventType,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Session logger for conversation persistence
///
/// Logs all agent conversations to ~/.bp6/sessions/<bead-id>/<session-id>-<timestamp>.jsonl
pub struct SessionLogger {
    file_path: PathBuf,
    writer: BufWriter<File>,
}

impl SessionLogger {
    /// Create a new session logger
    ///
    /// # Arguments
    /// * `bead_id` - The bead/issue ID (used for directory organization)
    /// * `session_id` - The session UUID
    ///
    /// # Returns
    /// A new SessionLogger instance or an IO error
    pub fn new(bead_id: Option<&str>, session_id: &str) -> std::io::Result<Self> {
        // Get home directory
        let home_dir = dirs::home_dir()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "Could not find home directory"))?;

        // Build path: ~/.bp6/sessions/<bead-id>/<session-id>-<timestamp>.jsonl
        let bp6_dir = home_dir.join(".bp6").join("sessions");

        let session_dir = if let Some(bid) = bead_id {
            bp6_dir.join(bid)
        } else {
            bp6_dir.join("untracked")
        };

        // Create directory if it doesn't exist
        fs::create_dir_all(&session_dir)?;

        // Generate filename with timestamp
        let timestamp = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let filename = format!("{}-{}.jsonl", session_id, timestamp);
        let file_path = session_dir.join(filename);

        // Open file for writing (append mode)
        let file = File::create(&file_path)?;
        let writer = BufWriter::new(file);

        Ok(SessionLogger { file_path, writer })
    }

    /// Log a structured event
    pub fn log_event(&mut self, event: LogEvent) -> std::io::Result<()> {
        let json = serde_json::to_string(&event)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        writeln!(self.writer, "{}", json)?;
        self.writer.flush()?;
        Ok(())
    }

    /// Log an agent chunk
    pub fn log_chunk(&mut self, session_id: &str, bead_id: Option<&str>, persona: &str, backend: &str, chunk: &crate::agent::plugin::AgentChunk) -> std::io::Result<()> {
        let event = LogEvent {
            timestamp: chrono::Utc::now().to_rfc3339(),
            session_id: session_id.to_string(),
            bead_id: bead_id.map(String::from),
            persona: persona.to_string(),
            backend: backend.to_string(),
            event_type: if chunk.is_done { LogEventType::SessionEnd } else { LogEventType::Chunk },
            content: chunk.content.clone(),
            metadata: None,
        };
        self.log_event(event)
    }

    /// Get the log file path
    pub fn file_path(&self) -> &PathBuf {
        &self.file_path
    }
}

// Old template constants and CliBackend enum removed - now using PersonaPlugin system







/// Helper function to extract the specialist role from a bead.
/// First checks labels for 'specialist:<role>' pattern, then falls back to extra_metadata['role'].
/// Returns None if no role is found.
fn get_role_from_bead(bead: &crate::Bead) -> Option<String> {
    // First check labels for 'specialist:<role>' pattern
    if let Some(labels) = &bead.labels {
        for label in labels {
            if let Some(role) = label.strip_prefix("specialist:") {
                return Some(role.to_string());
            }
        }
    }

    // Fall back to extra_metadata['role']
    bead.extra_metadata
        .get("role")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

// Old helper functions removed - now using PersonaPlugin system
// AgentChunk moved to plugin.rs

/// Global state for managing multiple concurrent agent sessions
pub struct AgentState {
    /// Map of session_id -> SessionState for all active sessions
    pub sessions: Mutex<HashMap<String, SessionState>>,
    /// Registry of available CLI backends (Gemini, ClaudeCode, etc.)
    pub backend_registry: crate::agent::registry::BackendRegistry,
    /// Default backend for new sessions (DEPRECATED: kept for backward compatibility with single-session code)
    #[allow(dead_code)]
    pub current_backend: Mutex<crate::agent::plugin::BackendId>,
    /// CLI session ID for resume capability (DEPRECATED: kept for backward compatibility with single-session code)
    pub current_session_id: Arc<Mutex<Option<String>>>,
    /// The currently active/focused session ID
    pub active_session_id: Arc<Mutex<Option<String>>>,
    /// Registry of available persona plugins
    pub persona_registry: crate::agent::persona::PersonaRegistry,
    /// Template loader for persona prompts
    pub template_loader: crate::agent::templates::TemplateLoader,
}

impl AgentState {
    pub fn new() -> Self {
        AgentState {
            sessions: Mutex::new(HashMap::new()),
            backend_registry: crate::agent::registry::BackendRegistry::with_defaults(),
            current_backend: Mutex::new(crate::agent::plugin::BackendId::Gemini),
            current_session_id: Arc::new(Mutex::new(None)),
            active_session_id: Arc::new(Mutex::new(None)),
            persona_registry: crate::agent::persona::PersonaRegistry::with_defaults(),
            template_loader: crate::agent::templates::TemplateLoader::new()
                .expect("Failed to initialize template loader"),
        }
    }
}

fn kill_process_group(pid: u32) {
    #[cfg(unix)]
    {
        unsafe {
            // Use SIGINT (2) to simulate CTRL-C
            libc::kill(-(pid as i32), libc::SIGINT);
            // Give it a moment to stop, then SIGKILL if it's still there
            std::thread::sleep(std::time::Duration::from_millis(50));
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
}

// Multi-session helper functions

/// Convert all sessions to SessionInfo and emit session-list-changed event
fn emit_session_list_changed(
    app_handle: &AppHandle,
    sessions: &HashMap<String, SessionState>,
) {
    let session_list = list_active_sessions_internal(sessions);
    let _ = app_handle.emit("session-list-changed", session_list);
}

/// Convert HashMap<String, SessionState> to Vec<SessionInfo> for UI consumption
fn list_active_sessions_internal(sessions: &HashMap<String, SessionState>) -> Vec<SessionInfo> {
    sessions
        .iter()
        .map(|(session_id, state)| SessionInfo {
            session_id: session_id.clone(),
            bead_id: state.bead_id.clone(),
            persona: state.persona.clone(),
            backend_id: state.backend_id,
            status: state.status.clone(),
            created_at: state
                .created_at
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            cli_session_id: state.cli_session_id.clone(),
        })
        .collect()
}

// Backend-specific functions removed - now handled by CliBackendPlugin implementations

/// Run CLI command for a specific session (multi-session architecture)
///
/// Spawns a CLI process, manages stdout/stderr reading in separate threads,
/// and includes session_id in all emitted chunks. Returns the Child process
/// handle (with stdout/stderr already taken) for storage in SessionState.
fn run_cli_command_for_session(
    backend_id: crate::agent::plugin::BackendId,
    app_handle: AppHandle,
    state: &AgentState,
    session_id: String,
    bead_id: Option<String>,
    persona: String,
    prompt: String,
    resume: bool,
    cli_session_id: Option<String>,
) -> Result<Child, String> {
    let repo_root = crate::bd::find_repo_root()
        .ok_or_else(|| "Could not locate project root (.beads directory). Please ensure a project is loaded.".to_string())?;

    eprintln!("🎯 Starting session {} in directory: {}", session_id, repo_root.display());

    let backend = state
        .backend_registry
        .get(backend_id)
        .ok_or_else(|| format!("Backend {:?} not registered", backend_id))?;

    let mut cmd = Command::new(backend.command_name());
    let args = backend.build_args(&prompt, resume, cli_session_id.as_deref());
    cmd.args(&args);
    cmd.current_dir(&repo_root);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                libc::setpgid(0, 0);
                Ok(())
            });
        }
    }

    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            let error_msg = if e.kind() == std::io::ErrorKind::NotFound {
                let install_cmd = match backend_id {
                    crate::agent::plugin::BackendId::Gemini => "npm install -g @google/generative-ai-cli",
                    crate::agent::plugin::BackendId::ClaudeCode => "See https://docs.anthropic.com/en/docs/claude-code for installation",
                };
                format!("{} CLI not found. Please install it first: {}", backend.command_name(), install_cmd)
            } else {
                format!("Failed to spawn {} in {}: {}", backend.command_name(), repo_root.display(), e)
            };
            let _ = app_handle.emit("agent-stderr", format!("[Error] {}", error_msg));
            error_msg
        })?;

    eprintln!("🚀 Session {} - Sending prompt:\n{}", session_id, prompt);
    let _ = app_handle.emit("agent-stderr", format!("[Session {}] Sending prompt:\n{}", session_id, prompt));

    // Extract stdout/stderr before spawning threads
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    // Spawn stdout reader thread with logging
    let handle_clone = app_handle.clone();
    let backend_clone = backend.clone();
    let session_id_clone = session_id.clone();
    let bead_id_clone = bead_id.clone();
    let persona_clone = persona.clone();
    let backend_name = backend.command_name().to_string();

    std::thread::spawn(move || {
        // Initialize session logger
        let mut logger = match SessionLogger::new(bead_id_clone.as_deref(), &session_id_clone) {
            Ok(logger) => {
                eprintln!("📝 Session {} logging to: {}", session_id_clone, logger.file_path().display());
                Some(logger)
            }
            Err(e) => {
                eprintln!("⚠️  Failed to create session logger: {}", e);
                None
            }
        };

        // Log session start event
        if let Some(ref mut logger) = logger {
            let start_event = LogEvent {
                timestamp: chrono::Utc::now().to_rfc3339(),
                session_id: session_id_clone.clone(),
                bead_id: bead_id_clone.clone(),
                persona: persona_clone.clone(),
                backend: backend_name.clone(),
                event_type: LogEventType::SessionStart,
                content: String::new(),
                metadata: Some(serde_json::json!({
                    "session_id": session_id_clone,
                })),
            };
            let _ = logger.log_event(start_event);
        }

        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line_str) = line {
                if line_str.trim().starts_with('{') {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line_str) {
                        // Parse using backend plugin
                        if let Some(mut chunk) = backend_clone.parse_stdout_line(&json) {
                            // Add session ID to chunk
                            chunk.session_id = Some(session_id_clone.clone());

                            // Log the chunk
                            if let Some(ref mut logger) = logger {
                                let _ = logger.log_chunk(
                                    &session_id_clone,
                                    bead_id_clone.as_deref(),
                                    &persona_clone,
                                    &backend_name,
                                    &chunk,
                                );
                            }

                            let _ = handle_clone.emit("agent-chunk", chunk);
                        }
                    }
                }
            }
        }

        // Emit final completion chunk
        let final_chunk = crate::agent::plugin::AgentChunk {
            content: "".to_string(),
            is_done: true,
            session_id: Some(session_id_clone.clone()),
        };

        // Log session end
        if let Some(ref mut logger) = logger {
            let end_event = LogEvent {
                timestamp: chrono::Utc::now().to_rfc3339(),
                session_id: session_id_clone.clone(),
                bead_id: bead_id_clone.clone(),
                persona: persona_clone.clone(),
                backend: backend_name.clone(),
                event_type: LogEventType::SessionEnd,
                content: String::new(),
                metadata: None,
            };
            let _ = logger.log_event(end_event);
        }

        let _ = handle_clone.emit("agent-chunk", final_chunk);
    });

    // Spawn stderr reader thread
    let handle_clone_stderr = app_handle.clone();
    let session_id_clone = session_id.clone();
    std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line_str) = line {
                eprintln!("🤖 Session {} Stderr: {}", session_id_clone, line_str);
                let _ = handle_clone_stderr.emit("agent-stderr", format!("[{}] {}", session_id_clone, line_str));
            }
        }
    });

    Ok(child)
}

/// DEPRECATED: Single-session CLI command runner (use run_cli_command_for_session for multi-session)
/// This wrapper is kept for backward compatibility with existing callers
#[allow(dead_code)]
fn run_cli_command(
    backend_id: crate::agent::plugin::BackendId,
    app_handle: AppHandle,
    state: &AgentState,
    prompt: String,
    resume: bool,
) -> Result<(), String> {
    // Get CLI session ID from state for backward compatibility
    let cli_session_id = state.current_session_id.lock().unwrap().clone();

    // Generate a temporary session ID for single-session mode
    let temp_session_id = Uuid::new_v4().to_string();

    // Call new function and discard Child handle
    let _child = run_cli_command_for_session(
        backend_id,
        app_handle,
        state,
        temp_session_id,
        None,  // No bead_id for deprecated single-session mode
        "unknown".to_string(),  // Default persona for deprecated mode
        prompt,
        resume,
        cli_session_id,
    )?;

    Ok(())
}

/// Build prompt using persona plugin system
fn build_prompt_with_persona(
    state: &AgentState,
    persona: &str,
    task: Option<&str>,
    bead_id: Option<&str>,
) -> Result<String, String> {
    use crate::agent::persona::{PersonaContext, PersonaType};

    // Map persona string to PersonaType
    let persona_type = match persona {
        "specialist" => PersonaType::Specialist,
        "product-manager" => PersonaType::ProductManager,
        "qa-engineer" => PersonaType::QaEngineer,
        _ => return Err(format!("Unknown persona: {}", persona)),
    };

    // Get persona plugin from registry
    let persona_plugin = state
        .persona_registry
        .get(persona_type)
        .ok_or_else(|| format!("Persona {:?} not registered", persona_type))?;

    // Get bead and extract information
    let (bead_json, issue_type, role) = if let Some(bid) = bead_id {
        let bead = crate::bd::get_bead_by_id(bid).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(&bead).ok();
        let issue_type = Some(bead.issue_type.clone());
        let role = get_role_from_bead(&bead);
        (json, issue_type, role)
    } else {
        (None, None, None)
    };

    // Build context for persona plugin
    let context = PersonaContext {
        task: task.map(String::from),
        issue_type,
        bead_id: bead_id.map(String::from),
        role,
    };

    // Get template name from persona plugin
    let template_name = persona_plugin.get_template_name(&context)?;

    // Load template using TemplateLoader
    let template_content = state
        .template_loader
        .load_template(persona_type.as_str(), &template_name)
        .map_err(|e| format!("Failed to load template: {}", e))?;

    // Build final prompt using persona plugin
    let prompt = persona_plugin.build_prompt(template_content, &context, bead_json);

    Ok(prompt)
}

#[tauri::command]
pub fn start_agent_session(
    app_handle: AppHandle,
    state: State<'_, AgentState>,
    settings_state: State<'_, crate::SettingsState>,
    persona: String,
    task: Option<String>,
    bead_id: Option<String>,
    cli_backend: Option<String>
) -> Result<String, String> {
    // Generate unique session ID
    let session_id = Uuid::new_v4().to_string();

    // Parse CLI backend from argument, falling back to persisted setting
    let backend = if let Some(backend_str) = cli_backend {
        match backend_str.to_lowercase().as_str() {
            "gemini" => crate::agent::plugin::BackendId::Gemini,
            "claude" | "claude-code" => crate::agent::plugin::BackendId::ClaudeCode,
            _ => {
                let settings = settings_state.settings.lock().map_err(|e| e.to_string())?;
                settings.cli_backend
            }
        }
    } else {
        let settings = settings_state.settings.lock().map_err(|e| e.to_string())?;
        settings.cli_backend
    };

    // Build initial prompt using persona plugin system
    let prompt = build_prompt_with_persona(
        &state,
        &persona,
        task.as_deref(),
        bead_id.as_deref(),
    )?;

    // Start the CLI process for this session
    let child = run_cli_command_for_session(
        backend,
        app_handle.clone(),
        &state,
        session_id.clone(),
        bead_id.clone(),
        persona.clone(),
        prompt,
        false, // resume = false for new session
        None,  // No CLI session ID for new session
    )?;

    // Create SessionState and store in HashMap
    let session_state = SessionState {
        process: child,
        bead_id: bead_id.clone(),
        persona: persona.clone(),
        backend_id: backend,
        status: SessionStatus::Running,
        created_at: SystemTime::now(),
        cli_session_id: None,
    };

    {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.insert(session_id.clone(), session_state);
    }

    // Update active session ID
    {
        let mut active = state.active_session_id.lock().unwrap();
        *active = Some(session_id.clone());
    }

    // Emit session-created event
    let _ = app_handle.emit("session-created", session_id.clone());

    // Emit session-list-changed event
    {
        let sessions = state.sessions.lock().unwrap();
        emit_session_list_changed(&app_handle, &sessions);
    }

    // Emit active-session-changed event
    let _ = app_handle.emit("active-session-changed", session_id.clone());

    Ok(session_id)
}

#[tauri::command]
pub fn send_agent_message(
    app_handle: AppHandle,
    session_id: String,
    message: String,
    state: State<'_, AgentState>
) -> Result<(), String> {
    // Get session info from HashMap
    let (backend_id, cli_session_id, bead_id, persona) = {
        let sessions = state.sessions.lock().unwrap();
        let session = sessions.get(&session_id)
            .ok_or_else(|| format!("Session {} not found", session_id))?;

        (
            session.backend_id,
            session.cli_session_id.clone(),
            session.bead_id.clone(),
            session.persona.clone(),
        )
    };

    // Resume the session with the message
    let _child = run_cli_command_for_session(
        backend_id,
        app_handle,
        &state,
        session_id,
        bead_id,
        persona,
        message,
        true, // resume = true
        cli_session_id,
    )?;

    Ok(())
}

#[tauri::command]
pub fn stop_agent_session(
    app_handle: AppHandle,
    session_id: String,
    state: State<'_, AgentState>
) -> Result<(), String> {
    // Remove session from HashMap and get the Child handle
    let child = {
        let mut sessions = state.sessions.lock().unwrap();
        let session_state = sessions.remove(&session_id)
            .ok_or_else(|| format!("Session {} not found", session_id))?;
        session_state.process
    };

    // Kill the process
    kill_process_group(child.id());

    // Update active session if this was the active one
    {
        let mut active = state.active_session_id.lock().unwrap();
        if active.as_ref() == Some(&session_id) {
            *active = None;
        }
    }

    // Emit session-terminated event
    let _ = app_handle.emit("session-terminated", session_id);

    // Emit session-list-changed event
    {
        let sessions = state.sessions.lock().unwrap();
        emit_session_list_changed(&app_handle, &sessions);
    }

    Ok(())
}

#[tauri::command]
pub fn approve_suggestion(command: String) -> Result<String, String> {
    if !command.starts_with("bd ") {
        return Err("Only 'bd' commands are supported for approval".to_string());
    }

    let args: Vec<String> = command
        .split_whitespace()
        .skip(1)
        .map(|s| s.to_string())
        .collect();

    crate::bd::execute_bd(args)
}

/// List all active agent sessions
///
/// Returns a vector of SessionInfo containing metadata for each active session.
/// Sessions are sorted by creation time (oldest first).
#[tauri::command]
pub fn list_active_sessions(state: State<'_, AgentState>) -> Result<Vec<SessionInfo>, String> {
    let sessions = state.sessions.lock().unwrap();
    let mut session_list = list_active_sessions_internal(&sessions);

    // Sort by creation time (oldest first)
    session_list.sort_by_key(|s| s.created_at);

    Ok(session_list)
}

/// Get the currently active session ID
///
/// Returns the session ID of the currently focused/active session, or None if no session is active.
#[tauri::command]
pub fn get_active_session_id(state: State<'_, AgentState>) -> Result<Option<String>, String> {
    let active_id = state.active_session_id.lock().unwrap();
    Ok(active_id.clone())
}

/// Switch the active session
///
/// Validates that the target session exists and updates the active_session_id.
/// Emits an "active-session-changed" event to notify the UI.
///
/// # Arguments
/// * `session_id` - The session ID to switch to
///
/// # Errors
/// Returns an error if the session doesn't exist
#[tauri::command]
pub fn switch_active_session(
    app_handle: AppHandle,
    session_id: String,
    state: State<'_, AgentState>,
) -> Result<(), String> {
    // Validate that the session exists
    {
        let sessions = state.sessions.lock().unwrap();
        if !sessions.contains_key(&session_id) {
            return Err(format!("Session {} not found", session_id));
        }
    }

    // Update active session ID
    {
        let mut active_id = state.active_session_id.lock().unwrap();
        *active_id = Some(session_id.clone());
    }

    // Emit event to notify UI
    let _ = app_handle.emit(
        "active-session-changed",
        serde_json::json!({ "sessionId": session_id }),
    );

    Ok(())
}

/// Terminate a specific session
///
/// Stops the CLI process for the given session, removes it from the sessions map,
/// and emits appropriate events. If the terminated session was the active session,
/// automatically switches to another session or sets active to None.
///
/// # Arguments
/// * `session_id` - The session ID to terminate
///
/// # Errors
/// Returns an error if the session doesn't exist
#[tauri::command]
pub fn terminate_session(
    app_handle: AppHandle,
    session_id: String,
    state: State<'_, AgentState>,
) -> Result<(), String> {
    // Remove session and get the process handle
    let child = {
        let mut sessions = state.sessions.lock().unwrap();
        let session_state = sessions
            .remove(&session_id)
            .ok_or_else(|| format!("Session {} not found", session_id))?;
        session_state.process
    };

    // Kill the process
    kill_process_group(child.id());

    // Update active session if needed
    {
        let mut active_id = state.active_session_id.lock().unwrap();
        if active_id.as_ref() == Some(&session_id) {
            // Find another session to make active
            let sessions = state.sessions.lock().unwrap();
            *active_id = sessions.keys().next().cloned();
        }
    }

    // Emit events
    let _ = app_handle.emit(
        "session-terminated",
        serde_json::json!({ "sessionId": session_id }),
    );

    // Emit session list changed event
    {
        let sessions = state.sessions.lock().unwrap();
        emit_session_list_changed(&app_handle, &sessions);
    }

    Ok(())
}
