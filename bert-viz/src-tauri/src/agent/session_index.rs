/// Session resume index - maps bead+persona to most recent session ID
///
/// This allows automatic session resumption when reopening a chat for the same bead/persona.
/// The index is persisted to ~/.bp6/session_index.json

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Key for looking up sessions: "{bead_id or 'untracked'}-{persona}"
type SessionKey = String;

/// Session metadata for resumption
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadata {
    /// The session UUID (our internal ID)
    pub session_id: String,
    /// The CLI-provided session ID (for resume)
    pub cli_session_id: Option<String>,
    /// When this session was last active
    pub last_active: u64,
    /// The backend used (gemini, claude-code)
    pub backend_id: String,
}

/// Session resume index
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionIndex {
    /// Map of "{bead_id}-{persona}" to session metadata
    sessions: HashMap<SessionKey, SessionMetadata>,
}

impl SessionIndex {
    /// Get the session index file path
    fn index_path() -> Result<PathBuf, String> {
        let home = dirs::home_dir().ok_or_else(|| "Could not find home directory".to_string())?;
        Ok(home.join(".bp6").join("session_index.json"))
    }

    /// Load the session index from disk
    pub fn load() -> Result<Self, String> {
        let path = Self::index_path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read session index: {}", e))?;

        let index: SessionIndex = serde_json::from_str(&contents)
            .map_err(|e| format!("Failed to parse session index: {}", e))?;

        Ok(index)
    }

    /// Save the session index to disk
    pub fn save(&self) -> Result<(), String> {
        let path = Self::index_path()?;

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create .bp6 directory: {}", e))?;
        }

        let json = serde_json::to_string_pretty(&self)
            .map_err(|e| format!("Failed to serialize session index: {}", e))?;

        fs::write(&path, json)
            .map_err(|e| format!("Failed to write session index: {}", e))?;

        Ok(())
    }

    /// Make a session key from bead_id and persona
    fn make_key(bead_id: Option<&str>, persona: &str) -> String {
        format!("{}-{}", bead_id.unwrap_or("untracked"), persona)
    }

    /// Record a session for a bead/persona combination
    pub fn record_session(
        &mut self,
        bead_id: Option<&str>,
        persona: &str,
        session_id: String,
        cli_session_id: Option<String>,
        backend_id: String,
    ) {
        let key = Self::make_key(bead_id, persona);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        self.sessions.insert(
            key,
            SessionMetadata {
                session_id,
                cli_session_id,
                last_active: now,
                backend_id,
            },
        );
    }

    /// Get the most recent session for a bead/persona combination
    pub fn get_session(&self, bead_id: Option<&str>, persona: &str) -> Option<&SessionMetadata> {
        let key = Self::make_key(bead_id, persona);
        self.sessions.get(&key)
    }

    /// Remove a session from the index
    pub fn remove_session(&mut self, bead_id: Option<&str>, persona: &str) {
        let key = Self::make_key(bead_id, persona);
        self.sessions.remove(&key);
    }

    /// Update the last active timestamp for a session
    pub fn touch_session(&mut self, bead_id: Option<&str>, persona: &str) {
        let key = Self::make_key(bead_id, persona);
        if let Some(meta) = self.sessions.get_mut(&key) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            meta.last_active = now;
        }
    }

    /// Clean up old sessions (older than 30 days)
    pub fn cleanup_old_sessions(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let thirty_days = 30 * 24 * 60 * 60;

        self.sessions.retain(|_, meta| {
            now - meta.last_active < thirty_days
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_key() {
        assert_eq!(
            SessionIndex::make_key(Some("bp6-123"), "product-manager"),
            "bp6-123-product-manager"
        );
        assert_eq!(
            SessionIndex::make_key(None, "qa-engineer"),
            "untracked-qa-engineer"
        );
    }

    #[test]
    fn test_record_and_get() {
        let mut index = SessionIndex::default();

        index.record_session(
            Some("bp6-123"),
            "product-manager",
            "session-uuid-1".to_string(),
            Some("cli-session-1".to_string()),
            "gemini".to_string(),
        );

        let meta = index.get_session(Some("bp6-123"), "product-manager").unwrap();
        assert_eq!(meta.session_id, "session-uuid-1");
        assert_eq!(meta.cli_session_id, Some("cli-session-1".to_string()));
        assert_eq!(meta.backend_id, "gemini");
    }

    #[test]
    fn test_remove_session() {
        let mut index = SessionIndex::default();

        index.record_session(
            Some("bp6-123"),
            "product-manager",
            "session-uuid-1".to_string(),
            None,
            "gemini".to_string(),
        );

        assert!(index.get_session(Some("bp6-123"), "product-manager").is_some());

        index.remove_session(Some("bp6-123"), "product-manager");

        assert!(index.get_session(Some("bp6-123"), "product-manager").is_none());
    }
}
