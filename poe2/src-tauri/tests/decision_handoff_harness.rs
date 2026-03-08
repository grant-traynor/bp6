//! Decision → PTY handoff → stream-json resume integration test.
//!
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │  HUMAN-RUNNABLE TEST  — opens a browser window                         │
//! │                                                                         │
//! │  Run:  cargo test --test decision_handoff_harness -- --nocapture        │
//! │  Log:  target/decision-handoff.log                                      │
//! └─────────────────────────────────────────────────────────────────────────┘
//!
//! Flow:
//!   1. Stream-json session 1: Claude emits poe:brief + poe:decision + poe:done.
//!      The session_id is captured from the init event.
//!   2. PTY handover: claude --resume <session_id> opens in an xterm.js browser
//!      window. You can read Claude's decision question and type any response.
//!      Close the browser tab when you are satisfied.
//!   3. Stream-json session 2: resumes with --resume <session_id> and instructs
//!      Claude to emit poe:done, confirming the continuation path works.
//!
//! What the test asserts:
//!   - Session 1: on_session_id fires, poe:decision received, poe:done received.
//!   - PTY: browser connection established (xterm.js loaded, WS handshake done).
//!   - Session 2: on_session_id fires, poe:done received, on_complete fires.

use futures_util::{SinkExt, StreamExt};
use poe2_lib::agent::text_extractor::TextBufExtractor;
use poe2_lib::agent::transport::{JsonStreamTransport, StreamCallbacks};
use poe2_lib::event_ingester::parse_poe_event;
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::io::{Read, Write};
use std::net::TcpListener as StdTcpListener;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;


// ── Logger ────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct Log {
    file: Arc<Mutex<std::fs::File>>,
    path: std::path::PathBuf,
}

impl Log {
    fn create(name: &str) -> Self {
        std::fs::create_dir_all("target").ok();
        let path = std::path::PathBuf::from(format!("target/{name}.log"));
        let file = std::fs::OpenOptions::new()
            .create(true).write(true).truncate(true)
            .open(&path)
            .unwrap_or_else(|e| panic!("failed to open {}: {}", path.display(), e));
        let log = Self { file: Arc::new(Mutex::new(file)), path };
        log.line(&format!("=== {name} started ==="));
        log
    }

    fn line(&self, msg: &str) {
        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(f, "{msg}");
            let _ = f.flush();
        }
        eprintln!("{msg}");
    }

    fn sep(&self) { self.line(""); }
    fn path(&self) -> &std::path::Path { &self.path }
}

// ── Guards ────────────────────────────────────────────────────────────────────

fn claude_on_path() -> bool {
    std::process::Command::new("claude").arg("--version")
        .output().map(|o| o.status.success()).unwrap_or(false)
}

// ── Transport runner (inline — test files cannot share code) ──────────────────

struct RunResult {
    session_ids: Vec<String>,
    poe_events: Vec<(String, serde_json::Value)>,
    completed: bool,
}

fn run_transport(
    bundle: &str,
    resume_session_id: Option<&str>,
    cwd: &std::path::Path,
    log: &Log,
) -> anyhow::Result<RunResult> {
    let session_ids: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let poe_events: Arc<Mutex<Vec<(String, serde_json::Value)>>> = Arc::new(Mutex::new(Vec::new()));
    let completed: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));

    let session_ids_c = session_ids.clone();
    let poe_for_chunk = poe_events.clone();
    let poe_for_complete = poe_events.clone();
    let completed_c = completed.clone();
    let log_pid = log.clone(); let log_session = log.clone();
    let log_chunk = log.clone(); let log_complete = log.clone();

    let extractor = Arc::new(Mutex::new(TextBufExtractor::new()));
    let ex_chunk = extractor.clone();
    let ex_complete = extractor.clone();

    JsonStreamTransport::run(
        bundle, resume_session_id, None, cwd,
        StreamCallbacks {
            on_pid: Some(Box::new(move |pid| {
                log_pid.line(&format!("[transport] pid={pid}"));
            })),
            on_session_id: Box::new(move |sid| {
                log_session.line(&format!("[transport] session_id={sid}"));
                session_ids_c.lock().unwrap().push(sid);
            }),
            on_text_chunk: Box::new(move |text| {
                let lines = ex_chunk.lock().unwrap().push(&text);
                for line in lines {
                    match parse_poe_event(&line) {
                        Some((ref t, ref j)) => {
                            log_chunk.line(&format!("[poe:{t}] {j}"));
                            poe_for_chunk.lock().unwrap().push((t.clone(), j.clone()));
                        }
                        None if line.trim().starts_with('{') => {
                            log_chunk.line(&format!("[text/json?] {}", &line[..line.len().min(120)]));
                        }
                        None => {
                            log_chunk.line(&format!("[text] {}", &line[..line.len().min(80)]));
                        }
                    }
                }
            }),
            on_raw_json: Box::new(|_| {}),
            on_complete: Box::new(move || {
                let tail = ex_complete.lock().unwrap().flush();
                if let Some(ref t) = tail {
                    if !t.trim().is_empty() {
                        if let Some((ref et, ref ej)) = parse_poe_event(t) {
                            log_complete.line(&format!("[poe:{et}] (tail) {ej}"));
                            poe_for_complete.lock().unwrap().push((et.clone(), ej.clone()));
                        }
                    }
                }
                *completed_c.lock().unwrap() = true;
                log_complete.line("[transport] on_complete fired");
            }),
        },
    )?;

    let out_session_ids = session_ids.lock().unwrap().clone();
    let out_poe_events = poe_events.lock().unwrap().clone();
    let out_completed = *completed.lock().unwrap();
    Ok(RunResult { session_ids: out_session_ids, poe_events: out_poe_events, completed: out_completed })
}

// ── xterm.js HTML page ────────────────────────────────────────────────────────

fn html_page(session_id: &str, ws_port: u16) -> String {
    format!(r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8">
  <title>POE — decision handoff — {sid}</title>
  <link rel="stylesheet" href="https://unpkg.com/@xterm/xterm@5.5.0/css/xterm.css">
  <style>
    * {{ margin:0; padding:0; box-sizing:border-box }}
    body {{ background:#1a1a1a; display:flex; flex-direction:column; height:100vh; font-family:monospace }}
    #header {{ padding:6px 12px; background:#111; color:#888; font-size:12px;
               border-bottom:1px solid #2a2a2a; display:flex; justify-content:space-between }}
    #header b {{ color:#3a8 }}
    #banner {{ padding:8px 12px; background:#1e2a1e; color:#8c8; font-size:12px; border-bottom:1px solid #2a4a2a }}
    #terminal {{ flex:1; overflow:hidden; padding:4px }}
  </style>
</head>
<body>
  <div id="header">
    <span>claude --resume <b>{sid}</b></span>
    <span id="status">connecting…</span>
  </div>
  <div id="banner">
    ✦ Decision handoff harness — read Claude's question, type your response, then <b>close this tab</b> to resume the test.
  </div>
  <div id="terminal"></div>
  <script src="https://unpkg.com/@xterm/xterm@5.5.0/lib/xterm.js"></script>
  <script src="https://unpkg.com/@xterm/addon-fit@0.10.0/lib/addon-fit.js"></script>
  <script>
    const term = new Terminal({{
      cursorBlink:true, fontFamily:'"Fira Code","SF Mono","Menlo",monospace',
      fontSize:14, theme:{{ background:'#1a1a1a', foreground:'#e0e0e0', cursor:'#e0e0e0' }}
    }});
    const fit = new FitAddon.FitAddon();
    term.loadAddon(fit); term.open(document.getElementById('terminal')); fit.fit();
    const status = document.getElementById('status');
    const ws = new WebSocket('ws://127.0.0.1:{ws_port}');
    ws.binaryType = 'arraybuffer';
    ws.onopen = () => {{
      status.textContent = 'connected'; status.style.color = '#3a8';
      ws.send(JSON.stringify({{type:'resize', cols:term.cols, rows:term.rows}}));
    }};
    ws.onmessage = e => {{
      if (e.data instanceof ArrayBuffer) term.write(new Uint8Array(e.data));
      else term.write(e.data);
    }};
    term.onData(d => {{ if (ws.readyState === WebSocket.OPEN) ws.send(d); }});
    term.onResize(({{cols,rows}}) => {{
      if (ws.readyState === WebSocket.OPEN)
        ws.send(JSON.stringify({{type:'resize', cols, rows}}));
    }});
    window.addEventListener('resize', () => fit.fit());
    ws.onclose = () => {{
      status.textContent = 'disconnected'; status.style.color = '#a44';
      term.write('\r\n\x1b[2m[disconnected — you may close this tab]\x1b[0m\r\n');
    }};
  </script>
</body>
</html>"#, sid = session_id, ws_port = ws_port)
}

// ── Minimal one-shot HTTP server ──────────────────────────────────────────────

fn serve_html_once(html: String, log: Log) -> u16 {
    let listener = StdTcpListener::bind("127.0.0.1:0").expect("http bind failed");
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            if let Ok(mut s) = stream {
                let mut buf = [0u8; 4096];
                s.read(&mut buf).ok();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                    html.len(), html
                );
                if s.write_all(response.as_bytes()).is_ok() {
                    log.line(&format!("[http] served HTML on port {port}"));
                    break;
                }
            }
        }
    });
    port
}

fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    { std::process::Command::new("open").arg(url).spawn().ok(); }
    #[cfg(target_os = "linux")]
    { std::process::Command::new("xdg-open").arg(url).spawn().ok(); }
    #[cfg(target_os = "windows")]
    { std::process::Command::new("cmd").args(["/c", "start", url]).spawn().ok(); }
}

// ── PTY + WebSocket bridge ────────────────────────────────────────────────────

async fn run_pty_handover(session_id: &str, cwd: &std::path::Path, log: Log) -> Result<(), String> {
    let pty_system = NativePtySystem::default();
    let pair = pty_system
        .openpty(PtySize { rows: 40, cols: 200, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| format!("openpty: {e}"))?;

    let mut cmd = CommandBuilder::new("claude");
    cmd.arg("--resume");
    cmd.arg(session_id);
    cmd.arg("--dangerously-skip-permissions");
    cmd.env_remove("CLAUDECODE");
    cmd.cwd(&cwd);

    let pty_writer: Arc<Mutex<Box<dyn Write + Send>>> =
        Arc::new(Mutex::new(pair.master.take_writer().map_err(|e| format!("take_writer: {e}"))?));
    let pty_reader = pair.master.try_clone_reader().map_err(|e| format!("clone_reader: {e}"))?;
    let master = Arc::new(Mutex::new(pair.master));
    let mut child = pair.slave.spawn_command(cmd).map_err(|e| format!("spawn: {e}"))?;
    let child_pid = child.process_id();
    log.line(&format!("[pty] spawned claude --resume {session_id} (pid={child_pid:?})"));

    // PTY reader → mpsc channel
    let (pty_tx, pty_rx) = mpsc::channel::<Vec<u8>>(512);
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
                    if pty_tx.blocking_send(buf[..n].to_vec()).is_err() { break; }
                }
            }
        }
        log_reader.line(&format!("[pty] reader EOF ({total} bytes)"));
    });

    // WebSocket listener
    let ws_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await.map_err(|e| format!("ws bind: {e}"))?;
    let ws_port = ws_listener.local_addr().unwrap().port();
    log.line(&format!("[ws] listening on port {ws_port}"));

    // HTTP server + browser launch
    let html = html_page(session_id, ws_port);
    let http_port = serve_html_once(html, log.clone());
    let url = format!("http://127.0.0.1:{http_port}");
    log.sep();
    log.line("╔══════════════════════════════════════════════════════════════╗");
    log.line("║  HUMAN ACTION REQUIRED                                       ║");
    log.line(&format!("║  URL: {url:<56}║"));
    log.line("║  1. Read Claude's decision question in the terminal.         ║");
    log.line("║  2. Type any response and press Enter (or just observe).     ║");
    log.line("║  3. Close the browser tab when done.                         ║");
    log.line("╚══════════════════════════════════════════════════════════════╝");
    log.sep();
    open_browser(&url);

    // Wait for browser WebSocket (up to 60 s)
    log.line("[ws] waiting for browser connection (60 s timeout)…");
    let (tcp_stream, peer) = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        ws_listener.accept(),
    ).await
    .map_err(|_| "timed out waiting for browser WebSocket connection (60 s)".to_string())?
    .map_err(|e| format!("ws accept: {e}"))?;
    log.line(&format!("[ws] TCP from {peer}"));

    let ws_stream = tokio_tungstenite::accept_async(tcp_stream)
        .await.map_err(|e| format!("ws handshake: {e}"))?;
    log.line("[ws] handshake complete — bridging PTY ↔ xterm.js");

    let (mut ws_tx, mut ws_rx) = ws_stream.split();
    let pty_writer_for_ws = pty_writer.clone();
    let master_for_resize = master.clone();

    let log_in = log.clone();
    let ws_in = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Text(s) => {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&s) {
                        if json.get("type").and_then(|v| v.as_str()) == Some("resize") {
                            let cols = json.get("cols").and_then(|v| v.as_u64()).unwrap_or(200) as u16;
                            let rows = json.get("rows").and_then(|v| v.as_u64()).unwrap_or(40) as u16;
                            master_for_resize.lock().unwrap()
                                .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }).ok();
                            continue;
                        }
                    }
                    pty_writer_for_ws.lock().unwrap().write_all(s.as_bytes()).ok();
                }
                Message::Binary(b) => { pty_writer_for_ws.lock().unwrap().write_all(&b).ok(); }
                Message::Close(_) => { log_in.line("[ws] client closed"); break; }
                _ => {}
            }
        }
    });

    let log_out = log.clone();
    let mut pty_rx = pty_rx;
    let ws_out = tokio::spawn(async move {
        while let Some(data) = pty_rx.recv().await {
            if ws_tx.send(Message::Binary(data.into())).await.is_err() {
                log_out.line("[ws] send failed");
                break;
            }
        }
    });

    log.line("[bridge] active — waiting for browser tab to close (10 min timeout)");
    tokio::select! {
        _ = ws_in  => { log.line("[bridge] input closed"); }
        _ = ws_out => { log.line("[bridge] output closed"); }
        _ = tokio::time::sleep(std::time::Duration::from_secs(600)) => {
            log.line("[bridge] 10-minute timeout");
        }
    }

    // Shutdown
    pty_writer.lock().unwrap().write_all(&[3u8]).ok(); // Ctrl-C
    drop(pty_writer);
    drop(master);
    if let Some(pid) = child_pid {
        log.line(&format!("[pty] SIGTERM pid={pid}"));
        std::process::Command::new("kill").arg(pid.to_string()).status().ok();
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if child.try_wait().ok().flatten().is_some() { log.line("[pty] exited"); break; }
        if std::time::Instant::now() >= deadline {
            if let Some(pid) = child_pid {
                std::process::Command::new("kill").args(["-9", &pid.to_string()]).status().ok();
            }
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let reap = tokio::task::spawn_blocking(move || { let _ = child.wait(); });
    tokio::time::timeout(std::time::Duration::from_secs(3), reap).await.ok();

    Ok(())
}

// ── Test ──────────────────────────────────────────────────────────────────────

/// End-to-end decision → PTY handoff → stream-json resume test.
///
/// HUMAN-RUNNABLE: a browser window opens automatically. Follow the on-screen
/// instructions, then close the tab to let the test continue.
///
/// Run: cargo test --test decision_handoff_harness -- --nocapture
/// Log: target/decision-handoff.log
#[test]
fn decision_handoff_to_xterm_and_resume() {
    let log = Log::create("decision-handoff");
    log.line(&format!("log: {}", log.path().display()));

    if !claude_on_path() {
        log.line("SKIP: claude not on PATH — skipping decision_handoff_to_xterm_and_resume");
        return;
    }

    // Use the same cwd for all three steps so Claude's session scoping (by project
    // directory) can find the session when resuming. Matches session_handoff_harness.
    let cwd = std::env::current_dir().expect("current_dir");

    // ── Session 1: emit poe:decision then poe:done ────────────────────────────
    log.sep();
    log.line("=== SESSION 1: emit poe:decision ===");

    let bundle_1 = concat!(
        "You are a test agent in autonomous mode.\n\n",
        "# Task\n\n",
        "Emit exactly these three lines in order, nothing else:\n",
        r#"{"poe":"brief","content":"Evaluating options for the handoff test."}"#, "\n",
        r#"{"poe":"decision","question":"Which path should the orchestrator take?","options":["Continue","Pause"]}"#, "\n",
        r#"{"poe":"done","summary":"Decision raised — session paused for human input."}"#, "\n",
    );

    let result_1 = run_transport(bundle_1, None, &cwd, &log)
        .expect("session 1 run_transport failed");

    assert!(
        !result_1.session_ids.is_empty(),
        "Session 1: no session_id captured. See {}",
        log.path().display()
    );
    let session_id = result_1.session_ids[0].clone();
    log.line(&format!("Session 1 session_id: {session_id}"));

    assert!(
        result_1.poe_events.iter().any(|(t, _)| t == "decision"),
        "Session 1: poe:decision not received. Events: {:?}. See {}",
        result_1.poe_events.iter().map(|(t, _)| t).collect::<Vec<_>>(),
        log.path().display()
    );
    log.line("Session 1 poe:decision: OK");

    assert!(
        result_1.poe_events.iter().any(|(t, _)| t == "done"),
        "Session 1: poe:done not received. See {}",
        log.path().display()
    );
    log.line("Session 1 poe:done: OK");

    // ── PTY handover: open xterm.js browser window ────────────────────────────
    log.sep();
    log.line("=== PTY HANDOVER ===");
    log.line(&format!("Resuming session {session_id} in xterm.js…"));

    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    rt.block_on(async {
        run_pty_handover(&session_id, &cwd, log.clone())
            .await
            .unwrap_or_else(|e| log.line(&format!("[pty handover error] {e}")));
    });

    log.line("PTY handover complete.");

    // ── Session 2: resume via stream-json, assert poe:done ────────────────────
    log.sep();
    log.line("=== SESSION 2: resume via stream-json ===");

    let bundle_2 = concat!(
        "The human has reviewed the decision and responded interactively.\n\n",
        "Emit this line and nothing else:\n",
        r#"{"poe":"done","summary":"Decision acknowledged — task resumed and complete."}"#, "\n",
    );

    let result_2 = run_transport(bundle_2, Some(&session_id), &cwd, &log)
        .expect("session 2 run_transport failed");

    log.sep();
    log.line("=== SESSION 2 ASSERTIONS ===");
    log.line(&format!("  events: {}, completed: {}", result_2.poe_events.len(), result_2.completed));

    assert!(
        !result_2.session_ids.is_empty(),
        "Session 2: on_session_id never fired. See {}",
        log.path().display()
    );
    log.line(&format!("  session_id: {}", result_2.session_ids[0]));

    assert!(
        result_2.poe_events.iter().any(|(t, _)| t == "done"),
        "Session 2: poe:done not received. Events: {:?}. See {}",
        result_2.poe_events.iter().map(|(t, _)| t).collect::<Vec<_>>(),
        log.path().display()
    );
    log.line("  poe:done: OK");

    assert!(
        result_2.completed,
        "Session 2: on_complete never fired. See {}",
        log.path().display()
    );
    log.line("  on_complete: OK");

    log.sep();
    log.line("=== ALL ASSERTIONS PASSED ===");
}
