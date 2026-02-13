use std::process::{Command, Stdio, Child};
use std::io::{BufRead, BufReader};
use std::time::SystemTime;
use tauri::{AppHandle, Emitter, State};
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};

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

// DEPRECATED: CliBackend enum replaced by plugin architecture
// Use crate::agent::plugin::BackendId instead
// This is kept temporarily for backwards compatibility but will be removed
#[deprecated(
    since = "0.2.0",
    note = "Use crate::agent::plugin::BackendId and BackendRegistry instead"
)]
#[allow(dead_code)]
pub enum CliBackend {
    Gemini,
    ClaudeCode,
}

// Old template constants removed - now loaded from templates/personas/*.md files












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

pub struct AgentState {
    pub current_process: Mutex<Option<Child>>,
    pub backend_registry: crate::agent::registry::BackendRegistry,
    pub current_backend: Mutex<crate::agent::plugin::BackendId>,
    pub current_session_id: Arc<Mutex<Option<String>>>,
    pub persona_registry: crate::agent::persona::PersonaRegistry,
    pub template_loader: crate::agent::templates::TemplateLoader,
}

impl AgentState {
    pub fn new() -> Self {
        AgentState {
            current_process: Mutex::new(None),
            backend_registry: crate::agent::registry::BackendRegistry::with_defaults(),
            current_backend: Mutex::new(crate::agent::plugin::BackendId::Gemini),
            current_session_id: Arc::new(Mutex::new(None)),
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

// Backend-specific functions removed - now handled by CliBackendPlugin implementations

fn run_cli_command(
    backend_id: crate::agent::plugin::BackendId,
    app_handle: AppHandle,
    state: &AgentState,
    prompt: String,
    resume: bool,
) -> Result<(), String> {
    // Get the project root directory to ensure agent runs in correct context
    let repo_root = crate::bd::find_repo_root()
        .ok_or_else(|| "Could not locate project root (.beads directory). Please ensure a project is loaded.".to_string())?;

    eprintln!("🎯 Starting agent in directory: {}", repo_root.display());

    // Get the backend plugin from registry
    let backend = state
        .backend_registry
        .get(backend_id)
        .ok_or_else(|| format!("Backend {:?} not registered", backend_id))?;

    let mut cmd = Command::new(backend.command_name());

    // Get current session ID for resume (if any)
    let session_id = state.current_session_id.lock().unwrap().clone();

    // Build CLI-specific arguments using plugin
    let args = backend.build_args(&prompt, resume, session_id.as_deref());
    cmd.args(&args);

    // Set working directory to project root
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

    let child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            let error_msg = if e.kind() == std::io::ErrorKind::NotFound {
                // CLI binary not found - provide installation instructions
                let install_cmd = match backend_id {
                    crate::agent::plugin::BackendId::Gemini => "npm install -g @google/generative-ai-cli",
                    crate::agent::plugin::BackendId::ClaudeCode => "See https://docs.anthropic.com/en/docs/claude-code for installation",
                };
                format!(
                    "{} CLI not found. Please install it first: {}",
                    backend.command_name(),
                    install_cmd
                )
            } else {
                format!("Failed to spawn {} in {}: {}", backend.command_name(), repo_root.display(), e)
            };

            // Emit error to UI
            let _ = app_handle.emit("agent-stderr", format!("[Error] {}", error_msg));
            error_msg
        })?;

    {
        let mut proc_guard = state.current_process.lock().unwrap();
        *proc_guard = Some(child);
    }

    // Log the prompt for debugging
    eprintln!("🚀 Sending prompt to agent:\n{}", prompt);
    let _ = app_handle.emit("agent-stderr", format!("[System] Sending prompt:\n{}", prompt));

    // We need to re-lock to get the child out for reading, but we don't want to hold the lock
    // while reading stdout/stderr.
    let (stdout, stderr) = {
        let mut proc_guard = state.current_process.lock().unwrap();
        let child = proc_guard.as_mut().unwrap();
        (child.stdout.take().unwrap(), child.stderr.take().unwrap())
    };

    let handle_clone = app_handle.clone();
    let backend_clone = backend.clone();
    let session_id_clone = Arc::clone(&state.current_session_id);

    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line_str) = line {
                if line_str.trim().starts_with('{') {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line_str) {
                        // Extract and store session_id from result messages
                        if json["type"] == "result" {
                            if let Some(session_id) = json["session_id"].as_str() {
                                let mut session_guard = session_id_clone.lock().unwrap();
                                *session_guard = Some(session_id.to_string());
                            }
                        }

                        // Use plugin to parse backend-specific JSON format
                        if let Some(chunk) = backend_clone.parse_stdout_line(&json) {
                            let _ = handle_clone.emit("agent-chunk", chunk);
                        }
                    }
                }
            }
        }
        // Emit final completion chunk
        let _ = handle_clone.emit("agent-chunk", crate::agent::plugin::AgentChunk {
            content: "".to_string(),
            is_done: true,
            session_id: None,
        });
    });

    let handle_clone_stderr = app_handle.clone();
    std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line_str) = line {
                eprintln!("🤖 Agent Stderr: {}", line_str);
                let _ = handle_clone_stderr.emit("agent-stderr", line_str);
            }
        }
    });

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
) -> Result<(), String> {
    // Stop any existing turn
    let mut process_guard = state.current_process.lock().unwrap();
    if let Some(child) = process_guard.take() {
        kill_process_group(child.id());
    }
    drop(process_guard);

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

    // Store the CLI backend in state for this session
    {
        let mut backend_guard = state.current_backend.lock().unwrap();
        *backend_guard = backend;
    }

    // Clear session ID for new session (will be set from first result)
    {
        let mut session_guard = state.current_session_id.lock().unwrap();
        *session_guard = None;
    }

    run_cli_command(backend, app_handle, &state, prompt, false)
}

#[tauri::command]
pub fn send_agent_message(
    app_handle: AppHandle,
    message: String,
    state: State<'_, AgentState>
) -> Result<(), String> {
    // Read the CLI backend from state to maintain consistency across session
    let backend = {
        let backend_guard = state.current_backend.lock().unwrap();
        *backend_guard
    };

    run_cli_command(backend, app_handle, &state, message, true)
}

#[tauri::command]
pub fn stop_agent_session(state: State<'_, AgentState>) -> Result<(), String> {
    let mut process_guard = state.current_process.lock().unwrap();
    if let Some(child) = process_guard.take() {
        kill_process_group(child.id());
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
