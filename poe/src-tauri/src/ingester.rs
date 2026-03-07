// bp6-pdr.1: poe: event ingester + SQLite writer
//
// Processes agent stdout line-by-line:
//   - Lines that are valid JSON with a "poe" key are dispatched as poe: events.
//   - All other lines are treated as raw PTY output (logged to stderr).
//
// After each DB write the relevant Tauri event is emitted so the frontend
// can update in real time.

use std::path::Path;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

// ── DAG-changed signal ────────────────────────────────────────────────────────

/// Sent to the orchestrator whenever DAG structure or task status changes.
/// The orchestrator uses this to evaluate which pending tasks can now be spawned.
pub struct DagChanged {
    pub project_id: String,
}

pub fn make_dag_channel() -> (
    tokio::sync::mpsc::Sender<DagChanged>,
    tokio::sync::mpsc::Receiver<DagChanged>,
) {
    tokio::sync::mpsc::channel(256)
}

// ── Tauri event payloads ──────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PoeEvent {
    task_id: String,
    event_type: String,
    payload: Value,
    created_at: i64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PoeTaskUpdate {
    task_id: String,
    status: String,
    title: Option<String>,
    updated_at: i64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PoeDecision {
    decision_id: i64,
    task_id: String,
    question: String,
    options: Option<Vec<String>>,
}

// ── Internal envelope ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct PoeEnvelope {
    poe: String,
    #[serde(flatten)]
    data: Value,
}

// ── Main entry point ──────────────────────────────────────────────────────────

/// Process a single line of agent stdout.
///
/// Lines that are not poe: events are logged to stderr and ignored.
/// All DB writes use `conn`; Tauri events are emitted via `app_handle`.
/// `dag_tx` receives a `DagChanged` signal whenever the DAG is mutated.
pub fn ingest_line(
    line: &str,
    task_id: &str,
    project_id: &str,
    project_path: &Path,
    conn: Arc<Mutex<Connection>>,
    app_handle: &AppHandle,
    dag_tx: &tokio::sync::mpsc::Sender<DagChanged>,
) {
    // Parse JSON envelope; skip non-JSON lines.
    let envelope: PoeEnvelope = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => {
            eprintln!("[{}] {}", task_id, line);
            return;
        }
    };

    let event_type = envelope.poe.as_str();
    let data = &envelope.data;
    let now = Utc::now().timestamp();

    let result = match event_type {
        "task" => handle_task(task_id, project_id, data, now, &conn, app_handle, dag_tx),
        "task:update" => handle_task_update(task_id, project_id, data, now, &conn, app_handle, dag_tx),
        "task:cancel" => handle_task_cancel(task_id, project_id, now, &conn, app_handle, dag_tx),
        "edge" => handle_edge(task_id, project_id, data, now, &conn, dag_tx),
        "edge:remove" => handle_edge_remove(task_id, project_id, data, now, &conn, dag_tx),
        "artifact" => handle_artifact(task_id, project_id, project_path, data, now, &conn, app_handle),
        "knowledge" => handle_knowledge(task_id, project_id, data, now, &conn, app_handle),
        "brief" => handle_log_only(task_id, event_type, data, now, &conn, app_handle),
        "step" => handle_log_only(task_id, event_type, data, now, &conn, app_handle),
        "decision" => handle_decision(task_id, project_id, data, now, &conn, app_handle, dag_tx),
        "review" => handle_log_only(task_id, event_type, data, now, &conn, app_handle),
        "done" => handle_done(task_id, project_id, now, &conn, app_handle, dag_tx),
        other => {
            eprintln!("[{}] unknown poe event type: {}", task_id, other);
            Ok(())
        }
    };

    if let Err(e) = result {
        eprintln!("[{}] ingester error on '{}': {}", task_id, event_type, e);
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn log_event(
    conn: &Connection,
    task_id: &str,
    event_type: &str,
    payload: &Value,
    now: i64,
) -> Result<(), String> {
    let payload_str = serde_json::to_string(payload).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO event_log (task_id, event_type, payload, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![task_id, event_type, payload_str, now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn send_dag_changed(
    dag_tx: &tokio::sync::mpsc::Sender<DagChanged>,
    project_id: &str,
) {
    let _ = dag_tx.try_send(DagChanged {
        project_id: project_id.to_string(),
    });
}

// ── Per-event handlers ────────────────────────────────────────────────────────

fn handle_task(
    _calling_task_id: &str,
    project_id: &str,
    data: &Value,
    now: i64,
    conn: &Arc<Mutex<Connection>>,
    app_handle: &AppHandle,
    dag_tx: &tokio::sync::mpsc::Sender<DagChanged>,
) -> Result<(), String> {
    let id = data["id"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let title = data["title"].as_str().unwrap_or("Untitled").to_string();
    let description = data["description"].as_str().map(|s| s.to_string());
    let phase_id = data["phaseId"].as_str().map(|s| s.to_string());
    let parent_id = data["parentId"].as_str().map(|s| s.to_string());
    let task_type = data["type"].as_str().unwrap_or("task").to_string();
    let skill = data["skill"].as_str().map(|s| s.to_string());

    let conn_guard = conn.lock().map_err(|e| format!("lock poisoned: {e}"))?;

    conn_guard
        .execute(
            "INSERT OR IGNORE INTO tasks \
             (id, project_id, phase_id, parent_id, title, description, type, skill, status, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending', ?9, ?9)",
            params![id, project_id, phase_id, parent_id, title, description, task_type, skill, now],
        )
        .map_err(|e| e.to_string())?;

    // Insert edges from depends_on
    if let Some(deps) = data["dependsOn"].as_array() {
        for dep in deps {
            if let Some(dep_id) = dep.as_str() {
                conn_guard
                    .execute(
                        "INSERT OR IGNORE INTO edges (from_id, to_id) VALUES (?1, ?2)",
                        params![dep_id, id],
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    log_event(&conn_guard, &id, "task", data, now)?;
    drop(conn_guard);

    app_handle
        .emit(
            "poe://task-update",
            PoeTaskUpdate {
                task_id: id,
                status: "pending".to_string(),
                title: Some(title),
                updated_at: now,
            },
        )
        .map_err(|e| e.to_string())?;

    send_dag_changed(dag_tx, project_id);
    Ok(())
}

fn handle_task_update(
    task_id: &str,
    project_id: &str,
    data: &Value,
    now: i64,
    conn: &Arc<Mutex<Connection>>,
    app_handle: &AppHandle,
    dag_tx: &tokio::sync::mpsc::Sender<DagChanged>,
) -> Result<(), String> {
    let title = data["title"].as_str().map(|s| s.to_string());
    let status = data["status"].as_str().unwrap_or("pending").to_string();
    let description = data["description"].as_str().map(|s| s.to_string());

    let conn_guard = conn.lock().map_err(|e| format!("lock poisoned: {e}"))?;

    conn_guard
        .execute(
            "UPDATE tasks SET \
             status = COALESCE(?1, status), \
             title = COALESCE(?2, title), \
             description = COALESCE(?3, description), \
             updated_at = ?4 \
             WHERE id = ?5",
            params![
                data["status"].as_str(),
                title.as_deref(),
                description.as_deref(),
                now,
                task_id
            ],
        )
        .map_err(|e| e.to_string())?;

    log_event(&conn_guard, task_id, "task:update", data, now)?;
    drop(conn_guard);

    app_handle
        .emit(
            "poe://task-update",
            PoeTaskUpdate {
                task_id: task_id.to_string(),
                status,
                title,
                updated_at: now,
            },
        )
        .map_err(|e| e.to_string())?;

    send_dag_changed(dag_tx, project_id);
    Ok(())
}

fn handle_task_cancel(
    task_id: &str,
    project_id: &str,
    now: i64,
    conn: &Arc<Mutex<Connection>>,
    app_handle: &AppHandle,
    dag_tx: &tokio::sync::mpsc::Sender<DagChanged>,
) -> Result<(), String> {
    let conn_guard = conn.lock().map_err(|e| format!("lock poisoned: {e}"))?;

    conn_guard
        .execute(
            "UPDATE tasks SET status = 'cancelled', updated_at = ?1 WHERE id = ?2",
            params![now, task_id],
        )
        .map_err(|e| e.to_string())?;

    log_event(&conn_guard, task_id, "task:cancel", &Value::Null, now)?;
    drop(conn_guard);

    app_handle
        .emit(
            "poe://task-update",
            PoeTaskUpdate {
                task_id: task_id.to_string(),
                status: "cancelled".to_string(),
                title: None,
                updated_at: now,
            },
        )
        .map_err(|e| e.to_string())?;

    send_dag_changed(dag_tx, project_id);
    Ok(())
}

fn handle_edge(
    task_id: &str,
    project_id: &str,
    data: &Value,
    now: i64,
    conn: &Arc<Mutex<Connection>>,
    dag_tx: &tokio::sync::mpsc::Sender<DagChanged>,
) -> Result<(), String> {
    let from_id = data["fromId"].as_str().ok_or("edge missing fromId")?;
    let to_id = data["toId"].as_str().ok_or("edge missing toId")?;

    let conn_guard = conn.lock().map_err(|e| format!("lock poisoned: {e}"))?;

    conn_guard
        .execute(
            "INSERT OR IGNORE INTO edges (from_id, to_id) VALUES (?1, ?2)",
            params![from_id, to_id],
        )
        .map_err(|e| e.to_string())?;

    log_event(&conn_guard, task_id, "edge", data, now)?;
    drop(conn_guard);

    send_dag_changed(dag_tx, project_id);
    Ok(())
}

fn handle_edge_remove(
    task_id: &str,
    project_id: &str,
    data: &Value,
    now: i64,
    conn: &Arc<Mutex<Connection>>,
    dag_tx: &tokio::sync::mpsc::Sender<DagChanged>,
) -> Result<(), String> {
    let from_id = data["fromId"].as_str().ok_or("edge:remove missing fromId")?;
    let to_id = data["toId"].as_str().ok_or("edge:remove missing toId")?;

    let conn_guard = conn.lock().map_err(|e| format!("lock poisoned: {e}"))?;

    conn_guard
        .execute(
            "DELETE FROM edges WHERE from_id = ?1 AND to_id = ?2",
            params![from_id, to_id],
        )
        .map_err(|e| e.to_string())?;

    log_event(&conn_guard, task_id, "edge:remove", data, now)?;
    drop(conn_guard);

    send_dag_changed(dag_tx, project_id);
    Ok(())
}

fn handle_artifact(
    task_id: &str,
    project_id: &str,
    project_path: &Path,
    data: &Value,
    now: i64,
    conn: &Arc<Mutex<Connection>>,
    app_handle: &AppHandle,
) -> Result<(), String> {
    let id = data["id"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let name = data["name"].as_str().ok_or("artifact missing name")?;
    let artifact_type = data["artifactType"].as_str().unwrap_or("file");
    let content = data["content"].as_str().unwrap_or("");

    // Write to {project_path}/docs/{name}
    let docs_dir = project_path.join("docs");
    std::fs::create_dir_all(&docs_dir)
        .map_err(|e| format!("Failed to create docs dir: {e}"))?;
    let file_path = docs_dir.join(name);
    std::fs::write(&file_path, content)
        .map_err(|e| format!("Failed to write artifact file: {e}"))?;
    let path_str = file_path.to_string_lossy().to_string();

    let conn_guard = conn.lock().map_err(|e| format!("lock poisoned: {e}"))?;

    conn_guard
        .execute(
            "INSERT INTO artifacts \
             (id, project_id, name, artifact_type, path, producing_task_id, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7) \
             ON CONFLICT(id) DO UPDATE SET \
               name = excluded.name, \
               artifact_type = excluded.artifact_type, \
               path = excluded.path, \
               updated_at = excluded.updated_at",
            params![id, project_id, name, artifact_type, path_str, task_id, now],
        )
        .map_err(|e| e.to_string())?;

    log_event(&conn_guard, task_id, "artifact", data, now)?;
    drop(conn_guard);

    app_handle
        .emit(
            "poe://event",
            PoeEvent {
                task_id: task_id.to_string(),
                event_type: "artifact".to_string(),
                payload: data.clone(),
                created_at: now,
            },
        )
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn handle_knowledge(
    task_id: &str,
    project_id: &str,
    data: &Value,
    now: i64,
    conn: &Arc<Mutex<Connection>>,
    app_handle: &AppHandle,
) -> Result<(), String> {
    let id = data["id"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let key = data["key"].as_str().ok_or("knowledge missing key")?;
    let content = data["content"].as_str().ok_or("knowledge missing content")?;
    let supersedes_id = data["supersedesId"].as_str().map(|s| s.to_string());

    let conn_guard = conn.lock().map_err(|e| format!("lock poisoned: {e}"))?;

    conn_guard
        .execute(
            "INSERT OR IGNORE INTO knowledge \
             (id, project_id, key, content, source_task_id, supersedes_id, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, project_id, key, content, task_id, supersedes_id, now],
        )
        .map_err(|e| e.to_string())?;

    log_event(&conn_guard, task_id, "knowledge", data, now)?;
    drop(conn_guard);

    app_handle
        .emit(
            "poe://event",
            PoeEvent {
                task_id: task_id.to_string(),
                event_type: "knowledge".to_string(),
                payload: data.clone(),
                created_at: now,
            },
        )
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn handle_log_only(
    task_id: &str,
    event_type: &str,
    data: &Value,
    now: i64,
    conn: &Arc<Mutex<Connection>>,
    app_handle: &AppHandle,
) -> Result<(), String> {
    let conn_guard = conn.lock().map_err(|e| format!("lock poisoned: {e}"))?;
    log_event(&conn_guard, task_id, event_type, data, now)?;
    drop(conn_guard);

    app_handle
        .emit(
            "poe://event",
            PoeEvent {
                task_id: task_id.to_string(),
                event_type: event_type.to_string(),
                payload: data.clone(),
                created_at: now,
            },
        )
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn handle_decision(
    task_id: &str,
    project_id: &str,
    data: &Value,
    now: i64,
    conn: &Arc<Mutex<Connection>>,
    app_handle: &AppHandle,
    dag_tx: &tokio::sync::mpsc::Sender<DagChanged>,
) -> Result<(), String> {
    let question = data["question"].as_str().ok_or("decision missing question")?;
    let options: Option<Vec<String>> = data["options"].as_array().map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    });
    let options_json = options
        .as_ref()
        .map(|o| serde_json::to_string(o).unwrap_or_default());

    let conn_guard = conn.lock().map_err(|e| format!("lock poisoned: {e}"))?;

    // Park the task
    conn_guard
        .execute(
            "UPDATE tasks SET status = 'waiting', updated_at = ?1 WHERE id = ?2",
            params![now, task_id],
        )
        .map_err(|e| e.to_string())?;

    // Insert decision record
    conn_guard
        .execute(
            "INSERT INTO decisions (task_id, question, options, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![task_id, question, options_json, now],
        )
        .map_err(|e| e.to_string())?;

    let decision_id = conn_guard.last_insert_rowid();

    log_event(&conn_guard, task_id, "decision", data, now)?;
    drop(conn_guard);

    app_handle
        .emit(
            "poe://decision",
            PoeDecision {
                decision_id,
                task_id: task_id.to_string(),
                question: question.to_string(),
                options,
            },
        )
        .map_err(|e| e.to_string())?;

    // Parking a task changes DAG readiness
    send_dag_changed(dag_tx, project_id);
    Ok(())
}

fn handle_done(
    task_id: &str,
    project_id: &str,
    now: i64,
    conn: &Arc<Mutex<Connection>>,
    app_handle: &AppHandle,
    dag_tx: &tokio::sync::mpsc::Sender<DagChanged>,
) -> Result<(), String> {
    let conn_guard = conn.lock().map_err(|e| format!("lock poisoned: {e}"))?;

    conn_guard
        .execute(
            "UPDATE tasks SET status = 'done', updated_at = ?1 WHERE id = ?2",
            params![now, task_id],
        )
        .map_err(|e| e.to_string())?;

    log_event(&conn_guard, task_id, "done", &Value::Null, now)?;
    drop(conn_guard);

    app_handle
        .emit(
            "poe://task-update",
            PoeTaskUpdate {
                task_id: task_id.to_string(),
                status: "done".to_string(),
                title: None,
                updated_at: now,
            },
        )
        .map_err(|e| e.to_string())?;

    send_dag_changed(dag_tx, project_id);
    Ok(())
}
