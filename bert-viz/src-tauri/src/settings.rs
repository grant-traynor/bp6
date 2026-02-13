use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::State;
use crate::agent::plugin::BackendId;
use crate::SettingsState;

/// Application settings structure
/// Stores user preferences including CLI backend choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(rename = "cliBackend")]
    pub cli_backend: BackendId,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            cli_backend: BackendId::Gemini,
        }
    }
}

impl AppSettings {
    /// Load settings from a JSON file
    /// Returns default settings if file doesn't exist or is invalid
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read settings file: {}", e))?;

        let settings: AppSettings = serde_json::from_str(&contents)
            .map_err(|e| format!("Failed to parse settings JSON: {}", e))?;

        // Validation happens automatically through CliBackend's Deserialize implementation
        Ok(settings)
    }

    /// Save settings to a JSON file
    /// Creates parent directories if they don't exist
    pub fn save_to_file(&self, path: &Path) -> Result<(), String> {
        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create settings directory: {}", e))?;
        }

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;

        fs::write(path, json)
            .map_err(|e| format!("Failed to write settings file: {}", e))?;

        Ok(())
    }
}

/// Get the OS-appropriate configuration directory path for settings.json
/// Creates parent directories if they don't exist
pub fn get_config_path() -> Result<PathBuf, String> {
    // Get the user's config directory based on OS
    let config_dir = if cfg!(target_os = "macos") {
        std::env::var("HOME")
            .map(|h| PathBuf::from(h).join("Library").join("Application Support"))
            .map_err(|_| "HOME environment variable not set")?
    } else if cfg!(target_os = "linux") {
        std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map_err(|_| "Neither XDG_CONFIG_HOME nor HOME environment variable is set")?
    } else if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .map_err(|_| "APPDATA environment variable not set")?
    } else {
        return Err("Unsupported operating system".to_string());
    };

    // Create app-specific subdirectory
    let app_dir = config_dir.join("com.pairti.bert");

    // Create directory if it doesn't exist
    if !app_dir.exists() {
        fs::create_dir_all(&app_dir)
            .map_err(|e| format!("Failed to create config directory {}: {}", app_dir.display(), e))?;
    }

    Ok(app_dir.join("settings.json"))
}

/// Tauri command to get the current CLI preference
#[tauri::command]
pub fn get_cli_preference(settings_state: State<'_, SettingsState>) -> Result<String, String> {
    let settings = settings_state.settings.lock()
        .map_err(|e| format!("Failed to acquire settings lock: {}", e))?;

    // Convert CliBackend to string representation
    let cli_str = match settings.cli_backend {
        BackendId::Gemini => "gemini",
        BackendId::ClaudeCode => "claude",
    };

    Ok(cli_str.to_string())
}

/// Tauri command to set the CLI preference and persist to disk
#[tauri::command]
pub fn set_cli_preference(
    cli_backend: String,
    settings_state: State<'_, SettingsState>
) -> Result<(), String> {
    // Parse and validate the CLI backend string
    let backend = match cli_backend.to_lowercase().as_str() {
        "gemini" => BackendId::Gemini,
        "claude" | "claude-code" => BackendId::ClaudeCode,
        _ => return Err(format!("Invalid CLI backend: '{}'. Valid options are: 'gemini', 'claude', 'claude-code'", cli_backend)),
    };

    // Update settings in state
    let mut settings = settings_state.settings.lock()
        .map_err(|e| format!("Failed to acquire settings lock: {}", e))?;

    settings.cli_backend = backend;

    // Persist to disk
    let config_path = get_config_path()?;
    settings.save_to_file(&config_path)?;

    eprintln!("✅ Updated CLI preference to: {}", cli_backend);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_default_settings() {
        let settings = AppSettings::default();
        assert_eq!(settings.cli_backend, BackendId::Gemini);
    }

    #[test]
    fn test_settings_serialization() {
        let settings = AppSettings {
            cli_backend: BackendId::ClaudeCode,
        };

        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("\"cliBackend\":\"claude\""));

        let deserialized: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.cli_backend, BackendId::ClaudeCode);
    }

    #[test]
    fn test_load_missing_file() {
        let temp_path = env::temp_dir().join("nonexistent_settings.json");
        let settings = AppSettings::load_from_file(&temp_path).unwrap();
        assert_eq!(settings.cli_backend, BackendId::Gemini);
    }

    #[test]
    fn test_save_and_load() {
        let temp_path = env::temp_dir().join("test_settings.json");

        // Clean up if exists
        let _ = fs::remove_file(&temp_path);

        // Save settings
        let settings = AppSettings {
            cli_backend: BackendId::ClaudeCode,
        };
        settings.save_to_file(&temp_path).unwrap();

        // Load settings
        let loaded = AppSettings::load_from_file(&temp_path).unwrap();
        assert_eq!(loaded.cli_backend, BackendId::ClaudeCode);

        // Clean up
        let _ = fs::remove_file(&temp_path);
    }

    #[test]
    fn test_get_config_path() {
        let config_path = get_config_path().unwrap();

        // Verify path ends with settings.json
        assert!(config_path.ends_with("settings.json"));

        // Verify it contains the app identifier
        assert!(config_path.to_string_lossy().contains("com.pairti.bert"));

        // Verify parent directory exists (created by get_config_path)
        assert!(config_path.parent().unwrap().exists());

        // Test platform-specific paths
        #[cfg(target_os = "macos")]
        {
            assert!(config_path.to_string_lossy().contains("Library/Application Support"));
        }

        #[cfg(target_os = "linux")]
        {
            let path_str = config_path.to_string_lossy();
            assert!(path_str.contains(".config") || path_str.contains("XDG_CONFIG_HOME"));
        }

        #[cfg(target_os = "windows")]
        {
            assert!(config_path.to_string_lossy().contains("AppData"));
        }
    }

    #[test]
    fn test_config_path_directory_creation() {
        // This test verifies that get_config_path creates directories
        let config_path = get_config_path().unwrap();
        let parent = config_path.parent().unwrap();

        // The directory should exist after calling get_config_path
        assert!(parent.exists());
        assert!(parent.is_dir());
    }

    #[test]
    fn test_cli_preference_round_trip() {
        // Create a temporary settings file
        let temp_dir = env::temp_dir().join("test_cli_pref");
        let _ = fs::remove_dir_all(&temp_dir); // Clean up if exists
        fs::create_dir_all(&temp_dir).unwrap();
        let temp_path = temp_dir.join("settings.json");

        // Create settings with Gemini
        let settings1 = AppSettings {
            cli_backend: BackendId::Gemini,
        };
        settings1.save_to_file(&temp_path).unwrap();

        // Load and verify
        let loaded1 = AppSettings::load_from_file(&temp_path).unwrap();
        assert_eq!(loaded1.cli_backend, BackendId::Gemini);

        // Update to Claude
        let settings2 = AppSettings {
            cli_backend: BackendId::ClaudeCode,
        };
        settings2.save_to_file(&temp_path).unwrap();

        // Load and verify
        let loaded2 = AppSettings::load_from_file(&temp_path).unwrap();
        assert_eq!(loaded2.cli_backend, BackendId::ClaudeCode);

        // Update back to Gemini
        let settings3 = AppSettings {
            cli_backend: BackendId::Gemini,
        };
        settings3.save_to_file(&temp_path).unwrap();

        // Load and verify
        let loaded3 = AppSettings::load_from_file(&temp_path).unwrap();
        assert_eq!(loaded3.cli_backend, BackendId::Gemini);

        // Verify file contents are valid JSON
        let contents = fs::read_to_string(&temp_path).unwrap();
        assert!(contents.contains("\"cliBackend\""));
        assert!(contents.contains("\"gemini\""));

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_cli_preference_validation() {
        // Test valid backend strings
        assert_eq!(
            match "gemini".to_lowercase().as_str() {
                "gemini" => Ok(BackendId::Gemini),
                "claude" | "claude-code" => Ok(BackendId::ClaudeCode),
                _ => Err("Invalid"),
            },
            Ok(BackendId::Gemini)
        );

        assert_eq!(
            match "claude".to_lowercase().as_str() {
                "gemini" => Ok(BackendId::Gemini),
                "claude" | "claude-code" => Ok(BackendId::ClaudeCode),
                _ => Err("Invalid"),
            },
            Ok(BackendId::ClaudeCode)
        );

        assert_eq!(
            match "claude-code".to_lowercase().as_str() {
                "gemini" => Ok(BackendId::Gemini),
                "claude" | "claude-code" => Ok(BackendId::ClaudeCode),
                _ => Err("Invalid"),
            },
            Ok(BackendId::ClaudeCode)
        );

        // Test invalid backend string
        assert_eq!(
            match "invalid-cli".to_lowercase().as_str() {
                "gemini" => Ok(BackendId::Gemini),
                "claude" | "claude-code" => Ok(BackendId::ClaudeCode),
                _ => Err("Invalid"),
            },
            Err("Invalid")
        );
    }
}
