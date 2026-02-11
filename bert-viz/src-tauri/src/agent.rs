use std::process::{Command, Stdio, Child, ChildStdin};
use std::io::{BufRead, BufReader, Write};
use tauri::{AppHandle, Emitter, State};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

const SYSTEM_PROMPT_PM: &str = "You are a Product Manager for BERT (Bead-based Epic and Requirement Tracker). \
Your goal is to help users elaborate epics and decompose them into features and tasks. \
When a user provides an epic, you should: \
1. Ask clarifying questions if the epic is vague. \
2. Suggest a detailed description and acceptance criteria. \
3. Propose a breakdown into child features and tasks using 'bd create' commands. \
Always output 'bd' commands in a code block so the user can easily approve them. \
Emphasize collaboration and ensure the user agrees with the breakdown before finalizing.";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentChunk {
    pub content: String,
    pub is_done: bool,
}

pub struct AgentProcess {
    pub child: Arc<Mutex<Child>>,
    pub stdin: Arc<Mutex<ChildStdin>>,
}

#[derive(Default)]
pub struct AgentState {
    pub process: Mutex<Option<AgentProcess>>,
}

impl AgentState {
    pub fn new() -> Self {
        AgentState {
            process: Mutex::new(None),
        }
    }
}

#[tauri::command]
pub fn start_agent_session(app_handle: AppHandle, state: State<'_, AgentState>, persona: String) -> Result<(), String> {
    let mut process_guard = state.process.lock().unwrap();
    
    if process_guard.is_some() {
        if let Some(old_process) = process_guard.take() {
            let mut child = old_process.child.lock().unwrap();
            let _ = child.kill();
        }
    }

    let mut cmd = Command::new("gemini");
    cmd.arg("--output-format").arg("stream-json");

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn gemini: {}", e))?;

    let mut stdin = child.stdin.take().ok_or("Failed to open stdin")?;
    let stdout = child.stdout.take().ok_or("Failed to open stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to open stderr")?;

    // Send initialization commands via stdin
    if persona == "product-manager" {
        writeln!(stdin, "activate_skill spec").map_err(|e| e.to_string())?;
        writeln!(stdin, "{}", SYSTEM_PROMPT_PM).map_err(|e| e.to_string())?;
        stdin.flush().map_err(|e| e.to_string())?;
    }
    
    let handle_clone = app_handle.clone();
    
    // Thread to read stdout (Agent response chunks)
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line_str) = line {
                if line_str.trim().starts_with('{') {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line_str) {
                        if json["type"] == "message" && json["role"] == "assistant" {
                            if let Some(content) = json["content"].as_str() {
                                let _ = handle_clone.emit("agent-chunk", AgentChunk {
                                    content: content.to_string(),
                                    is_done: false,
                                });
                            }
                        } else if json["type"] == "result" {
                             let _ = handle_clone.emit("agent-chunk", AgentChunk {
                                    content: "".to_string(),
                                    is_done: true,
                             });
                        }
                    }
                }
            }
        }
        let _ = handle_clone.emit("agent-chunk", AgentChunk {
            content: "".to_string(),
            is_done: true,
        });
    });

    std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line_str) = line {
                eprintln!("🤖 Agent Stderr: {}", line_str);
            }
        }
    });

    *process_guard = Some(AgentProcess { 
        child: Arc::new(Mutex::new(child)), 
        stdin: Arc::new(Mutex::new(stdin)) 
    });
    
    Ok(())
}

#[tauri::command]
pub fn send_agent_message(message: String, state: State<'_, AgentState>) -> Result<(), String> {
    let process_guard = state.process.lock().unwrap();
    
    if let Some(process) = process_guard.as_ref() {
        let mut stdin = process.stdin.lock().unwrap();
        writeln!(stdin, "{}", message).map_err(|e| format!("Failed to write to stdin: {}", e))?;
        stdin.flush().map_err(|e| format!("Failed to flush stdin: {}", e))?;
        Ok(())
    } else {
        Err("No active agent session".to_string())
    }
}

#[tauri::command]
pub fn stop_agent_session(state: State<'_, AgentState>) -> Result<(), String> {
    let mut process_guard = state.process.lock().unwrap();
    
    if let Some(process) = process_guard.take() {
        let mut child = process.child.lock().unwrap();
        let _ = child.kill();
        Ok(())
    } else {
        Ok(())
    }
}

#[tauri::command]
pub fn approve_suggestion(command: String) -> Result<String, String> {
    if !command.starts_with("bd ") {
        return Err("Only 'bd' commands are supported for approval".to_string());
    }

    let args: Vec<String> = command
        .split_whitespace()
        .skip(1)
        .map(|s| s.to_string())
        .collect();

    crate::bd::execute_bd(args)
}
