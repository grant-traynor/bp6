use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// Callbacks fired by JsonStreamTransport::run() as events arrive.
pub struct StreamCallbacks {
    /// Fired once with the session_id from the init event.
    pub on_session_id: Box<dyn Fn(String) + Send>,
    /// Fired for each text chunk from assistant or content_block_delta events.
    pub on_text_chunk: Box<dyn Fn(String) + Send>,
    /// Fired for every JSON object received (for logging/debugging).
    pub on_raw_json: Box<dyn Fn(serde_json::Value) + Send>,
    /// Fired when the result event is received (process completing).
    pub on_complete: Box<dyn Fn() + Send>,
    /// Fired once after the child process spawns with the OS pid. Optional.
    pub on_pid: Option<Box<dyn Fn(u32) + Send>>,
}

pub struct JsonStreamTransport;

impl JsonStreamTransport {
    /// Spawn claude with stream-json transport, write the bundle to stdin (then
    /// close stdin for EOF), read stdout as newline-delimited JSON, and fire
    /// callbacks for key events.
    ///
    /// Command: `claude --output-format stream-json --verbose -p
    ///           --dangerously-skip-permissions [--resume <id>]`
    ///
    /// Blocking — intended to be called from `std::thread::spawn`.
    pub fn run(
        bundle: &str,
        resume_session_id: Option<&str>,
        cwd: &Path,
        callbacks: StreamCallbacks,
    ) -> Result<()> {
        let mut cmd = std::process::Command::new("claude");
        cmd.arg("--output-format").arg("stream-json");
        cmd.arg("--verbose");
        cmd.arg("-p");
        cmd.arg("--dangerously-skip-permissions");
        if let Some(sid) = resume_session_id {
            cmd.arg("--resume").arg(sid);
        }
        cmd.current_dir(cwd);
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::null());
        // Remove CLAUDECODE env var to prevent re-entrancy issues.
        cmd.env_remove("CLAUDECODE");

        let mut child = cmd.spawn().context("Failed to spawn claude process")?;

        // Fire on_pid immediately after spawn.
        if let Some(ref on_pid) = callbacks.on_pid {
            on_pid(child.id());
        }

        // Write bundle to stdin, then drop to signal EOF.
        {
            let mut stdin = child.stdin.take().context("Failed to acquire claude stdin")?;
            stdin
                .write_all(bundle.as_bytes())
                .context("Failed to write bundle to claude stdin")?;
            // stdin dropped here — EOF sent to claude.
        }

        // Read stdout as newline-delimited JSON.
        let stdout = child.stdout.take().context("Failed to acquire claude stdout")?;
        let reader = BufReader::new(stdout);

        for line in reader.lines() {
            let line = line.context("Failed to read claude stdout line")?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let json: serde_json::Value = match serde_json::from_str(trimmed) {
                Ok(v) => v,
                Err(_) => {
                    eprintln!(
                        "[transport] non-JSON stdout: {}",
                        &trimmed[..trimmed.len().min(120)]
                    );
                    continue;
                }
            };

            (callbacks.on_raw_json)(json.clone());

            match json.get("type").and_then(|v| v.as_str()).unwrap_or("") {
                "system" => {
                    if json
                        .get("subtype")
                        .and_then(|v| v.as_str())
                        == Some("init")
                    {
                        if let Some(sid) =
                            json.get("session_id").and_then(|v| v.as_str())
                        {
                            (callbacks.on_session_id)(sid.to_owned());
                        }
                    }
                }
                "assistant" => {
                    // Extract text from message.content[].{type:text}.text
                    if let Some(blocks) = json
                        .get("message")
                        .and_then(|m| m.get("content"))
                        .and_then(|c| c.as_array())
                    {
                        for block in blocks {
                            if block
                                .get("type")
                                .and_then(|t| t.as_str())
                                == Some("text")
                            {
                                if let Some(text) =
                                    block.get("text").and_then(|t| t.as_str())
                                {
                                    (callbacks.on_text_chunk)(text.to_owned());
                                }
                            }
                        }
                    }
                }
                "content_block_delta" => {
                    if let Some(delta) = json.get("delta") {
                        if delta
                            .get("type")
                            .and_then(|t| t.as_str())
                            == Some("text_delta")
                        {
                            if let Some(text) =
                                delta.get("text").and_then(|t| t.as_str())
                            {
                                (callbacks.on_text_chunk)(text.to_owned());
                            }
                        }
                    }
                }
                "result" => {
                    (callbacks.on_complete)();
                }
                _ => {}
            }
        }

        child.wait().context("Failed to wait for claude process")?;
        Ok(())
    }
}
