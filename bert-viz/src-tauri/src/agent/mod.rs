/// Agent module for managing AI agent sessions and CLI backends
///
/// This module provides a plugin-based architecture for integrating different
/// CLI backends (Gemini, Claude Code, etc.) and persona templates.

pub mod backends;
pub mod plugin;
pub mod registry;
pub mod session;

// Re-export commonly used types from plugin module (for future use)
#[allow(unused_imports)]
pub use plugin::{AgentChunk, BackendId, CliBackendPlugin};

// Re-export existing agent session functionality
pub use session::{AgentState, CliBackend};
