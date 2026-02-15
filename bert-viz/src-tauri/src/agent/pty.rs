/// PTY module for spawning and managing pseudo-terminal sessions
///
/// This module provides basic plumbing for spawning PTY processes and
/// managing their lifecycle. Used for terminal-based agent sessions.
///
/// Note: tauri-plugin-pty is registered in the Tauri builder and provides
/// frontend-accessible commands (spawn, write, read, resize, kill). This
/// module provides backend utilities for direct PTY usage if needed.

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

// Re-export portable-pty types for convenience
pub use portable_pty::{Child, ChildKiller, CommandBuilder, PtyPair, PtySize};

/// Session holds a PTY pair and the child process
pub struct PtySession {
    pub pair: PtyPair,
    pub child: Box<dyn Child + Send + Sync>,
    pub child_killer: Box<dyn ChildKiller + Send + Sync>,
    pub writer: Box<dyn Write + Send>,
    pub reader: Box<dyn Read + Send>,
}

/// PtyManager handles spawning and cleanup of PTY processes
pub struct PtyManager {
    /// Active PTY sessions, keyed by session ID
    sessions: Arc<Mutex<HashMap<String, Arc<Mutex<PtySession>>>>>,
}

impl PtyManager {
    /// Create a new PtyManager instance
    pub fn new() -> Self {
        PtyManager {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Spawn a new PTY process running the given command
    ///
    /// # Arguments
    /// * `session_id` - Unique identifier for this PTY session
    /// * `command` - The command to execute in the PTY (e.g., "bash", "zsh", "sh")
    /// * `args` - Arguments to pass to the command
    /// * `working_dir` - Optional working directory for the process
    /// * `cols` - Terminal columns (default: 80)
    /// * `rows` - Terminal rows (default: 24)
    ///
    /// # Returns
    /// * `Ok(())` if the PTY was spawned successfully
    /// * `Err(String)` if spawning failed
    pub fn spawn(
        &self,
        session_id: String,
        command: String,
        args: Vec<String>,
        working_dir: Option<String>,
        cols: Option<u16>,
        rows: Option<u16>,
    ) -> Result<(), String> {
        let pty_system = portable_pty::native_pty_system();

        // Create PTY with specified size
        let pair = pty_system
            .openpty(PtySize {
                rows: rows.unwrap_or(24),
                cols: cols.unwrap_or(80),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to create PTY: {}", e))?;

        // Get reader and writer
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to get PTY writer: {}", e))?;
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("Failed to get PTY reader: {}", e))?;

        // Build command
        let mut cmd = CommandBuilder::new(command.clone());
        cmd.args(args.clone());

        if let Some(dir) = working_dir {
            cmd.cwd(OsString::from(dir));
        }

        // Spawn the command in the PTY
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to spawn command '{}': {}", command, e))?;

        let child_killer = child.clone_killer();

        // Create session
        let session = PtySession {
            pair,
            child,
            child_killer,
            writer,
            reader,
        };

        // Store in sessions map
        let mut sessions = self.sessions.lock().unwrap();
        sessions.insert(session_id.clone(), Arc::new(Mutex::new(session)));

        eprintln!("✅ PTY spawned for session: {}", session_id);
        Ok(())
    }

    /// Write data to a PTY session
    ///
    /// # Arguments
    /// * `session_id` - The session to write to
    /// * `data` - The data to write (typically user input)
    ///
    /// # Returns
    /// * `Ok(())` if write succeeded
    /// * `Err(String)` if session not found or write failed
    pub fn write(&self, session_id: &str, data: &[u8]) -> Result<(), String> {
        let sessions = self.sessions.lock().unwrap();

        let session_arc = sessions
            .get(session_id)
            .ok_or_else(|| format!("PTY session not found: {}", session_id))?;

        let mut session = session_arc.lock().unwrap();

        session
            .writer
            .write_all(data)
            .map_err(|e| format!("Failed to write to PTY: {}", e))
    }

    /// Read available data from a PTY session (blocking up to buffer size)
    ///
    /// # Arguments
    /// * `session_id` - The session to read from
    ///
    /// # Returns
    /// * `Ok(Vec<u8>)` - Data read from PTY (may be empty if EOF)
    /// * `Err(String)` if session not found or read failed
    pub fn read(&self, session_id: &str) -> Result<Vec<u8>, String> {
        let sessions = self.sessions.lock().unwrap();

        let session_arc = sessions
            .get(session_id)
            .ok_or_else(|| format!("PTY session not found: {}", session_id))?;

        let mut session = session_arc.lock().unwrap();

        // Read up to 4096 bytes
        let mut buf = vec![0u8; 4096];
        match session.reader.read(&mut buf) {
            Ok(n) => {
                buf.truncate(n);
                Ok(buf)
            }
            Err(e) => Err(format!("Failed to read from PTY: {}", e)),
        }
    }

    /// Resize a PTY session
    ///
    /// # Arguments
    /// * `session_id` - The session to resize
    /// * `cols` - Number of columns
    /// * `rows` - Number of rows
    ///
    /// # Returns
    /// * `Ok(())` if resize succeeded
    /// * `Err(String)` if session not found or resize failed
    pub fn resize(&self, session_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let sessions = self.sessions.lock().unwrap();

        let session_arc = sessions
            .get(session_id)
            .ok_or_else(|| format!("PTY session not found: {}", session_id))?;

        let session = session_arc.lock().unwrap();

        session
            .pair
            .master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to resize PTY: {}", e))
    }

    /// Kill a PTY session and remove it from the manager
    ///
    /// # Arguments
    /// * `session_id` - The session to kill
    ///
    /// # Returns
    /// * `Ok(())` if session was killed
    /// * `Err(String)` if session not found or kill failed
    pub fn kill(&self, session_id: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock().unwrap();

        let session_arc = sessions
            .remove(session_id)
            .ok_or_else(|| format!("PTY session not found: {}", session_id))?;

        let mut session = session_arc.lock().unwrap();

        // Kill the process
        session
            .child_killer
            .kill()
            .map_err(|e| format!("Failed to kill PTY: {}", e))?;

        eprintln!("✅ PTY killed for session: {}", session_id);
        Ok(())
    }

    /// Check if a PTY session exists
    pub fn has_session(&self, session_id: &str) -> bool {
        let sessions = self.sessions.lock().unwrap();
        sessions.contains_key(session_id)
    }

    /// Get list of active session IDs
    pub fn list_sessions(&self) -> Vec<String> {
        let sessions = self.sessions.lock().unwrap();
        sessions.keys().cloned().collect()
    }
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_manager_creation() {
        let manager = PtyManager::new();
        assert_eq!(manager.list_sessions().len(), 0);
    }

    #[test]
    fn test_has_session() {
        let manager = PtyManager::new();
        assert!(!manager.has_session("test-session"));
    }

    // Note: Actual PTY spawning tests require a real TTY environment
    // and are better suited for integration tests
}
