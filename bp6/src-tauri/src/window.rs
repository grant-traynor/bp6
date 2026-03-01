/// Window management module for multi-window session support
///
/// This module provides Tauri commands to create, manage, and track session-specific windows.
/// Each window is associated with a session ID and can display an independent agent conversation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::path::PathBuf;
use std::fs;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

/// WindowInfo contains metadata about a session window
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub window_label: String,
    pub session_id: String,
    pub created_at: String,
}

/// WindowRegistry tracks session ID to window label mappings
/// Thread-safe using RwLock for concurrent access
pub struct WindowRegistry {
    /// Map from session_id to window_label
    session_to_window: Arc<RwLock<HashMap<String, String>>>,
    /// Map from window_label to session_id (reverse lookup)
    window_to_session: Arc<RwLock<HashMap<String, String>>>,
}

impl WindowRegistry {
    pub fn new() -> Self {
        WindowRegistry {
            session_to_window: Arc::new(RwLock::new(HashMap::new())),
            window_to_session: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new window-session mapping
    pub fn register(&self, session_id: String, window_label: String) {
        let mut session_map = self.session_to_window.write().unwrap();
        let mut window_map = self.window_to_session.write().unwrap();

        session_map.insert(session_id.clone(), window_label.clone());
        window_map.insert(window_label, session_id);
    }

    /// Unregister a window by session ID
    pub fn unregister_by_session(&self, session_id: &str) -> Option<String> {
        let mut session_map = self.session_to_window.write().unwrap();
        let mut window_map = self.window_to_session.write().unwrap();

        if let Some(window_label) = session_map.remove(session_id) {
            window_map.remove(&window_label);
            Some(window_label)
        } else {
            None
        }
    }

    /// Unregister a window by window label
    #[allow(dead_code)]
    pub fn unregister_by_window(&self, window_label: &str) -> Option<String> {
        let mut session_map = self.session_to_window.write().unwrap();
        let mut window_map = self.window_to_session.write().unwrap();

        if let Some(session_id) = window_map.remove(window_label) {
            session_map.remove(&session_id);
            Some(session_id)
        } else {
            None
        }
    }

    /// Get window label for a session ID
    pub fn get_window_label(&self, session_id: &str) -> Option<String> {
        let session_map = self.session_to_window.read().unwrap();
        session_map.get(session_id).cloned()
    }

    /// Get session ID for a window label
    pub fn get_session_id(&self, window_label: &str) -> Option<String> {
        let window_map = self.window_to_session.read().unwrap();
        window_map.get(window_label).cloned()
    }

    /// Get all window-session mappings
    pub fn get_all_windows(&self) -> Vec<WindowInfo> {
        let window_map = self.window_to_session.read().unwrap();
        window_map
            .iter()
            .map(|(window_label, session_id)| WindowInfo {
                window_label: window_label.clone(),
                session_id: session_id.clone(),
                created_at: chrono::Utc::now().to_rfc3339(),
            })
            .collect()
    }

    /// Check if a session already has a window
    pub fn has_window_for_session(&self, session_id: &str) -> bool {
        let session_map = self.session_to_window.read().unwrap();
        session_map.contains_key(session_id)
    }
}

/// Create a new window for a specific session
///
/// # Arguments
/// * `app` - Tauri AppHandle
/// * `session_id` - UUID of the session to display in this window
///
/// # Returns
/// The window label (e.g., "agent-session-{uuid}")
#[tauri::command]
#[allow(non_snake_case)]
pub async fn create_session_window(
    app: AppHandle,
    sessionId: String,
) -> Result<String, String> {
    eprintln!("🪟 create_session_window: session_id={}", sessionId);

    // Generate window label from session ID
    let window_label = format!("agent-session-{}", sessionId);

    // Get WindowRegistry from managed state
    let registry = app.state::<WindowRegistry>();

    // Check if window already exists for this session (duplicate prevention)
    if registry.has_window_for_session(&sessionId) {
        let existing_label = registry.get_window_label(&sessionId).unwrap();
        eprintln!("⚠️  Window already exists for session {}: {}", sessionId, existing_label);

        // Try to focus existing window
        if let Some(window) = app.get_webview_window(&existing_label) {
            let _ = window.set_focus();
            return Ok(existing_label);
        }
    }

    // Load saved window state if it exists
    let saved_state = load_window_state(sessionId.clone()).await.ok().flatten();

    // Create new window with session context
    let url = WebviewUrl::App(format!("index.html?session_id={}", sessionId).into());

    // Build window with saved state or defaults
    let mut builder = WebviewWindowBuilder::new(&app, &window_label, url)
        .title(format!("Agent Session - {}", sessionId))
        .resizable(true)
        .always_on_top(true);

    // Apply saved state if available
    if let Some(state) = saved_state {
        eprintln!("📍 Restoring window state: x={}, y={}, w={}, h={}, maximized={}",
                  state.x, state.y, state.width, state.height, state.is_maximized);

        builder = builder
            .position(state.x as f64, state.y as f64)
            .inner_size(state.width as f64, state.height as f64);

        if state.is_maximized {
            builder = builder.maximized(true);
        }
    } else {
        // Default size if no saved state
        builder = builder.inner_size(800.0, 600.0);
    }

    let _window = builder
        .build()
        .map_err(|e| format!("Failed to create window: {}", e))?;

    eprintln!("✅ Created window: {}", window_label);

    // Register window in registry
    registry.register(sessionId.clone(), window_label.clone());

    // Emit window-created event
    let _ = app.emit("window-created", WindowInfo {
        window_label: window_label.clone(),
        session_id: sessionId,
        created_at: chrono::Utc::now().to_rfc3339(),
    });

    Ok(window_label)
}

/// Get the session ID associated with a window label
///
/// # Arguments
/// * `app` - Tauri AppHandle
/// * `windowLabel` - The window label to lookup
///
/// # Returns
/// Optional session ID if window exists in registry
#[tauri::command]
#[allow(non_snake_case)]
pub async fn get_window_session_id(
    app: AppHandle,
    windowLabel: String,
) -> Result<Option<String>, String> {
    let registry = app.state::<WindowRegistry>();
    Ok(registry.get_session_id(&windowLabel))
}

/// Close a session window by session ID
///
/// # Arguments
/// * `app` - Tauri AppHandle
/// * `sessionId` - The session ID whose window should be closed
///
/// # Returns
/// Unit result
#[tauri::command]
#[allow(non_snake_case)]
pub async fn close_session_window(
    app: AppHandle,
    sessionId: String,
) -> Result<(), String> {
    eprintln!("🗑️  close_session_window: session_id={}", sessionId);

    let registry = app.state::<WindowRegistry>();

    // Get window label for session
    let window_label = registry.get_window_label(&sessionId)
        .ok_or_else(|| format!("No window found for session {}", sessionId))?;

    // Close the window
    if let Some(window) = app.get_webview_window(&window_label) {
        window.close().map_err(|e| format!("Failed to close window: {}", e))?;
        eprintln!("✅ Closed window: {}", window_label);
    } else {
        eprintln!("⚠️  Window {} not found (may already be closed)", window_label);
    }

    // Unregister from registry
    registry.unregister_by_session(&sessionId);

    // Emit window-closed event
    let _ = app.emit("window-closed", sessionId);

    Ok(())
}

/// List all session windows
///
/// # Returns
/// Vector of WindowInfo for all tracked windows
#[tauri::command]
pub async fn list_session_windows(
    app: AppHandle,
) -> Result<Vec<WindowInfo>, String> {
    let registry = app.state::<WindowRegistry>();
    Ok(registry.get_all_windows())
}

// ============================================================================
// Window State Persistence
// ============================================================================

/// Window state for persistence across app restarts
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    pub session_id: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_maximized: bool,
    pub last_updated: u64,
}

/// Get the path to the window state file (~/.bp6/window-state.json)
fn get_window_state_file_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Failed to get home directory")?;
    let bp6_dir = home.join(".bp6");

    // Ensure directory exists
    if !bp6_dir.exists() {
        fs::create_dir_all(&bp6_dir)
            .map_err(|e| format!("Failed to create .bp6 directory: {}", e))?;
    }

    Ok(bp6_dir.join("window-state.json"))
}

/// Load all window states from disk
fn load_window_states() -> Result<HashMap<String, WindowState>, String> {
    let path = get_window_state_file_path()?;

    if !path.exists() {
        return Ok(HashMap::new());
    }

    let contents = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read window state file: {}", e))?;

    serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse window state file: {}", e))
}

/// Save all window states to disk
fn save_window_states(states: &HashMap<String, WindowState>) -> Result<(), String> {
    let path = get_window_state_file_path()?;

    let contents = serde_json::to_string_pretty(states)
        .map_err(|e| format!("Failed to serialize window states: {}", e))?;

    fs::write(&path, contents)
        .map_err(|e| format!("Failed to write window state file: {}", e))
}

/// Clean up stale window states (sessions closed > 30 days ago)
fn cleanup_stale_states(states: &mut HashMap<String, WindowState>) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let thirty_days_seconds = 30 * 24 * 60 * 60;

    states.retain(|_, state| {
        now - state.last_updated < thirty_days_seconds
    });
}

/// Save window state for a session
///
/// # Arguments
/// * `session_id` - The session ID
/// * `x` - Window X position
/// * `y` - Window Y position
/// * `width` - Window width
/// * `height` - Window height
/// * `isMaximized` - Whether window is maximized
#[tauri::command]
#[allow(non_snake_case)]
pub async fn save_window_state(
    sessionId: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    isMaximized: bool,
) -> Result<(), String> {
    let mut states = load_window_states()?;

    // Cleanup stale entries before saving
    cleanup_stale_states(&mut states);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    states.insert(sessionId.clone(), WindowState {
        session_id: sessionId,
        x,
        y,
        width,
        height,
        is_maximized: isMaximized,
        last_updated: now,
    });

    save_window_states(&states)?;

    Ok(())
}

/// Load window state for a session
///
/// # Arguments
/// * `sessionId` - The session ID to load state for
///
/// # Returns
/// Optional WindowState if saved state exists
#[tauri::command]
#[allow(non_snake_case)]
pub async fn load_window_state(
    sessionId: String,
) -> Result<Option<WindowState>, String> {
    let states = load_window_states()?;
    Ok(states.get(&sessionId).cloned())
}

/// Toggle always-on-top for a window
///
/// # Arguments
/// * `app` - Tauri AppHandle
/// * `windowLabel` - The window label to toggle
/// * `alwaysOnTop` - Whether to enable always-on-top
#[tauri::command]
#[allow(non_snake_case)]
pub async fn toggle_window_always_on_top(
    app: AppHandle,
    windowLabel: String,
    alwaysOnTop: bool,
) -> Result<(), String> {
    eprintln!("🔄 toggle_window_always_on_top: window={}, state={}", windowLabel, alwaysOnTop);

    if let Some(window) = app.get_webview_window(&windowLabel) {
        window.set_always_on_top(alwaysOnTop)
            .map_err(|e| format!("Failed to set always-on-top: {}", e))?;
        eprintln!("✅ Set always-on-top={} for window: {}", alwaysOnTop, windowLabel);
        Ok(())
    } else {
        Err(format!("Window not found: {}", windowLabel))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_registry_register() {
        let registry = WindowRegistry::new();
        registry.register("session-1".to_string(), "window-1".to_string());

        assert_eq!(registry.get_window_label("session-1"), Some("window-1".to_string()));
        assert_eq!(registry.get_session_id("window-1"), Some("session-1".to_string()));
    }

    #[test]
    fn test_window_registry_unregister() {
        let registry = WindowRegistry::new();
        registry.register("session-1".to_string(), "window-1".to_string());

        let removed = registry.unregister_by_session("session-1");
        assert_eq!(removed, Some("window-1".to_string()));
        assert_eq!(registry.get_window_label("session-1"), None);
        assert_eq!(registry.get_session_id("window-1"), None);
    }

    #[test]
    fn test_window_registry_has_window() {
        let registry = WindowRegistry::new();
        assert!(!registry.has_window_for_session("session-1"));

        registry.register("session-1".to_string(), "window-1".to_string());
        assert!(registry.has_window_for_session("session-1"));

        registry.unregister_by_session("session-1");
        assert!(!registry.has_window_for_session("session-1"));
    }
}
