use pulldown_cmark::{html, Options, Parser};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

/// Status of an agent session
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Running,
    Stopped,
    Error,
}

/// Internal session state tracking a running agent PTY session
#[derive(Debug)]
pub struct SessionState {
    /// The bead/issue ID this session is working on (if any)
    pub bead_id: Option<String>,
    /// The persona/role for this session (specialist, product-manager, qa-engineer)
    pub persona: String,
    /// The specific task string passed at session creation time
    pub task: Option<String>,
    /// The specialist role (e.g., flutter, tauri) if persona is 'specialist'
    pub role: Option<String>,
    /// The CLI backend being used (Gemini, ClaudeCode)
    pub backend_id: crate::agent::plugin::BackendId,
    /// Current status of the session
    pub status: SessionStatus,
    /// When this session was created
    pub created_at: SystemTime,
    /// The CLI-provided session ID for resume capability (if available)
    pub cli_session_id: Option<String>,
    /// Timestamp of last activity (last chunk received)
    pub last_activity: SystemTime,
    /// Whether this session has unread messages
    pub has_unread: bool,
    /// Number of messages in this session
    pub message_count: usize,
    /// The project root path at the time this session was created
    pub project_path: String,
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
    /// The specific task string passed at session creation time
    pub task: Option<String>,
    /// The specialist role (e.g., flutter, tauri) if persona is 'specialist'
    pub role: Option<String>,
    /// The CLI backend being used
    pub backend_id: crate::agent::plugin::BackendId,
    /// Current status of the session
    pub status: SessionStatus,
    /// When this session was created (seconds since UNIX epoch)
    pub created_at: u64,
    /// The CLI-provided session ID for resume capability (if available)
    pub cli_session_id: Option<String>,
    /// Timestamp of last activity (seconds since UNIX epoch)
    pub last_activity: u64,
    /// Whether this session has unread messages
    pub has_unread: bool,
    /// Number of messages in this session
    pub message_count: usize,
    /// The project root path at the time this session was created
    pub project_path: String,
}

/// Type of log event
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogEventType {
    SessionStart,
    Message,
    Chunk,
    SessionEnd,
    /// Raw PTY/terminal output captured while the session was in terminal mode.
    /// ANSI escape codes are stripped before storage.
    PtyOutput,
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

/// Conversation message for UI display (reconstructed from log events)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessage {
    /// Message role (user, assistant, system)
    pub role: String,
    /// Message content
    pub content: String,
    /// Timestamp (ISO 8601 format)
    pub timestamp: String,
    /// Structured tool use data (Edit/Write calls) — enables diff rendering in UI
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use: Option<crate::agent::plugin::ToolUseData>,
}

/// Convert markdown text to HTML
///
/// Uses pulldown-cmark to parse markdown and convert to safe HTML.
/// Enables tables, strikethrough, and footnotes extensions.
///
/// # Arguments
/// * `markdown` - The markdown text to convert
///
/// # Returns
/// HTML string with markdown rendered
fn markdown_to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
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
        use std::fs::OpenOptions;

        // Get home directory
        let home_dir = dirs::home_dir().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not find home directory",
            )
        })?;

        // Build path: ~/.bp6/sessions/<bead-id>/<session-id>-<timestamp>.jsonl
        let bp6_dir = home_dir.join(".bp6").join("sessions");

        let session_dir = if let Some(bid) = bead_id {
            bp6_dir.join(bid)
        } else {
            bp6_dir.join("untracked")
        };

        // Create directory if it doesn't exist
        fs::create_dir_all(&session_dir)?;

        // Look for existing log file for this session
        let existing_file = fs::read_dir(&session_dir)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with(&format!("{}-", session_id)))
                    .unwrap_or(false)
            });

        let file_path = if let Some(existing) = existing_file {
            // Reuse existing file
            existing
        } else {
            // Generate new filename with timestamp
            let timestamp = SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let filename = format!("{}-{}.jsonl", session_id, timestamp);
            session_dir.join(filename)
        };

        // Open file in append mode (create if it doesn't exist)
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)?;
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
    pub fn log_chunk(
        &mut self,
        session_id: &str,
        bead_id: Option<&str>,
        persona: &str,
        backend: &str,
        chunk: &crate::agent::plugin::AgentChunk,
    ) -> std::io::Result<()> {
        // Persist tool_use data in metadata so history can reconstruct diff views
        let metadata = chunk.tool_use.as_ref().and_then(|tu| {
            serde_json::to_value(tu).ok().map(|v| serde_json::json!({ "tool_use": v }))
        });
        let event = LogEvent {
            timestamp: chrono::Utc::now().to_rfc3339(),
            session_id: session_id.to_string(),
            bead_id: bead_id.map(String::from),
            persona: persona.to_string(),
            backend: backend.to_string(),
            event_type: if chunk.is_done {
                LogEventType::SessionEnd
            } else {
                LogEventType::Chunk
            },
            content: chunk.content.clone(),
            metadata,
        };
        self.log_event(event)
    }

    /// Log a PTY/terminal output block (ANSI codes should already be stripped)
    pub fn log_pty_output(
        &mut self,
        session_id: &str,
        bead_id: Option<&str>,
        persona: &str,
        backend: &str,
        content: &str,
    ) -> std::io::Result<()> {
        let event = LogEvent {
            timestamp: chrono::Utc::now().to_rfc3339(),
            session_id: session_id.to_string(),
            bead_id: bead_id.map(String::from),
            persona: persona.to_string(),
            backend: backend.to_string(),
            event_type: LogEventType::PtyOutput,
            content: content.to_string(),
            metadata: None,
        };
        self.log_event(event)
    }

    /// Get the log file path
    pub fn file_path(&self) -> &PathBuf {
        &self.file_path
    }
}

/// Strip ANSI escape codes from a string, leaving plain text.
///
/// Handles CSI sequences (`ESC[...m`), OSC sequences (`ESC]...BEL/ST`),
/// and bare ESC + single char sequences common in terminal output.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\x1b' {
            out.push(ch);
            continue;
        }
        match chars.peek() {
            Some(&'[') => {
                // CSI sequence: ESC [ ... <final byte 0x40–0x7E>
                chars.next(); // consume '['
                for c in chars.by_ref() {
                    if c >= '\x40' && c <= '\x7e' {
                        break;
                    }
                }
            }
            Some(&']') => {
                // OSC sequence: ESC ] ... BEL or ESC \
                chars.next(); // consume ']'
                for c in chars.by_ref() {
                    if c == '\x07' {
                        break; // BEL terminator
                    }
                    if c == '\x1b' {
                        // ESC \ (ST) terminator — consume the '\'
                        if chars.peek() == Some(&'\\') {
                            chars.next();
                        }
                        break;
                    }
                }
            }
            Some(_) => {
                // Bare ESC + one char (e.g. ESC M, ESC 7, ESC 8)
                chars.next();
            }
            None => {}
        }
    }
    out
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
    /// PTY manager for terminal sessions
    pub pty_manager: crate::agent::pty::PtyManager,
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
            pty_manager: crate::agent::pty::PtyManager::new(),
        }
    }
}

// Multi-session helper functions

/// Convert all sessions to SessionInfo and emit session-list-changed event
fn emit_session_list_changed(app_handle: &AppHandle, sessions: &HashMap<String, SessionState>) {
    let session_list = list_active_sessions_internal(sessions);
    println!(
        "📡 Emitting session-list-changed with {} sessions",
        session_list.len()
    );
    for session in &session_list {
        println!(
            "  - Session {}: beadId={:?}",
            session.session_id, session.bead_id
        );
    }
    // Wrap in object to match TypeScript interface: { sessions: SessionInfo[] }
    let payload = serde_json::json!({ "sessions": session_list });
    println!(
        "📡 Payload: {}",
        serde_json::to_string_pretty(&payload).unwrap_or_default()
    );
    // Broadcast to ALL windows using Emitter trait
    let _ = app_handle.emit_to(tauri::EventTarget::Any, "session-list-changed", payload);
}

/// Convert HashMap<String, SessionState> to Vec<SessionInfo> for UI consumption
fn list_active_sessions_internal(sessions: &HashMap<String, SessionState>) -> Vec<SessionInfo> {
    sessions
        .iter()
        .map(|(session_id, state)| SessionInfo {
            session_id: session_id.clone(),
            bead_id: state.bead_id.clone(),
            persona: state.persona.clone(),
            task: state.task.clone(),
            role: state.role.clone(),
            backend_id: state.backend_id,
            status: state.status.clone(),
            created_at: state
                .created_at
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            cli_session_id: state.cli_session_id.clone(),
            last_activity: state
                .last_activity
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            has_unread: state.has_unread,
            message_count: state.message_count,
            project_path: state.project_path.clone(),
        })
        .collect()
}

/// Build prompt using persona plugin system
fn build_prompt_with_persona(
    state: &AgentState,
    persona: &str,
    task: Option<&str>,
    bead_id: Option<&str>,
    explicit_role: Option<&str>,
) -> Result<String, String> {
    use crate::agent::persona::{PersonaContext, PersonaType};

    // Map persona string to PersonaType
    let persona_type = match persona {
        "specialist" => PersonaType::Specialist,
        "product-manager" => PersonaType::ProductManager,
        "qa-engineer" => PersonaType::QaEngineer,
        "qc-engineer" => PersonaType::QcEngineer,
        "architect" => PersonaType::Architect,
        "customer" => PersonaType::Customer,
        _ => return Err(format!("Unknown persona: {}", persona)),
    };

    // Get persona plugin from registry
    let persona_plugin = state
        .persona_registry
        .get(persona_type)
        .ok_or_else(|| format!("Persona {:?} not registered", persona_type))?;

    // Get bead and extract information
    let (bead_json, issue_type, bead_role) = if let Some(bid) = bead_id {
        let bead = crate::bd::get_bead_by_id(bid).map_err(|e| e.to_string())?;
        let markdown = Some(bead.to_markdown());
        let issue_type = Some(bead.issue_type.clone());
        let role = get_role_from_bead(&bead);
        (markdown, issue_type, role)
    } else {
        (None, None, None)
    };

    // Use explicit role if provided, otherwise fall back to bead role
    let role = explicit_role.map(String::from).or(bead_role);

    // Build context for persona plugin
    let context = PersonaContext {
        task: task.map(String::from),
        issue_type,
        bead_id: bead_id.map(String::from),
        role,
    };

    // Get template name from persona plugin
    let template_name = persona_plugin.get_template_name(&context)?;

    // Load template using TemplateLoader (two-file architecture)
    let template_content = state
        .template_loader
        .load_persona_prompt(persona_type.as_str(), &template_name)
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
    cli_backend: Option<String>,
    role: Option<String>,
    project_path: Option<String>,
) -> Result<String, String> {
    let session_id = Uuid::new_v4().to_string();

    // Parse CLI backend from argument, falling back to persisted setting
    let backend_id = if let Some(backend_str) = cli_backend {
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

    // Build context prompt — injected into PTY stdin after session starts
    let prompt = build_prompt_with_persona(&state, &persona, task.as_deref(), bead_id.as_deref(), role.as_deref())?;

    // Resolve project root for the PTY working directory
    let resolved_project_path = project_path.unwrap_or_default();
    let repo_root: std::path::PathBuf = if !resolved_project_path.is_empty() {
        std::path::PathBuf::from(&resolved_project_path)
    } else {
        crate::bd::find_repo_root()
            .ok_or_else(|| "Could not locate project root (.beads directory). Please ensure a project is loaded.".to_string())?
    };

    // Get backend plugin and resolve CLI binary path
    let backend = state
        .backend_registry
        .get(backend_id)
        .ok_or_else(|| format!("Backend {:?} not registered", backend_id))?;

    let cli_path = crate::bd::resolve_cli_path(backend.command_name())
        .ok_or_else(|| format!(
            "The '{}' CLI is not found. Please ensure it is installed and available in your PATH.",
            backend.command_name()
        ))?;

    // Build PTY args for a new session (no --resume)
    let args = match backend_id {
        crate::agent::plugin::BackendId::ClaudeCode => vec![
            "--session-id".to_string(),
            session_id.clone(),
            "--dangerously-skip-permissions".to_string(),
        ],
        crate::agent::plugin::BackendId::Gemini => vec![
            "--yolo".to_string(),
        ],
    };

    eprintln!("🚀 Starting PTY session {} ({:?}) in {}", session_id, backend_id, repo_root.display());

    // Spawn the agent CLI in a PTY — this is the single entry point, no mode switching
    state.pty_manager.spawn(
        session_id.clone(),
        cli_path.to_string_lossy().to_string(),
        args,
        Some(repo_root.to_string_lossy().to_string()),
        Some(80),
        Some(24),
    )?;

    // Inject context prompt into PTY stdin after a short delay to allow the agent to initialize
    {
        let pty_manager = state.pty_manager.clone();
        let session_id_clone = session_id.clone();
        let prompt_clone = prompt.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let prompt_with_newline = format!("{}\n", prompt_clone);
            if let Err(e) = pty_manager.write(&session_id_clone, prompt_with_newline.as_bytes()) {
                eprintln!("⚠️  Failed to inject context prompt for session {}: {}", session_id_clone, e);
            }
        });
    }

    // Start reader thread: forwards PTY output as pty-data events and logs to JSONL
    {
        let pty_manager = state.pty_manager.clone();
        let app_handle_clone = app_handle.clone();
        let session_id_clone = session_id.clone();
        let bead_id_for_log = bead_id.clone();
        let persona_for_log = persona.clone();
        let backend_name_for_log = format!("{:?}", backend_id).to_lowercase();

        std::thread::spawn(move || {
            let mut utf8_buf: Vec<u8> = Vec::new();
            let mut log_text_buf = String::new();
            let mut last_log_flush = std::time::Instant::now();
            const LOG_FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

            let mut logger = SessionLogger::new(bead_id_for_log.as_deref(), &session_id_clone).ok();

            loop {
                match pty_manager.read(&session_id_clone) {
                    Ok(data) => {
                        if data.is_empty() {
                            break; // EOF — agent exited
                        }
                        utf8_buf.extend_from_slice(&data);

                        let valid_len = match std::str::from_utf8(&utf8_buf) {
                            Ok(_) => utf8_buf.len(),
                            Err(e) => e.valid_up_to(),
                        };

                        if valid_len > 0 {
                            let output = unsafe {
                                std::str::from_utf8_unchecked(&utf8_buf[..valid_len])
                            }.to_string();
                            utf8_buf.drain(..valid_len);

                            log_text_buf.push_str(&strip_ansi(&output));

                            let _ = app_handle_clone.emit(
                                "pty-data",
                                serde_json::json!({
                                    "sessionId": session_id_clone,
                                    "data": output,
                                }),
                            );
                        }

                        if last_log_flush.elapsed() >= LOG_FLUSH_INTERVAL && !log_text_buf.is_empty() {
                            if let Some(ref mut log) = logger {
                                let _ = log.log_pty_output(
                                    &session_id_clone,
                                    bead_id_for_log.as_deref(),
                                    &persona_for_log,
                                    &backend_name_for_log,
                                    &log_text_buf,
                                );
                            }
                            log_text_buf.clear();
                            last_log_flush = std::time::Instant::now();
                        }
                    }
                    Err(e) => {
                        eprintln!("PTY read error for session {}: {}", session_id_clone, e);
                        break;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }

            // Final flush on PTY close
            if !log_text_buf.is_empty() {
                if let Some(ref mut log) = logger {
                    let _ = log.log_pty_output(
                        &session_id_clone,
                        bead_id_for_log.as_deref(),
                        &persona_for_log,
                        &backend_name_for_log,
                        &log_text_buf,
                    );
                }
            }

            eprintln!("PTY stream ended for session: {}", session_id_clone);
        });
    }

    // Store SessionState
    let now = SystemTime::now();
    let session_state = SessionState {
        bead_id: bead_id.clone(),
        persona: persona.clone(),
        task: task.clone(),
        role: role.clone(),
        backend_id,
        status: SessionStatus::Running,
        created_at: now,
        cli_session_id: Some(session_id.clone()),
        last_activity: now,
        has_unread: false,
        message_count: 0,
        project_path: resolved_project_path,
    };

    {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.insert(session_id.clone(), session_state);
    }

    // Update active session
    {
        let mut active = state.active_session_id.lock().unwrap();
        *active = Some(session_id.clone());
    }

    let _ = app_handle.emit("session-created", session_id.clone());
    {
        let sessions = state.sessions.lock().unwrap();
        emit_session_list_changed(&app_handle, &sessions);
    }
    let _ = app_handle.emit("active-session-changed", session_id.clone());

    Ok(session_id)
}

/// List all active agent sessions
///
/// Returns a vector of SessionInfo containing metadata for each active session.
/// Sessions are sorted by creation time (oldest first).
#[tauri::command]
pub fn list_active_sessions(state: State<'_, AgentState>) -> Result<Vec<SessionInfo>, String> {
    let sessions = state.sessions.lock().unwrap();
    println!(
        "🔍 list_active_sessions: HashMap has {} entries",
        sessions.len()
    );
    for (id, session) in sessions.iter() {
        println!(
            "  - Session {}: bead_id={:?}, persona={}, status={:?}",
            id, session.bead_id, session.persona, session.status
        );
    }
    let mut session_list = list_active_sessions_internal(&sessions);
    println!(
        "🔍 list_active_sessions: Returning {} sessions",
        session_list.len()
    );

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
#[allow(non_snake_case)]
pub fn switch_active_session(
    app_handle: AppHandle,
    sessionId: String,
    state: State<'_, AgentState>,
) -> Result<(), String> {
    // Validate that the session exists
    {
        let sessions = state.sessions.lock().unwrap();
        if !sessions.contains_key(&sessionId) {
            return Err(format!("Session {} not found", sessionId));
        }
    }

    // Update active session ID
    {
        let mut active_id = state.active_session_id.lock().unwrap();
        *active_id = Some(sessionId.clone());
    }

    // Emit event to notify UI
    let _ = app_handle.emit(
        "active-session-changed",
        serde_json::json!({ "sessionId": sessionId }),
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
#[allow(non_snake_case)]
pub fn terminate_session(
    app_handle: AppHandle,
    sessionId: String,
    state: State<'_, AgentState>,
) -> Result<(), String> {
    eprintln!("🗑️  Terminating session: {}", sessionId);

    // Close any windows associated with this session (before terminating)
    // Get WindowRegistry from app state
    if let Some(window_registry) = app_handle.try_state::<crate::window::WindowRegistry>() {
        if let Some(window_label) = window_registry.get_window_label(&sessionId) {
            eprintln!("  🪟 Closing window for session: {}", window_label);

            // Close the window
            if let Some(window) = app_handle.get_webview_window(&window_label) {
                let _ = window.close();
            }

            // Unregister from WindowRegistry
            window_registry.unregister_by_session(&sessionId);
        }
    }

    // Kill the PTY (session_id == PTY key in the PTY-only architecture)
    let _ = state.pty_manager.kill(&sessionId);

    // Remove session from map
    {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.remove(&sessionId);
    }

    // Update active session if needed
    {
        let mut active_id = state.active_session_id.lock().unwrap();
        if active_id.as_ref() == Some(&sessionId) {
            // Find another session to make active
            let sessions = state.sessions.lock().unwrap();
            *active_id = sessions.keys().next().cloned();
        }
    }

    // Emit events
    let _ = app_handle.emit(
        "session-terminated",
        serde_json::json!({ "sessionId": sessionId }),
    );

    // Emit session list changed event
    {
        let sessions = state.sessions.lock().unwrap();
        emit_session_list_changed(&app_handle, &sessions);
    }

    Ok(())
}

/// Get session conversation history from JSONL log files
///
/// Reads conversation logs from ~/.bp6/sessions/<bead-id>/<session-id>-*.jsonl
/// and reconstructs the conversation history for UI display.
///
/// # Arguments
/// * `session_id` - The session UUID to load history for
/// * `bead_id` - Optional bead ID (if None, checks 'untracked' directory)
///
/// # Returns
/// A chronologically ordered vector of conversation messages
///
/// # Errors
/// Returns an error if the log file cannot be found or parsed
#[tauri::command]
#[allow(non_snake_case)]
pub fn get_session_history(
    sessionId: String,
    beadId: Option<String>,
) -> Result<Vec<ConversationMessage>, String> {
    // Get home directory
    let home_dir = dirs::home_dir().ok_or_else(|| "Could not find home directory".to_string())?;

    // Build path: ~/.bp6/sessions/<bead-id>/
    let bp6_dir = home_dir.join(".bp6").join("sessions");
    let session_dir = if let Some(bid) = beadId.as_ref() {
        bp6_dir.join(bid)
    } else {
        bp6_dir.join("untracked")
    };

    // Find the log file for this session
    let log_file = if session_dir.exists() {
        fs::read_dir(&session_dir)
            .map_err(|e| format!("Failed to read session directory: {}", e))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| {
                        name.starts_with(&format!("{}-", sessionId)) && name.ends_with(".jsonl")
                    })
                    .unwrap_or(false)
            })
    } else {
        None
    };

    let log_file_path = log_file.ok_or_else(|| {
        format!(
            "No log file found for session {} in {}",
            sessionId,
            session_dir.display()
        )
    })?;

    // Read and parse JSONL file
    let file = File::open(&log_file_path).map_err(|e| format!("Failed to open log file: {}", e))?;
    let reader = BufReader::new(file);

    let mut messages = Vec::new();
    let mut current_assistant_message: Option<String> = None;
    let mut current_timestamp: Option<String> = None;
    // Consecutive PtyOutput events are merged into one terminal block
    let mut current_pty_output: Option<String> = None;
    let mut current_pty_timestamp: Option<String> = None;

    for line in reader.lines() {
        let line = line.map_err(|e| format!("Failed to read line: {}", e))?;

        // Parse LogEvent
        let event: LogEvent =
            serde_json::from_str(&line).map_err(|e| format!("Failed to parse log event: {}", e))?;

        match event.event_type {
            LogEventType::PtyOutput => {
                // Flush any in-progress assistant message before the terminal block
                if let Some(content) = current_assistant_message.take() {
                    messages.push(ConversationMessage {
                        role: "assistant".to_string(),
                        content: markdown_to_html(&content),
                        timestamp: current_timestamp
                            .take()
                            .unwrap_or_else(|| event.timestamp.clone()),
                        tool_use: None,
                    });
                }
                // Accumulate consecutive PTY chunks into a single terminal block
                let buf = current_pty_output.get_or_insert_with(String::new);
                if current_pty_timestamp.is_none() {
                    current_pty_timestamp = Some(event.timestamp.clone());
                }
                buf.push_str(&event.content);
            }
            other => {
                // Any non-PTY event flushes any accumulated terminal block first
                if let Some(content) = current_pty_output.take() {
                    messages.push(ConversationMessage {
                        role: "terminal".to_string(),
                        content,
                        timestamp: current_pty_timestamp.take().unwrap_or_default(),
                        tool_use: None,
                    });
                }
                match other {
                    LogEventType::Message => {
                        // User message - convert markdown to HTML
                        messages.push(ConversationMessage {
                            role: "user".to_string(),
                            content: markdown_to_html(&event.content),
                            timestamp: event.timestamp,
                            tool_use: None,
                        });
                    }
                    LogEventType::Chunk => {
                        // Check for persisted tool_use data first
                        let tool_use = event.metadata.as_ref()
                            .and_then(|m| m.get("tool_use"))
                            .and_then(|v| serde_json::from_value::<crate::agent::plugin::ToolUseData>(v.clone()).ok());

                        if let Some(tool_use_data) = tool_use {
                            // Flush any in-progress text before inserting the diff
                            if let Some(content) = current_assistant_message.take() {
                                messages.push(ConversationMessage {
                                    role: "assistant".to_string(),
                                    content: markdown_to_html(&content),
                                    timestamp: current_timestamp
                                        .take()
                                        .unwrap_or_else(|| event.timestamp.clone()),
                                    tool_use: None,
                                });
                            }
                            // Emit as a standalone diff message
                            messages.push(ConversationMessage {
                                role: "assistant".to_string(),
                                content: String::new(),
                                timestamp: event.timestamp,
                                tool_use: Some(tool_use_data),
                            });
                        } else {
                            // Plain text chunk - accumulate until done
                            if current_assistant_message.is_none() {
                                current_assistant_message = Some(String::new());
                                current_timestamp = Some(event.timestamp);
                            }
                            if let Some(ref mut msg) = current_assistant_message {
                                msg.push_str(&event.content);
                            }
                        }
                    }
                    LogEventType::SessionEnd => {
                        // Flush accumulated assistant message - convert markdown to HTML
                        if let Some(content) = current_assistant_message.take() {
                            messages.push(ConversationMessage {
                                role: "assistant".to_string(),
                                content: markdown_to_html(&content),
                                timestamp: current_timestamp
                                    .take()
                                    .unwrap_or_else(|| event.timestamp.clone()),
                                tool_use: None,
                            });
                        }
                    }
                    LogEventType::SessionStart | LogEventType::PtyOutput => {
                        // SessionStart: metadata only; PtyOutput: handled above
                    }
                }
            }
        }
    }

    // Flush any trailing PTY block (session ended while in terminal mode)
    if let Some(content) = current_pty_output.take() {
        messages.push(ConversationMessage {
            role: "terminal".to_string(),
            content,
            timestamp: current_pty_timestamp.take().unwrap_or_default(),
            tool_use: None,
        });
    }

    // DO NOT flush remaining assistant message - it means the session is still streaming
    // and that message will come through live via agent-chunk events.
    // Only completed messages (with SessionEnd markers) should be in history.
    // This prevents duplicate messages when switching to an active session.

    Ok(messages)
}

/// Mark a session as read (clear unread indicator)
///
/// Clears the has_unread flag for a session when the user views it.
/// Re-emits session-list-changed to update UI indicators.
///
/// # Arguments
/// * `app_handle` - Tauri app handle for event emission
/// * `session_id` - The session ID to mark as read
/// * `state` - AgentState containing sessions
///
/// # Returns
/// Ok(()) on success, or error if session not found
#[tauri::command]
#[allow(non_snake_case)]
pub fn mark_session_read(
    app_handle: AppHandle,
    sessionId: String,
    state: State<'_, AgentState>,
) -> Result<(), String> {
    // Update has_unread flag
    {
        let mut sessions = state.sessions.lock().unwrap();
        let session = sessions
            .get_mut(&sessionId)
            .ok_or_else(|| format!("Session {} not found", sessionId))?;

        session.has_unread = false;

        // Re-emit session list with updated state
        emit_session_list_changed(&app_handle, &sessions);
    }

    Ok(())
}


// ============================================================================
// Session Resume Index Commands
// ============================================================================

/// Check if there's a recent session for a bead/persona combination
///
/// This allows the UI to automatically resume conversations when reopening
/// a chat for the same bead and persona.
///
/// # Arguments
/// * `bead_id` - Optional bead ID (None for untracked)
/// * `persona` - The persona (product-manager, qa-engineer, etc.)
///
/// # Returns
/// The session metadata if found, None otherwise
#[tauri::command]
#[allow(non_snake_case)]
pub fn find_recent_session(
    beadId: Option<String>,
    persona: String,
    backendId: String,
) -> Result<Option<super::session_index::SessionMetadata>, String> {
    let index = super::session_index::SessionIndex::load()?;
    Ok(index.get_session(beadId.as_deref(), &persona, &backendId).cloned())
}

/// Record a session in the resume index
///
/// Called when a session is created or becomes active to enable
/// automatic resumption later.
///
/// # Arguments
/// * `bead_id` - Optional bead ID (None for untracked)
/// * `persona` - The persona
/// * `session_id` - The session UUID
/// * `cli_session_id` - The CLI-provided session ID (for resume)
/// * `backend_id` - The backend being used (gemini, claude-code)
#[tauri::command]
#[allow(non_snake_case)]
pub fn record_session_for_resume(
    beadId: Option<String>,
    persona: String,
    sessionId: String,
    cliSessionId: Option<String>,
    backendId: String,
) -> Result<(), String> {
    let mut index = super::session_index::SessionIndex::load()?;
    index.record_session(
        beadId.as_deref(),
        &persona,
        sessionId,
        cliSessionId,
        backendId,
    );
    index.save()?;
    Ok(())
}

/// Update the last active timestamp for a session
///
/// Called when a message is sent to keep the session fresh for resumption.
#[tauri::command]
#[allow(non_snake_case)]
pub fn touch_session(
    beadId: Option<String>,
    persona: String,
    backendId: String,
) -> Result<(), String> {
    let mut index = super::session_index::SessionIndex::load()?;
    index.touch_session(beadId.as_deref(), &persona, &backendId);
    index.save()?;
    Ok(())
}


// ============================================================================
// PTY Terminal Interface Commands
// ============================================================================

/// Write data to a PTY session
///
/// In the PTY-only architecture, session_id == PTY key for agent sessions.
/// Project shells (spawn_project_shell) also use their session_id as PTY key,
/// so this function handles both transparently.
#[tauri::command]
#[allow(non_snake_case)]
pub fn write_to_pty(
    sessionId: String,
    data: String,
    state: State<AgentState>,
) -> Result<(), String> {
    state.pty_manager.write(&sessionId, data.as_bytes())
}

/// Resize a PTY session
#[tauri::command]
#[allow(non_snake_case)]
pub fn resize_pty(
    sessionId: String,
    cols: u16,
    rows: u16,
    state: State<AgentState>,
) -> Result<(), String> {
    state.pty_manager.resize(&sessionId, cols, rows)
}


/// Derive a safe tmux session name from a project path.
/// Uses the last path component, sanitizes to alphanumeric + hyphens, max 20 chars.
fn tmux_session_name(project_path: &str) -> String {
    let dir = std::path::Path::new(project_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");

    let name: String = dir
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .take(20)
        .collect();

    let trimmed = name.trim_matches('-').to_string();
    if trimmed.is_empty() { "project".to_string() } else { trimmed }
}

/// Spawn a standalone shell PTY anchored to a project directory.
/// Uses tmux if available: attaches to an existing session named after the project,
/// or creates a new one. Falls back to $SHELL if tmux is not installed.
/// Idempotent: if a PTY already exists for `session_id`, returns Ok(false) immediately.
/// Returns Ok(true) if a new session was created, Ok(false) if it already existed.
#[tauri::command]
pub fn spawn_project_shell(
    session_id: String,
    project_path: String,
    cols: Option<u16>,
    rows: Option<u16>,
    state: State<AgentState>,
    app_handle: AppHandle,
) -> Result<bool, String> {
    // Idempotent: don't spawn a second shell if one already exists
    if state.pty_manager.has_session(&session_id) {
        eprintln!("⚡ spawn_project_shell: PTY {} already exists, skipping", session_id);
        return Ok(false);
    }

    // Prefer tmux: `tmux new-session -A` attaches to existing session or creates new one
    let tmux_path = crate::bd::resolve_cli_path("tmux");

    let (cmd, args) = if let Some(tmux) = tmux_path {
        let session_name = tmux_session_name(&project_path);
        eprintln!("🖥️  spawn_project_shell: tmux session '{}' in {}", session_name, project_path);
        (
            tmux.to_string_lossy().to_string(),
            vec![
                "new-session".to_string(),
                "-A".to_string(),          // attach if session exists, else create
                "-s".to_string(), session_name,
                "-c".to_string(), project_path.clone(), // working dir (new session only)
            ],
        )
    } else {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| {
            if cfg!(target_os = "macos") { "/bin/zsh".to_string() } else { "/bin/bash".to_string() }
        });
        eprintln!("🖥️  spawn_project_shell: tmux not found, using {} in {}", shell, project_path);
        (shell, vec![])
    };

    state.pty_manager.spawn(
        session_id.clone(),
        cmd,
        args,
        Some(project_path),
        Some(cols.unwrap_or(80)),
        Some(rows.unwrap_or(24)),
    )?;

    // Start reader thread that streams PTY output as pty-data events
    let pty_manager = state.pty_manager.clone();
    let app_handle_clone = app_handle.clone();
    let session_id_clone = session_id.clone();

    std::thread::spawn(move || {
        // Buffer for incomplete UTF-8 sequences split across PTY reads.
        // from_utf8_lossy would replace partial sequences with U+FFFD; instead
        // we carry over the trailing incomplete bytes to the next read.
        let mut utf8_buf: Vec<u8> = Vec::new();

        loop {
            match pty_manager.read(&session_id_clone) {
                Ok(data) => {
                    if data.is_empty() {
                        break; // EOF — shell exited naturally
                    }
                    utf8_buf.extend_from_slice(&data);

                    // Find the longest valid UTF-8 prefix and hold back any
                    // trailing incomplete multi-byte sequence for the next read.
                    let valid_len = match std::str::from_utf8(&utf8_buf) {
                        Ok(_) => utf8_buf.len(),
                        Err(e) => e.valid_up_to(),
                    };

                    if valid_len > 0 {
                        // Safety: valid_len is guaranteed valid UTF-8 by from_utf8
                        let output = unsafe {
                            std::str::from_utf8_unchecked(&utf8_buf[..valid_len])
                        }.to_string();
                        utf8_buf.drain(..valid_len);

                        let _ = app_handle_clone.emit(
                            "pty-data",
                            serde_json::json!({
                                "sessionId": session_id_clone,
                                "data": output,
                            }),
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Project shell PTY read error ({}): {}", session_id_clone, e);
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        // Remove dead session from PtyManager so the next spawn_project_shell call
        // doesn't see has_session() = true and skip the respawn.
        pty_manager.try_remove(&session_id_clone);

        // Tell the frontend the shell has exited so it can respawn.
        let _ = app_handle_clone.emit(
            "project-shell-exited",
            serde_json::json!({ "sessionId": session_id_clone }),
        );

        eprintln!("Project shell PTY stream ended: {}", session_id_clone);
    });

    Ok(true) // New session was created
}

/// Kill the project shell PTY for a given session ID.
/// Safe to call even if the shell has already exited.
#[tauri::command]
pub fn kill_project_shell(
    session_id: String,
    state: State<AgentState>,
) -> Result<(), String> {
    if !state.pty_manager.has_session(&session_id) {
        return Ok(()); // Already gone
    }
    state.pty_manager.kill(&session_id)?;
    eprintln!("🗑️  kill_project_shell: killed PTY {}", session_id);
    Ok(())
}
