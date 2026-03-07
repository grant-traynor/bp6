// bp6-pdr.1: SQLite database layer for POE2
//
// Owns the DB connection pool (one connection per project), runs schema
// migrations, and exposes the `get_project_state` Tauri command for
// frontend hydration on startup.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use tauri::State;

// ── Schema ─────────────────────────────────────────────────────────────────────

const MIGRATIONS: &str = r#"
CREATE TABLE IF NOT EXISTS projects (
  id          TEXT    PRIMARY KEY,
  name        TEXT    NOT NULL,
  path        TEXT    NOT NULL,
  created_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS phases (
  id          TEXT    PRIMARY KEY,
  project_id  TEXT    NOT NULL REFERENCES projects(id),
  name        TEXT    NOT NULL,
  stage_type  TEXT    NOT NULL,
  status      TEXT    NOT NULL DEFAULT 'pending',
  pdca_state  TEXT    NOT NULL DEFAULT 'plan',
  position    INTEGER NOT NULL,
  created_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS tasks (
  id          TEXT    PRIMARY KEY,
  project_id  TEXT    NOT NULL REFERENCES projects(id),
  phase_id    TEXT    REFERENCES phases(id),
  parent_id   TEXT    REFERENCES tasks(id),
  title       TEXT    NOT NULL,
  description TEXT,
  type        TEXT    NOT NULL DEFAULT 'task',
  skill       TEXT,
  status      TEXT    NOT NULL DEFAULT 'pending',
  session_id  TEXT,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS edges (
  from_id  TEXT NOT NULL REFERENCES tasks(id),
  to_id    TEXT NOT NULL REFERENCES tasks(id),
  PRIMARY KEY (from_id, to_id)
);

CREATE TABLE IF NOT EXISTS event_log (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id     TEXT    NOT NULL REFERENCES tasks(id),
  event_type  TEXT    NOT NULL,
  payload     TEXT    NOT NULL,
  created_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS decisions (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id     TEXT    NOT NULL REFERENCES tasks(id),
  question    TEXT    NOT NULL,
  options     TEXT,
  resolution  TEXT,
  created_at  INTEGER NOT NULL,
  resolved_at INTEGER
);

CREATE TABLE IF NOT EXISTS artifacts (
  id                TEXT    PRIMARY KEY,
  project_id        TEXT    NOT NULL REFERENCES projects(id),
  name              TEXT    NOT NULL,
  artifact_type     TEXT    NOT NULL,
  path              TEXT    NOT NULL,
  producing_task_id TEXT    REFERENCES tasks(id),
  created_at        INTEGER NOT NULL,
  updated_at        INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS knowledge (
  id             TEXT    PRIMARY KEY,
  project_id     TEXT    NOT NULL REFERENCES projects(id),
  key            TEXT    NOT NULL,
  content        TEXT    NOT NULL,
  source_task_id TEXT    REFERENCES tasks(id),
  supersedes_id  TEXT    REFERENCES knowledge(id),
  created_at     INTEGER NOT NULL
);
"#;

// ── Row types (camelCase for frontend) ────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRow {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PhaseRow {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub stage_type: String,
    pub status: String,
    pub pdca_state: String,
    pub position: i64,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaskRow {
    pub id: String,
    pub project_id: String,
    pub phase_id: Option<String>,
    pub parent_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub task_type: String,
    pub skill: Option<String>,
    pub status: String,
    pub session_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EdgeRow {
    pub from_id: String,
    pub to_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EventLogRow {
    pub id: i64,
    pub task_id: String,
    pub event_type: String,
    pub payload: String,
    pub created_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DecisionRow {
    pub id: i64,
    pub task_id: String,
    pub question: String,
    pub options: Option<String>,
    pub resolution: Option<String>,
    pub created_at: i64,
    pub resolved_at: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactRow {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub artifact_type: String,
    pub path: String,
    pub producing_task_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeRow {
    pub id: String,
    pub project_id: String,
    pub key: String,
    pub content: String,
    pub source_task_id: Option<String>,
    pub supersedes_id: Option<String>,
    pub created_at: i64,
}

// ── Snapshot returned to frontend ─────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStateSnapshot {
    pub project: Option<ProjectRow>,
    pub phases: Vec<PhaseRow>,
    pub tasks: Vec<TaskRow>,
    pub edges: Vec<EdgeRow>,
    pub recent_events: Vec<EventLogRow>,
    pub open_decisions: Vec<DecisionRow>,
}

// ── DbState — managed Tauri state ─────────────────────────────────────────────

pub struct DbState {
    /// project_id → Arc<Mutex<Connection>>
    pub connections: Mutex<HashMap<String, Arc<Mutex<Connection>>>>,
}

impl DbState {
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
        }
    }
}

// ── open_project_db ────────────────────────────────────────────────────────────

/// Opens (or retrieves from cache) the SQLite DB for a project.
/// Creates `{project_path}/.poe/poe.db` and runs schema migrations.
pub fn open_project_db(
    project_id: &str,
    project_path: &Path,
    state: &DbState,
) -> Result<Arc<Mutex<Connection>>, String> {
    let mut conns = state
        .connections
        .lock()
        .map_err(|e| format!("DbState lock poisoned: {e}"))?;

    if let Some(conn) = conns.get(project_id) {
        return Ok(Arc::clone(conn));
    }

    // Build path: {project_path}/.poe/poe.db
    let db_dir = project_path.join(".poe");
    std::fs::create_dir_all(&db_dir)
        .map_err(|e| format!("Failed to create .poe dir: {e}"))?;

    let db_path = db_dir.join("poe.db");

    let conn = Connection::open(&db_path)
        .map_err(|e| format!("Failed to open SQLite at {}: {e}", db_path.display()))?;

    // Enable WAL mode for better concurrent read performance
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .map_err(|e| format!("PRAGMA setup failed: {e}"))?;

    // Run migrations
    conn.execute_batch(MIGRATIONS)
        .map_err(|e| format!("Migration failed: {e}"))?;

    let conn = Arc::new(Mutex::new(conn));
    conns.insert(project_id.to_string(), Arc::clone(&conn));

    Ok(conn)
}

// ── Tauri command: get_project_state ──────────────────────────────────────────

#[tauri::command]
pub async fn get_project_state(
    project_id: String,
    db_state: State<'_, DbState>,
) -> Result<ProjectStateSnapshot, String> {
    let conns = db_state
        .connections
        .lock()
        .map_err(|e| format!("DbState lock poisoned: {e}"))?;

    let conn_arc = conns
        .get(&project_id)
        .ok_or_else(|| format!("No open DB for project {project_id}"))?
        .clone();

    drop(conns); // release DbState lock before locking the connection

    let conn = conn_arc
        .lock()
        .map_err(|e| format!("Connection lock poisoned: {e}"))?;

    // ── project ──
    let project = conn
        .prepare("SELECT id, name, path, created_at FROM projects WHERE id = ?1")
        .map_err(|e| e.to_string())?
        .query_row(params![project_id], |row| {
            Ok(ProjectRow {
                id: row.get(0)?,
                name: row.get(1)?,
                path: row.get(2)?,
                created_at: row.get(3)?,
            })
        })
        .optional()
        .map_err(|e: rusqlite::Error| e.to_string())?;

    // ── phases ──
    let phases: Vec<PhaseRow> = {
        let mut stmt = conn
            .prepare(
                "SELECT id, project_id, name, stage_type, status, pdca_state, position, created_at \
                 FROM phases WHERE project_id = ?1 ORDER BY position",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt.query_map(params![project_id], |row| {
            Ok(PhaseRow {
                id: row.get(0)?,
                project_id: row.get(1)?,
                name: row.get(2)?,
                stage_type: row.get(3)?,
                status: row.get(4)?,
                pdca_state: row.get(5)?,
                position: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
        let result: Result<Vec<_>, _> = rows.collect();
        result.map_err(|e: rusqlite::Error| e.to_string())?
    };

    // ── tasks ──
    let tasks: Vec<TaskRow> = {
        let mut stmt = conn
            .prepare(
                "SELECT id, project_id, phase_id, parent_id, title, description, \
                        type, skill, status, session_id, created_at, updated_at \
                 FROM tasks WHERE project_id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt.query_map(params![project_id], |row| {
            Ok(TaskRow {
                id: row.get(0)?,
                project_id: row.get(1)?,
                phase_id: row.get(2)?,
                parent_id: row.get(3)?,
                title: row.get(4)?,
                description: row.get(5)?,
                task_type: row.get(6)?,
                skill: row.get(7)?,
                status: row.get(8)?,
                session_id: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?;
        let result: Result<Vec<_>, _> = rows.collect();
        result.map_err(|e: rusqlite::Error| e.to_string())?
    };

    // ── edges (where either endpoint belongs to this project's tasks) ──
    let task_ids: Vec<String> = tasks.iter().map(|t| t.id.clone()).collect();
    let edges: Vec<EdgeRow> = if task_ids.is_empty() {
        vec![]
    } else {
        // Build a parameterised IN clause
        let placeholders: String = task_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT from_id, to_id FROM edges \
             WHERE from_id IN ({placeholders}) OR to_id IN ({placeholders})"
        );
        // rusqlite positional params: bind each id twice (IN clause appears twice)
        let double_ids: Vec<String> = task_ids
            .iter()
            .chain(task_ids.iter())
            .cloned()
            .collect();
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(double_ids.iter()),
            |row| {
                Ok(EdgeRow {
                    from_id: row.get(0)?,
                    to_id: row.get(1)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;
        let result: Result<Vec<_>, _> = rows.collect();
        result.map_err(|e: rusqlite::Error| e.to_string())?
    };

    // ── recent_events (last 50 for tasks in this project) ──
    let recent_events: Vec<EventLogRow> = if task_ids.is_empty() {
        vec![]
    } else {
        let placeholders: String = task_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT id, task_id, event_type, payload, created_at \
             FROM event_log WHERE task_id IN ({placeholders}) \
             ORDER BY id DESC LIMIT 50"
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(task_ids.iter()),
            |row| {
                Ok(EventLogRow {
                    id: row.get(0)?,
                    task_id: row.get(1)?,
                    event_type: row.get(2)?,
                    payload: row.get(3)?,
                    created_at: row.get(4)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;
        let result: Result<Vec<_>, _> = rows.collect();
        result.map_err(|e: rusqlite::Error| e.to_string())?
    };

    // ── open_decisions (resolution IS NULL) ──
    let open_decisions: Vec<DecisionRow> = if task_ids.is_empty() {
        vec![]
    } else {
        let placeholders: String = task_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT id, task_id, question, options, resolution, created_at, resolved_at \
             FROM decisions WHERE task_id IN ({placeholders}) AND resolution IS NULL"
        );
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(task_ids.iter()),
            |row| {
                Ok(DecisionRow {
                    id: row.get(0)?,
                    task_id: row.get(1)?,
                    question: row.get(2)?,
                    options: row.get(3)?,
                    resolution: row.get(4)?,
                    created_at: row.get(5)?,
                    resolved_at: row.get(6)?,
                })
            },
        )
        .map_err(|e| e.to_string())?;
        let result: Result<Vec<_>, _> = rows.collect();
        result.map_err(|e: rusqlite::Error| e.to_string())?
    };

    Ok(ProjectStateSnapshot {
        project,
        phases,
        tasks,
        edges,
        recent_events,
        open_decisions,
    })
}
