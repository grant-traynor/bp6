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
    ///           --dangerously-skip-permissions [--model <id>] [--resume <id>]`
    ///
    /// Blocking — intended to be called from `std::thread::spawn`.
    pub fn run(
        bundle: &str,
        resume_session_id: Option<&str>,
        model: Option<&str>,
        cwd: &Path,
        callbacks: StreamCallbacks,
    ) -> Result<()> {
        let mut cmd = std::process::Command::new("claude");
        cmd.arg("--output-format").arg("stream-json");
        cmd.arg("--verbose");
        cmd.arg("-p");
        cmd.arg("--dangerously-skip-permissions");
        if let Some(m) = model {
            cmd.arg("--model").arg(m);
        }
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
        Self::process_ndjson_reader(reader, &callbacks)?;

        child.wait().context("Failed to wait for claude process")?;
        Ok(())
    }

    /// Process a newline-delimited JSON stream, firing callbacks for each event.
    ///
    /// Extracted from `run()` for testability — pass a `Cursor<Vec<u8>>` or any
    /// other `BufRead` to exercise the full parsing logic without spawning claude.
    pub(crate) fn process_ndjson_reader<R: BufRead>(
        reader: R,
        callbacks: &StreamCallbacks,
    ) -> Result<()> {
        for line in reader.lines() {
            let line = line.context("Failed to read NDJSON line")?;
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
                    if json.get("subtype").and_then(|v| v.as_str()) == Some("init") {
                        if let Some(sid) = json.get("session_id").and_then(|v| v.as_str()) {
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
                            if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                    (callbacks.on_text_chunk)(text.to_owned());
                                }
                            }
                        }
                    }
                }
                "content_block_delta" => {
                    if let Some(delta) = json.get("delta") {
                        if delta.get("type").and_then(|t| t.as_str()) == Some("text_delta") {
                            if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
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
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::{Arc, Mutex};

    /// Build a `StreamCallbacks` that records every firing into shared Vecs.
    fn recording_callbacks() -> (
        StreamCallbacks,
        Arc<Mutex<Vec<String>>>,            // session_ids
        Arc<Mutex<Vec<String>>>,            // text_chunks
        Arc<Mutex<Vec<serde_json::Value>>>, // raw_jsons
        Arc<Mutex<bool>>,                   // completed
    ) {
        let session_ids = Arc::new(Mutex::new(Vec::<String>::new()));
        let text_chunks = Arc::new(Mutex::new(Vec::<String>::new()));
        let raw_jsons = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let completed = Arc::new(Mutex::new(false));

        let s = session_ids.clone();
        let t = text_chunks.clone();
        let r = raw_jsons.clone();
        let c = completed.clone();

        let callbacks = StreamCallbacks {
            on_session_id: Box::new(move |sid| s.lock().unwrap().push(sid)),
            on_text_chunk: Box::new(move |chunk| t.lock().unwrap().push(chunk)),
            on_raw_json: Box::new(move |json| r.lock().unwrap().push(json)),
            on_complete: Box::new(move || *c.lock().unwrap() = true),
            on_pid: None,
        };
        (callbacks, session_ids, text_chunks, raw_jsons, completed)
    }

    fn run_reader(input: &str, callbacks: StreamCallbacks) {
        let reader = Cursor::new(input.as_bytes().to_vec());
        JsonStreamTransport::process_ndjson_reader(reader, &callbacks).unwrap();
    }

    // ── session_id ────────────────────────────────────────────────────────────

    #[test]
    fn session_id_extracted_from_init_event() {
        let (cb, session_ids, _, _, _) = recording_callbacks();
        run_reader(
            r#"{"type":"system","subtype":"init","session_id":"abc-123"}"#,
            cb,
        );
        assert_eq!(*session_ids.lock().unwrap(), vec!["abc-123"]);
    }

    #[test]
    fn system_non_init_subtype_does_not_fire_session_id() {
        let (cb, session_ids, _, _, _) = recording_callbacks();
        run_reader(
            r#"{"type":"system","subtype":"other","session_id":"should-not-appear"}"#,
            cb,
        );
        assert!(session_ids.lock().unwrap().is_empty());
    }

    // ── text extraction ───────────────────────────────────────────────────────

    #[test]
    fn text_extracted_from_assistant_message_content() {
        let (cb, _, text_chunks, _, _) = recording_callbacks();
        run_reader(
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hello"},{"type":"text","text":" world"}]}}"#,
            cb,
        );
        assert_eq!(*text_chunks.lock().unwrap(), vec!["hello", " world"]);
    }

    #[test]
    fn non_text_content_blocks_are_skipped() {
        let (cb, _, text_chunks, _, _) = recording_callbacks();
        run_reader(
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1"},{"type":"text","text":"only this"}]}}"#,
            cb,
        );
        assert_eq!(*text_chunks.lock().unwrap(), vec!["only this"]);
    }

    #[test]
    fn text_extracted_from_content_block_delta() {
        let (cb, _, text_chunks, _, _) = recording_callbacks();
        run_reader(
            r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"streaming chunk"}}"#,
            cb,
        );
        assert_eq!(*text_chunks.lock().unwrap(), vec!["streaming chunk"]);
    }

    #[test]
    fn non_text_delta_type_does_not_fire_on_text_chunk() {
        let (cb, _, text_chunks, _, _) = recording_callbacks();
        run_reader(
            r#"{"type":"content_block_delta","delta":{"type":"input_json_delta","partial_json":"{}"}}"#,
            cb,
        );
        assert!(text_chunks.lock().unwrap().is_empty());
    }

    // ── result / completion ───────────────────────────────────────────────────

    #[test]
    fn result_event_fires_on_complete() {
        let (cb, _, _, _, completed) = recording_callbacks();
        run_reader(
            r#"{"type":"result","subtype":"success","cost_usd":0.01}"#,
            cb,
        );
        assert!(*completed.lock().unwrap());
    }

    #[test]
    fn on_complete_not_fired_without_result_event() {
        let (cb, _, _, _, completed) = recording_callbacks();
        run_reader(
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"still working"}]}}"#,
            cb,
        );
        assert!(!*completed.lock().unwrap());
    }

    // ── raw_json ──────────────────────────────────────────────────────────────

    #[test]
    fn raw_json_fires_for_every_valid_json_line() {
        let (cb, _, _, raw_jsons, _) = recording_callbacks();
        run_reader(
            "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"s1\"}\n\
             {\"type\":\"result\"}\n",
            cb,
        );
        assert_eq!(raw_jsons.lock().unwrap().len(), 2);
    }

    // ── error tolerance ───────────────────────────────────────────────────────

    #[test]
    fn non_json_lines_are_silently_skipped() {
        let (cb, session_ids, _, raw_jsons, _) = recording_callbacks();
        run_reader(
            "not json at all\n\
             {\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"ok\"}\n\
             also not json\n",
            cb,
        );
        assert_eq!(*session_ids.lock().unwrap(), vec!["ok"]);
        assert_eq!(raw_jsons.lock().unwrap().len(), 1);
    }

    #[test]
    fn empty_and_whitespace_only_lines_are_skipped() {
        let (cb, _, _, raw_jsons, _) = recording_callbacks();
        run_reader("\n   \n\t\n", cb);
        assert!(raw_jsons.lock().unwrap().is_empty());
    }

    #[test]
    fn unknown_event_types_fire_raw_json_only() {
        let (cb, session_ids, text_chunks, raw_jsons, completed) = recording_callbacks();
        run_reader(
            r#"{"type":"message_start","message":{"id":"m1","type":"message"}}"#,
            cb,
        );
        assert!(session_ids.lock().unwrap().is_empty());
        assert!(text_chunks.lock().unwrap().is_empty());
        assert!(!*completed.lock().unwrap());
        assert_eq!(raw_jsons.lock().unwrap().len(), 1);
    }

    // ── full realistic stream ─────────────────────────────────────────────────

    #[test]
    fn realistic_stream_fires_all_callbacks_correctly() {
        let (cb, session_ids, text_chunks, raw_jsons, completed) = recording_callbacks();
        // Simulate a minimal complete claude stream-json session:
        // init → two text deltas → result
        let stream = concat!(
            "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"sess-xyz\"}\n",
            "{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"chunk-one\"}}\n",
            "{\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"chunk-two\"}}\n",
            "{\"type\":\"result\",\"subtype\":\"success\"}\n",
        );
        run_reader(stream, cb);
        assert_eq!(*session_ids.lock().unwrap(), vec!["sess-xyz"]);
        assert_eq!(*text_chunks.lock().unwrap(), vec!["chunk-one", "chunk-two"]);
        assert!(*completed.lock().unwrap());
        assert_eq!(raw_jsons.lock().unwrap().len(), 4);
    }
}
