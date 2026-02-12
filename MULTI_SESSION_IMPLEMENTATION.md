# Multi-Session Agent Implementation Guide

## Feature: bp6-643.001 - Backend Multi-Session Management

### Overview
This document provides the complete implementation for converting the single-agent session system to support multiple concurrent sessions with UUID-based tracking.

### Dependencies
Add to `bert-viz/src-tauri/Cargo.toml`:
```toml
uuid = { version = "1.11", features = ["v4", "serde"] }
```

### File: `bert-viz/src-tauri/src/agent.rs`

#### 1. Add imports (add to existing imports at top of file):
```rust
use std::collections::HashMap;
use std::time::SystemTime;
use uuid::Uuid;
```

#### 2. Update AgentChunk structure:
```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentChunk {
    pub content: String,
    pub is_done: bool,
    pub session_id: Option<String>,  // ADD THIS FIELD
}
```

#### 3. Add new structures (after AgentChunk):
```rust
/// Represents the status of an agent session
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Running,
    Paused,
}

/// Represents a single agent session with its process and metadata
#[derive(Debug)]
pub struct SessionState {
    pub process: Child,
    pub bead_id: String,
    pub persona: String,
    pub status: SessionStatus,
    pub created_at: SystemTime,
    pub cli_backend: CliBackend,
}

/// Information about a session for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub session_id: String,
    pub bead_id: String,
    pub persona: String,
    pub status: SessionStatus,
    pub created_at: u64,
    pub cli_backend: String,
}
```

#### 4. Replace AgentState structure:
```rust
pub struct AgentState {
    pub sessions: Mutex<HashMap<String, SessionState>>,
    pub active_session_id: Mutex<Option<String>>,
}

impl AgentState {
    pub fn new() -> Self {
        AgentState {
            sessions: Mutex::new(HashMap::new()),
            active_session_id: Mutex::new(None),
        }
    }
}
```

#### 5. Replace run_cli_command function:
```rust
fn run_cli_command(
    cli_backend: CliBackend,
    app_handle: AppHandle,
    session_id: String,
    prompt: String,
    resume: bool,
) -> Result<Child, String> {
    let repo_root = crate::bd::find_repo_root()
        .ok_or_else(|| "Could not locate project root (.beads directory). Please ensure a project is loaded.".to_string())?;

    eprintln!("🎯 Starting agent session {} in directory: {}", session_id, repo_root.display());

    let mut cmd = Command::new(cli_backend.as_command_name());

    let args = match cli_backend {
        CliBackend::Gemini => build_gemini_args(&prompt, resume),
        CliBackend::ClaudeCode => build_claude_args(&prompt, resume),
    };
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
        .map_err(|e| format!("Failed to spawn {} in {}: {}", cli_backend.as_command_name(), repo_root.display(), e))?;

    eprintln!("🚀 Sending prompt to agent session {}:\n{}", session_id, prompt);
    let _ = app_handle.emit("agent-stderr", format!("[System] Session {} - Sending prompt:\n{}", session_id, prompt));

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let handle_clone = app_handle.clone();
    let session_id_clone = session_id.clone();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line_str) = line {
                if line_str.trim().starts_with('{') {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line_str) {
                        if json["type"] == "message" && json["role"] == "assistant" {
                            if let Some(content) = json["content"].as_str() {
                                let _ = handle_clone.emit("agent-chunk", AgentChunk {
                                    content: content.to_string(),
                                    is_done: false,
                                    session_id: Some(session_id_clone.clone()),
                                });
                            }
                        } else if json["type"] == "result" {
                             let _ = handle_clone.emit("agent-chunk", AgentChunk {
                                    content: "".to_string(),
                                    is_done: true,
                                    session_id: Some(session_id_clone.clone()),
                             });
                        }
                    }
                }
            }
        }
        let _ = handle_clone.emit("agent-chunk", AgentChunk {
            content: "".to_string(),
            is_done: true,
            session_id: Some(session_id_clone.clone()),
        });
    });

    let handle_clone_stderr = app_handle.clone();
    let session_id_clone = session_id.clone();
    std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line_str) = line {
                eprintln!("🤖 Agent Stderr [{}]: {}", session_id_clone, line_str);
                let _ = handle_clone_stderr.emit("agent-stderr", format!("[{}] {}", session_id_clone, line_str));
            }
        }
    });

    Ok(child)
}
```

#### 6. Replace start_agent_session command:
Change the return type from `Result<(), String>` to `Result<String, String>` and replace entire function body:

```rust
#[tauri::command]
pub fn start_agent_session(
    app_handle: AppHandle,
    state: State<'_, AgentState>,
    persona: String,
    task: Option<String>,
    bead_id: Option<String>,
    cli_backend: Option<String>
) -> Result<String, String> {
    let session_id = Uuid::new_v4().to_string();

    let backend = cli_backend
        .as_deref()
        .and_then(|s| match s.to_lowercase().as_str() {
            "gemini" => Some(CliBackend::Gemini),
            "claude" | "claude-code" => Some(CliBackend::ClaudeCode),
            _ => None,
        })
        .unwrap_or(CliBackend::Gemini);

    let mut prompt = String::new();
    let effective_bead_id = bead_id.clone().unwrap_or_else(|| "unknown".to_string());

    // [Keep all the existing persona/template logic - just copy it from the original function]
    // ... (all the if persona == "specialist" { ... } logic stays the same)

    let child = run_cli_command(backend, app_handle.clone(), session_id.clone(), prompt, false)?;

    let session_state = SessionState {
        process: child,
        bead_id: effective_bead_id,
        persona: persona.clone(),
        status: SessionStatus::Running,
        created_at: SystemTime::now(),
        cli_backend: backend,
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
    emit_session_list_changed(&app_handle, &state);

    Ok(session_id)
}
```

#### 7. Replace send_agent_message command:
Add `session_id: Option<String>` parameter and update logic:

```rust
#[tauri::command]
pub fn send_agent_message(
    app_handle: AppHandle,
    message: String,
    state: State<'_, AgentState>,
    session_id: Option<String>,
) -> Result<(), String> {
    let target_session_id = if let Some(sid) = session_id {
        sid
    } else {
        let active = state.active_session_id.lock().unwrap();
        active.clone().ok_or_else(|| "No active session".to_string())?
    };

    let backend = {
        let sessions = state.sessions.lock().unwrap();
        let session = sessions.get(&target_session_id)
            .ok_or_else(|| format!("Session {} not found", target_session_id))?;
        session.cli_backend
    };

    let child = run_cli_command(backend, app_handle.clone(), target_session_id.clone(), message, true)?;

    {
        let mut sessions = state.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(&target_session_id) {
            session.process = child;
        }
    }

    Ok(())
}
```

#### 8. Replace stop_agent_session command:
Add `session_id: Option<String>` parameter:

```rust
#[tauri::command]
pub fn stop_agent_session(
    app_handle: AppHandle,
    state: State<'_, AgentState>,
    session_id: Option<String>,
) -> Result<(), String> {
    let target_session_id = if let Some(sid) = session_id {
        sid
    } else {
        let active = state.active_session_id.lock().unwrap();
        active.clone().ok_or_else(|| "No active session".to_string())?
    };

    {
        let mut sessions = state.sessions.lock().unwrap();
        if let Some(session) = sessions.remove(&target_session_id) {
            kill_process_group(session.process.id());
        } else {
            return Err(format!("Session {} not found", target_session_id));
        }
    }

    {
        let mut active = state.active_session_id.lock().unwrap();
        if active.as_ref() == Some(&target_session_id) {
            *active = None;
        }
    }

    let _ = app_handle.emit("session-terminated", target_session_id);
    emit_session_list_changed(&app_handle, &state);

    Ok(())
}
```

#### 9. Add new commands (after approve_suggestion):
```rust
/// Helper function to emit session-list-changed event
fn emit_session_list_changed(app_handle: &AppHandle, state: &AgentState) {
    let sessions_info = list_active_sessions_internal(state);
    let _ = app_handle.emit("session-list-changed", sessions_info);
}

/// Get list of all active sessions (internal helper)
fn list_active_sessions_internal(state: &AgentState) -> Vec<SessionInfo> {
    let sessions = state.sessions.lock().unwrap();
    sessions.iter().map(|(id, session)| {
        SessionInfo {
            session_id: id.clone(),
            bead_id: session.bead_id.clone(),
            persona: session.persona.clone(),
            status: session.status.clone(),
            created_at: session.created_at
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            cli_backend: format!("{}", session.cli_backend),
        }
    }).collect()
}

#[tauri::command]
pub fn list_active_sessions(state: State<'_, AgentState>) -> Result<Vec<SessionInfo>, String> {
    Ok(list_active_sessions_internal(&state))
}

#[tauri::command]
pub fn switch_active_session(
    app_handle: AppHandle,
    state: State<'_, AgentState>,
    session_id: String,
) -> Result<(), String> {
    {
        let sessions = state.sessions.lock().unwrap();
        if !sessions.contains_key(&session_id) {
            return Err(format!("Session {} not found", session_id));
        }
    }

    {
        let mut active = state.active_session_id.lock().unwrap();
        *active = Some(session_id.clone());
    }

    let _ = app_handle.emit("active-session-changed", session_id);

    Ok(())
}

#[tauri::command]
pub fn get_active_session_id(state: State<'_, AgentState>) -> Result<Option<String>, String> {
    let active = state.active_session_id.lock().unwrap();
    Ok(active.clone())
}

#[tauri::command]
pub fn terminate_session(
    app_handle: AppHandle,
    state: State<'_, AgentState>,
    session_id: String,
) -> Result<(), String> {
    stop_agent_session(app_handle, state, Some(session_id))
}
```

### File: `bert-viz/src-tauri/src/lib.rs`

Update the `invoke_handler` to register the new commands:
```rust
.invoke_handler(tauri::generate_handler![
    bd::get_beads, get_processed_data, get_project_view_model, bd::update_bead, bd::create_bead, bd::close_bead, bd::reopen_bead, bd::claim_bead,
    get_projects, add_project, remove_project, open_project, toggle_favorite,
    get_current_dir,
    agent::start_agent_session, agent::send_agent_message, agent::stop_agent_session, agent::approve_suggestion,
    agent::list_active_sessions, agent::switch_active_session, agent::get_active_session_id, agent::terminate_session
])
```

### Testing
1. Build the project: `cd bert-viz/src-tauri && cargo build`
2. Run the application
3. Test starting multiple sessions
4. Test switching between sessions
5. Test terminating individual sessions
6. Verify session-created, session-terminated, session-list-changed events are emitted

### Events Emitted
- `session-created`: When a new session starts (payload: session_id string)
- `session-terminated`: When a session is terminated (payload: session_id string)
- `session-list-changed`: When the session list changes (payload: Vec<SessionInfo>)
- `active-session-changed`: When the active session changes (payload: session_id string)
- `agent-chunk`: Now includes optional session_id field

### Frontend Integration Notes
The frontend will need to:
1. Handle the new session_id return value from startAgentSession
2. Subscribe to session-list-changed events
3. Update UI to show/hide/switch sessions
4. Pass session_id to sendAgentMessage and stopAgentSession when appropriate

## Status
Implementation code written but not persisted due to file locking conflicts. This document serves as the complete implementation guide.
