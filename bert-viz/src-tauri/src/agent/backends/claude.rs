/// Anthropic Claude Code CLI backend implementation
use crate::agent::plugin::{AgentChunk, CliBackendPlugin};
use serde_json::Value;

/// Claude Code CLI backend plugin
///
/// Implements the CliBackendPlugin trait for Anthropic's Claude Code CLI.
/// Handles command execution and JSON output parsing specific to Claude's format.
pub struct ClaudeCodeBackend;

impl ClaudeCodeBackend {
    /// Create a new Claude Code backend instance
    pub fn new() -> Self {
        ClaudeCodeBackend
    }
}

impl CliBackendPlugin for ClaudeCodeBackend {
    fn command_name(&self) -> &str {
        "claude"
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn build_args(&self, prompt: &str, resume: bool, session_id: Option<&str>) -> Vec<String> {
        let mut args = vec![
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
            "--dangerously-skip-permissions".to_string(),
        ];

        if resume {
            args.push("--resume".to_string());
            // Claude Code requires a valid UUID session ID, not "latest"
            if let Some(sid) = session_id {
                args.push(sid.to_string());
            } else {
                eprintln!("⚠️  Warning: Claude Code backend requires session ID for resume, but none provided");
            }
        } else if let Some(sid) = session_id {
            // For new sessions, use --session-id to specify the UUID
            args.push("--session-id".to_string());
            args.push(sid.to_string());
        }

        // Claude Code takes the prompt as a positional argument, not --prompt
        args.push(prompt.to_string());

        args
    }

    fn parse_stdout_line(&self, json: &Value) -> Option<AgentChunk> {
        // Handle Claude Code message format:
        // {"type": "assistant", "message": {"content": [...]}}
        if json["type"] == "assistant" {
            if let Some(message) = json["message"].as_object() {
                if let Some(content_array) = message["content"].as_array() {
                    for content_block in content_array {
                        // Handle text content
                        if content_block["type"] == "text" {
                            if let Some(text) = content_block["text"].as_str() {
                                return Some(AgentChunk {
                                    content: text.to_string(),
                                    is_done: false,
                                    session_id: None,
                                });
                            }
                        }

                        // Handle tool use - emit notification so UI shows activity
                        if content_block["type"] == "tool_use" {
                            if let Some(tool_name) = content_block["name"].as_str() {
                                let description =
                                    content_block["input"]["description"].as_str().unwrap_or("");

                                let message = if !description.is_empty() {
                                    format!("🔧 {}: {}", tool_name, description)
                                } else {
                                    format!("🔧 Using tool: {}", tool_name)
                                };

                                return Some(AgentChunk {
                                    content: message,
                                    is_done: false,
                                    session_id: None,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Handle completion: {"type": "result"}
        if json["type"] == "result" {
            // Check for errors in result
            if json["is_error"].as_bool().unwrap_or(false) {
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
                            session_id: None,
                        });
                    }
                }
            }

            // Normal completion
            return Some(AgentChunk {
                content: String::new(),
                is_done: true,
                session_id: None,
            });
        }

        // Ignore other JSON types (user messages, etc.)
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_command_name() {
        let backend = ClaudeCodeBackend::new();
        assert_eq!(backend.command_name(), "claude");
    }

    #[test]
    fn test_supports_streaming() {
        let backend = ClaudeCodeBackend::new();
        assert!(backend.supports_streaming());
    }

    #[test]
    fn test_build_args_basic() {
        let backend = ClaudeCodeBackend::new();
        let args = backend.build_args("test prompt", false, None);

        assert_eq!(args[0], "--output-format");
        assert_eq!(args[1], "stream-json");
        assert_eq!(args[2], "--verbose");
        assert_eq!(args[3], "--dangerously-skip-permissions");
        assert_eq!(args[4], "test prompt"); // Positional, not --prompt
        assert_eq!(args.len(), 5);
    }

    #[test]
    fn test_build_args_with_session_id() {
        let backend = ClaudeCodeBackend::new();
        let session_id = "550e8400-e29b-41d4-a716-446655440000";
        let args = backend.build_args("test prompt", false, Some(session_id));

        assert!(args.contains(&"--session-id".to_string()));
        assert!(args.contains(&session_id.to_string()));
        assert_eq!(args.last().unwrap(), "test prompt"); // Prompt still last
    }

    #[test]
    fn test_build_args_with_resume() {
        let backend = ClaudeCodeBackend::new();
        let session_id = "550e8400-e29b-41d4-a716-446655440000";
        let args = backend.build_args("test prompt", true, Some(session_id));

        assert!(args.contains(&"--resume".to_string()));
        assert!(args.contains(&session_id.to_string()));
        // Should NOT have --session-id when resuming
        assert!(!args.contains(&"--session-id".to_string()));
        assert_eq!(args.last().unwrap(), "test prompt"); // Prompt still last
    }

    #[test]
    fn test_parse_message() {
        let backend = ClaudeCodeBackend::new();
        let json = json!({
            "type": "assistant",
            "message": {
                "content": [
                    {
                        "type": "text",
                        "text": "Hello from Claude!"
                    }
                ]
            }
        });

        let chunk = backend.parse_stdout_line(&json).unwrap();
        assert_eq!(chunk.content, "Hello from Claude!");
        assert!(!chunk.is_done);
    }

    #[test]
    fn test_parse_message_multiple_blocks() {
        let backend = ClaudeCodeBackend::new();
        let json = json!({
            "type": "assistant",
            "message": {
                "content": [
                    {
                        "type": "text",
                        "text": "First block"
                    },
                    {
                        "type": "text",
                        "text": "Second block"
                    }
                ]
            }
        });

        // Should return first text block found
        let chunk = backend.parse_stdout_line(&json).unwrap();
        assert_eq!(chunk.content, "First block");
        assert!(!chunk.is_done);
    }

    #[test]
    fn test_parse_result() {
        let backend = ClaudeCodeBackend::new();
        let json = json!({
            "type": "result"
        });

        let chunk = backend.parse_stdout_line(&json).unwrap();
        assert_eq!(chunk.content, "");
        assert!(chunk.is_done);
    }

    #[test]
    fn test_parse_invalid() {
        let backend = ClaudeCodeBackend::new();
        let json = json!({
            "type": "other"
        });

        assert!(backend.parse_stdout_line(&json).is_none());
    }
}
