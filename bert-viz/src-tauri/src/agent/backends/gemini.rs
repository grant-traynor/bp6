/// Google Gemini CLI backend implementation

use crate::agent::plugin::{AgentChunk, CliBackendPlugin};
use serde_json::Value;

/// Gemini CLI backend plugin
///
/// Implements the CliBackendPlugin trait for Google's Gemini CLI.
/// Handles command execution and JSON output parsing specific to Gemini's format.
pub struct GeminiBackend;

impl GeminiBackend {
    /// Create a new Gemini backend instance
    pub fn new() -> Self {
        GeminiBackend
    }
}

impl CliBackendPlugin for GeminiBackend {
    fn command_name(&self) -> &str {
        "gemini"
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn build_args(&self, prompt: &str, resume: bool, session_id: Option<&str>) -> Vec<String> {
        let mut args = vec![
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--yolo".to_string(),
        ];

        if resume {
            args.push("--resume".to_string());
            // Gemini supports "latest" as a session ID
            args.push(session_id.unwrap_or("latest").to_string());
        }

        args.push("--prompt".to_string());
        args.push(prompt.to_string());

        args
    }

    fn parse_stdout_line(&self, json: &Value) -> Option<AgentChunk> {
        // Handle Gemini message format: {"type": "message", "role": "assistant", "content": "..."}
        if json["type"] == "message" && json["role"] == "assistant" {
            if let Some(content) = json["content"].as_str() {
                return Some(AgentChunk {
                    content: content.to_string(),
                    is_done: false,
                });
            }
        }

        // Handle tool use: {"type": "tool_use", "tool_name": "...", ...}
        if json["type"] == "tool_use" {
            if let Some(tool_name) = json["tool_name"].as_str() {
                return Some(AgentChunk {
                    content: format!("🔧 Using tool: {}", tool_name),
                    is_done: false,
                });
            }
        }

        // Handle tool results: {"type": "tool_result", "status": "success", ...}
        // Only show failed tool results to avoid clutter
        if json["type"] == "tool_result" {
            if let Some(status) = json["status"].as_str() {
                if status != "success" {
                    return Some(AgentChunk {
                        content: format!("⚠️ Tool execution {}", status),
                        is_done: false,
                    });
                }
            }
        }

        // Handle completion: {"type": "result"}
        if json["type"] == "result" {
            // Check for errors in result
            if json["subtype"] == "error_during_execution" || json["is_error"].as_bool().unwrap_or(false) {
                if let Some(errors) = json["errors"].as_array() {
                    let error_messages: Vec<String> = errors
                        .iter()
                        .filter_map(|e| e.as_str())
                        .map(|s| s.to_string())
                        .collect();

                    if !error_messages.is_empty() {
                        return Some(AgentChunk {
                            content: format!("❌ Error: {}", error_messages.join("; ")),
                            is_done: true,
                        });
                    }
                }
            }

            // Normal completion
            return Some(AgentChunk {
                content: String::new(),
                is_done: true,
            });
        }

        // Ignore other JSON types (user messages, init, etc.)
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_command_name() {
        let backend = GeminiBackend::new();
        assert_eq!(backend.command_name(), "gemini");
    }

    #[test]
    fn test_supports_streaming() {
        let backend = GeminiBackend::new();
        assert!(backend.supports_streaming());
    }

    #[test]
    fn test_build_args_basic() {
        let backend = GeminiBackend::new();
        let args = backend.build_args("test prompt", false, None);

        assert_eq!(args[0], "--output-format");
        assert_eq!(args[1], "stream-json");
        assert_eq!(args[2], "--yolo");
        assert_eq!(args[3], "--prompt");
        assert_eq!(args[4], "test prompt");
        assert_eq!(args.len(), 5);
    }

    #[test]
    fn test_build_args_with_resume() {
        let backend = GeminiBackend::new();
        let args = backend.build_args("test prompt", true, None);

        assert!(args.contains(&"--resume".to_string()));
        assert!(args.contains(&"latest".to_string()));
    }

    #[test]
    fn test_parse_message() {
        let backend = GeminiBackend::new();
        let json = json!({
            "type": "message",
            "role": "assistant",
            "content": "Hello, world!"
        });

        let chunk = backend.parse_stdout_line(&json).unwrap();
        assert_eq!(chunk.content, "Hello, world!");
        assert!(!chunk.is_done);
    }

    #[test]
    fn test_parse_result() {
        let backend = GeminiBackend::new();
        let json = json!({
            "type": "result"
        });

        let chunk = backend.parse_stdout_line(&json).unwrap();
        assert_eq!(chunk.content, "");
        assert!(chunk.is_done);
    }

    #[test]
    fn test_parse_invalid() {
        let backend = GeminiBackend::new();
        let json = json!({
            "type": "other"
        });

        assert!(backend.parse_stdout_line(&json).is_none());
    }
}
