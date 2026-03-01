// Allow dead code for now - these will be used in subsequent features
#![allow(dead_code)]

/// Plugin architecture for CLI backend integrations
///
/// This module defines the trait-based plugin system that allows different
/// CLI backends (Gemini, Claude Code, etc.) to be implemented independently
/// and registered dynamically.
use serde::{Deserialize, Serialize};

/// Type-safe identifier for CLI backends
///
/// Used for registry lookup and configuration. Each variant corresponds
/// to a specific CLI backend implementation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum BackendId {
    /// Google Gemini CLI backend
    Gemini,
    /// Anthropic Claude Code CLI backend
    #[serde(rename = "claude")]
    ClaudeCode,
}

impl BackendId {
    /// Returns a human-readable display name for this backend
    pub fn display_name(&self) -> &str {
        match self {
            BackendId::Gemini => "Gemini",
            BackendId::ClaudeCode => "Claude Code",
        }
    }
}

impl std::fmt::Display for BackendId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Structured data for a file edit/create tool call.
/// Populated when the agent calls Edit or Write — enables diff rendering in the UI.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolUseData {
    /// Tool name: "Edit", "Write", etc.
    pub name: String,
    /// Absolute path of the file being modified
    pub file_path: String,
    /// Previous file content (empty string for new files)
    pub old_string: String,
    /// New file content / replacement string
    pub new_string: String,
}

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
    /// When true, the frontend should REPLACE the streaming buffer with this content
    /// rather than appending. Used by backends (e.g. Gemini) that emit cumulative
    /// full-text in each event rather than incremental deltas.
    #[serde(default)]
    pub is_replacement: bool,
    /// Optional session identifier (UUID v4) for multi-session support
    /// Serializes as "sessionId" in JSON
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Structured tool use data for Edit/Write calls — drives diff rendering in UI
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use: Option<ToolUseData>,
}

/// Plugin trait for CLI backend implementations
///
/// Each CLI backend (Gemini, Claude Code, etc.) implements this trait to provide
/// a common interface for:
/// - Command execution (binary name, arguments)
/// - Output parsing (streaming JSON format)
/// - Feature detection (streaming support, etc.)
///
/// # Thread Safety
///
/// Implementations must be Send + Sync to support concurrent agent sessions.
///
/// # Example
///
/// ```ignore
/// struct GeminiBackend;
///
/// impl CliBackendPlugin for GeminiBackend {
///     fn command_name(&self) -> &str {
///         "gemini"
///     }
///
///     fn supports_streaming(&self) -> bool {
///         true
///     }
///
///     fn build_args(&self, prompt: &str, resume: bool) -> Vec<String> {
///         let mut args = vec![
///             "--output-format".to_string(),
///             "stream-json".to_string(),
///         ];
///         if resume {
///             args.push("--resume".to_string());
///             args.push("latest".to_string());
///         }
///         args.push("--prompt".to_string());
///         args.push(prompt.to_string());
///         args
///     }
///
///     fn parse_stdout_line(&self, json: &serde_json::Value) -> Option<AgentChunk> {
///         // Parse Gemini-specific JSON format
///         if json["type"] == "message" && json["role"] == "assistant" {
///             json["content"].as_str().map(|content| AgentChunk {
///                 content: content.to_string(),
///                 is_done: false,
///                 session_id: None,
///             })
///         } else if json["type"] == "result" {
///             Some(AgentChunk {
///                 content: String::new(),
///                 is_done: true,
///                 session_id: None,
///             })
///         } else {
///             None
///         }
///     }
/// }
/// ```
pub trait CliBackendPlugin: Send + Sync {
    /// Returns the CLI command name to execute (e.g., "gemini", "claude")
    ///
    /// This is the binary name that will be spawned as a subprocess.
    fn command_name(&self) -> &str;

    /// Returns whether this backend supports streaming output
    ///
    /// All current backends support streaming, but this allows for future
    /// backends that may not.
    fn supports_streaming(&self) -> bool;

    /// Builds the command-line arguments for this backend
    ///
    /// # Arguments
    ///
    /// * `prompt` - The prompt text to send to the agent
    /// * `resume` - Whether to resume the previous session
    /// * `session_id` - Optional session ID for resume (required for some backends)
    ///
    /// # Returns
    ///
    /// A vector of command-line arguments to pass to the CLI binary.
    /// The command name itself should NOT be included.
    fn build_args(&self, prompt: &str, resume: bool, session_id: Option<&str>) -> Vec<String>;

    /// Builds the command-line arguments for this backend in PTY mode
    ///
    /// PTY mode is used for interactive terminal sessions. This differs
    /// from standard mode in that it should NOT request JSON output,
    /// but instead should enable terminal UI features.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The CLI-provided session ID to resume (required)
    ///
    /// # Returns
    ///
    /// A vector of command-line arguments for PTY mode.
    fn build_pty_args(&self, session_id: &str) -> Vec<String>;

    /// Parses a line of JSON output from the CLI's stdout
    ///
    /// Each backend has its own JSON format for streaming output. This method
    /// is responsible for parsing backend-specific JSON into the common
    /// AgentChunk format.
    ///
    /// # Arguments
    ///
    /// * `json` - A parsed JSON value from a single line of stdout
    ///
    /// # Returns
    ///
    /// * `Some(AgentChunk)` if this line contains parseable content or completion signal
    /// * `None` if this line should be ignored (e.g., non-message JSON)
    fn parse_stdout_line(&self, json: &serde_json::Value) -> Option<AgentChunk>;
}
