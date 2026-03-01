/// Anthropic Claude Code CLI backend implementation
use crate::agent::plugin::{AgentChunk, CliBackendPlugin, ToolUseData};
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

    fn build_pty_args(&self, session_id: &str) -> Vec<String> {
        vec![
            "--resume".to_string(),
            session_id.to_string(),
            "--dangerously-skip-permissions".to_string(),
        ]
    }

    fn parse_stdout_line(&self, json: &Value) -> Option<AgentChunk> {
        // Handle Claude Code message format:
        // {"type": "assistant", "message": {"content": [...]}}
        // Claude sends incremental deltas — is_replacement is always false.
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
                                    is_replacement: false,
                                    session_id: None,
                                    tool_use: None,
                                });
                            }
                        }

                        // Handle tool use
                        if content_block["type"] == "tool_use" {
                            if let Some(tool_name) = content_block["name"].as_str() {
                                let input = &content_block["input"];

                                // For Edit/Write emit structured diff data; UI renders a DiffView
                                if tool_name == "Edit" {
                                    return Some(AgentChunk {
                                        content: String::new(),
                                        is_done: false,
                                        is_replacement: false,
                                        session_id: None,
                                        tool_use: Some(ToolUseData {
                                            name: tool_name.to_string(),
                                            file_path: input["file_path"].as_str().unwrap_or("").to_string(),
                                            old_string: input["old_string"].as_str().unwrap_or("").to_string(),
                                            new_string: input["new_string"].as_str().unwrap_or("").to_string(),
                                        }),
                                    });
                                }

                                if tool_name == "Write" {
                                    return Some(AgentChunk {
                                        content: String::new(),
                                        is_done: false,
                                        is_replacement: false,
                                        session_id: None,
                                        tool_use: Some(ToolUseData {
                                            name: tool_name.to_string(),
                                            file_path: input["file_path"].as_str().unwrap_or("").to_string(),
                                            old_string: String::new(),
                                            new_string: input["content"].as_str().unwrap_or("").to_string(),
                                        }),
                                    });
                                }

                                // All other tools: plain text notification with relevant input detail
                                let detail = match tool_name {
                                    "Read" => input["file_path"].as_str()
                                        .map(|s| s.to_string()),
                                    "Glob" => {
                                        let pattern = input["pattern"].as_str().unwrap_or("");
                                        let path = input["path"].as_str().unwrap_or("");
                                        if !path.is_empty() {
                                            Some(format!("{} in {}", pattern, path))
                                        } else {
                                            Some(pattern.to_string())
                                        }
                                    }
                                    "Grep" => {
                                        let pattern = input["pattern"].as_str().unwrap_or("");
                                        let path = input["path"].as_str()
                                            .or_else(|| input["glob"].as_str())
                                            .unwrap_or("");
                                        if !path.is_empty() {
                                            Some(format!("{} in {}", pattern, path))
                                        } else {
                                            Some(pattern.to_string())
                                        }
                                    }
                                    "Bash" => input["description"].as_str()
                                        .filter(|s| !s.is_empty())
                                        .or_else(|| input["command"].as_str())
                                        .map(|s| s.to_string()),
                                    "WebFetch" | "WebSearch" => input["url"].as_str()
                                        .or_else(|| input["query"].as_str())
                                        .map(|s| s.to_string()),
                                    _ => input["description"].as_str()
                                        .filter(|s| !s.is_empty())
                                        .map(|s| s.to_string()),
                                };
                                let message = if let Some(d) = detail.filter(|s| !s.is_empty()) {
                                    format!("🔧 {}: {}\n", tool_name, d)
                                } else {
                                    format!("🔧 {}\n", tool_name)
                                };
                                return Some(AgentChunk {
                                    content: message,
                                    is_done: false,
                                    is_replacement: false,
                                    session_id: None,
                                    tool_use: None,
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
                            is_replacement: false,
                            session_id: None,
                            tool_use: None,
                        });
                    }
                }
            }

            // Normal completion
            return Some(AgentChunk {
                content: String::new(),
                is_done: true,
                is_replacement: false,
                session_id: None,
                tool_use: None,
            });
        }

        // Ignore other JSON types (user messages, etc.)
        None
    }
}
