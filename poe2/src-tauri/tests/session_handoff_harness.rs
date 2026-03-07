//! Session handoff harness: run the stream-json test, then continue the session
//! interactively in an xterm.js browser window via PTY.
//!
//! Flow:
//!   1. Get session_id (from target/stdio-json-session-id.txt written by
//!      json_stream_basic, or run the stream-json flow inline if not present)
//!   2. Spawn `claude --resume <session-id>` in a PTY — no -p, full interactive mode
//!   3. Start a WebSocket bridge server (PTY <-> WS)
//!   4. Serve an HTML page with xterm.js connected to the WS server
//!   5. Open the browser — you can now talk to Claude with full session context
//!   6. Test completes when you close the browser tab (10-minute timeout)
//!
//! Run:  cargo test --test session_handoff_harness -- --nocapture
//! Log:  target/session-handoff.log

use futures_util::{SinkExt, StreamExt};
use poe2_lib::event_ingester;
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener as StdTcpListener;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

// ── HarnessLog ────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct HarnessLog {
    file: Arc<Mutex<std::fs::File>>,
    path: std::path::PathBuf,
}

impl HarnessLog {
    fn create(name: &str) -> Self {
        std::fs::create_dir_all("target").ok();
        let path = std::path::PathBuf::from(format!("target/{name}.log"));
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .unwrap_or_else(|e| panic!("failed to open {}: {}", path.display(), e));
        let log = Self { file: Arc::new(Mutex::new(file)), path };
        log.line(&format!(
            "=== {} started at {} ===",
            name,
            chrono::Utc::now().to_rfc3339()
        ));
        log
    }

    fn line(&self, msg: &str) {
        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(f, "{}", msg);
            let _ = f.flush();
        }
        eprintln!("{}", msg);
    }

    fn sep(&self) {
        self.line("");
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

// ── Helpers (duplicated from stdio_json_harness for test isolation) ───────────

const LONG_CONTENT: &str = concat!(
    "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA",
    "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA",
    "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA",
    "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA",
    "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA",
    "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA", "AAAAAAAAAA",
);

fn test_skill(long_content: &str) -> String {
    format!(
        r#"---
id: poe-test-harness
name: POE Protocol Test Harness
description: Deterministic test skill. Emits a fixed sequence of poe: events for protocol validation.
tags: [test]
protocol_version: v2
---

# POE Protocol Test Harness

You are running inside an automated integration test for the POE v2 protocol.
This is a deterministic test — not a real task.

## Strict Instructions

1. Output ONLY the JSON lines listed in "Required Output" below.
2. Do NOT add any explanation, commentary, or markdown formatting.
3. Do NOT wrap events in code fences or backticks.
4. Each JSON object must be on a SINGLE line — no newlines inside a JSON object.
5. Emit lines in EXACTLY the order shown.
6. Do NOT modify field values, including the long string of repeated A characters.

Any output that is not one of the listed JSON lines will be ignored by the test harness.

## Required Output

Output these lines in this exact order, each on its own line:

{{"poe": "brief", "content": "Protocol harness test: validating all poe event types."}}
{{"poe": "step", "name": "wire-format-coverage", "detail": "Emitting all poe event types in sequence."}}
{{"poe": "knowledge", "key": "test-key", "content": "test-value"}}
{{"poe": "artifact", "name": "test-short.txt", "artifact_type": "test", "content": "Short artifact content for protocol harness validation."}}
{{"poe": "task", "id": "test-task-001", "title": "Test subtask alpha", "description": "Harness subtask", "skill": "implementer", "type": "task"}}
{{"poe": "task:update", "id": "test-task-001", "title": "Updated test subtask alpha"}}
{{"poe": "task:cancel", "id": "test-task-001", "reason": "cancelled by harness"}}
{{"poe": "edge", "from": "test-task-001", "to": "test-task-002"}}
{{"poe": "edge:remove", "from": "test-task-001", "to": "test-task-002"}}
{{"poe": "decision", "question": "Harness test decision: which option?", "options": ["Option A", "Option B"]}}
{{"poe": "step", "name": "long-artifact", "detail": "Emitting artifact with content longer than 220 chars to test line fragmentation."}}
{{"poe": "artifact", "name": "test-long.txt", "artifact_type": "test", "content": "{long_content}"}}
{{"poe": "done", "summary": "Protocol harness test complete."}}
"#
    )
}

fn build_bundle(skill: &str) -> String {
    format!(
        "{skill}\n\n---\n\n# Task\n\n**Protocol Harness Validation**\n\n\
         Emit all required poe: events exactly as specified in your skill instructions. \
         Output nothing else.\n"
    )
}

fn claude_on_path() -> bool {
    Command::new("claude")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn extract_text(json: &serde_json::Value) -> Option<String> {
    match json.get("type")?.as_str()? {
        "assistant" => {
            let content = json.get("message")?.get("content")?.as_array()?;
            let text: String = content
                .iter()
                .filter_map(|block| {
                    if block.get("type")?.as_str() == Some("text") {
                        block.get("text")?.as_str().map(str::to_owned)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("");
            if text.is_empty() { None } else { Some(text) }
        }
        "content_block_delta" => {
            let delta = json.get("delta")?;
            if delta.get("type")?.as_str() == Some("text_delta") {
                let t = delta.get("text")?.as_str()?.to_owned();
                if t.is_empty() { None } else { Some(t) }
            } else {
                None
            }
        }
        _ => None,
    }
}

fn run_json_stream(
    bundle: &str,
    log: &HarnessLog,
) -> Result<(Option<String>, Vec<(String, serde_json::Value)>), String> {
    let mut cmd = Command::new("claude");
    cmd.arg("--output-format").arg("stream-json");
    cmd.arg("--verbose");
    cmd.arg("-p");
    cmd.arg("--dangerously-skip-permissions");
    cmd.env_remove("CLAUDECODE");
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("spawn failed: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(bundle.as_bytes()).map_err(|e| format!("stdin write: {e}"))?;
    }

    let stdout = child.stdout.take().ok_or("no stdout")?;
    let mut session_id: Option<String> = None;
    let mut poe_events = Vec::new();
    let mut text_buf = String::new();

    for (idx, line_result) in BufReader::new(stdout).lines().enumerate() {
        let line = line_result.map_err(|e| format!("read: {e}"))?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
            if session_id.is_none() {
                if let Some(sid) = json.get("session_id").and_then(|v| v.as_str()) {
                    session_id = Some(sid.to_owned());
                    log.line(&format!("[{idx:4}] [SESSION_ID] {sid}"));
                }
            }
            if let Some(text) = extract_text(&json) {
                text_buf.push_str(&text);
                while let Some(nl) = text_buf.find('\n') {
                    let candidate = text_buf[..nl].to_owned();
                    text_buf = text_buf[nl + 1..].to_owned();
                    if let Some((t, v)) = event_ingester::parse_poe_event(&candidate) {
                        log.line(&format!("[{idx:4}] [poe:{t}] {v}"));
                        poe_events.push((t, v));
                    }
                }
            }
            let event_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("?");
            log.line(&format!("[{idx:4}] [json:{event_type}] {}", &line[..line.len().min(200)]));
        }
    }
    let tail = text_buf.trim().to_owned();
    if !tail.is_empty() {
        if let Some((t, v)) = event_ingester::parse_poe_event(&tail) {
            poe_events.push((t, v));
        }
    }
    if let Some(stderr) = child.stderr.take() {
        for line in BufReader::new(stderr).lines().flatten() {
            log.line(&format!("[stderr] {line}"));
        }
    }
    child.wait().map_err(|e| format!("wait: {e}"))?;
    Ok((session_id, poe_events))
}

// ── HTML page (xterm.js via CDN) ──────────────────────────────────────────────

fn html_page(session_id: &str, ws_port: u16) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8">
  <title>POE — claude --resume {sid}</title>
  <link rel="stylesheet" href="https://unpkg.com/@xterm/xterm@5.5.0/css/xterm.css">
  <style>
    * {{ margin: 0; padding: 0; box-sizing: border-box; }}
    body {{ background: #1a1a1a; display: flex; flex-direction: column; height: 100vh; font-family: monospace; }}
    #header {{
      padding: 5px 12px; background: #111; color: #555; font-size: 11px;
      border-bottom: 1px solid #2a2a2a; display: flex; justify-content: space-between;
    }}
    #header span {{ color: #3a8; }}
    #terminal {{ flex: 1; overflow: hidden; padding: 4px; }}
  </style>
</head>
<body>
  <div id="header">
    <span>claude --resume {sid}</span>
    <span id="status">connecting…</span>
  </div>
  <div id="terminal"></div>
  <script src="https://unpkg.com/@xterm/xterm@5.5.0/lib/xterm.js"></script>
  <script src="https://unpkg.com/@xterm/addon-fit@0.10.0/lib/addon-fit.js"></script>
  <script>
    const term = new Terminal({{
      cursorBlink: true,
      fontFamily: '"Fira Code", "SF Mono", "Menlo", monospace',
      fontSize: 14,
      theme: {{
        background: '#1a1a1a', foreground: '#e0e0e0', cursor: '#e0e0e0',
        selectionBackground: '#444', black: '#1a1a1a', brightBlack: '#666'
      }}
    }});
    const fitAddon = new FitAddon.FitAddon();
    term.loadAddon(fitAddon);
    term.open(document.getElementById('terminal'));
    fitAddon.fit();

    const status = document.getElementById('status');
    const ws = new WebSocket('ws://127.0.0.1:{ws_port}');
    ws.binaryType = 'arraybuffer';

    ws.onopen = () => {{
      status.textContent = 'connected';
      status.style.color = '#3a8';
      ws.send(JSON.stringify({{type: 'resize', cols: term.cols, rows: term.rows}}));
    }};

    ws.onmessage = e => {{
      if (e.data instanceof ArrayBuffer) {{
        term.write(new Uint8Array(e.data));
      }} else {{
        term.write(e.data);
      }}
    }};

    term.onData(data => {{
      if (ws.readyState === WebSocket.OPEN) ws.send(data);
    }});

    term.onResize(({{cols, rows}}) => {{
      if (ws.readyState === WebSocket.OPEN) {{
        ws.send(JSON.stringify({{type: 'resize', cols, rows}}));
      }}
    }});

    window.addEventListener('resize', () => fitAddon.fit());

    ws.onclose = () => {{
      status.textContent = 'disconnected';
      status.style.color = '#a44';
      term.write('\r\n\x1b[2m[connection closed — you may close this tab]\x1b[0m\r\n');
    }};
    ws.onerror = () => {{
      status.textContent = 'error';
      status.style.color = '#a44';
    }};
  </script>
</body>
</html>"#,
        sid = session_id,
        ws_port = ws_port
    )
}

// ── Minimal HTTP server — serves the HTML page once then stops ─────────────────

fn serve_html_once(html: String, log: HarnessLog) -> u16 {
    let listener = StdTcpListener::bind("127.0.0.1:0").expect("http bind failed");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        // Serve until one successful response is sent.
        for stream in listener.incoming() {
            match stream {
                Ok(mut s) => {
                    let mut buf = [0u8; 4096];
                    s.read(&mut buf).ok();
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
                         Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                        html.len(),
                        html
                    );
                    if s.write_all(response.as_bytes()).is_ok() {
                        log.line(&format!("[http] served HTML (port {port})"));
                        break;
                    }
                }
                Err(e) => {
                    log.line(&format!("[http] accept error: {e}"));
                    break;
                }
            }
        }
    });
    port
}

// ── Browser launcher ──────────────────────────────────────────────────────────

fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn().ok();
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn().ok();
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", url])
            .spawn()
            .ok();
    }
}

// ── PTY + WebSocket bridge ────────────────────────────────────────────────────

async fn run_ws_pty_bridge(session_id: &str, log: HarnessLog) -> Result<(), String> {
    // ── Spawn PTY ──────────────────────────────────────────────────────────────
    let pty_system = NativePtySystem::default();
    let pair = pty_system
        .openpty(PtySize { rows: 50, cols: 220, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| format!("openpty: {e}"))?;

    // CWD must be set to a real project directory. We use the src-tauri directory
    // (the test's working directory when run via cargo test).
    let cwd = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."));

    let mut cmd = CommandBuilder::new("claude");
    cmd.arg("--resume");
    cmd.arg(session_id);
    cmd.arg("--dangerously-skip-permissions");
    cmd.env_remove("CLAUDECODE");
    cmd.cwd(&cwd);
    log.line(&format!("[pty] cwd: {}", cwd.display()));

    // Get writer and reader from master before taking ownership of the pair.
    let pty_writer: Arc<Mutex<Box<dyn Write + Send>>> =
        Arc::new(Mutex::new(pair.master.take_writer().map_err(|e| format!("take_writer: {e}"))?));
    let pty_reader = pair.master.try_clone_reader().map_err(|e| format!("clone_reader: {e}"))?;

    // Keep master alive for PTY resize.
    let master = Arc::new(Mutex::new(pair.master));

    let mut child = pair.slave.spawn_command(cmd).map_err(|e| format!("spawn: {e}"))?;
    let child_pid = child.process_id();
    log.line(&format!("[pty] spawned claude --resume {session_id} (pid={child_pid:?}"));

    // ── PTY reader → mpsc channel ──────────────────────────────────────────────
    // Buffers PTY output so it can be forwarded to the WebSocket once connected.
    let (pty_out_tx, pty_out_rx) = mpsc::channel::<Vec<u8>>(512);
    let log_reader = log.clone();
    tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 4096];
        let mut reader = pty_reader;
        let mut total = 0usize;
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    total += n;
                    if pty_out_tx.blocking_send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
        log_reader.line(&format!("[pty] reader EOF ({total} bytes total)"));
    });

    // ── WebSocket listener ─────────────────────────────────────────────────────
    let ws_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| format!("ws bind: {e}"))?;
    let ws_port = ws_listener.local_addr().unwrap().port();
    log.line(&format!("[ws] listening on port {ws_port}"));

    // ── HTTP server ────────────────────────────────────────────────────────────
    let html = html_page(session_id, ws_port);
    let http_port = serve_html_once(html, log.clone());
    let url = format!("http://127.0.0.1:{http_port}");

    log.sep();
    log.line(&format!(">>> OPEN BROWSER: {url}"));
    log.sep();
    open_browser(&url);

    // ── Wait for the browser to connect via WebSocket ──────────────────────────
    log.line("[ws] waiting for browser connection…");
    let (tcp_stream, peer) = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        ws_listener.accept(),
    )
    .await
    .map_err(|_| "timed out waiting for browser WebSocket connection (30s)".to_string())?
    .map_err(|e| format!("ws accept: {e}"))?;
    log.line(&format!("[ws] TCP connection from {peer}"));

    let ws_stream = tokio_tungstenite::accept_async(tcp_stream)
        .await
        .map_err(|e| format!("ws handshake: {e}"))?;
    log.line("[ws] handshake complete — bridging PTY <-> xterm.js");

    let (mut ws_tx, mut ws_rx) = ws_stream.split();
    let pty_writer_for_ws = pty_writer.clone();
    let master_for_resize = master.clone();

    // ── WS → PTY (keyboard input + resize) ────────────────────────────────────
    let log_in = log.clone();
    let ws_in_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Text(s) => {
                    // Distinguish resize JSON from raw keyboard input.
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&s) {
                        if json.get("type").and_then(|v| v.as_str()) == Some("resize") {
                            let cols = json.get("cols").and_then(|v| v.as_u64()).unwrap_or(220) as u16;
                            let rows = json.get("rows").and_then(|v| v.as_u64()).unwrap_or(50) as u16;
                            log_in.line(&format!("[ws] resize → {cols}×{rows}"));
                            master_for_resize
                                .lock()
                                .unwrap()
                                .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
                                .ok();
                            continue;
                        }
                    }
                    pty_writer_for_ws.lock().unwrap().write_all(s.as_bytes()).ok();
                }
                Message::Binary(b) => {
                    pty_writer_for_ws.lock().unwrap().write_all(&b).ok();
                }
                Message::Close(_) => {
                    log_in.line("[ws] client closed connection");
                    break;
                }
                _ => {}
            }
        }
        log_in.line("[ws] input task ended");
    });

    // ── PTY → WS (terminal output) ─────────────────────────────────────────────
    let log_out = log.clone();
    let mut pty_out_rx = pty_out_rx;
    let ws_out_task = tokio::spawn(async move {
        while let Some(data) = pty_out_rx.recv().await {
            if ws_tx.send(Message::Binary(data.into())).await.is_err() {
                log_out.line("[ws] send failed — client disconnected");
                break;
            }
        }
        log_out.line("[ws] output task ended");
    });

    // ── Wait for bridge to end ─────────────────────────────────────────────────
    log.line("[bridge] active — close the browser tab to end the session (10 min timeout)");

    tokio::select! {
        _ = ws_in_task => { log.line("[bridge] input closed"); }
        _ = ws_out_task => { log.line("[bridge] output closed"); }
        _ = tokio::time::sleep(std::time::Duration::from_secs(600)) => {
            log.line("[bridge] 10-minute timeout reached");
        }
    }

    // ── Shutdown ───────────────────────────────────────────────────────────────
    log.line("[pty] shutting down");

    // 1. Ctrl-C — interrupt whatever Claude is doing.
    pty_writer.lock().unwrap().write_all(&[3u8]).ok();
    drop(pty_writer);

    // 2. Drop PTY master — closes the controlling terminal fd.
    //    This sends SIGHUP to the process and is necessary for the kernel
    //    to fully release the process so waitpid() can return.
    drop(master);

    // 3. SIGTERM — polite exit request.
    if let Some(pid) = child_pid {
        log.line(&format!("[pty] SIGTERM pid={pid}"));
        std::process::Command::new("kill").arg(pid.to_string()).status().ok();
    }

    // 4. Poll up to 5 s for the process to exit, then SIGKILL.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if child.try_wait().ok().flatten().is_some() {
            log.line("[pty] process exited (SIGTERM)");
            break;
        }
        if std::time::Instant::now() >= deadline {
            if let Some(pid) = child_pid {
                log.line(&format!("[pty] SIGKILL pid={pid}"));
                std::process::Command::new("kill")
                    .args(["-9", &pid.to_string()])
                    .status()
                    .ok();
            }
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // 5. Final reap — run in spawn_blocking with a hard 3 s timeout so a
    //    stuck waitpid() can never hang the test indefinitely.
    let reap = tokio::task::spawn_blocking(move || { let _ = child.wait(); });
    match tokio::time::timeout(std::time::Duration::from_secs(3), reap).await {
        Ok(_) => log.line("[pty] process reaped"),
        Err(_) => log.line("[pty] reap timed out — process may be a zombie (harmless)"),
    }

    Ok(())
}

// ── Test ──────────────────────────────────────────────────────────────────────

/// Run the stream-json protocol test, then hand the session off to an
/// interactive xterm.js window so you can continue talking to Claude.
///
/// Prereq: `cargo test --test stdio_json_harness json_stream_basic` should have
/// been run first (writes target/stdio-json-session-id.txt). If the file is
/// absent this test will run the stream-json flow inline.
///
/// Run: cargo test --test session_handoff_harness -- --nocapture
/// Log: target/session-handoff.log
#[test]
fn session_handoff_to_xterm() {
    let log = HarnessLog::create("session-handoff");
    log.line(&format!("log: {}", log.path().display()));

    if !claude_on_path() {
        log.line("SKIP: claude not found on PATH");
        return;
    }

    // ── Step 1: Get or generate session_id ────────────────────────────────────
    let session_id = match std::fs::read_to_string("target/stdio-json-session-id.txt") {
        Ok(s) if !s.trim().is_empty() => {
            let sid = s.trim().to_owned();
            log.line(&format!("[step1] using saved session_id: {sid}"));
            sid
        }
        _ => {
            log.line("[step1] no saved session_id — running stream-json test inline");
            let skill = test_skill(LONG_CONTENT);
            let bundle = build_bundle(&skill);

            log.sep();
            log.line(&format!("=== BUNDLE ({} bytes) ===", bundle.len()));
            log.sep();
            log.line("=== RUNNING STREAM-JSON TEST ===");

            let (session_id, events) =
                run_json_stream(&bundle, &log).expect("run_json_stream failed");

            let sid = session_id.expect("no session_id from stream-json run");

            // Verify poe:done was emitted so we know the test completed.
            let got_done = events.iter().any(|(t, _)| t == "done");
            assert!(got_done, "stream-json run did not emit poe:done — session may be incomplete");

            log.line(&format!("[step1] stream-json complete. session_id: {sid}"));
            std::fs::write("target/stdio-json-session-id.txt", &sid).ok();
            sid
        }
    };

    // ── Step 2-6: Resume in xterm.js ──────────────────────────────────────────
    log.sep();
    log.line(&format!("[step2] resuming session {session_id} via PTY + xterm.js"));

    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async {
        if let Err(e) = run_ws_pty_bridge(&session_id, log.clone()).await {
            log.line(&format!("[error] {e}"));
            panic!("session_handoff_to_xterm failed: {e}");
        }
    });

    log.sep();
    log.line("=== SESSION HANDOFF COMPLETE ===");
}
