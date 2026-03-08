//! PTY + WebSocket bridge for human agent handover.
//!
//! Spawns `claude --resume <session_id> --dangerously-skip-permissions` in a PTY,
//! then bridges raw bytes bidirectionally to a WebSocket so that xterm.js can
//! render the session in the frontend.
//!
//! This is the ONLY remaining use of portable_pty in production code.
//! The primary agent transport (JsonStreamTransport) uses plain stdio.
//!
//! Protocol.md §5 "Human handover" specifies this flow.

use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use tokio_tungstenite::{accept_async, tungstenite::Message};

// ── Handover session state ────────────────────────────────────────────────────

pub struct HandoverSession {
    /// OS pid of the spawned claude --resume process.
    pid: Option<u32>,
    /// WebSocket port the bridge is listening on.
    ws_port: u16,
    /// PTY master writer — shared so WS→PTY bridge can write keyboard input.
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
}

/// Global map of node_id → active handover session.
pub type HandoverMap = Arc<Mutex<HashMap<String, HandoverSession>>>;

pub fn new_handover_map() -> HandoverMap {
    Arc::new(Mutex::new(HashMap::new()))
}

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Open a PTY handover session for the given node.
///
/// Two modes:
/// - **Resume**: if a prior agent session_id exists in the DB, spawns
///   `claude --resume <session_id> --dangerously-skip-permissions` in a PTY.
/// - **Fresh**: if no session exists (e.g. CONOPS bootstrap, new interactive task),
///   marks the node as `running` to prevent autonomous dispatch, creates an agent
///   record, and spawns a plain `claude --dangerously-skip-permissions` session.
///
/// Starts a WebSocket server on a random loopback port and bridges raw bytes
/// between the PTY and the WebSocket. Frontend connects xterm.js to
/// `ws://127.0.0.1:<port>`.
#[tauri::command]
pub async fn agent_handover_open(
    node_id: String,
    registry: tauri::State<'_, crate::dag_store::ProjectRegistry>,
    handover_map: tauri::State<'_, HandoverMap>,
    dag_tx: tauri::State<'_, tokio::sync::mpsc::UnboundedSender<crate::event_ingester::DagChanged>>,
) -> Result<u16, String> {
    // 1. Look up session_id and project_path from the registry.
    //    session_id is None for brand-new tasks that have never been run.
    let (session_id_opt, project_path, project_id) = {
        let reg = registry.lock().unwrap();
        let mut found: Option<(Option<String>, String, String)> = None;
        for db in reg.values() {
            let conn = db.conn.lock().unwrap();
            // Check if this project owns the node.
            let node_exists: bool = conn
                .query_row("SELECT 1 FROM nodes WHERE id = ?1", [&node_id], |_| Ok(()))
                .is_ok();
            if !node_exists {
                continue;
            }
            let sid: Option<String> = conn
                .query_row(
                    "SELECT session_id FROM agents WHERE task_id = ?1 AND session_id IS NOT NULL ORDER BY started_at DESC LIMIT 1",
                    [&node_id],
                    |row| row.get(0),
                )
                .ok()
                .flatten();
            found = Some((sid, db.project.path.clone(), db.project.id.clone()));
            break;
        }
        found.ok_or_else(|| format!("Node {} not found in any open project", node_id))?
    };

    let is_fresh = session_id_opt.is_none();

    eprintln!(
        "[pty_handover] opening handover for node={} session={:?} fresh={}",
        node_id, session_id_opt, is_fresh
    );

    // 2. For fresh sessions: mark node 'running' so the autonomous orchestrator
    //    doesn't race to dispatch the same task.
    if is_fresh {
        let reg = registry.lock().unwrap();
        if let Some(db) = reg.values().find(|db| db.project.id == project_id) {
            let conn = db.conn.lock().unwrap();
            let update = crate::dag_store::UpdateNodeInput {
                status: Some(crate::dag_store::NodeStatus::Running),
                title: None,
                description: None,
                skill_id: None,
                assignee: None,
            };
            let _ = crate::dag_store::db_update_node(&conn, &node_id, &update);
            // Create an agent record so session_id is captured when claude emits init.
            // Skill ID is best-effort; 'interactive' marks it as a human-driven session.
            let _ = crate::dag_store::db_create_agent(
                &conn,
                &project_id,
                "interactive",
                &node_id,
                None,
            );
        }
    }

    // 2. Bind to a random loopback port.
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|e| format!("Failed to bind WebSocket listener: {}", e))?;
    let ws_port = listener
        .local_addr()
        .map(|a| a.port())
        .map_err(|e| format!("Failed to get local port: {}", e))?;

    eprintln!("[pty_handover] node={} ws_port={}", node_id, ws_port);

    // 3. Spawn PTY.
    //    Resume: claude --resume <session_id> --dangerously-skip-permissions
    //    Fresh:  claude --dangerously-skip-permissions (new interactive session)
    let pty_system = NativePtySystem::default();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 220,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {}", e))?;

    let mut cmd = CommandBuilder::new("claude");
    if let Some(ref sid) = session_id_opt {
        cmd.arg("--resume");
        cmd.arg(sid);
    }
    cmd.arg("--dangerously-skip-permissions");
    cmd.cwd(&project_path); // CRITICAL: must match original session cwd

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn claude --resume: {}", e))?;

    let child_pid = child.process_id();
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("Failed to get PTY writer: {}", e))?;
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("Failed to get PTY reader: {}", e))?;

    let shared_writer: Arc<Mutex<Box<dyn Write + Send>>> = Arc::new(Mutex::new(writer));

    // 4. Store in handover map (keeps writer alive, prevents PTY close).
    let node_id_clone = node_id.clone();
    {
        let mut map = handover_map.lock().unwrap();
        map.insert(
            node_id.clone(),
            HandoverSession {
                pid: child_pid,
                ws_port,
                writer: shared_writer.clone(),
            },
        );
    }

    let writer_clone = shared_writer.clone();

    // 5. Spawn async task to accept the WebSocket connection and bridge bytes.
    tokio::spawn(async move {
        // Accept exactly one connection.
        listener.set_nonblocking(false).ok();
        match listener.accept() {
            Ok((tcp_stream, peer)) => {
                eprintln!("[pty_handover] node={} WS connected from {}", node_id_clone, peer);
                tcp_stream.set_nodelay(true).ok();
                // tokio requires non-blocking mode before from_std.
                if let Err(e) = tcp_stream.set_nonblocking(true) {
                    eprintln!("[pty_handover] node={} set_nonblocking error: {}", node_id_clone, e);
                    return;
                }
                // Convert std TcpStream to tokio for tungstenite.
                let tokio_stream = match tokio::net::TcpStream::from_std(tcp_stream) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("[pty_handover] node={} stream convert error: {}", node_id_clone, e);
                        return;
                    }
                };
                let ws = match accept_async(tokio_stream).await {
                    Ok(ws) => ws,
                    Err(e) => {
                        eprintln!("[pty_handover] node={} WS handshake error: {}", node_id_clone, e);
                        return;
                    }
                };
                let (mut ws_sink, mut ws_stream) = ws.split();

                // PTY → WS: raw bytes as Binary messages (xterm.js renders ANSI natively).
                let node_id_pty = node_id_clone.clone();
                let pty_to_ws = tokio::task::spawn_blocking(move || {
                    let mut reader = reader;
                    let mut buf = [0u8; 4096];
                    let rt = tokio::runtime::Handle::current();
                    loop {
                        match reader.read(&mut buf) {
                            Ok(0) => break, // EOF
                            Ok(n) => {
                                let data = buf[..n].to_vec();
                                if rt
                                    .block_on(ws_sink.send(Message::Binary(data.into())))
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    eprintln!("[pty_handover] node={} PTY→WS bridge ended", node_id_pty);
                });

                // WS → PTY: keyboard bytes and resize messages.
                while let Some(msg) = ws_stream.next().await {
                    match msg {
                        Ok(Message::Binary(data)) => {
                            // Keyboard input → PTY stdin.
                            if let Ok(mut w) = writer_clone.lock() {
                                let _ = w.write_all(&data);
                            }
                        }
                        Ok(Message::Text(text)) => {
                            // Resize message: {"type":"resize","cols":N,"rows":N}
                            if let Ok(json) =
                                serde_json::from_str::<serde_json::Value>(&text)
                            {
                                if json.get("type").and_then(|v| v.as_str())
                                    == Some("resize")
                                {
                                    let cols = json
                                        .get("cols")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(220) as u16;
                                    let rows = json
                                        .get("rows")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(24) as u16;
                                    eprintln!(
                                        "[pty_handover] node={} resize {}x{}",
                                        node_id_clone, cols, rows
                                    );
                                    // Resize is handled via the PTY master — requires
                                    // access to pair.master which is not cloneable here.
                                    // Log and continue; full resize support can be added
                                    // when the PTY master handle is stored in HandoverSession.
                                }
                            }
                        }
                        Ok(Message::Close(_)) | Err(_) => break,
                        _ => {}
                    }
                }

                pty_to_ws.abort();
                eprintln!("[pty_handover] node={} WS→PTY bridge ended", node_id_clone);
            }
            Err(e) => {
                eprintln!(
                    "[pty_handover] node={} listener accept error: {}",
                    node_id_clone, e
                );
            }
        }
    });

    Ok(ws_port)
}

/// Close the PTY handover session for the given node.
///
/// Sends SIGTERM to the claude --resume process, then SIGKILL after 5s.
/// Marks the node `complete` — closing the modal means the user is done.
#[tauri::command]
pub async fn agent_handover_close(
    node_id: String,
    handover_map: tauri::State<'_, HandoverMap>,
    registry: tauri::State<'_, crate::dag_store::ProjectRegistry>,
    dag_tx: tauri::State<'_, tokio::sync::mpsc::UnboundedSender<crate::event_ingester::DagChanged>>,
) -> Result<(), String> {
    let session = {
        let mut map = handover_map.lock().unwrap();
        map.remove(&node_id)
    };

    // `None` is fine — the modal can be closed after the process already exited.
    let session = session;

    eprintln!("[pty_handover] closing handover for node={}", node_id);

    if let Some(ref s) = session {
        if let Some(pid) = s.pid {
            // SIGTERM first, then SIGKILL after 5s.
            let _ = std::process::Command::new("kill")
                .arg(pid.to_string())
                .status();

            // Poll for exit with 5s timeout, then SIGKILL.
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                let exited = std::process::Command::new("kill")
                    .args(["-0", &pid.to_string()])
                    .status()
                    .map(|s| !s.success())
                    .unwrap_or(true);
                if exited || std::time::Instant::now() >= deadline {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            let _ = std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .status();
        }
    }
    // session._writer is dropped here, closing the PTY master fd.
    drop(session);

    // Mark the node complete — closing the session means the user is done.
    // Only updates if the node is currently running/waiting; already-complete nodes
    // (e.g. autonomous run finished before user closed modal) are left as-is.
    {
        let reg = registry.lock().unwrap();
        for db in reg.values() {
            let conn = db.conn.lock().unwrap();
            let node_exists: bool = conn
                .query_row("SELECT 1 FROM nodes WHERE id = ?1", [&node_id], |_| Ok(()))
                .is_ok();
            if !node_exists { continue; }
            // Only complete if still running or waiting (don't overwrite an already-complete node).
            let status: Option<String> = conn
                .query_row("SELECT status FROM nodes WHERE id = ?1", [&node_id], |row| row.get(0))
                .ok();
            if matches!(status.as_deref(), Some("running") | Some("waiting")) {
                let update = crate::dag_store::UpdateNodeInput {
                    status: Some(crate::dag_store::NodeStatus::Complete),
                    title: None, description: None, skill_id: None, assignee: None,
                };
                let _ = crate::dag_store::db_update_node(&conn, &node_id, &update);
                let project_id = db.project.id.clone();
                drop(conn);
                drop(reg);
                let _ = dag_tx.send(crate::event_ingester::DagChanged::NodeStatusChanged {
                    project_id,
                    node_id: node_id.clone(),
                });
                eprintln!("[pty_handover] node={} marked complete on session close", node_id);
                return Ok(());
            }
            break;
        }
    }

    Ok(())
}
