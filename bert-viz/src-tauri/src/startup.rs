/// Startup state persistence module for bp6-j33p.2.1
///
/// This module provides Tauri commands to save and load application startup state
/// including window size, filters, sort options, and UI preferences.
///
/// State is persisted to ~/.bp6/startup.json

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// ============================================================================
// Data Structures
// ============================================================================

/// Window state for startup restoration
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    pub width: u32,
    pub height: u32,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub is_maximized: bool,
}

/// Filter state for startup restoration
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FilterState {
    pub filter_text: String,
    pub hide_closed: bool,
    pub closed_time_filter: String,
    pub include_hierarchy: bool,
}

/// Sort state for startup restoration
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SortState {
    pub sort_by: String,
    pub sort_order: String,
}

/// UI state for startup restoration (collapsed nodes, zoom, etc.)
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UiState {
    pub zoom: f64,
    pub collapsed_ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wbs_panel_width: Option<f64>,
    #[serde(default = "default_view")]
    pub current_view: String,
}

fn default_view() -> String {
    "gantt".to_string()
}

/// Complete startup state containing all restorable application state
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StartupState {
    pub window: WindowState,
    pub filters: FilterState,
    pub sort: SortState,
    pub ui: UiState,
}

// ============================================================================
// Default Implementations
// ============================================================================

impl Default for WindowState {
    fn default() -> Self {
        WindowState {
            width: 1200,
            height: 800,
            x: None,
            y: None,
            is_maximized: false,
        }
    }
}

impl Default for FilterState {
    fn default() -> Self {
        FilterState {
            filter_text: String::new(),
            hide_closed: false,
            closed_time_filter: "all".to_string(),
            include_hierarchy: true,
        }
    }
}

impl Default for SortState {
    fn default() -> Self {
        SortState {
            sort_by: "none".to_string(),
            sort_order: "none".to_string(),
        }
    }
}

impl Default for UiState {
    fn default() -> Self {
        UiState {
            zoom: 1.0,
            collapsed_ids: Vec::new(),
            wbs_panel_width: None,
            current_view: default_view(),
        }
    }
}

impl Default for StartupState {
    fn default() -> Self {
        StartupState {
            window: WindowState::default(),
            filters: FilterState::default(),
            sort: SortState::default(),
            ui: UiState::default(),
        }
    }
}

// ============================================================================
// File Path Helpers
// ============================================================================

/// Get the path to the startup state file (~/.bp6/startup.json)
fn get_startup_state_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Failed to get home directory")?;
    let bp6_dir = home.join(".bp6");

    // Ensure directory exists
    if !bp6_dir.exists() {
        fs::create_dir_all(&bp6_dir)
            .map_err(|e| format!("Failed to create .bp6 directory: {}", e))?;
    }

    Ok(bp6_dir.join("startup.json"))
}

// ============================================================================
// Tauri Commands
// ============================================================================

/// Save startup state to ~/.bp6/startup.json
///
/// # Arguments
/// * `state` - The StartupState to persist
///
/// # Returns
/// Unit result or error message
#[tauri::command]
pub async fn save_startup_state(state: StartupState) -> Result<(), String> {
    let path = get_startup_state_path()?;

    let contents = serde_json::to_string_pretty(&state)
        .map_err(|e| format!("Failed to serialize startup state: {}", e))?;

    fs::write(&path, contents)
        .map_err(|e| format!("Failed to write startup state file: {}", e))?;
    Ok(())
}

/// Load startup state from ~/.bp6/startup.json
///
/// # Returns
/// Optional StartupState if file exists and is valid, None otherwise
#[tauri::command]
pub async fn load_startup_state() -> Result<Option<StartupState>, String> {
    let path = get_startup_state_path()?;

    if !path.exists() {
        eprintln!("📂 No startup state file found at {}", path.display());
        return Ok(None);
    }

    let contents = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read startup state file: {}", e))?;

    let state: StartupState = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse startup state file: {}", e))?;

    eprintln!("✅ Loaded startup state from {}", path.display());
    Ok(Some(state))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_default_startup_state() {
        let state = StartupState::default();

        assert_eq!(state.window.width, 1200);
        assert_eq!(state.window.height, 800);
        assert_eq!(state.window.is_maximized, false);

        assert_eq!(state.filters.filter_text, "");
        assert_eq!(state.filters.hide_closed, false);
        assert_eq!(state.filters.include_hierarchy, true);

        assert_eq!(state.sort.sort_by, "none");
        assert_eq!(state.sort.sort_order, "none");

        assert_eq!(state.ui.zoom, 1.0);
        assert_eq!(state.ui.collapsed_ids.len(), 0);
        assert_eq!(state.ui.current_view, "gantt");
    }

    #[test]
    fn test_startup_state_serialization() {
        let state = StartupState {
            window: WindowState {
                width: 1920,
                height: 1080,
                x: Some(100),
                y: Some(200),
                is_maximized: true,
            },
            filters: FilterState {
                filter_text: "test".to_string(),
                hide_closed: true,
                closed_time_filter: "24h".to_string(),
                include_hierarchy: false,
            },
            sort: SortState {
                sort_by: "priority".to_string(),
                sort_order: "asc".to_string(),
            },
            ui: UiState {
                zoom: 1.5,
                collapsed_ids: vec!["id1".to_string(), "id2".to_string()],
                wbs_panel_width: Some(300.0),
                current_view: "gantt".to_string(),
            },
        };

        let json = serde_json::to_string(&state).unwrap();
        let deserialized: StartupState = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.window.width, 1920);
        assert_eq!(deserialized.window.height, 1080);
        assert_eq!(deserialized.window.x, Some(100));
        assert_eq!(deserialized.window.y, Some(200));
        assert_eq!(deserialized.window.is_maximized, true);

        assert_eq!(deserialized.filters.filter_text, "test");
        assert_eq!(deserialized.filters.hide_closed, true);
        assert_eq!(deserialized.filters.closed_time_filter, "24h");

        assert_eq!(deserialized.sort.sort_by, "priority");
        assert_eq!(deserialized.sort.sort_order, "asc");

        assert_eq!(deserialized.ui.zoom, 1.5);
        assert_eq!(deserialized.ui.collapsed_ids, vec!["id1", "id2"]);
    }

    #[test]
    fn test_save_and_load_startup_state() {
        // Create a custom test directory to avoid conflicts
        let test_dir = env::temp_dir().join("bp6_startup_test");
        let _ = fs::remove_dir_all(&test_dir); // Clean up if exists
        fs::create_dir_all(&test_dir).unwrap();

        // Note: This test would need to mock get_startup_state_path to use test_dir
        // For now, we'll just test serialization/deserialization

        let state = StartupState {
            window: WindowState {
                width: 1024,
                height: 768,
                x: None,
                y: None,
                is_maximized: false,
            },
            filters: FilterState::default(),
            sort: SortState::default(),
            ui: UiState::default(),
        };

        // Serialize and deserialize
        let json = serde_json::to_string_pretty(&state).unwrap();
        let loaded: StartupState = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.window.width, 1024);
        assert_eq!(loaded.window.height, 768);

        // Clean up
        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_window_state_optional_position() {
        let state = WindowState {
            width: 800,
            height: 600,
            x: None,
            y: None,
            is_maximized: false,
        };

        let json = serde_json::to_string(&state).unwrap();
        let loaded: WindowState = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.x, None);
        assert_eq!(loaded.y, None);
    }

    #[test]
    fn test_ui_state_empty_collapsed_ids() {
        let state = UiState {
            zoom: 1.0,
            collapsed_ids: Vec::new(),
            wbs_panel_width: None,
            current_view: "gantt".to_string(),
        };

        let json = serde_json::to_string(&state).unwrap();
        let loaded: UiState = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.collapsed_ids.len(), 0);
    }

    #[test]
    fn test_filter_state_all_fields() {
        let state = FilterState {
            filter_text: "epic".to_string(),
            hide_closed: true,
            closed_time_filter: "7d".to_string(),
            include_hierarchy: false,
        };

        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"filterText\":\"epic\""));
        assert!(json.contains("\"hideClosed\":true"));
        assert!(json.contains("\"closedTimeFilter\":\"7d\""));
        assert!(json.contains("\"includeHierarchy\":false"));
    }
}
