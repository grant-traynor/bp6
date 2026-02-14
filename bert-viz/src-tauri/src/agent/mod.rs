/// Agent module for managing AI agent sessions and CLI backends
///
/// This module provides a plugin-based architecture for integrating different
/// CLI backends (Gemini, Claude Code, etc.) and persona templates.

pub mod backends;
pub mod persona;
pub mod personas;
pub mod plugin;
pub mod registry;
pub mod session;
pub mod templates;

// Re-export commonly used types from plugin module (for future use)
#[allow(unused_imports)]
pub use plugin::{AgentChunk, BackendId, CliBackendPlugin};

// Re-export persona types
#[allow(unused_imports)]
pub use persona::{PersonaContext, PersonaPlugin, PersonaRegistry, PersonaType};

// Re-export agent session types
pub use session::AgentState;

// Re-export multi-session types
#[allow(unused_imports)]
pub use session::{SessionInfo, SessionState, SessionStatus};

// Re-export logging types
#[allow(unused_imports)]
pub use session::{LogEvent, LogEventType};
