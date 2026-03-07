pub mod commands;
pub mod schema;
pub mod types;

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub use types::*;

/// A project database open in memory.
pub struct ProjectDb {
    pub project: Project,
    pub conn: Mutex<Connection>,
}

/// Global registry of open project databases, keyed by project path.
pub type ProjectRegistry = Arc<Mutex<HashMap<String, Arc<ProjectDb>>>>;

pub fn new_registry() -> ProjectRegistry {
    Arc::new(Mutex::new(HashMap::new()))
}

/// Open (or re-open) the SQLite database for a project at `project_path`.
/// Creates `.poe/dag.db` if it does not exist.
pub fn open_project_db(project_path: &Path) -> Result<(Project, Connection)> {
    let poe_dir = project_path.join(".poe");
    std::fs::create_dir_all(&poe_dir)
        .with_context(|| format!("Failed to create .poe dir at {:?}", poe_dir))?;

    let db_path = poe_dir.join("dag.db");
    let conn = Connection::open(&db_path)
        .with_context(|| format!("Failed to open SQLite at {:?}", db_path))?;

    conn.execute_batch(schema::CREATE_TABLES)
        .context("Failed to apply schema")?;

    // Upsert the project record based on path
    let path_str = project_path.to_string_lossy().to_string();
    let name = project_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path_str.clone());

    let now = chrono::Utc::now().to_rfc3339();
    let project_id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO projects (id, name, path, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(path) DO UPDATE SET updated_at = excluded.updated_at",
        rusqlite::params![project_id, name, path_str, now, now],
    )
    .context("Failed to upsert project")?;

    let project = conn
        .query_row(
            "SELECT id, name, path, conops_ref, active_phase_id, created_at, updated_at
             FROM projects WHERE path = ?1",
            [&path_str],
            |row| {
                Ok(Project {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                    conops_ref: row.get(3)?,
                    active_phase_id: row.get(4)?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            },
        )
        .context("Failed to read project record")?;

    Ok((project, conn))
}

/// Parse a `NodeType` from a string stored in SQLite.
fn parse_node_type(s: String) -> rusqlite::Result<NodeType> {
    s.parse().map_err(|e: String| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, e.into())
    })
}

/// Parse a `NodeStatus` from a string stored in SQLite.
fn parse_node_status(s: String) -> rusqlite::Result<NodeStatus> {
    s.parse().map_err(|e: String| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, e.into())
    })
}

/// Parse a `EdgeType` from a string stored in SQLite.
fn parse_edge_type(s: String) -> rusqlite::Result<EdgeType> {
    s.parse().map_err(|e: String| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, e.into())
    })
}

/// Parse a `PhaseLifecycleStage` from a string stored in SQLite.
fn parse_lifecycle_stage(s: String) -> rusqlite::Result<PhaseLifecycleStage> {
    s.parse().map_err(|e: String| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, e.into())
    })
}

// ── Node helpers ──────────────────────────────────────────────────────────────

pub fn db_create_node(conn: &Connection, input: &CreateNodeInput) -> Result<Node> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO nodes (id, project_id, phase_id, parent_id, node_type, title, description, status, skill_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8, ?9, ?10)",
        rusqlite::params![
            id, input.project_id, input.phase_id, input.parent_id,
            input.node_type.to_string(), input.title, input.description,
            input.skill_id, now, now
        ],
    )
    .context("Failed to insert node")?;
    db_get_node(conn, &id)
}

pub fn db_get_node(conn: &Connection, node_id: &str) -> Result<Node> {
    conn.query_row(
        "SELECT id, project_id, phase_id, parent_id, node_type, title, description, status, skill_id, assignee, created_at, updated_at
         FROM nodes WHERE id = ?1",
        [node_id],
        |row| {
            Ok(Node {
                id: row.get(0)?,
                project_id: row.get(1)?,
                phase_id: row.get(2)?,
                parent_id: row.get(3)?,
                node_type: parse_node_type(row.get(4)?)?,
                title: row.get(5)?,
                description: row.get(6)?,
                status: parse_node_status(row.get(7)?)?,
                skill_id: row.get(8)?,
                assignee: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            })
        },
    )
    .with_context(|| format!("Node not found: {}", node_id))
}

pub fn db_update_node(conn: &Connection, node_id: &str, input: &UpdateNodeInput) -> Result<Node> {
    let now = chrono::Utc::now().to_rfc3339();
    if let Some(ref title) = input.title {
        conn.execute("UPDATE nodes SET title = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![title, now, node_id])?;
    }
    if let Some(ref description) = input.description {
        conn.execute("UPDATE nodes SET description = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![description, now, node_id])?;
    }
    if let Some(ref status) = input.status {
        conn.execute("UPDATE nodes SET status = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![status.to_string(), now, node_id])?;
    }
    if let Some(ref skill_id) = input.skill_id {
        conn.execute("UPDATE nodes SET skill_id = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![skill_id, now, node_id])?;
    }
    if let Some(ref assignee) = input.assignee {
        conn.execute("UPDATE nodes SET assignee = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![assignee, now, node_id])?;
    }
    db_get_node(conn, node_id)
}

pub fn db_cancel_node(conn: &Connection, node_id: &str) -> Result<Node> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE nodes SET status = 'cancelled', updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, node_id],
    )
    .context("Failed to cancel node")?;
    db_get_node(conn, node_id)
}

pub fn db_list_nodes(conn: &Connection, project_id: &str, phase_id: Option<&str>) -> Result<Vec<Node>> {
    let mut rows = Vec::new();
    if let Some(pid) = phase_id {
        let mut stmt = conn.prepare(
            "SELECT id, project_id, phase_id, parent_id, node_type, title, description, status, skill_id, assignee, created_at, updated_at
             FROM nodes WHERE project_id = ?1 AND phase_id = ?2 ORDER BY created_at"
        )?;
        for row in stmt.query_map(rusqlite::params![project_id, pid], |row| {
            Ok((row.get::<_,String>(0)?, row.get::<_,String>(1)?, row.get::<_,Option<String>>(2)?,
                row.get::<_,Option<String>>(3)?, row.get::<_,String>(4)?, row.get::<_,String>(5)?,
                row.get::<_,Option<String>>(6)?, row.get::<_,String>(7)?, row.get::<_,Option<String>>(8)?,
                row.get::<_,Option<String>>(9)?, row.get::<_,String>(10)?, row.get::<_,String>(11)?))
        })? {
            let (id, proj, phase, parent, nt, title, desc, status, skill, assignee, cat, uat) = row?;
            rows.push(Node {
                id, project_id: proj, phase_id: phase, parent_id: parent,
                node_type: parse_node_type(nt)?, title, description: desc,
                status: parse_node_status(status)?, skill_id: skill, assignee,
                created_at: cat, updated_at: uat,
            });
        }
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, project_id, phase_id, parent_id, node_type, title, description, status, skill_id, assignee, created_at, updated_at
             FROM nodes WHERE project_id = ?1 ORDER BY created_at"
        )?;
        for row in stmt.query_map([project_id], |row| {
            Ok((row.get::<_,String>(0)?, row.get::<_,String>(1)?, row.get::<_,Option<String>>(2)?,
                row.get::<_,Option<String>>(3)?, row.get::<_,String>(4)?, row.get::<_,String>(5)?,
                row.get::<_,Option<String>>(6)?, row.get::<_,String>(7)?, row.get::<_,Option<String>>(8)?,
                row.get::<_,Option<String>>(9)?, row.get::<_,String>(10)?, row.get::<_,String>(11)?))
        })? {
            let (id, proj, phase, parent, nt, title, desc, status, skill, assignee, cat, uat) = row?;
            rows.push(Node {
                id, project_id: proj, phase_id: phase, parent_id: parent,
                node_type: parse_node_type(nt)?, title, description: desc,
                status: parse_node_status(status)?, skill_id: skill, assignee,
                created_at: cat, updated_at: uat,
            });
        }
    }
    Ok(rows)
}

/// Return the full WBS ancestry chain for a node (parent → grandparent → ... → root).
pub fn db_get_ancestry(conn: &Connection, node_id: &str) -> Result<Vec<Node>> {
    let mut ancestry = Vec::new();
    let mut current_id = Some(node_id.to_owned());

    while let Some(id) = current_id {
        let node = db_get_node(conn, &id)?;
        current_id = node.parent_id.clone();
        ancestry.push(node);
    }
    Ok(ancestry)
}

// ── Edge helpers ──────────────────────────────────────────────────────────────

pub fn db_create_edge(conn: &Connection, from_id: &str, to_id: &str, edge_type: EdgeType) -> Result<Edge> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR IGNORE INTO edges (id, from_id, to_id, edge_type, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, from_id, to_id, edge_type.to_string(), now],
    )
    .context("Failed to insert edge")?;
    conn.query_row(
        "SELECT id, from_id, to_id, edge_type, created_at FROM edges WHERE from_id = ?1 AND to_id = ?2 AND edge_type = ?3",
        rusqlite::params![from_id, to_id, edge_type.to_string()],
        |row| Ok(Edge {
            id: row.get(0)?,
            from_id: row.get(1)?,
            to_id: row.get(2)?,
            edge_type: parse_edge_type(row.get(3)?)?,
            created_at: row.get(4)?,
        }),
    )
    .context("Failed to read edge after insert")
}

pub fn db_remove_edge(conn: &Connection, from_id: &str, to_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM edges WHERE from_id = ?1 AND to_id = ?2",
        rusqlite::params![from_id, to_id],
    )
    .context("Failed to remove edge")?;
    Ok(())
}

/// Return all `depends_on` edges pointing to `node_id` (i.e. its dependencies).
pub fn db_get_dependencies(conn: &Connection, node_id: &str) -> Result<Vec<Edge>> {
    let mut stmt = conn.prepare(
        "SELECT id, from_id, to_id, edge_type, created_at FROM edges WHERE to_id = ?1 AND edge_type = 'depends_on'"
    )?;
    let rows = stmt
        .query_map([node_id], |row| {
            Ok(Edge {
                id: row.get(0)?,
                from_id: row.get(1)?,
                to_id: row.get(2)?,
                edge_type: parse_edge_type(row.get(3)?)?,
                created_at: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to query dependencies")?;
    Ok(rows)
}

// ── Artifact helpers ──────────────────────────────────────────────────────────

pub fn db_upsert_artifact(conn: &Connection, input: &CreateArtifactInput) -> Result<Artifact> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO artifacts (id, project_id, phase_id, artifact_type, filename, produced_by_stage, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(project_id, filename) DO UPDATE SET artifact_type = excluded.artifact_type,
             produced_by_stage = excluded.produced_by_stage, updated_at = excluded.updated_at",
        rusqlite::params![id, input.project_id, input.phase_id, input.artifact_type,
            input.filename, input.produced_by_stage, now, now],
    )
    .context("Failed to upsert artifact")?;
    conn.query_row(
        "SELECT id, project_id, phase_id, artifact_type, filename, produced_by_stage, created_at, updated_at
         FROM artifacts WHERE project_id = ?1 AND filename = ?2",
        rusqlite::params![input.project_id, input.filename],
        |row| Ok(Artifact {
            id: row.get(0)?,
            project_id: row.get(1)?,
            phase_id: row.get(2)?,
            artifact_type: row.get(3)?,
            filename: row.get(4)?,
            produced_by_stage: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        }),
    )
    .context("Failed to read artifact after upsert")
}

pub fn db_list_artifacts(conn: &Connection, project_id: &str) -> Result<Vec<Artifact>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, phase_id, artifact_type, filename, produced_by_stage, created_at, updated_at
         FROM artifacts WHERE project_id = ?1 ORDER BY created_at"
    )?;
    let rows = stmt
        .query_map([project_id], |row| {
            Ok(Artifact {
                id: row.get(0)?,
                project_id: row.get(1)?,
                phase_id: row.get(2)?,
                artifact_type: row.get(3)?,
                filename: row.get(4)?,
                produced_by_stage: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to list artifacts")?;
    Ok(rows)
}

// ── Knowledge helpers ─────────────────────────────────────────────────────────

pub fn db_create_knowledge(conn: &Connection, input: &CreateKnowledgeInput) -> Result<KnowledgeEntry> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO knowledge (id, project_id, key, value, source, supersedes_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![id, input.project_id, input.key, input.value, input.source, input.supersedes_id, now],
    )
    .context("Failed to insert knowledge entry")?;
    Ok(KnowledgeEntry {
        id,
        project_id: input.project_id.clone(),
        key: input.key.clone(),
        value: input.value.clone(),
        source: input.source.clone(),
        supersedes_id: input.supersedes_id.clone(),
        created_at: now,
    })
}

pub fn db_list_knowledge(conn: &Connection, project_id: &str) -> Result<Vec<KnowledgeEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, key, value, source, supersedes_id, created_at
         FROM knowledge WHERE project_id = ?1 ORDER BY created_at"
    )?;
    let rows = stmt
        .query_map([project_id], |row| {
            Ok(KnowledgeEntry {
                id: row.get(0)?,
                project_id: row.get(1)?,
                key: row.get(2)?,
                value: row.get(3)?,
                source: row.get(4)?,
                supersedes_id: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to list knowledge")?;
    Ok(rows)
}

// ── Queue helpers ─────────────────────────────────────────────────────────────

pub fn db_create_queue_item(
    conn: &Connection,
    project_id: &str,
    agent_id: Option<&str>,
    task_id: Option<&str>,
    question: &str,
    options: Option<&str>,
) -> Result<QueueItem> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO queue_items (id, project_id, agent_id, task_id, question, options, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![id, project_id, agent_id, task_id, question, options, now],
    )
    .context("Failed to insert queue item")?;
    Ok(QueueItem {
        id,
        project_id: project_id.to_owned(),
        agent_id: agent_id.map(str::to_owned),
        task_id: task_id.map(str::to_owned),
        question: question.to_owned(),
        options: options.map(str::to_owned),
        resolution: None,
        created_at: now,
        resolved_at: None,
    })
}

pub fn db_resolve_queue_item(conn: &Connection, item_id: &str, resolution: &str) -> Result<QueueItem> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE queue_items SET resolution = ?1, resolved_at = ?2 WHERE id = ?3",
        rusqlite::params![resolution, now, item_id],
    )
    .context("Failed to resolve queue item")?;
    conn.query_row(
        "SELECT id, project_id, agent_id, task_id, question, options, resolution, created_at, resolved_at
         FROM queue_items WHERE id = ?1",
        [item_id],
        |row| Ok(QueueItem {
            id: row.get(0)?,
            project_id: row.get(1)?,
            agent_id: row.get(2)?,
            task_id: row.get(3)?,
            question: row.get(4)?,
            options: row.get(5)?,
            resolution: row.get(6)?,
            created_at: row.get(7)?,
            resolved_at: row.get(8)?,
        }),
    )
    .context("Failed to read queue item after resolve")
}

pub fn db_list_queue_items(conn: &Connection, project_id: &str, unresolved_only: bool) -> Result<Vec<QueueItem>> {
    let sql = if unresolved_only {
        "SELECT id, project_id, agent_id, task_id, question, options, resolution, created_at, resolved_at
         FROM queue_items WHERE project_id = ?1 AND resolved_at IS NULL ORDER BY created_at"
    } else {
        "SELECT id, project_id, agent_id, task_id, question, options, resolution, created_at, resolved_at
         FROM queue_items WHERE project_id = ?1 ORDER BY created_at"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt
        .query_map([project_id], |row| {
            Ok(QueueItem {
                id: row.get(0)?,
                project_id: row.get(1)?,
                agent_id: row.get(2)?,
                task_id: row.get(3)?,
                question: row.get(4)?,
                options: row.get(5)?,
                resolution: row.get(6)?,
                created_at: row.get(7)?,
                resolved_at: row.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to list queue items")?;
    Ok(rows)
}

// ── Event log helpers ─────────────────────────────────────────────────────────

pub fn db_log_event(
    conn: &Connection,
    project_id: &str,
    agent_id: Option<&str>,
    task_id: Option<&str>,
    event_type: &str,
    payload: &str,
) -> Result<EventRecord> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO events (id, project_id, agent_id, task_id, event_type, payload, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![id, project_id, agent_id, task_id, event_type, payload, now],
    )
    .context("Failed to log event")?;
    Ok(EventRecord {
        id,
        project_id: project_id.to_owned(),
        agent_id: agent_id.map(str::to_owned),
        task_id: task_id.map(str::to_owned),
        event_type: event_type.to_owned(),
        payload: payload.to_owned(),
        created_at: now,
    })
}

pub fn db_list_events(conn: &Connection, project_id: &str, since: Option<&str>) -> Result<Vec<EventRecord>> {
    let mut rows = Vec::new();
    if let Some(since_ts) = since {
        let mut stmt = conn.prepare(
            "SELECT id, project_id, agent_id, task_id, event_type, payload, created_at
             FROM events WHERE project_id = ?1 AND created_at >= ?2 ORDER BY created_at"
        )?;
        for row in stmt.query_map(rusqlite::params![project_id, since_ts], |row| {
            Ok((row.get::<_,String>(0)?, row.get::<_,String>(1)?, row.get::<_,Option<String>>(2)?,
                row.get::<_,Option<String>>(3)?, row.get::<_,String>(4)?, row.get::<_,String>(5)?,
                row.get::<_,String>(6)?))
        })? {
            let (id, proj, agent_id, task_id, event_type, payload, created_at) = row?;
            rows.push(EventRecord { id, project_id: proj, agent_id, task_id, event_type, payload, created_at });
        }
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, project_id, agent_id, task_id, event_type, payload, created_at
             FROM events WHERE project_id = ?1 ORDER BY created_at"
        )?;
        for row in stmt.query_map([project_id], |row| {
            Ok((row.get::<_,String>(0)?, row.get::<_,String>(1)?, row.get::<_,Option<String>>(2)?,
                row.get::<_,Option<String>>(3)?, row.get::<_,String>(4)?, row.get::<_,String>(5)?,
                row.get::<_,String>(6)?))
        })? {
            let (id, proj, agent_id, task_id, event_type, payload, created_at) = row?;
            rows.push(EventRecord { id, project_id: proj, agent_id, task_id, event_type, payload, created_at });
        }
    }
    Ok(rows)
}

// ── Agent record helpers ──────────────────────────────────────────────────────

pub fn db_create_agent(
    conn: &Connection,
    project_id: &str,
    skill_id: &str,
    task_id: &str,
    session_id: Option<&str>,
) -> Result<AgentRecord> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO agents (id, project_id, skill_id, task_id, status, session_id, started_at)
         VALUES (?1, ?2, ?3, ?4, 'running', ?5, ?6)",
        rusqlite::params![id, project_id, skill_id, task_id, session_id, now],
    )
    .context("Failed to create agent record")?;
    Ok(AgentRecord {
        id,
        project_id: project_id.to_owned(),
        skill_id: skill_id.to_owned(),
        task_id: task_id.to_owned(),
        status: "running".to_owned(),
        session_id: session_id.map(str::to_owned),
        started_at: now,
        ended_at: None,
    })
}

pub fn db_end_agent(conn: &Connection, agent_id: &str, status: &str) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE agents SET status = ?1, ended_at = ?2 WHERE id = ?3",
        rusqlite::params![status, now, agent_id],
    )
    .context("Failed to end agent record")?;
    Ok(())
}

pub fn db_update_agent_session(conn: &Connection, agent_id: &str, session_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE agents SET session_id = ?1 WHERE id = ?2",
        rusqlite::params![session_id, agent_id],
    )
    .context("Failed to update agent session_id")?;
    Ok(())
}

pub fn db_list_agents_by_status(conn: &Connection, project_id: &str, status: &str) -> Result<Vec<AgentRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, skill_id, task_id, status, session_id, started_at, ended_at
         FROM agents WHERE project_id = ?1 AND status = ?2 ORDER BY started_at"
    )?;
    let rows = stmt
        .query_map(rusqlite::params![project_id, status], |row| {
            Ok(AgentRecord {
                id: row.get(0)?,
                project_id: row.get(1)?,
                skill_id: row.get(2)?,
                task_id: row.get(3)?,
                status: row.get(4)?,
                session_id: row.get(5)?,
                started_at: row.get(6)?,
                ended_at: row.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to list agents")?;
    Ok(rows)
}

// ── Phase helpers ─────────────────────────────────────────────────────────────

pub fn db_get_phase(conn: &Connection, phase_id: &str) -> Result<Phase> {
    conn.query_row(
        "SELECT id, project_id, number, title, lifecycle_stage, gate_held, created_at, updated_at
         FROM phases WHERE id = ?1",
        [phase_id],
        |row| {
            Ok(Phase {
                id: row.get(0)?,
                project_id: row.get(1)?,
                number: row.get(2)?,
                title: row.get(3)?,
                lifecycle_stage: parse_lifecycle_stage(row.get(4)?)?,
                gate_held: row.get::<_, i64>(5)? != 0,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        },
    )
    .with_context(|| format!("Phase not found: {}", phase_id))
}

pub fn db_advance_stage_gate(conn: &Connection, phase_id: &str) -> Result<Phase> {
    let phase = db_get_phase(conn, phase_id)?;
    let next_stage = match phase.lifecycle_stage {
        PhaseLifecycleStage::Planning => PhaseLifecycleStage::Execution,
        PhaseLifecycleStage::Execution => PhaseLifecycleStage::Retrospective,
        PhaseLifecycleStage::Retrospective => PhaseLifecycleStage::Complete,
        PhaseLifecycleStage::Complete => return Ok(phase),
    };
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE phases SET lifecycle_stage = ?1, gate_held = 0, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![next_stage.to_string(), now, phase_id],
    )
    .context("Failed to advance stage gate")?;
    db_get_phase(conn, phase_id)
}

/// Find all pending tasks whose dependencies are all complete and whose phase is in execution mode.
/// Tasks with no phase (e.g. CONOPS bootstrap tasks) are always eligible — they have no gate.
pub fn db_find_ready_tasks(conn: &Connection, project_id: &str) -> Result<Vec<Node>> {
    let mut stmt = conn.prepare(
        "SELECT n.id, n.project_id, n.phase_id, n.parent_id, n.node_type, n.title, n.description,
                n.status, n.skill_id, n.assignee, n.created_at, n.updated_at
         FROM nodes n
         LEFT JOIN phases p ON p.id = n.phase_id
         WHERE n.project_id = ?1
           AND n.status = 'pending'
           AND (n.phase_id IS NULL OR (p.lifecycle_stage = 'execution' AND p.gate_held = 0))
           AND n.node_type IN ('task', 'bug', 'chore', 'subtask')
           AND NOT EXISTS (
               SELECT 1 FROM edges e
               JOIN nodes dep ON dep.id = e.from_id
               WHERE e.to_id = n.id
                 AND e.edge_type = 'depends_on'
                 AND dep.status != 'complete'
           )"
    )?;
    let rows = stmt
        .query_map([project_id], |row| {
            Ok(Node {
                id: row.get(0)?,
                project_id: row.get(1)?,
                phase_id: row.get(2)?,
                parent_id: row.get(3)?,
                node_type: parse_node_type(row.get(4)?)?,
                title: row.get(5)?,
                description: row.get(6)?,
                status: parse_node_status(row.get(7)?)?,
                skill_id: row.get(8)?,
                assignee: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to find ready tasks")?;
    Ok(rows)
}

pub fn db_count_running_agents(conn: &Connection, project_id: &str) -> Result<usize> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM agents WHERE project_id = ?1 AND status = 'running'",
        [project_id],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

pub fn db_count_running_agents_global(conn: &Connection) -> Result<usize> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM agents WHERE status = 'running'",
        [],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}
