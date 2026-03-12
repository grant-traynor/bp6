pub mod commands;
pub mod schema;
pub mod types;

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

pub use types::*;

/// Apply a single migration SQL statement. Silently ignores "duplicate column"
/// errors (expected when re-running ADD COLUMN on an existing database). All
/// other errors are logged as warnings — migrations must not crash startup.
fn apply_migration(conn: &Connection, sql: &str) {
    if let Err(e) = conn.execute_batch(sql) {
        let msg = e.to_string().to_lowercase();
        if !msg.contains("duplicate column") {
            eprintln!("[dag_store] migration warning: {}", e);
        }
    }
}

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

    // Migrate: add promoted column to knowledge if not present (ignore error if exists)
    apply_migration(&conn, "ALTER TABLE knowledge ADD COLUMN promoted INTEGER NOT NULL DEFAULT 0");

    // Migrate: add new node columns for existing databases (SQLite < 3.37 has no IF NOT EXISTS
    // for ALTER TABLE ADD COLUMN, so we ignore the "duplicate column name" error).
    apply_migration(&conn, "ALTER TABLE nodes ADD COLUMN yield_reason TEXT");
    apply_migration(&conn, "ALTER TABLE nodes ADD COLUMN session_id TEXT");
    apply_migration(&conn, "ALTER TABLE nodes ADD COLUMN requesting_task_id TEXT REFERENCES nodes(id)");
    // Add index for requesting_task_id (CREATE INDEX IF NOT EXISTS is safe to re-run).
    apply_migration(&conn, "CREATE INDEX IF NOT EXISTS idx_nodes_requesting_task_id ON nodes(requesting_task_id)");
    apply_migration(&conn, "ALTER TABLE nodes ADD COLUMN review_id TEXT");
    apply_migration(&conn, "ALTER TABLE nodes ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0");

    // Migrate: create chat_turns table for existing databases.
    // NOTE: task_id has a REFERENCES nodes(id) FK in the CREATE TABLE schema; SQLite does not
    // support adding FK constraints via ALTER TABLE, so the constraint is enforced on new rows
    // via the schema definition and this migration simply ensures the table exists on older DBs.
    apply_migration(&conn,
        "CREATE TABLE IF NOT EXISTS chat_turns (
            id          TEXT PRIMARY KEY,
            task_id     TEXT NOT NULL,
            content     TEXT NOT NULL,
            response    TEXT,
            created_at  TEXT NOT NULL,
            responded_at TEXT
        )"
    );
    apply_migration(&conn, "CREATE INDEX IF NOT EXISTS idx_chat_turns_task ON chat_turns(task_id)");

    // Phase 3 migrations
    apply_migration(&conn, "ALTER TABLE phases ADD COLUMN stage_type TEXT NOT NULL DEFAULT 'execution'");
    apply_migration(&conn, "ALTER TABLE phases ADD COLUMN status TEXT NOT NULL DEFAULT 'pending'");
    apply_migration(&conn, "ALTER TABLE nodes ADD COLUMN sort_order INTEGER");
    apply_migration(&conn, "ALTER TABLE nodes ADD COLUMN skill_modes TEXT");

    // u7s.4: verdict column stores poe:review-outcome verdict on reviewer nodes.
    apply_migration(&conn, "ALTER TABLE nodes ADD COLUMN verdict TEXT");

    // Migrate: create advisor_turns table for existing databases
    apply_migration(&conn,
        "CREATE TABLE IF NOT EXISTS advisor_turns (
            id           TEXT PRIMARY KEY,
            task_id      TEXT NOT NULL REFERENCES nodes(id),
            content      TEXT NOT NULL,
            response     TEXT,
            created_at   TEXT NOT NULL,
            responded_at TEXT
        )"
    );
    apply_migration(&conn, "CREATE INDEX IF NOT EXISTS idx_advisor_turns_task ON advisor_turns(task_id)");

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
    let id = input.id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let now = chrono::Utc::now().to_rfc3339();
    let status = input.initial_status.as_ref()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "pending".to_owned());
    let retry_count = input.retry_count.unwrap_or(0);
    conn.execute(
        "INSERT INTO nodes (id, project_id, phase_id, parent_id, node_type, title, description, status, skill_id, requesting_task_id, review_id, retry_count, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        rusqlite::params![
            id, input.project_id, input.phase_id, input.parent_id,
            input.node_type.to_string(), input.title, input.description,
            status, input.skill_id, input.requesting_task_id, input.review_id,
            retry_count, now, now
        ],
    )
    .context("Failed to insert node")?;
    db_get_node(conn, &id)
}

pub fn db_get_node(conn: &Connection, node_id: &str) -> Result<Node> {
    conn.query_row(
        "SELECT id, project_id, phase_id, parent_id, node_type, title, description, status, skill_id, assignee, yield_reason, session_id, requesting_task_id, review_id, retry_count, sort_order, skill_modes, verdict, created_at, updated_at
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
                yield_reason: row.get(10)?,
                session_id: row.get(11)?,
                requesting_task_id: row.get(12)?,
                review_id: row.get(13)?,
                retry_count: row.get(14)?,
                sort_order: row.get(15)?,
                skill_modes: row.get(16)?,
                verdict: row.get(17)?,
                created_at: row.get(18)?,
                updated_at: row.get(19)?,
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
    if let Some(ref yield_reason) = input.yield_reason {
        conn.execute("UPDATE nodes SET yield_reason = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![yield_reason, now, node_id])?;
    }
    if let Some(ref session_id) = input.session_id {
        conn.execute("UPDATE nodes SET session_id = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![session_id, now, node_id])?;
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
            "SELECT id, project_id, phase_id, parent_id, node_type, title, description, status, skill_id, assignee, yield_reason, session_id, requesting_task_id, review_id, retry_count, sort_order, skill_modes, verdict, created_at, updated_at
             FROM nodes WHERE project_id = ?1 AND phase_id = ?2 ORDER BY COALESCE(sort_order, 999999), created_at"
        )?;
        for row in stmt.query_map(rusqlite::params![project_id, pid], |row| {
            Ok((row.get::<_,String>(0)?, row.get::<_,String>(1)?, row.get::<_,Option<String>>(2)?,
                row.get::<_,Option<String>>(3)?, row.get::<_,String>(4)?, row.get::<_,String>(5)?,
                row.get::<_,Option<String>>(6)?, row.get::<_,String>(7)?, row.get::<_,Option<String>>(8)?,
                row.get::<_,Option<String>>(9)?, row.get::<_,Option<String>>(10)?,
                row.get::<_,Option<String>>(11)?, row.get::<_,Option<String>>(12)?,
                row.get::<_,Option<String>>(13)?, row.get::<_,i64>(14)?,
                row.get::<_,Option<i64>>(15)?, row.get::<_,Option<String>>(16)?,
                row.get::<_,Option<String>>(17)?,
                row.get::<_,String>(18)?, row.get::<_,String>(19)?))
        })? {
            let (id, proj, phase, parent, nt, title, desc, status, skill, assignee,
                 yield_reason, session_id, requesting_task_id, review_id, retry_count,
                 sort_order, skill_modes, verdict, cat, uat) = row?;
            rows.push(Node {
                id, project_id: proj, phase_id: phase, parent_id: parent,
                node_type: parse_node_type(nt)?, title, description: desc,
                status: parse_node_status(status)?, skill_id: skill, assignee,
                yield_reason, session_id, requesting_task_id, review_id, retry_count,
                sort_order, skill_modes, verdict, created_at: cat, updated_at: uat,
            });
        }
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, project_id, phase_id, parent_id, node_type, title, description, status, skill_id, assignee, yield_reason, session_id, requesting_task_id, review_id, retry_count, sort_order, skill_modes, verdict, created_at, updated_at
             FROM nodes WHERE project_id = ?1 ORDER BY COALESCE(sort_order, 999999), created_at"
        )?;
        for row in stmt.query_map([project_id], |row| {
            Ok((row.get::<_,String>(0)?, row.get::<_,String>(1)?, row.get::<_,Option<String>>(2)?,
                row.get::<_,Option<String>>(3)?, row.get::<_,String>(4)?, row.get::<_,String>(5)?,
                row.get::<_,Option<String>>(6)?, row.get::<_,String>(7)?, row.get::<_,Option<String>>(8)?,
                row.get::<_,Option<String>>(9)?, row.get::<_,Option<String>>(10)?,
                row.get::<_,Option<String>>(11)?, row.get::<_,Option<String>>(12)?,
                row.get::<_,Option<String>>(13)?, row.get::<_,i64>(14)?,
                row.get::<_,Option<i64>>(15)?, row.get::<_,Option<String>>(16)?,
                row.get::<_,Option<String>>(17)?,
                row.get::<_,String>(18)?, row.get::<_,String>(19)?))
        })? {
            let (id, proj, phase, parent, nt, title, desc, status, skill, assignee,
                 yield_reason, session_id, requesting_task_id, review_id, retry_count,
                 sort_order, skill_modes, verdict, cat, uat) = row?;
            rows.push(Node {
                id, project_id: proj, phase_id: phase, parent_id: parent,
                node_type: parse_node_type(nt)?, title, description: desc,
                status: parse_node_status(status)?, skill_id: skill, assignee,
                yield_reason, session_id, requesting_task_id, review_id, retry_count,
                sort_order, skill_modes, verdict, created_at: cat, updated_at: uat,
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

/// Count unresolved queue items for a given task.
pub fn db_count_unresolved_queue_items_for_task(conn: &Connection, task_id: &str) -> Result<usize> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM queue_items WHERE task_id = ?1 AND resolved_at IS NULL",
        [task_id],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

/// Atomically increment retry_count for a reviewer task (watchdog use).
pub fn db_increment_retry_count(conn: &Connection, node_id: &str) -> Result<i64> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE nodes SET retry_count = retry_count + 1, updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, node_id],
    )
    .context("Failed to increment retry_count")?;
    let count: i64 = conn.query_row(
        "SELECT retry_count FROM nodes WHERE id = ?1",
        [node_id],
        |row| row.get(0),
    )
    .with_context(|| format!("Node not found after retry increment: {}", node_id))?;
    Ok(count)
}

/// Write session_id to nodes.session_id — canonical location per Protocol.md §1.
pub fn db_update_node_session(conn: &Connection, task_id: &str, session_id: &str) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE nodes SET session_id = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![session_id, now, task_id],
    )
    .context("Failed to update nodes.session_id")?;
    Ok(())
}

/// Get the session_id for a task. Reads from nodes.session_id (canonical) first,
/// falling back to agents.session_id for backwards compatibility.
pub fn db_get_session_id_for_task(conn: &Connection, task_id: &str) -> Result<Option<String>> {
    // Try nodes.session_id first (canonical per Protocol.md §1).
    let node_result: rusqlite::Result<Option<String>> = conn.query_row(
        "SELECT session_id FROM nodes WHERE id = ?1",
        [task_id],
        |row| row.get(0),
    );
    if let Ok(Some(sid)) = node_result {
        return Ok(Some(sid));
    }

    // Fallback: agents table (legacy path).
    let agent_result: rusqlite::Result<Option<String>> = conn.query_row(
        "SELECT session_id FROM agents WHERE task_id = ?1 ORDER BY started_at DESC LIMIT 1",
        [task_id],
        |row| row.get(0),
    );
    match agent_result {
        Ok(v) => Ok(v),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Get the most recent session_id from the agents table for a given task.
/// Kept for backwards compatibility; prefer db_get_session_id_for_task.
pub fn db_get_agent_session_for_task(conn: &Connection, task_id: &str) -> Result<Option<String>> {
    let result: rusqlite::Result<Option<String>> = conn.query_row(
        "SELECT session_id FROM agents WHERE task_id = ?1 ORDER BY started_at DESC LIMIT 1",
        [task_id],
        |row| row.get(0),
    );
    match result {
        Ok(v) => Ok(v),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// List all phases for a project ordered by number.
pub fn db_list_phases(conn: &Connection, project_id: &str) -> Result<Vec<Phase>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, number, title, lifecycle_stage, gate_held, stage_type, status, created_at, updated_at
         FROM phases WHERE project_id = ?1 ORDER BY number"
    )?;
    let rows = stmt
        .query_map([project_id], |row| {
            Ok(Phase {
                id: row.get(0)?,
                project_id: row.get(1)?,
                number: row.get(2)?,
                title: row.get(3)?,
                lifecycle_stage: parse_lifecycle_stage(row.get(4)?)?,
                gate_held: row.get::<_, i64>(5)? != 0,
                stage_type: row.get::<_, Option<String>>(6)?.unwrap_or_else(|| "execution".to_owned()),
                status: row.get::<_, Option<String>>(7)?.unwrap_or_else(|| "pending".to_owned()),
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to list phases")?;
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
        "SELECT id, project_id, number, title, lifecycle_stage, gate_held, stage_type, status, created_at, updated_at
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
                stage_type: row.get::<_, Option<String>>(6)?.unwrap_or_else(|| "execution".to_owned()),
                status: row.get::<_, Option<String>>(7)?.unwrap_or_else(|| "pending".to_owned()),
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
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
                n.status, n.skill_id, n.assignee, n.yield_reason, n.session_id, n.requesting_task_id,
                n.review_id, n.retry_count, n.sort_order, n.skill_modes, n.verdict, n.created_at, n.updated_at
         FROM nodes n
         LEFT JOIN phases p ON p.id = n.phase_id
         WHERE n.project_id = ?1
           AND n.status = 'pending'
           AND (n.phase_id IS NULL OR (p.lifecycle_stage = 'execution' AND p.gate_held = 0)
                OR (n.phase_id IS NOT NULL AND p.status = 'running' AND p.gate_held = 0))
           AND n.node_type IN ('task', 'bug', 'chore', 'subtask', 'plan_review', 'advisor')
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
                yield_reason: row.get(10)?,
                session_id: row.get(11)?,
                requesting_task_id: row.get(12)?,
                review_id: row.get(13)?,
                retry_count: row.get(14)?,
                sort_order: row.get(15)?,
                skill_modes: row.get(16)?,
                verdict: row.get(17)?,
                created_at: row.get(18)?,
                updated_at: row.get(19)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to find ready tasks")?;
    Ok(rows)
}

/// Query all `poe:review` events logged for a given task.
/// Returns a list of (review_id, reviewer_skill) pairs parsed from event payloads.
/// Events without a valid `id` field are skipped (single-reviewer path — id is optional per Protocol.md §2).
pub fn db_list_review_events_for_task(conn: &Connection, task_id: &str) -> Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT payload FROM events WHERE task_id = ?1 AND event_type = 'poe:review' ORDER BY created_at"
    )?;
    let mut results = Vec::new();
    for row in stmt.query_map([task_id], |row| row.get::<_, String>(0))? {
        let payload_str = row?;
        let payload: serde_json::Value = serde_json::from_str(&payload_str).unwrap_or_default();
        if let (Some(id), Some(skill)) = (
            payload.get("id").and_then(|v| v.as_str()).map(str::to_owned),
            payload.get("reviewer_skill").and_then(|v| v.as_str()).map(str::to_owned),
        ) {
            results.push((id, skill));
        }
    }
    Ok(results)
}

/// Check reviewer completion for a waiting task.
/// Returns (expected_ids, answered_ids) where answered_ids = review_id values
/// from reviewer nodes with status=complete or status=cancelled.
pub fn db_reviewer_completion_status(conn: &Connection, requesting_task_id: &str) -> Result<(Vec<String>, Vec<String>)> {
    // answered_ids: reviewer nodes that have finished (done or cancelled)
    let mut stmt = conn.prepare(
        "SELECT review_id FROM nodes WHERE requesting_task_id = ?1 AND status IN ('complete', 'cancelled') AND review_id IS NOT NULL"
    )?;
    let answered: Vec<String> = stmt
        .query_map([requesting_task_id], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to query answered reviewer nodes")?;

    // expected_ids: all reviewer nodes created for this task
    let mut stmt2 = conn.prepare(
        "SELECT review_id FROM nodes WHERE requesting_task_id = ?1 AND review_id IS NOT NULL"
    )?;
    let expected: Vec<String> = stmt2
        .query_map([requesting_task_id], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to query expected reviewer nodes")?;

    Ok((expected, answered))
}

/// List all nodes for a project with a given status string (e.g. "waiting", "running").
pub fn db_list_nodes_by_status(conn: &Connection, project_id: &str, status: &str) -> Result<Vec<Node>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, phase_id, parent_id, node_type, title, description, status,
                skill_id, assignee, yield_reason, session_id, requesting_task_id, review_id,
                retry_count, sort_order, skill_modes, verdict, created_at, updated_at
         FROM nodes WHERE project_id = ?1 AND status = ?2 ORDER BY created_at"
    )?;
    let rows = stmt
        .query_map(rusqlite::params![project_id, status], |row| {
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
                yield_reason: row.get(10)?,
                session_id: row.get(11)?,
                requesting_task_id: row.get(12)?,
                review_id: row.get(13)?,
                retry_count: row.get(14)?,
                sort_order: row.get(15)?,
                skill_modes: row.get(16)?,
                verdict: row.get(17)?,
                created_at: row.get(18)?,
                updated_at: row.get(19)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to list nodes by status")?;
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

/// u7s.3 — Return all agent rows with status='running' whose agent_id is NOT in the
/// provided set of live agent IDs. These are ghost agents: rows that were never
/// closed because the app crashed before `db_end_agent` was called.
///
/// The caller is responsible for marking them failed and optionally resetting their
/// associated node back to pending.
pub fn db_list_ghost_agents(
    conn: &Connection,
    project_id: &str,
    live_agent_ids: &std::collections::HashSet<String>,
) -> Result<Vec<AgentRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, project_id, skill_id, task_id, status, session_id, started_at, ended_at
         FROM agents WHERE project_id = ?1 AND status = 'running' ORDER BY started_at"
    )?;
    let rows = stmt
        .query_map(rusqlite::params![project_id], |row| {
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
        .context("Failed to list running agents for ghost check")?;
    // Filter to only those not present in the live set
    Ok(rows.into_iter().filter(|a| !live_agent_ids.contains(&a.id)).collect())
}

// ── Chat turn helpers ─────────────────────────────────────────────────────────

pub fn db_insert_chat_turn(conn: &Connection, id: &str, task_id: &str, content: &str) -> Result<ChatTurn> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO chat_turns (id, task_id, content, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![id, task_id, content, now],
    )
    .context("Failed to insert chat turn")?;
    Ok(ChatTurn {
        id: id.to_owned(),
        task_id: task_id.to_owned(),
        content: content.to_owned(),
        response: None,
        created_at: now,
        responded_at: None,
    })
}

pub fn db_list_chat_turns(conn: &Connection, task_id: &str) -> Result<Vec<ChatTurn>> {
    let mut stmt = conn.prepare(
        "SELECT id, task_id, content, response, created_at, responded_at
         FROM chat_turns WHERE task_id = ?1 ORDER BY created_at ASC"
    )?;
    let rows = stmt
        .query_map([task_id], |row| {
            Ok(ChatTurn {
                id: row.get(0)?,
                task_id: row.get(1)?,
                content: row.get(2)?,
                response: row.get(3)?,
                created_at: row.get(4)?,
                responded_at: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to list chat turns")?;
    Ok(rows)
}

pub fn db_count_running_agents_global(conn: &Connection) -> Result<usize> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM agents WHERE status = 'running'",
        [],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

// ── Phase 3 helpers ───────────────────────────────────────────────────────────

/// List all edges for a project by joining with nodes to get project_id.
pub fn db_list_edges(conn: &Connection, project_id: &str) -> Result<Vec<Edge>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.from_id, e.to_id, e.edge_type, e.created_at
         FROM edges e
         JOIN nodes n ON n.id = e.from_id
         WHERE n.project_id = ?1"
    )?;
    let rows = stmt
        .query_map([project_id], |row| {
            Ok(Edge {
                id: row.get(0)?,
                from_id: row.get(1)?,
                to_id: row.get(2)?,
                edge_type: parse_edge_type(row.get(3)?)?,
                created_at: row.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to list edges")?;
    Ok(rows)
}

/// Update sort_order on a node (used by Matrix drag-to-reorder).
pub fn db_update_node_sort_order(conn: &Connection, node_id: &str, sort_order: Option<i32>) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE nodes SET sort_order = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![sort_order, now, node_id],
    )
    .context("Failed to update sort_order")?;
    Ok(())
}

/// Walk parent_id chain to root, returning [root, ..., parent, node] ordered root-first.
pub fn db_get_node_ancestry(conn: &Connection, node_id: &str) -> Result<Vec<Node>> {
    // Walk up the chain collecting nodes
    let mut chain = Vec::new();
    let mut current_id = Some(node_id.to_owned());
    while let Some(id) = current_id {
        let node = db_get_node(conn, &id)?;
        current_id = node.parent_id.clone();
        chain.push(node);
    }
    // chain is [node, parent, ..., root]; reverse to get root-first
    chain.reverse();
    Ok(chain)
}

/// Insert into advisor_turns.
pub fn db_insert_advisor_turn(conn: &Connection, id: &str, task_id: &str, content: &str) -> Result<AdvisorTurn> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO advisor_turns (id, task_id, content, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![id, task_id, content, now],
    )
    .context("Failed to insert advisor turn")?;
    Ok(AdvisorTurn {
        id: id.to_owned(),
        task_id: task_id.to_owned(),
        content: content.to_owned(),
        response: None,
        created_at: now,
        responded_at: None,
    })
}

/// List advisor_turns for a task ordered by created_at.
pub fn db_list_advisor_turns(conn: &Connection, task_id: &str) -> Result<Vec<AdvisorTurn>> {
    let mut stmt = conn.prepare(
        "SELECT id, task_id, content, response, created_at, responded_at
         FROM advisor_turns WHERE task_id = ?1 ORDER BY created_at ASC"
    )?;
    let rows = stmt
        .query_map([task_id], |row| {
            Ok(AdvisorTurn {
                id: row.get(0)?,
                task_id: row.get(1)?,
                content: row.get(2)?,
                response: row.get(3)?,
                created_at: row.get(4)?,
                responded_at: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("Failed to list advisor turns")?;
    Ok(rows)
}

/// Get the responded advisor turn for a waiting task (used by SF-4 advisor arm).
pub fn db_get_responded_advisor_turn(conn: &Connection, task_id: &str) -> Result<Option<AdvisorTurn>> {
    let result = conn.query_row(
        "SELECT id, task_id, content, response, created_at, responded_at
         FROM advisor_turns WHERE task_id = ?1 AND responded_at IS NOT NULL
         ORDER BY responded_at DESC LIMIT 1",
        [task_id],
        |row| {
            Ok(AdvisorTurn {
                id: row.get(0)?,
                task_id: row.get(1)?,
                content: row.get(2)?,
                response: row.get(3)?,
                created_at: row.get(4)?,
                responded_at: row.get(5)?,
            })
        },
    );
    match result {
        Ok(turn) => Ok(Some(turn)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Create a phase with stage_type and status fields.
pub fn db_create_phase(
    conn: &Connection,
    project_id: &str,
    title: &str,
    number: i64,
    stage_type: &str,
) -> Result<Phase> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO phases (id, project_id, number, title, lifecycle_stage, gate_held, stage_type, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'planning', 0, ?5, 'pending', ?6, ?7)",
        rusqlite::params![id, project_id, number, title, stage_type, now, now],
    )
    .context("Failed to insert phase")?;
    db_get_phase(conn, &id)
}

/// Update skill_modes on a node (written at SF-1 dispatch time).
pub fn db_update_node_skill_modes(conn: &Connection, node_id: &str, skill_modes_json: &str) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE nodes SET skill_modes = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![skill_modes_json, now, node_id],
    )
    .context("Failed to update skill_modes")?;
    Ok(())
}

/// Get a knowledge entry by id.
pub fn db_get_knowledge(conn: &Connection, id: &str) -> Result<KnowledgeEntry> {
    conn.query_row(
        "SELECT id, project_id, key, value, source, supersedes_id, created_at
         FROM knowledge WHERE id = ?1",
        [id],
        |row| {
            Ok(KnowledgeEntry {
                id: row.get(0)?,
                project_id: row.get(1)?,
                key: row.get(2)?,
                value: row.get(3)?,
                source: row.get(4)?,
                supersedes_id: row.get(5)?,
                created_at: row.get(6)?,
            })
        },
    )
    .with_context(|| format!("Knowledge entry not found: {}", id))
}

/// Check whether an advisor_turns row exists for a given id.
pub fn db_advisor_turn_exists(conn: &Connection, turn_id: &str) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM advisor_turns WHERE id = ?1",
        rusqlite::params![turn_id],
        |row| row.get::<_, i64>(0),
    )
    .unwrap_or(0) > 0
}

/// Get all done/cancelled task IDs in a phase.
pub fn db_count_active_tasks_in_phase(conn: &Connection, phase_id: &str) -> Result<(usize, usize)> {
    // (total non-cancelled tasks, done tasks)
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM nodes WHERE phase_id = ?1 AND status != 'cancelled' AND node_type IN ('task','bug','chore','subtask','plan_review','advisor')",
        [phase_id], |row| row.get(0),
    ).unwrap_or(0);
    let done: i64 = conn.query_row(
        "SELECT COUNT(*) FROM nodes WHERE phase_id = ?1 AND status IN ('complete', 'done') AND node_type IN ('task','bug','chore','subtask','plan_review','advisor')",
        [phase_id], |row| row.get(0),
    ).unwrap_or(0);
    Ok((total as usize, done as usize))
}

/// u7s.1 — Atomically claim a waiting node for resume by transitioning it from
/// `waiting` → `resuming`. Returns `Ok(true)` if this caller won the claim
/// (rows_changed == 1), `Ok(false)` if another caller already claimed it
/// (rows_changed == 0 means the node was not in `waiting` status).
pub fn db_claim_node_resuming(conn: &Connection, node_id: &str) -> Result<bool> {
    let now = chrono::Utc::now().to_rfc3339();
    let rows = conn.execute(
        "UPDATE nodes SET status = 'resuming', updated_at = ?1 WHERE id = ?2 AND status = 'waiting'",
        rusqlite::params![now, node_id],
    )?;
    Ok(rows == 1)
}

/// u7s.2 — Atomically claim a running node for retry by transitioning it from
/// `running` → `pending` and incrementing retry_count. Returns `Ok(true)` if
/// this caller won (rows_changed == 1), `Ok(false)` if another path already
/// handled it (node was not in `running` status).
pub fn db_claim_node_retry(conn: &Connection, node_id: &str) -> Result<bool> {
    let now = chrono::Utc::now().to_rfc3339();
    let rows = conn.execute(
        "UPDATE nodes SET status = 'pending', retry_count = retry_count + 1, updated_at = ?1 WHERE id = ?2 AND status = 'running'",
        rusqlite::params![now, node_id],
    )?;
    Ok(rows == 1)
}

/// u7s.5 — Returns the parent_id of a node, or None if the node has no parent
/// or the node does not exist.
pub fn db_get_node_parent(conn: &Connection, node_id: &str) -> Result<Option<String>> {
    let result: rusqlite::Result<Option<String>> = conn.query_row(
        "SELECT parent_id FROM nodes WHERE id = ?1",
        [node_id],
        |row| row.get(0),
    );
    match result {
        Ok(v) => Ok(v),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// u7s.5 — Returns true if all children of `parent_id` are in a terminal state
/// (`complete` or `cancelled`). Returns false if any child is non-terminal,
/// or if there are no children at all (empty container does not auto-close).
pub fn db_all_children_terminal(conn: &Connection, parent_id: &str) -> Result<bool> {
    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM nodes WHERE parent_id = ?1",
        [parent_id],
        |row| row.get(0),
    )?;
    if total == 0 {
        return Ok(false); // no children → do not auto-close
    }
    let non_terminal: i64 = conn.query_row(
        "SELECT COUNT(*) FROM nodes WHERE parent_id = ?1 AND status NOT IN ('complete', 'cancelled')",
        [parent_id],
        |row| row.get(0),
    )?;
    Ok(non_terminal == 0)
}

// ── Verdict helpers (u7s.4) ───────────────────────────────────────────────────

/// u7s.4 — Store the reviewer's verdict on the node.
/// Called by the `poe:review-outcome` event handler.
/// verdict must be one of: APPROVED | APPROVED_WITH_CONDITIONS | BLOCKED | FAILED
pub fn db_set_node_verdict(conn: &Connection, node_id: &str, verdict: &str) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE nodes SET verdict = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![verdict, now, node_id],
    )
    .context("Failed to set node verdict")?;
    Ok(())
}

/// u7s.4 — Read the stored verdict for a reviewer node.
/// Returns None if the node has no verdict (poe:review-outcome not yet received).
pub fn db_get_node_verdict(conn: &Connection, node_id: &str) -> Result<Option<String>> {
    let result: rusqlite::Result<Option<String>> = conn.query_row(
        "SELECT verdict FROM nodes WHERE id = ?1",
        [node_id],
        |row| row.get(0),
    );
    match result {
        Ok(v) => Ok(v),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}
