use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use chrono::Utc;
use uuid::Uuid;

// ── Node & Edge types ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum NodeType {
    Project,
    Epic,
    Feature,
    Task,
    Decision,
    KnowledgeArtifact,
    AgentOutput,
    Review,
}

impl NodeType {
    pub fn as_str(&self) -> &str {
        match self {
            NodeType::Project => "Project",
            NodeType::Epic => "Epic",
            NodeType::Feature => "Feature",
            NodeType::Task => "Task",
            NodeType::Decision => "Decision",
            NodeType::KnowledgeArtifact => "KnowledgeArtifact",
            NodeType::AgentOutput => "AgentOutput",
            NodeType::Review => "Review",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "Project" => Ok(NodeType::Project),
            "Epic" => Ok(NodeType::Epic),
            "Feature" => Ok(NodeType::Feature),
            "Task" => Ok(NodeType::Task),
            "Decision" => Ok(NodeType::Decision),
            "KnowledgeArtifact" => Ok(NodeType::KnowledgeArtifact),
            "AgentOutput" => Ok(NodeType::AgentOutput),
            "Review" => Ok(NodeType::Review),
            other => Err(format!("Unknown node type: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeType {
    Blocks,
    DependsOn,
    GeneratedBy,
    ApprovedBy,
    DiscoveredFrom,
    Implements,
    Tests,
    Contradicts,
}

impl EdgeType {
    pub fn as_str(&self) -> &str {
        match self {
            EdgeType::Blocks => "blocks",
            EdgeType::DependsOn => "depends-on",
            EdgeType::GeneratedBy => "generated-by",
            EdgeType::ApprovedBy => "approved-by",
            EdgeType::DiscoveredFrom => "discovered-from",
            EdgeType::Implements => "implements",
            EdgeType::Tests => "tests",
            EdgeType::Contradicts => "contradicts",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "blocks" => Ok(EdgeType::Blocks),
            "depends-on" => Ok(EdgeType::DependsOn),
            "generated-by" => Ok(EdgeType::GeneratedBy),
            "approved-by" => Ok(EdgeType::ApprovedBy),
            "discovered-from" => Ok(EdgeType::DiscoveredFrom),
            "implements" => Ok(EdgeType::Implements),
            "tests" => Ok(EdgeType::Tests),
            "contradicts" => Ok(EdgeType::Contradicts),
            other => Err(format!("Unknown edge type: {}", other)),
        }
    }
}

// ── Data structures ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DagNode {
    pub id: String,
    pub node_type: String,
    pub project_id: String,
    pub data: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DagEdge {
    pub id: String,
    pub from_id: String,
    pub to_id: String,
    pub edge_type: String,
    pub data: serde_json::Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DagSnapshot {
    pub nodes: Vec<DagNode>,
    pub edges: Vec<DagEdge>,
}

// ── Schema migrations ──────────────────────────────────────────────────────────

const MIGRATIONS: &[&str] = &[
    // v1: initial schema
    r#"
    CREATE TABLE IF NOT EXISTS schema_migrations (
        version INTEGER PRIMARY KEY,
        applied_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS nodes (
        id          TEXT PRIMARY KEY,
        type        TEXT NOT NULL,
        project_id  TEXT NOT NULL,
        data        TEXT NOT NULL DEFAULT '{}',
        created_at  TEXT NOT NULL,
        updated_at  TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_nodes_project ON nodes(project_id);
    CREATE INDEX IF NOT EXISTS idx_nodes_type    ON nodes(type);

    CREATE TABLE IF NOT EXISTS edges (
        id          TEXT PRIMARY KEY,
        from_id     TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
        to_id       TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
        type        TEXT NOT NULL,
        data        TEXT NOT NULL DEFAULT '{}',
        created_at  TEXT NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_edges_from ON edges(from_id);
    CREATE INDEX IF NOT EXISTS idx_edges_to   ON edges(to_id);
    CREATE INDEX IF NOT EXISTS idx_edges_type ON edges(type);
    "#,
];

// ── DagStore ───────────────────────────────────────────────────────────────────

pub struct DagStore {
    conn: Connection,
}

impl DagStore {
    /// Open or create the DAG database at `path`. Enables WAL mode and runs migrations.
    pub fn open(path: &std::path::Path) -> Result<Self, String> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create .poe dir: {}", e))?;
        }

        let conn = Connection::open(path)
            .map_err(|e| format!("Failed to open database at {}: {}", path.display(), e))?;

        // WAL mode for concurrent reads during writes
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| format!("Failed to set PRAGMA: {}", e))?;

        let mut store = DagStore { conn };
        store.run_migrations()?;
        Ok(store)
    }

    fn run_migrations(&mut self) -> Result<(), String> {
        // Create migrations table first if it doesn't exist
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS schema_migrations (
                    version    INTEGER PRIMARY KEY,
                    applied_at TEXT NOT NULL
                );",
            )
            .map_err(|e| format!("Failed to create migrations table: {}", e))?;

        let applied: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .map_err(|e| format!("Failed to query migrations: {}", e))?;

        for (i, migration) in MIGRATIONS.iter().enumerate() {
            let version = (i + 1) as i64;
            if version > applied {
                self.conn
                    .execute_batch(migration)
                    .map_err(|e| format!("Migration v{} failed: {}", version, e))?;
                self.conn
                    .execute(
                        "INSERT OR IGNORE INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                        params![version, Utc::now().to_rfc3339()],
                    )
                    .map_err(|e| format!("Failed to record migration v{}: {}", version, e))?;
            }
        }
        Ok(())
    }

    // ── Node CRUD ──────────────────────────────────────────────────────────────

    pub fn upsert_node(
        &self,
        node_type: &NodeType,
        project_id: &str,
        data: serde_json::Value,
    ) -> Result<DagNode, String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let data_str = serde_json::to_string(&data)
            .map_err(|e| format!("Failed to serialize node data: {}", e))?;

        self.conn
            .execute(
                "INSERT INTO nodes (id, type, project_id, data, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![id, node_type.as_str(), project_id, data_str, now],
            )
            .map_err(|e| format!("Failed to insert node: {}", e))?;

        Ok(DagNode {
            id,
            node_type: node_type.as_str().to_string(),
            project_id: project_id.to_string(),
            data,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn update_node(&self, id: &str, data: serde_json::Value) -> Result<DagNode, String> {
        let now = Utc::now().to_rfc3339();
        let data_str = serde_json::to_string(&data)
            .map_err(|e| format!("Failed to serialize node data: {}", e))?;

        let rows = self
            .conn
            .execute(
                "UPDATE nodes SET data = ?1, updated_at = ?2 WHERE id = ?3",
                params![data_str, now, id],
            )
            .map_err(|e| format!("Failed to update node: {}", e))?;

        if rows == 0 {
            return Err(format!("Node not found: {}", id));
        }

        self.get_node(id)
    }

    pub fn get_node(&self, id: &str) -> Result<DagNode, String> {
        self.conn
            .query_row(
                "SELECT id, type, project_id, data, created_at, updated_at FROM nodes WHERE id = ?1",
                params![id],
                |row| {
                    let data_str: String = row.get(3)?;
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, data_str, row.get::<_, String>(4)?, row.get::<_, String>(5)?))
                },
            )
            .map_err(|e| format!("Failed to get node {}: {}", id, e))
            .and_then(|(id, node_type, project_id, data_str, created_at, updated_at)| {
                let data = serde_json::from_str(&data_str)
                    .map_err(|e| format!("Failed to parse node data: {}", e))?;
                Ok(DagNode { id, node_type, project_id, data, created_at, updated_at })
            })
    }

    pub fn delete_node(&self, id: &str) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM nodes WHERE id = ?1", params![id])
            .map_err(|e| format!("Failed to delete node {}: {}", id, e))?;
        Ok(())
    }

    pub fn list_nodes(&self, project_id: &str) -> Result<Vec<DagNode>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, type, project_id, data, created_at, updated_at
                 FROM nodes WHERE project_id = ?1 ORDER BY created_at ASC",
            )
            .map_err(|e| format!("Failed to prepare nodes query: {}", e))?;

        let nodes = stmt
            .query_map(params![project_id], |row| {
                let data_str: String = row.get(3)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    data_str,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })
            .map_err(|e| format!("Failed to query nodes: {}", e))?
            .filter_map(|r| r.ok())
            .filter_map(|(id, node_type, project_id, data_str, created_at, updated_at)| {
                serde_json::from_str(&data_str).ok().map(|data| DagNode {
                    id,
                    node_type,
                    project_id,
                    data,
                    created_at,
                    updated_at,
                })
            })
            .collect();

        Ok(nodes)
    }

    // ── Edge CRUD ──────────────────────────────────────────────────────────────

    pub fn add_edge(
        &self,
        from_id: &str,
        to_id: &str,
        edge_type: &EdgeType,
        data: serde_json::Value,
    ) -> Result<DagEdge, String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let data_str = serde_json::to_string(&data)
            .map_err(|e| format!("Failed to serialize edge data: {}", e))?;

        self.conn
            .execute(
                "INSERT INTO edges (id, from_id, to_id, type, data, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, from_id, to_id, edge_type.as_str(), data_str, now],
            )
            .map_err(|e| format!("Failed to insert edge: {}", e))?;

        Ok(DagEdge {
            id,
            from_id: from_id.to_string(),
            to_id: to_id.to_string(),
            edge_type: edge_type.as_str().to_string(),
            data,
            created_at: now,
        })
    }

    pub fn delete_edge(&self, id: &str) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM edges WHERE id = ?1", params![id])
            .map_err(|e| format!("Failed to delete edge {}: {}", id, e))?;
        Ok(())
    }

    pub fn list_edges(&self, project_id: &str) -> Result<Vec<DagEdge>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT e.id, e.from_id, e.to_id, e.type, e.data, e.created_at
                 FROM edges e
                 JOIN nodes n ON e.from_id = n.id
                 WHERE n.project_id = ?1
                 ORDER BY e.created_at ASC",
            )
            .map_err(|e| format!("Failed to prepare edges query: {}", e))?;

        let edges = stmt
            .query_map(params![project_id], |row| {
                let data_str: String = row.get(4)?;
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    data_str,
                    row.get::<_, String>(5)?,
                ))
            })
            .map_err(|e| format!("Failed to query edges: {}", e))?
            .filter_map(|r| r.ok())
            .filter_map(|(id, from_id, to_id, edge_type, data_str, created_at)| {
                serde_json::from_str(&data_str).ok().map(|data| DagEdge {
                    id,
                    from_id,
                    to_id,
                    edge_type,
                    data,
                    created_at,
                })
            })
            .collect();

        Ok(edges)
    }

    /// Load entire project snapshot (all nodes + edges) in one pass.
    pub fn snapshot(&self, project_id: &str) -> Result<DagSnapshot, String> {
        Ok(DagSnapshot {
            nodes: self.list_nodes(project_id)?,
            edges: self.list_edges(project_id)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_open_and_migrate() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("dag.db");
        let store = DagStore::open(&db_path).unwrap();
        // Second open should be idempotent
        drop(store);
        DagStore::open(&db_path).unwrap();
    }

    #[test]
    fn test_node_crud() {
        let dir = tempdir().unwrap();
        let store = DagStore::open(&dir.path().join("dag.db")).unwrap();

        let node = store
            .upsert_node(
                &NodeType::Task,
                "proj-1",
                serde_json::json!({ "title": "Hello" }),
            )
            .unwrap();

        assert_eq!(node.node_type, "Task");

        let fetched = store.get_node(&node.id).unwrap();
        assert_eq!(fetched.data["title"], "Hello");

        store
            .update_node(&node.id, serde_json::json!({ "title": "Updated" }))
            .unwrap();
        let updated = store.get_node(&node.id).unwrap();
        assert_eq!(updated.data["title"], "Updated");

        store.delete_node(&node.id).unwrap();
        assert!(store.get_node(&node.id).is_err());
    }

    #[test]
    fn test_edge_crud() {
        let dir = tempdir().unwrap();
        let store = DagStore::open(&dir.path().join("dag.db")).unwrap();

        let a = store
            .upsert_node(&NodeType::Feature, "p1", serde_json::json!({}))
            .unwrap();
        let b = store
            .upsert_node(&NodeType::Task, "p1", serde_json::json!({}))
            .unwrap();

        let edge = store
            .add_edge(&a.id, &b.id, &EdgeType::Blocks, serde_json::json!({}))
            .unwrap();

        assert_eq!(edge.edge_type, "blocks");

        let edges = store.list_edges("p1").unwrap();
        assert_eq!(edges.len(), 1);

        store.delete_edge(&edge.id).unwrap();
        assert!(store.list_edges("p1").unwrap().is_empty());
    }
}
