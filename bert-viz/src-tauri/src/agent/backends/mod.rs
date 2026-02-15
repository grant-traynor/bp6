/// CLI backend implementations
///
/// This module contains concrete implementations of the CliBackendPlugin trait
/// for various AI CLI backends (Gemini, Claude Code, etc.)
pub mod claude;
pub mod gemini;

pub use claude::ClaudeCodeBackend;
pub use gemini::GeminiBackend;
