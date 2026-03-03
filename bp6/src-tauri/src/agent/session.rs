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
    /// Title of the bead this session is working on (if any)
    pub bead_title: Option<String>,
    /// First two lines of the bead description (if any)
    pub bead_description_preview: Option<String>,
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
    /// Title of the bead this session is working on (if any)
    pub bead_title: Option<String>,
    /// First two lines of the bead description (if any)
    pub bead_description_preview: Option<String>,
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
    /// Text submitted by the human (flushed on Enter).
    UserInput,
    /// Image pasted by the human — content is the saved file path.
    UserImage,
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

    /// Log a line of text submitted by the human (on Enter)
    pub fn log_user_input(
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
            event_type: LogEventType::UserInput,
            content: content.to_string(),
            metadata: None,
        };
        self.log_event(event)
    }

    /// Log an image pasted by the human — content is the saved file path
    pub fn log_user_image(
        &mut self,
        session_id: &str,
        bead_id: Option<&str>,
        persona: &str,
        backend: &str,
        image_path: &str,
    ) -> std::io::Result<()> {
        let event = LogEvent {
            timestamp: chrono::Utc::now().to_rfc3339(),
            session_id: session_id.to_string(),
            bead_id: bead_id.map(String::from),
            persona: persona.to_string(),
            backend: backend.to_string(),
            event_type: LogEventType::UserImage,
            content: image_path.to_string(),
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
    /// Shared loggers keyed by session ID (used by write_to_pty for user input logging)
    pub session_loggers: Arc<Mutex<HashMap<String, Arc<Mutex<SessionLogger>>>>>,
    /// Per-session input buffers for accumulating keystrokes until Enter is pressed
    pub input_buffers: Arc<Mutex<HashMap<String, String>>>,
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
            session_loggers: Arc::new(Mutex::new(HashMap::new())),
            input_buffers: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

/// Write a small sidecar metadata file alongside the session JSONL.
///
/// Path: ~/.bp6/sessions/<bead-id>/<session-id>.session.json
/// Records the backend, persona, and task so resume logic can filter correctly.
fn write_session_meta(
    bead_id: Option<&str>,
    session_id: &str,
    backend: crate::agent::plugin::BackendId,
    persona: &str,
    task: Option<&str>,
) {
    let home_dir = match dirs::home_dir() {
        Some(h) => h,
        None => return,
    };
    let session_dir = home_dir
        .join(".bp6")
        .join("sessions")
        .join(bead_id.unwrap_or("untracked"));
    let _ = std::fs::create_dir_all(&session_dir);

    let backend_str = match backend {
        crate::agent::plugin::BackendId::ClaudeCode => "claude",
        crate::agent::plugin::BackendId::Gemini => "gemini",
    };
    let meta = serde_json::json!({
        "session_id": session_id,
        "backend": backend_str,
        "persona": persona,
        "task": task,
        "bead_id": bead_id,
        "started_at": SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    });
    if let Ok(json) = serde_json::to_string_pretty(&meta) {
        let _ = std::fs::write(
            session_dir.join(format!("{}.session.json", session_id)),
            json,
        );
    }
}

/// Find the most recent session UUID for a given bead + backend combination by scanning
/// ~/.bp6/sessions/<bead-id>/.
///
/// JSONL files are named `<session-uuid>-<unix-timestamp>.jsonl`. Each has a sidecar
/// `<session-uuid>.session.json` recording the backend. We only resume sessions that
/// match the requested backend (you shouldn't resume a Gemini session with Claude or
/// vice versa).
///
/// Returns None if no matching prior session exists or bead_id is not provided.
/// Find the most recent Gemini session UUID for a given project directory by running
/// `gemini --list-sessions` as a subprocess.
///
/// This is more reliable than scanning the filesystem directly because:
/// - Gemini only saves sessions after meaningful interaction, so files may not exist yet
/// - It uses Gemini's own project-registry lookup (no need to replicate the path→id mapping)
/// - Output format: "  N. Title (time ago) [uuid]" — we parse the UUID from the last line
///
/// Sessions are listed oldest-first, so the last line is the most recent.
/// Returns None if no sessions exist or gemini cannot be run.
/// Send a prompt to Gemini in non-interactive JSON mode and return the session_id.
///
/// Uses `gemini --prompt "<prompt>" --output-format json` which is fast, non-blocking
/// from the UI's perspective (called via spawn_blocking), and returns structured JSON
/// containing `session_id`. This is the canonical way to create a new Gemini session
/// and capture its UUID without any PTY/TUI overhead.
///
/// The stdout contains non-JSON preamble lines ("Loaded cached credentials." etc.)
/// before the JSON object — we skip to the first `{` to parse.
fn create_gemini_session_via_json(
    gemini_cli: &str,
    project_path: &str,
    prompt: &str,
) -> Option<String> {
    let output = std::process::Command::new(gemini_cli)
        .args(["--prompt", prompt, "--output-format", "json"])
        .current_dir(project_path)
        .env("PWD", project_path)
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    eprintln!("🔍 gemini JSON init output:\n{}", stdout.trim());

    // Skip non-JSON preamble (credentials, server info) and parse from first `{`
    let json_start = stdout.find('{')?;
    let json: serde_json::Value = serde_json::from_str(&stdout[json_start..]).ok()?;
    let session_id = json["session_id"].as_str().map(String::from);

    if let Some(ref id) = session_id {
        eprintln!("✅ Gemini JSON session created: {}", id);
    }
    session_id
}

/// Update the sidecar file for a session with the Gemini-assigned session UUID.
/// This UUID is what `gemini --resume <uuid>` needs, distinct from our own session_id.
fn store_gemini_session_id(bead_id: Option<&str>, our_session_id: &str, gemini_uuid: &str) {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return,
    };
    let meta_path = home
        .join(".bp6")
        .join("sessions")
        .join(bead_id.unwrap_or("untracked"))
        .join(format!("{}.session.json", our_session_id));

    if let Ok(content) = std::fs::read_to_string(&meta_path) {
        if let Ok(mut meta) = serde_json::from_str::<serde_json::Value>(&content) {
            meta["gemini_session_id"] = serde_json::json!(gemini_uuid);
            if let Ok(json) = serde_json::to_string_pretty(&meta) {
                let _ = std::fs::write(&meta_path, json);
                eprintln!("💾 Stored gemini_session_id {} for {}", gemini_uuid, our_session_id);
            }
        }
    }
}

/// Returns `(our_session_id, gemini_session_id)` for the most recent session matching
/// the given bead + backend. `gemini_session_id` is only populated for Gemini sessions
/// that have been through the JSON-init flow (stored in the sidecar).
fn find_latest_session_id(
    bead_id: Option<&str>,
    backend: crate::agent::plugin::BackendId,
) -> Option<(String, Option<String>)> {
    let home_dir = dirs::home_dir()?;
    let session_dir = home_dir
        .join(".bp6")
        .join("sessions")
        .join(bead_id?);

    if !session_dir.exists() {
        return None;
    }

    let backend_str = match backend {
        crate::agent::plugin::BackendId::ClaudeCode => "claude",
        crate::agent::plugin::BackendId::Gemini => "gemini",
    };

    let mut entries: Vec<(u64, String, Option<String>)> = std::fs::read_dir(&session_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let stem = name.strip_suffix(".jsonl")?;
            let last_dash = stem.rfind('-')?;
            let ts: u64 = stem[last_dash + 1..].parse().ok()?;
            let session_id = stem[..last_dash].to_string();

            let meta_path = session_dir.join(format!("{}.session.json", session_id));
            let meta = std::fs::read_to_string(&meta_path).ok()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())?;

            if meta["backend"].as_str().map_or(true, |b| b != backend_str) {
                return None; // Wrong backend — skip
            }

            let gemini_uuid = meta["gemini_session_id"].as_str().map(String::from);
            Some((ts, session_id, gemini_uuid))
        })
        .collect();

    entries.sort_by(|a, b| b.0.cmp(&a.0));
    entries.into_iter().next().map(|(_, sid, gemini_uuid)| (sid, gemini_uuid))
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
            bead_title: state.bead_title.clone(),
            bead_description_preview: state.bead_description_preview.clone(),
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
pub async fn start_agent_session(
    app_handle: AppHandle,
    state: State<'_, AgentState>,
    settings_state: State<'_, crate::SettingsState>,
    persona: String,
    task: Option<String>,
    bead_id: Option<String>,
    cli_backend: Option<String>,
    role: Option<String>,
    project_path: Option<String>,
    force_new: Option<bool>,
) -> Result<String, String> {
    let force_new = force_new.unwrap_or(false);

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

    // Resume the most recent session for this bead + backend by scanning
    // ~/.bp6/sessions/<bead-id>/, unless the caller explicitly requests a fresh session.
    let (session_id, is_resume, stored_gemini_uuid) = if !force_new {
        match find_latest_session_id(bead_id.as_deref(), backend_id) {
            Some((existing_id, gemini_uuid)) => (existing_id, true, gemini_uuid),
            None => (Uuid::new_v4().to_string(), false, None),
        }
    } else {
        (Uuid::new_v4().to_string(), false, None)
    };

    eprintln!(
        "🔄 Session {} ({:?}) — {}",
        session_id, backend_id,
        if is_resume { "resuming" } else { "new" }
    );

    // Fetch bead metadata for the session header (title + description preview)
    let (bead_title, bead_description_preview) = if let Some(ref bid) = bead_id {
        match crate::bd::get_bead_by_id(bid) {
            Ok(bead) => {
                let preview = bead.description.as_deref().map(|desc| {
                    desc.lines().take(2).collect::<Vec<_>>().join("\n")
                });
                (Some(bead.title), preview)
            }
            Err(_) => (None, None),
        }
    } else {
        (None, None)
    };

    // Build context prompt — passed as positional arg to the CLI
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

    // Build PTY args.
    //
    // Resume vs fresh differs per backend:
    //
    //   Claude Code:
    //     Fresh:  `claude --session-id <new-uuid> --dangerously-skip-permissions "<prompt>"`
    //     Resume: `claude -r <uuid> --dangerously-skip-permissions`
    //
    //   Gemini:
    //     Fresh:  run `gemini --prompt "<prompt>" --output-format json` (non-PTY subprocess)
    //             to send the initial context prompt and capture the session_id from JSON.
    //             Store session_id in sidecar, then PTY with `gemini --yolo -r <uuid>`.
    //     Resume: read stored gemini_session_id from sidecar → PTY with `gemini --yolo -r <uuid>`.
    //             If the UUID is stale, "Error resuming session:" appears in PTY output;
    //             the reader thread detects this, creates a new session via JSON init, and respawns.
    let args = match backend_id {
        crate::agent::plugin::BackendId::ClaudeCode => {
            if is_resume {
                vec![
                    "-r".to_string(),
                    session_id.clone(),
                    "--dangerously-skip-permissions".to_string(),
                ]
            } else {
                vec![
                    "--session-id".to_string(),
                    session_id.clone(),
                    "--dangerously-skip-permissions".to_string(),
                    prompt.clone(),
                ]
            }
        }
        crate::agent::plugin::BackendId::Gemini => {
            // Determine which Gemini session UUID to use:
            //
            //   Resume + stored UUID → use it directly, no subprocess
            //   Fresh (or resume without stored UUID) → run JSON init to create a new
            //     session and capture its UUID; the initial prompt is sent there so the
            //     PTY opens with context already loaded via `-r <uuid>`
            let gemini_uuid = if is_resume {
                stored_gemini_uuid
            } else {
                // Fresh session: send the initial prompt via non-interactive JSON mode.
                // `gemini --prompt "<prompt>" --output-format json` returns session_id
                // in the JSON output without any TUI/PTY overhead or --list-sessions hang.
                let path = repo_root.to_str().unwrap_or("").to_string();
                let cli = cli_path.to_string_lossy().to_string();
                let init_prompt = prompt.clone();
                let bead_id_for_store = bead_id.clone();
                let session_id_for_store = session_id.clone();

                let uuid = tauri::async_runtime::spawn_blocking(move || {
                    create_gemini_session_via_json(&cli, &path, &init_prompt)
                })
                .await
                .unwrap_or(None);

                if let Some(ref u) = uuid {
                    store_gemini_session_id(bead_id_for_store.as_deref(), &session_id_for_store, u);
                }
                uuid
            };

            if let Some(uuid) = gemini_uuid {
                // Resume the session (or the newly JSON-initialised one)
                vec!["--yolo".to_string(), "-r".to_string(), uuid]
            } else {
                // JSON init failed — fall back to interactive prompt
                vec![
                    "--yolo".to_string(),
                    "--prompt-interactive".to_string(),
                    prompt.clone(),
                ]
            }
        }
    };

    eprintln!("🚀 Starting PTY session {} ({:?}) in {} | cli: {}", session_id, backend_id, repo_root.display(), cli_path.display());

    // Write sidecar metadata before spawning so it exists even if spawn fails
    write_session_meta(
        bead_id.as_deref(),
        &session_id,
        backend_id,
        &persona,
        task.as_deref(),
    );

    // Spawn the agent CLI in a PTY — this is the single entry point, no mode switching
    state.pty_manager.spawn(
        session_id.clone(),
        cli_path.to_string_lossy().to_string(),
        args,
        Some(repo_root.to_string_lossy().to_string()),
        Some(80),
        Some(24),
    )?;

    // Create logger and share between reader thread and write_to_pty
    let backend_name_for_log = format!("{:?}", backend_id).to_lowercase();
    let shared_logger: Option<Arc<Mutex<SessionLogger>>> =
        SessionLogger::new(bead_id.as_deref(), &session_id)
            .ok()
            .map(|l| Arc::new(Mutex::new(l)));

    if let Some(ref l) = shared_logger {
        state
            .session_loggers
            .lock()
            .unwrap()
            .insert(session_id.clone(), l.clone());
    }

    // Start reader thread: forwards PTY output as pty-data events and logs to JSONL.
    //
    // For Gemini resume attempts we also watch for "Error resuming session:" in the
    // output. If Gemini exits with that error we automatically respawn it as a fresh
    // session (same session ID, same terminal window — transparent to the user).
    {
        let pty_manager = state.pty_manager.clone();
        let app_handle_clone = app_handle.clone();
        let session_id_clone = session_id.clone();
        let bead_id_for_log = bead_id.clone();
        let persona_for_log = persona.clone();
        let backend_name_for_log = backend_name_for_log.clone();
        let logger_for_thread = shared_logger.clone();

        // Captures for Gemini resume-failure auto-restart.
        // If the PTY prints "Error resuming session:" the stored UUID is stale.
        // On EOF we create a fresh session via JSON init, store the new UUID, and respawn.
        let watch_resume_error =
            is_resume && backend_id == crate::agent::plugin::BackendId::Gemini;
        let fresh_restart_cli = cli_path.to_string_lossy().to_string();
        let fresh_restart_prompt = prompt.clone();
        let fresh_restart_dir = repo_root.to_string_lossy().to_string();
        let fresh_restart_bead_id = bead_id.clone();
        let fresh_restart_session_id = session_id.clone();

        std::thread::spawn(move || {
            const LOG_FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

            // Outer loop: runs once normally, re-runs if a resume-failure restart occurs.
            'session: loop {
                let mut utf8_buf: Vec<u8> = Vec::new();
                let mut log_text_buf = String::new();
                let mut last_log_flush = std::time::Instant::now();
                let mut resume_error_seen = false;

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

                                // Detect Gemini resume failure before emitting to UI
                                if watch_resume_error && !resume_error_seen
                                    && strip_ansi(&output).contains("Error resuming session:")
                                {
                                    resume_error_seen = true;
                                    eprintln!(
                                        "🔄 Gemini resume failed for {} — will restart fresh on EOF",
                                        session_id_clone
                                    );
                                }

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
                                if let Some(ref l) = logger_for_thread {
                                    let _ = l.lock().unwrap().log_pty_output(
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
                            break 'session;
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }

                // Final log flush for this PTY lifetime
                if !log_text_buf.is_empty() {
                    if let Some(ref l) = logger_for_thread {
                        let _ = l.lock().unwrap().log_pty_output(
                            &session_id_clone,
                            bead_id_for_log.as_deref(),
                            &persona_for_log,
                            &backend_name_for_log,
                            &log_text_buf,
                        );
                    }
                }

                // If the resume UUID was stale, create a fresh session via JSON init,
                // store the new UUID, and respawn the PTY with `-r <new_uuid>`.
                if resume_error_seen {
                    eprintln!("🔄 Gemini UUID stale — creating fresh session for {}", session_id_clone);
                    let new_uuid = create_gemini_session_via_json(
                        &fresh_restart_cli,
                        &fresh_restart_dir,
                        &fresh_restart_prompt,
                    );
                    if let Some(ref u) = new_uuid {
                        store_gemini_session_id(
                            fresh_restart_bead_id.as_deref(),
                            &fresh_restart_session_id,
                            u,
                        );
                        let restart_args = vec![
                            "--yolo".to_string(),
                            "-r".to_string(),
                            u.clone(),
                        ];
                        if pty_manager
                            .spawn(
                                session_id_clone.clone(),
                                fresh_restart_cli.clone(),
                                restart_args,
                                Some(fresh_restart_dir.clone()),
                                Some(80),
                                Some(24),
                            )
                            .is_ok()
                        {
                            continue 'session;
                        }
                    }
                }

                break 'session;
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
        bead_title,
        bead_description_preview,
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

    // Clean up logger and input buffer
    state.session_loggers.lock().unwrap().remove(&sessionId);
    state.input_buffers.lock().unwrap().remove(&sessionId);

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
                    LogEventType::UserInput => {
                        messages.push(ConversationMessage {
                            role: "user".to_string(),
                            content: event.content.clone(),
                            timestamp: event.timestamp,
                            tool_use: None,
                        });
                    }
                    LogEventType::UserImage => {
                        // content is the saved image file path
                        messages.push(ConversationMessage {
                            role: "user".to_string(),
                            content: format!("[image: {}]", event.content),
                            timestamp: event.timestamp,
                            tool_use: None,
                        });
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
    // Write to PTY first
    state.pty_manager.write(&sessionId, data.as_bytes())?;

    // Buffer keystrokes and log complete submissions (flushed on Enter / \r)
    let mut line_to_log: Option<String> = None;
    {
        let mut buffers = state.input_buffers.lock().unwrap();
        let buf = buffers.entry(sessionId.clone()).or_default();
        for ch in data.chars() {
            match ch {
                '\r' | '\n' => {
                    if !buf.is_empty() {
                        line_to_log = Some(std::mem::take(buf));
                    }
                }
                '\x7f' | '\x08' => { buf.pop(); } // backspace / DEL
                c if !c.is_control() => buf.push(c),
                _ => {} // ignore other control chars (arrows, escape sequences)
            }
        }
    } // input_buffers lock released

    if let Some(line) = line_to_log {
        // Look up session metadata (no lock held from above)
        let (bead_id, persona, backend) = {
            let sessions = state.sessions.lock().unwrap();
            sessions.get(&sessionId).map(|s| (
                s.bead_id.clone(),
                s.persona.clone(),
                format!("{:?}", s.backend_id).to_lowercase(),
            )).unwrap_or_default()
        };
        let loggers = state.session_loggers.lock().unwrap();
        if let Some(logger) = loggers.get(&sessionId) {
            let _ = logger.lock().unwrap().log_user_input(
                &sessionId,
                bead_id.as_deref(),
                &persona,
                &backend,
                &line,
            );
        }
    }

    Ok(())
}

/// Save an image pasted by the human and log a UserImage event to the session JSONL.
///
/// Called from the frontend when the user pastes an image into the terminal.
/// Saves the raw image bytes as a PNG under ~/.bp6/sessions/<bead-id>/<session-id>/images/
/// and writes a UserImage log entry with the saved file path as content.
#[tauri::command]
#[allow(non_snake_case)]
pub fn save_session_image(
    sessionId: String,
    imageData: Vec<u8>,
    state: State<'_, AgentState>,
) -> Result<String, String> {
    // Look up session metadata
    let (bead_id, persona, backend) = {
        let sessions = state.sessions.lock().unwrap();
        sessions.get(&sessionId).map(|s| (
            s.bead_id.clone(),
            s.persona.clone(),
            format!("{:?}", s.backend_id).to_lowercase(),
        )).unwrap_or_default()
    };

    // Build images directory path
    let home = std::env::var("HOME").unwrap_or_default();
    let session_dir = match &bead_id {
        Some(bid) => format!("{}/.bp6/sessions/{}/{}", home, bid, sessionId),
        None => format!("{}/.bp6/sessions/untracked/{}", home, sessionId),
    };
    let images_dir = format!("{}/images", session_dir);
    std::fs::create_dir_all(&images_dir).map_err(|e| e.to_string())?;

    // Save image with ISO timestamp filename
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ").to_string();
    let image_path = format!("{}/{}.png", images_dir, ts);
    std::fs::write(&image_path, &imageData).map_err(|e| e.to_string())?;

    // Log the event
    let loggers = state.session_loggers.lock().unwrap();
    if let Some(logger) = loggers.get(&sessionId) {
        let _ = logger.lock().unwrap().log_user_image(
            &sessionId,
            bead_id.as_deref(),
            &persona,
            &backend,
            &image_path,
        );
    }

    Ok(image_path)
}

/// Statistics extracted from a completed or in-progress agent session.
///
/// For Claude Code sessions these are parsed from Claude's own session JSONL
/// (`~/.claude/projects/<encoded-path>/<session-id>.jsonl`).
/// For other backends, only fields derivable from our own logs are populated.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStats {
    pub session_id: String,
    pub bead_id: Option<String>,
    pub persona: String,
    pub backend: String,
    /// Model name, e.g. "claude-sonnet-4-5-20250929"
    pub model: Option<String>,
    /// Total input tokens across all turns
    pub input_tokens: u64,
    /// Total output (generated) tokens across all turns
    pub output_tokens: u64,
    /// Tokens written to cache (Claude-specific)
    pub cache_creation_tokens: u64,
    /// Tokens read from cache (Claude-specific)
    pub cache_read_tokens: u64,
    /// Number of assistant turns
    pub turn_count: u32,
    /// Number of user submissions logged
    pub user_input_count: u32,
    /// Session duration in seconds (first to last timestamp)
    pub duration_secs: Option<u64>,
    /// Tool call counts: tool_name -> count
    pub tool_counts: std::collections::HashMap<String, u32>,
    /// Unique files accessed (Read/Edit/Write/Glob/Grep)
    pub files_touched: Vec<String>,
    /// Source of these stats
    pub stats_source: String,
}

/// Encode a filesystem path to the directory name Claude Code uses under ~/.claude/projects/
/// e.g. "/Users/gkt/src/project" -> "-Users-gkt-src-project"
fn encode_claude_project_path(project_path: &str) -> String {
    project_path.replace('/', "-")
}

/// Parse Claude Code's session JSONL and return SessionStats.
/// Returns None if the file doesn't exist or can't be parsed.
fn parse_claude_session_stats(
    session_id: &str,
    project_path: &str,
    bead_id: Option<&str>,
    persona: &str,
) -> Option<SessionStats> {
    let home = std::env::var("HOME").unwrap_or_default();
    let encoded = encode_claude_project_path(project_path);
    let jsonl_path = format!(
        "{}/.claude/projects/{}/{}.jsonl",
        home, encoded, session_id
    );

    let file = std::fs::File::open(&jsonl_path).ok()?;
    let reader = std::io::BufReader::new(file);

    let mut input_tokens: u64 = 0;
    let mut output_tokens: u64 = 0;
    let mut cache_creation_tokens: u64 = 0;
    let mut cache_read_tokens: u64 = 0;
    let mut turn_count: u32 = 0;
    let mut model: Option<String> = None;
    let mut tool_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    let mut files: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut ts_first: Option<String> = None;
    let mut ts_last: Option<String> = None;

    for line in reader.lines() {
        let line = match line { Ok(l) => l, Err(_) => continue };
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v, Err(_) => continue,
        };

        let ts = v.get("timestamp").and_then(|t| t.as_str()).map(String::from);
        if let Some(ref t) = ts {
            if ts_first.is_none() { ts_first = Some(t.clone()); }
            ts_last = Some(t.clone());
        }

        match v.get("type").and_then(|t| t.as_str()) {
            Some("assistant") => {
                turn_count += 1;
                // Extract model
                if model.is_none() {
                    if let Some(m) = v.get("message").and_then(|m| m.get("model")).and_then(|m| m.as_str()) {
                        model = Some(m.to_string());
                    }
                }
                // Token usage
                if let Some(usage) = v.get("message").and_then(|m| m.get("usage")) {
                    input_tokens += usage.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    output_tokens += usage.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    cache_creation_tokens += usage.get("cache_creation_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    cache_read_tokens += usage.get("cache_read_input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                }
                // Tool calls in content blocks
                if let Some(content) = v.get("message").and_then(|m| m.get("content")).and_then(|c| c.as_array()) {
                    for blk in content {
                        if blk.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                            let name = blk.get("name").and_then(|n| n.as_str()).unwrap_or("unknown");
                            *tool_counts.entry(name.to_string()).or_insert(0) += 1;
                            // Extract file paths
                            let inp = &blk["input"];
                            for key in &["file_path", "path", "notebook_path"] {
                                if let Some(fp) = inp.get(key).and_then(|v| v.as_str()) {
                                    if !fp.is_empty() { files.insert(fp.to_string()); }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let mut files_vec: Vec<String> = files.into_iter().collect();
    files_vec.sort();

    Some(SessionStats {
        session_id: session_id.to_string(),
        bead_id: bead_id.map(String::from),
        persona: persona.to_string(),
        backend: "claude".to_string(),
        model,
        input_tokens,
        output_tokens,
        cache_creation_tokens,
        cache_read_tokens,
        turn_count,
        user_input_count: 0, // filled in from caller
        duration_secs: {
            let parse_ts = |s: &str| -> Option<i64> {
                let s = s.trim_end_matches('Z');
                let parts: Vec<&str> = s.splitn(2, 'T').collect();
                if parts.len() != 2 { return None; }
                let date_parts: Vec<u32> = parts[0].split('-').filter_map(|p| p.parse().ok()).collect();
                let time_parts: Vec<f64> = parts[1].split(':').filter_map(|p| p.parse().ok()).collect();
                if date_parts.len() < 3 || time_parts.len() < 3 { return None; }
                let y = date_parts[0] as i64;
                let m = date_parts[1] as i64;
                let d = date_parts[2] as i64;
                let days = (y - 1970) * 365 + (y - 1969) / 4 + [0i64,31,59,90,120,151,181,212,243,273,304,334][(m-1) as usize] + d - 1;
                let secs = days * 86400 + time_parts[0] as i64 * 3600 + time_parts[1] as i64 * 60 + time_parts[2] as i64;
                Some(secs)
            };
            match (ts_first.as_deref().and_then(parse_ts), ts_last.as_deref().and_then(parse_ts)) {
                (Some(s), Some(e)) if e > s => Some((e - s) as u64),
                _ => None,
            }
        },
        tool_counts,
        files_touched: files_vec,
        stats_source: jsonl_path,
    })
}

/// Parse user input count from our own JSONL log (works for any backend)
fn count_user_inputs_from_log(bead_id: Option<&str>, session_id: &str) -> u32 {
    let home = std::env::var("HOME").unwrap_or_default();
    let session_dir = match bead_id {
        Some(bid) => format!("{}/.bp6/sessions/{}/{}", home, bid, session_id),
        None => format!("{}/.bp6/sessions/untracked/{}", home, session_id),
    };

    // Find all JSONL files that start with session_id
    let prefix = format!("{}-", session_id);
    let paths: Vec<std::path::PathBuf> = std::fs::read_dir(&session_dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.extension().and_then(|x| x.to_str()) == Some("jsonl")
                        && p.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with(&prefix)).unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default();

    let mut count = 0u32;
    for path in paths {
        if let Ok(file) = std::fs::File::open(&path) {
            for line in std::io::BufReader::new(file).lines().flatten() {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                    if v.get("event_type").and_then(|t| t.as_str()) == Some("userinput") {
                        count += 1;
                    }
                }
            }
        }
    }
    count
}

/// Get statistics for a session.
///
/// For Claude Code: parsed from Claude's own session JSONL (tokens, tools, files, model, duration).
/// For other backends: returns basic stats from our JSONL log (user input count only).
#[tauri::command]
#[allow(non_snake_case)]
pub fn get_session_stats(
    sessionId: String,
    state: State<'_, AgentState>,
) -> Result<SessionStats, String> {
    let (bead_id, persona, backend_id, project_path) = {
        let sessions = state.sessions.lock().unwrap();
        sessions.get(&sessionId).map(|s| (
            s.bead_id.clone(),
            s.persona.clone(),
            s.backend_id,
            s.project_path.clone(),
        )).ok_or_else(|| format!("Session not found: {}", sessionId))?
    };

    let backend_name = format!("{:?}", backend_id).to_lowercase();
    let user_input_count = count_user_inputs_from_log(bead_id.as_deref(), &sessionId);

    // For Claude Code: parse the rich session JSONL
    if matches!(backend_id, crate::agent::plugin::BackendId::ClaudeCode) {
        if let Some(mut stats) = parse_claude_session_stats(
            &sessionId,
            &project_path,
            bead_id.as_deref(),
            &persona,
        ) {
            stats.user_input_count = user_input_count;
            return Ok(stats);
        }
    }

    // Fallback: return minimal stats from what we know
    Ok(SessionStats {
        session_id: sessionId,
        bead_id,
        persona,
        backend: backend_name,
        model: None,
        input_tokens: 0,
        output_tokens: 0,
        cache_creation_tokens: 0,
        cache_read_tokens: 0,
        turn_count: 0,
        user_input_count,
        duration_secs: None,
        tool_counts: std::collections::HashMap::new(),
        files_touched: vec![],
        stats_source: "bp6-log".to_string(),
    })
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

/// Scan ~/.bp6/sessions/ for all historical session sidecars.
/// Returns a Vec<SessionInfo> with status=Stopped for every sidecar found.
/// Called once at startup to emit the `sessions-discovered` event.
pub fn scan_historical_sessions() -> Vec<SessionInfo> {
    let home_dir = match dirs::home_dir() {
        Some(h) => h,
        None => return Vec::new(),
    };
    let sessions_dir = home_dir.join(".bp6").join("sessions");
    if !sessions_dir.exists() {
        return Vec::new();
    }

    let mut results = Vec::new();

    let bead_dirs = match std::fs::read_dir(&sessions_dir) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    for bead_entry in bead_dirs.flatten() {
        let bead_dir = bead_entry.path();
        if !bead_dir.is_dir() {
            continue;
        }

        let files = match std::fs::read_dir(&bead_dir) {
            Ok(f) => f,
            Err(_) => continue,
        };

        for file_entry in files.flatten() {
            let path = file_entry.path();
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };

            if !name.ends_with(".session.json") {
                continue;
            }

            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("⚠️ scan_historical_sessions: Failed to read {}: {}", path.display(), e);
                    continue;
                }
            };

            let meta: serde_json::Value = match serde_json::from_str(&content) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("⚠️ scan_historical_sessions: Failed to parse {}: {}", path.display(), e);
                    continue;
                }
            };

            let session_id = match meta["session_id"].as_str() {
                Some(s) => s.to_string(),
                None => continue,
            };

            let backend_str = meta["backend"].as_str().unwrap_or("claude");
            let backend_id = match backend_str {
                "gemini" => crate::agent::plugin::BackendId::Gemini,
                _ => crate::agent::plugin::BackendId::ClaudeCode,
            };

            let persona = meta["persona"].as_str().unwrap_or("specialist").to_string();
            let task = meta["task"].as_str().map(String::from);
            let bead_id = meta["bead_id"].as_str().map(String::from);
            let started_at = meta["started_at"].as_u64().unwrap_or(0);

            results.push(SessionInfo {
                session_id: session_id.clone(),
                bead_id,
                persona,
                task,
                role: None,
                backend_id,
                status: SessionStatus::Stopped,
                created_at: started_at,
                cli_session_id: Some(session_id),
                last_activity: started_at,
                has_unread: false,
                message_count: 0,
                project_path: String::new(),
                bead_title: None,
                bead_description_preview: None,
            });
        }
    }

    results
}

/// Find the sidecar file for a specific session ID by searching all bead subdirs.
fn find_session_sidecar(session_id: &str) -> Option<serde_json::Value> {
    let home_dir = dirs::home_dir()?;
    let sessions_dir = home_dir.join(".bp6").join("sessions");

    let bead_dirs = std::fs::read_dir(&sessions_dir).ok()?;

    for bead_entry in bead_dirs.flatten() {
        let bead_dir = bead_entry.path();
        if !bead_dir.is_dir() {
            continue;
        }
        let sidecar_path = bead_dir.join(format!("{}.session.json", session_id));
        if sidecar_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&sidecar_path) {
                if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&content) {
                    return Some(meta);
                }
            }
        }
    }

    None
}

/// Resume a specific historical session by its session ID.
///
/// Reads the sidecar to get backend/persona/task/bead_id, then spawns the PTY
/// in resume mode. Registers the session in AgentState and emits session-list-changed.
///
/// Returns the session_id (frontend should call create_session_window with it).
#[tauri::command]
pub async fn resume_specific_session(
    app_handle: AppHandle,
    state: State<'_, AgentState>,
    session_id: String,
) -> Result<String, String> {
    // 1. If already running in AgentState, return as-is
    {
        let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
        if sessions.contains_key(&session_id) {
            return Ok(session_id);
        }
    }

    // 2. Find the sidecar for this session
    let meta = find_session_sidecar(&session_id)
        .ok_or_else(|| format!("Session '{}' not found in sidecar files", session_id))?;

    let backend_str = meta["backend"].as_str().unwrap_or("claude");
    let backend_id = match backend_str {
        "gemini" => crate::agent::plugin::BackendId::Gemini,
        _ => crate::agent::plugin::BackendId::ClaudeCode,
    };
    let persona = meta["persona"].as_str().unwrap_or("specialist").to_string();
    let task = meta["task"].as_str().map(String::from);
    let bead_id = meta["bead_id"].as_str().map(String::from);
    let gemini_session_id = meta["gemini_session_id"].as_str().map(String::from);

    // 3. Resolve project path
    let repo_root = crate::bd::find_repo_root()
        .ok_or_else(|| "Could not locate project root".to_string())?;

    // 4. Get backend plugin and CLI path
    let backend = state
        .backend_registry
        .get(backend_id)
        .ok_or_else(|| format!("Backend {:?} not registered", backend_id))?;
    let cli_path = crate::bd::resolve_cli_path(backend.command_name())
        .ok_or_else(|| format!("The '{}' CLI is not found in PATH", backend.command_name()))?;

    // 5. Build resume args
    let args = match backend_id {
        crate::agent::plugin::BackendId::ClaudeCode => {
            vec![
                "-r".to_string(),
                session_id.clone(),
                "--dangerously-skip-permissions".to_string(),
            ]
        }
        crate::agent::plugin::BackendId::Gemini => {
            if let Some(uuid) = gemini_session_id {
                vec!["--yolo".to_string(), "-r".to_string(), uuid]
            } else {
                // No gemini UUID — build fresh prompt and init via JSON
                let prompt = build_prompt_with_persona(
                    &state,
                    &persona,
                    task.as_deref(),
                    bead_id.as_deref(),
                    None,
                )?;
                let path = repo_root.to_str().unwrap_or("").to_string();
                let cli = cli_path.to_string_lossy().to_string();
                let bead_id_clone = bead_id.clone();
                let session_id_clone = session_id.clone();

                let uuid = tauri::async_runtime::spawn_blocking(move || {
                    create_gemini_session_via_json(&cli, &path, &prompt)
                })
                .await
                .unwrap_or(None);

                if let Some(ref u) = uuid {
                    store_gemini_session_id(bead_id_clone.as_deref(), &session_id_clone, u);
                    vec!["--yolo".to_string(), "-r".to_string(), u.clone()]
                } else {
                    let prompt2 = build_prompt_with_persona(
                        &state,
                        &persona,
                        task.as_deref(),
                        bead_id.as_deref(),
                        None,
                    )?;
                    vec!["--yolo".to_string(), "--prompt-interactive".to_string(), prompt2]
                }
            }
        }
    };

    // 6. Spawn PTY
    state.pty_manager.spawn(
        session_id.clone(),
        cli_path.to_string_lossy().to_string(),
        args,
        Some(repo_root.to_str().unwrap_or("").to_string()),
        Some(80),
        Some(24),
    )?;

    // 7. Create logger and start reader thread
    let backend_name_for_log = format!("{:?}", backend_id).to_lowercase();
    let shared_logger: Option<Arc<Mutex<SessionLogger>>> =
        SessionLogger::new(bead_id.as_deref(), &session_id)
            .ok()
            .map(|l| Arc::new(Mutex::new(l)));

    if let Some(ref l) = shared_logger {
        state
            .session_loggers
            .lock()
            .unwrap()
            .insert(session_id.clone(), l.clone());
    }

    {
        let pty_manager = state.pty_manager.clone();
        let app_handle_clone = app_handle.clone();
        let session_id_clone = session_id.clone();
        let bead_id_for_log = bead_id.clone();
        let persona_for_log = persona.clone();
        let backend_name_clone = backend_name_for_log.clone();
        let logger_for_thread = shared_logger;

        std::thread::spawn(move || {
            loop {
                match pty_manager.read(&session_id_clone) {
                    Ok(data) => {
                        if data.is_empty() {
                            break;
                        }
                        let output = String::from_utf8_lossy(&data).to_string();
                        let _ = app_handle_clone.emit(
                            "pty-data",
                            serde_json::json!({
                                "sessionId": session_id_clone,
                                "data": output,
                            }),
                        );
                        if let Some(ref l) = logger_for_thread {
                            let _ = l.lock().unwrap().log_pty_output(
                                &session_id_clone,
                                bead_id_for_log.as_deref(),
                                &persona_for_log,
                                &backend_name_clone,
                                &strip_ansi(&output),
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("PTY read error for resumed session {}: {}", session_id_clone, e);
                        break;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            eprintln!("PTY stream ended for resumed session: {}", session_id_clone);
        });
    }

    // 8. Fetch bead metadata (optional)
    let (bead_title, bead_description_preview) = if let Some(ref bid) = bead_id {
        match crate::bd::get_bead_by_id(bid) {
            Ok(bead) => {
                let preview = bead.description.as_deref().map(|d| {
                    d.lines().take(2).collect::<Vec<_>>().join("\n")
                });
                (Some(bead.title), preview)
            }
            Err(_) => (None, None),
        }
    } else {
        (None, None)
    };

    // 9. Store SessionState
    let now = SystemTime::now();
    let session_state = SessionState {
        bead_id: bead_id.clone(),
        persona: persona.clone(),
        task: task.clone(),
        role: None,
        backend_id,
        status: SessionStatus::Running,
        created_at: now,
        cli_session_id: Some(session_id.clone()),
        last_activity: now,
        has_unread: false,
        message_count: 0,
        project_path: repo_root.to_str().unwrap_or("").to_string(),
        bead_title,
        bead_description_preview,
    };

    {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.insert(session_id.clone(), session_state);
    }

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
