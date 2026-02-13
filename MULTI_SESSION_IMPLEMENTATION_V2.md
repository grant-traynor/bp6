# Multi-Session Agent Implementation Guide v2.0

**Epic:** bp6-643
**Feature:** bp6-643.001 - Backend Multi-Session Management
**Updated:** 2026-02-13
**Architecture:** Plugin-based (post epic 6dk)

## Overview

This document provides the complete implementation for converting the single-agent session system to support multiple concurrent sessions with UUID-based tracking, aligned with the new plugin architecture introduced in epic 6dk.

## Architecture Changes from v1.0

### Key Differences from Original Design:
1. **Modular file structure**: Changes split across `agent/session.rs`, `agent/plugin.rs`, `agent/mod.rs`
2. **Plugin architecture**: Uses `BackendId` and `BackendRegistry` instead of `CliBackend` enum
3. **Registry pattern**: Mirror the existing `BackendRegistry` pattern for `SessionRegistry`
4. **Preserved API**: `run_cli_command` signature remains unchanged, add new wrapper function
5. **Session tracking**: Build on existing `current_session_id` field

## Dependencies

Already installed in `bert-viz/src-tauri/Cargo.toml`:
- `uuid` (version 1.11+, features: v4, serde) ✅
- `serde` ✅
- `tauri` ✅

## Implementation Steps

### Step 1: Update AgentChunk to Include Session ID

**File:** `bert-viz/src-tauri/src/agent/plugin.rs`

**Location:** Lines 42-52 (AgentChunk definition)

**Change:**
```rust
/// Represents a chunk of output from an agent session
///
/// This type is shared across all CLI backends for streaming output.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentChunk {
    /// The text content of this chunk
    pub content: String,
    /// Whether this is the final chunk (session complete)
    pub is_done: bool,
    /// Optional session ID for multi-session support
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}
```

**Rationale:** Add session_id field to route chunks to correct UI session. Use `skip_serializing_if` to maintain backwards compatibility.

---

### Step 2: Define Session Types and Structures

**File:** `bert-viz/src-tauri/src/agent/session.rs`

**Location:** After line 50 (after `get_role_from_bead`), before `pub struct AgentState`

**Add:**
```rust
use std::collections::HashMap;
use std::time::SystemTime;
use uuid::Uuid;

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
    pub backend_id: crate::agent::plugin::BackendId,
    pub status: SessionStatus,
    pub created_at: SystemTime,
    pub cli_session_id: Option<String>, // Session ID from CLI backend (for resume)
}

/// Information about a session for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub session_id: String,
    pub bead_id: String,
    pub persona: String,
    pub backend_id: String,
    pub status: SessionStatus,
    pub created_at: u64, // Unix timestamp in seconds
}

impl SessionInfo {
    fn from_session(session_id: &str, state: &SessionState) -> Self {
        SessionInfo {
            session_id: session_id.to_string(),
            bead_id: state.bead_id.clone(),
            persona: state.persona.clone(),
            backend_id: state.backend_id.to_string(),
            status: state.status.clone(),
            created_at: state
                .created_at
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}
```

**Rationale:**
- `SessionState` stores per-session data including process, backend, and metadata
- `SessionInfo` is a serializable snapshot for UI display
- `cli_session_id` distinguishes our internal UUID from backend-specific session IDs

---

### Step 3: Refactor AgentState for Multi-Session Support

**File:** `bert-viz/src-tauri/src/agent/session.rs`

**Location:** Lines 55-76 (AgentState struct and impl)

**Replace:**
```rust
pub struct AgentState {
    // Multi-session support
    pub sessions: Mutex<HashMap<String, SessionState>>,
    pub active_session_id: Mutex<Option<String>>,

    // Shared resources (not per-session)
    pub backend_registry: crate::agent::registry::BackendRegistry,
    pub persona_registry: crate::agent::persona::PersonaRegistry,
    pub template_loader: crate::agent::templates::TemplateLoader,
}

impl AgentState {
    pub fn new() -> Self {
        AgentState {
            sessions: Mutex::new(HashMap::new()),
            active_session_id: Mutex::new(None),
            backend_registry: crate::agent::registry::BackendRegistry::with_defaults(),
            persona_registry: crate::agent::persona::PersonaRegistry::with_defaults(),
            template_loader: crate::agent::templates::TemplateLoader::new()
                .expect("Failed to initialize template loader"),
        }
    }
}
```

**Rationale:**
- `sessions: HashMap<String, SessionState>` replaces `current_process`
- `active_session_id` replaces both `current_backend` and `current_session_id`
- Registries remain shared across all sessions (they're already thread-safe)

---

### Step 4: Create Multi-Session run_cli_command Wrapper

**File:** `bert-viz/src-tauri/src/agent/session.rs`

**Location:** After the existing `run_cli_command` function (after line 232)

**Add:**
```rust
/// Multi-session variant that spawns a process for a specific session
/// Returns the Child process instead of storing it internally
fn run_cli_command_for_session(
    backend_id: crate::agent::plugin::BackendId,
    app_handle: AppHandle,
    state: &AgentState,
    session_id: String,
    prompt: String,
    resume: bool,
    cli_session_id: Option<String>, // Backend-specific session ID for resume
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

    // Spawn stdout reader thread
    let handle_clone = app_handle.clone();
    let backend_clone = backend.clone();
    let session_id_clone = session_id.clone();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line_str) = line {
                if line_str.trim().starts_with('{') {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line_str) {
                        // Parse using backend plugin
                        if let Some(mut chunk) = backend_clone.parse_stdout_line(&json) {
                            // Add session ID to chunk
                            chunk.session_id = Some(session_id_clone.clone());
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
            session_id: Some(session_id_clone.clone()),
        });
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
```

**Rationale:**
- New function specifically for multi-session use
- Returns `Child` instead of storing it (caller manages storage)
- Adds `session_id` to all emitted chunks
- Preserves existing `run_cli_command` API for backwards compatibility

---

### Step 5: Helper Functions for Session Management

**File:** `bert-viz/src-tauri/src/agent/session.rs`

**Location:** After `run_cli_command_for_session` (new function above)

**Add:**
```rust
/// Helper function to emit session-list-changed event
fn emit_session_list_changed(app_handle: &AppHandle, state: &AgentState) {
    let sessions_info = list_active_sessions_internal(state);
    let _ = app_handle.emit("session-list-changed", sessions_info);
}

/// Get list of all active sessions (internal helper)
fn list_active_sessions_internal(state: &AgentState) -> Vec<SessionInfo> {
    let sessions = state.sessions.lock().unwrap();
    sessions
        .iter()
        .map(|(id, session)| SessionInfo::from_session(id, session))
        .collect()
}
```

---

### Step 6: Refactor start_agent_session Command

**File:** `bert-viz/src-tauri/src/agent/session.rs`

**Location:** Lines 291-344 (start_agent_session function)

**Change return type:** `Result<(), String>` → `Result<String, String>`

**Replace function body:**
```rust
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
    // Generate new session ID
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

    // Spawn the CLI process
    let child = run_cli_command_for_session(
        backend,
        app_handle.clone(),
        &state,
        session_id.clone(),
        prompt,
        false,
        None, // No CLI session ID for new sessions
    )?;

    // Create session state
    let session_state = SessionState {
        process: child,
        bead_id: bead_id.unwrap_or_else(|| "unknown".to_string()),
        persona: persona.clone(),
        backend_id: backend,
        status: SessionStatus::Running,
        created_at: SystemTime::now(),
        cli_session_id: None, // Will be set from first result message
    };

    // Store session
    {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.insert(session_id.clone(), session_state);
    }

    // Set as active session
    {
        let mut active = state.active_session_id.lock().unwrap();
        *active = Some(session_id.clone());
    }

    // Emit events
    let _ = app_handle.emit("session-created", session_id.clone());
    emit_session_list_changed(&app_handle, &state);

    Ok(session_id)
}
```

**Rationale:**
- Returns session ID to frontend
- Uses new `run_cli_command_for_session` wrapper
- Stores session in HashMap instead of single process
- Emits session events for UI updates

---

### Step 7: Refactor send_agent_message Command

**File:** `bert-viz/src-tauri/src/agent/session.rs`

**Location:** Lines 346-359 (send_agent_message function)

**Add `session_id` parameter:**
```rust
#[tauri::command]
pub fn send_agent_message(
    app_handle: AppHandle,
    message: String,
    state: State<'_, AgentState>,
    session_id: Option<String>,
) -> Result<(), String> {
    // Determine target session
    let target_session_id = if let Some(sid) = session_id {
        sid
    } else {
        let active = state.active_session_id.lock().unwrap();
        active.clone().ok_or_else(|| "No active session".to_string())?
    };

    // Get session info
    let (backend_id, cli_session_id) = {
        let sessions = state.sessions.lock().unwrap();
        let session = sessions
            .get(&target_session_id)
            .ok_or_else(|| format!("Session {} not found", target_session_id))?;
        (session.backend_id, session.cli_session_id.clone())
    };

    // Spawn new process for this turn (resume mode)
    let child = run_cli_command_for_session(
        backend_id,
        app_handle.clone(),
        &state,
        target_session_id.clone(),
        message,
        true, // resume=true
        cli_session_id.as_deref(),
    )?;

    // Replace process in session
    {
        let mut sessions = state.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(&target_session_id) {
            session.process = child;
        }
    }

    Ok(())
}
```

---

### Step 8: Refactor stop_agent_session Command

**File:** `bert-viz/src-tauri/src/agent/session.rs`

**Location:** Lines 361-368 (stop_agent_session function)

**Add `session_id` parameter:**
```rust
#[tauri::command]
pub fn stop_agent_session(
    app_handle: AppHandle,
    state: State<'_, AgentState>,
    session_id: Option<String>,
) -> Result<(), String> {
    // Determine target session
    let target_session_id = if let Some(sid) = session_id {
        sid
    } else {
        let active = state.active_session_id.lock().unwrap();
        active.clone().ok_or_else(|| "No active session".to_string())?
    };

    // Remove session and kill process
    {
        let mut sessions = state.sessions.lock().unwrap();
        if let Some(session) = sessions.remove(&target_session_id) {
            kill_process_group(session.process.id());
        } else {
            return Err(format!("Session {} not found", target_session_id));
        }
    }

    // Clear active session if it was this one
    {
        let mut active = state.active_session_id.lock().unwrap();
        if active.as_ref() == Some(&target_session_id) {
            *active = None;
        }
    }

    // Emit events
    let _ = app_handle.emit("session-terminated", target_session_id.clone());
    emit_session_list_changed(&app_handle, &state);

    Ok(())
}
```

---

### Step 9: Add New Session Management Commands

**File:** `bert-viz/src-tauri/src/agent/session.rs`

**Location:** After `approve_suggestion` function (after line 383)

**Add:**
```rust
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
    // Verify session exists
    {
        let sessions = state.sessions.lock().unwrap();
        if !sessions.contains_key(&session_id) {
            return Err(format!("Session {} not found", session_id));
        }
    }

    // Set as active
    {
        let mut active = state.active_session_id.lock().unwrap();
        *active = Some(session_id.clone());
    }

    // Emit event
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

---

### Step 10: Update Module Exports

**File:** `bert-viz/src-tauri/src/agent/mod.rs`

**Location:** Lines 14-24 (re-exports)

**Add:**
```rust
// Re-export session management types
#[allow(unused_imports)]
pub use session::{SessionInfo, SessionState, SessionStatus};
```

---

### Step 11: Register New Tauri Commands

**File:** `bert-viz/src-tauri/src/lib.rs`

**Location:** In the `.invoke_handler()` call (search for `agent::start_agent_session`)

**Update:**
```rust
.invoke_handler(tauri::generate_handler![
    bd::get_beads, get_processed_data, get_project_view_model, bd::update_bead, bd::create_bead, bd::close_bead, bd::reopen_bead, bd::claim_bead,
    get_projects, add_project, remove_project, open_project, toggle_favorite,
    get_current_dir,
    agent::start_agent_session, agent::send_agent_message, agent::stop_agent_session, agent::approve_suggestion,
    agent::list_active_sessions, agent::switch_active_session, agent::get_active_session_id, agent::terminate_session
])
```

---

## Testing Checklist

After implementation, verify:

```bash
# 1. Build compiles without errors
cd bert-viz/src-tauri && cargo build

# 2. Run all tests
cargo test

# 3. Manual testing (in UI):
- [ ] Start multiple sessions on different beads
- [ ] Verify each session gets unique UUID
- [ ] Switch between active sessions
- [ ] Send messages to specific sessions
- [ ] Verify chunks include session_id
- [ ] Terminate individual sessions
- [ ] Verify session list updates in UI
```

## Events Emitted

| Event Name | Payload Type | Description |
|------------|--------------|-------------|
| `session-created` | `String` | New session UUID |
| `session-terminated` | `String` | Terminated session UUID |
| `session-list-changed` | `Vec<SessionInfo>` | Updated session list |
| `active-session-changed` | `String` | New active session UUID |
| `agent-chunk` | `AgentChunk` | Streaming output (now includes session_id) |

## Frontend Integration Notes

The frontend needs to:
1. Handle `startAgentSession` now returning a session ID
2. Subscribe to all session events
3. Maintain local state mapping session IDs to UI components
4. Pass `session_id` to `sendAgentMessage` and `stopAgentSession` when targeting specific sessions
5. Display session indicators on WBS tree (bp6-643.003)
6. Show session list in chat UI (bp6-643.004)

## Comparison to v1.0

| Aspect | v1.0 (Pre-6dk) | v2.0 (Post-6dk) |
|--------|----------------|------------------|
| File Structure | Single `agent.rs` | Modular: `agent/session.rs`, `agent/plugin.rs` |
| Backend Enum | `CliBackend` enum | `BackendId` + Registry |
| State Storage | Direct HashMap replacement | Preserve existing structure, extend carefully |
| run_cli_command | Replaced entirely | Preserved, new wrapper added |
| Persona System | Hardcoded templates | Plugin-based with TemplateLoader |

## Migration Path from v1.0 Doc

**DO NOT** use `MULTI_SESSION_IMPLEMENTATION.md` (v1.0). It targets the old architecture and will cause conflicts.

**ARCHIVE** the old doc for reference:
```bash
mv MULTI_SESSION_IMPLEMENTATION.md archive/MULTI_SESSION_IMPLEMENTATION_V1_DEPRECATED.md
```

---

**Status:** Design complete, ready for implementation
**Next Steps:** Apply changes sequentially, test after each step, then proceed to bp6-643.002 (Logging)
