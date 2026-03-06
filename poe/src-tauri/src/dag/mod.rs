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

// ── Queue item types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItemOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
}

/// Input for creating a new queue item.
pub struct NewQueueItem {
    pub project_id: String,
    pub agent_id: String,
    pub workflow_id: Option<String>,
    pub awakeable_id: Option<String>,
    pub question: String,
    pub options: Vec<QueueItemOption>,
    pub context_snapshot: serde_json::Value,
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItem {
    pub id: String,
    pub project_id: String,
    pub agent_id: String,
    pub workflow_id: Option<String>,
    pub awakeable_id: Option<String>,
    pub question: String,
    pub options: Vec<QueueItemOption>,
    pub context_snapshot: serde_json::Value,
    pub priority: i32,
    pub status: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
    pub resolution: Option<serde_json::Value>,
}

// ── Data structures ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeData {
    pub node: DagNode,
    pub incoming_edges: Vec<DagEdge>,
    pub outgoing_edges: Vec<DagEdge>,
    pub decisions: Vec<DagNode>,
    pub artifacts: Vec<DagNode>,
    pub connected_nodes: Vec<DagNode>,
}

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

// ── Workflow types ─────────────────────────────────────────────────────────────

/// Input for creating a new workflow record.
pub struct NewWorkflow {
    pub project_id: String,
    pub node_id: String,
    pub agent_id: Option<String>,
    pub workflow_type: String,
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRecord {
    pub id: String,
    pub project_id: String,
    pub node_id: String,
    pub agent_id: Option<String>,
    pub workflow_type: String,
    pub status: String,
    pub current_step: Option<String>,
    pub config: serde_json::Value,
    pub started_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub error: Option<String>,
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
    // v2: queue items
    r#"
    CREATE TABLE IF NOT EXISTS queue_items (
        id               TEXT PRIMARY KEY,
        project_id       TEXT NOT NULL,
        agent_id         TEXT NOT NULL,
        workflow_id      TEXT,
        awakeable_id     TEXT,
        question         TEXT NOT NULL,
        options          TEXT NOT NULL DEFAULT '[]',
        context_snapshot TEXT NOT NULL DEFAULT '{}',
        priority         INTEGER NOT NULL DEFAULT 2,
        status           TEXT NOT NULL DEFAULT 'pending',
        created_at       TEXT NOT NULL,
        resolved_at      TEXT,
        resolution       TEXT
    );

    CREATE INDEX IF NOT EXISTS idx_queue_items_project ON queue_items(project_id);
    CREATE INDEX IF NOT EXISTS idx_queue_items_status  ON queue_items(status);
    "#,
    // v3: workflows
    r#"
    CREATE TABLE IF NOT EXISTS workflows (
        id            TEXT PRIMARY KEY,
        project_id    TEXT NOT NULL,
        node_id       TEXT NOT NULL,
        agent_id      TEXT,
        workflow_type TEXT NOT NULL,
        status        TEXT NOT NULL DEFAULT 'pending',
        current_step  TEXT,
        config        TEXT NOT NULL DEFAULT '{}',
        started_at    TEXT NOT NULL,
        updated_at    TEXT NOT NULL,
        completed_at  TEXT,
        error         TEXT
    );

    CREATE INDEX IF NOT EXISTS idx_workflows_project ON workflows(project_id);
    CREATE INDEX IF NOT EXISTS idx_workflows_node    ON workflows(node_id);
    CREATE INDEX IF NOT EXISTS idx_workflows_status  ON workflows(status);
    "#,
];

// ── Helpers ────────────────────────────────────────────────────────────────────

type QueueItemRow = (String, String, String, Option<String>, Option<String>, String, String, String, i32, String, String, Option<String>, Option<String>);

fn parse_queue_item_row(
    (id, project_id, agent_id, workflow_id, awakeable_id, question, options_str, context_str, priority, status, created_at, resolved_at, resolution_str): QueueItemRow,
) -> Result<QueueItem, String> {
    let options: Vec<QueueItemOption> = serde_json::from_str(&options_str)
        .map_err(|e| format!("Failed to parse options: {}", e))?;
    let context_snapshot: serde_json::Value = serde_json::from_str(&context_str)
        .map_err(|e| format!("Failed to parse context_snapshot: {}", e))?;
    let resolution: Option<serde_json::Value> = resolution_str
        .map(|s| serde_json::from_str(&s))
        .transpose()
        .map_err(|e| format!("Failed to parse resolution: {}", e))?;
    Ok(QueueItem {
        id,
        project_id,
        agent_id,
        workflow_id,
        awakeable_id,
        question,
        options,
        context_snapshot,
        priority,
        status,
        created_at,
        resolved_at,
        resolution,
    })
}

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

    /// Probe: returns full context for a single node — its edges and linked nodes.
    pub fn probe_node(&self, node_id: &str) -> Result<ProbeData, String> {
        let node = self.get_node(node_id)?;

        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, from_id, to_id, type, data, created_at
                 FROM edges WHERE from_id = ?1 OR to_id = ?1",
            )
            .map_err(|e| format!("probe_node prepare edges: {}", e))?;

        let all_edges: Vec<DagEdge> = stmt
            .query_map(params![node_id], |row| {
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
            .map_err(|e| format!("probe_node query edges: {}", e))?
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

        let incoming_edges: Vec<DagEdge> =
            all_edges.iter().filter(|e| e.to_id == node_id).cloned().collect();
        let outgoing_edges: Vec<DagEdge> =
            all_edges.iter().filter(|e| e.from_id == node_id).cloned().collect();

        // Fetch all nodes connected by any edge
        let connected_ids: std::collections::HashSet<&str> = all_edges
            .iter()
            .flat_map(|e| [e.from_id.as_str(), e.to_id.as_str()])
            .filter(|&id| id != node_id)
            .collect();

        let mut connected_nodes = Vec::new();
        for id in &connected_ids {
            if let Ok(n) = self.get_node(id) {
                connected_nodes.push(n);
            }
        }

        let decisions: Vec<DagNode> = connected_nodes
            .iter()
            .filter(|n| {
                n.node_type == "Decision"
                    && incoming_edges.iter().any(|e| e.from_id == n.id && e.edge_type == "approved-by")
            })
            .cloned()
            .collect();

        let artifacts: Vec<DagNode> = connected_nodes
            .iter()
            .filter(|n| {
                n.node_type == "AgentOutput"
                    && outgoing_edges.iter().any(|e| e.to_id == n.id && e.edge_type == "generated-by")
            })
            .cloned()
            .collect();

        Ok(ProbeData {
            node,
            incoming_edges,
            outgoing_edges,
            decisions,
            artifacts,
            connected_nodes,
        })
    }

    /// Provenance: returns the subgraph within `depth` hops of `node_id`.
    pub fn get_provenance(
        &self,
        node_id: &str,
        project_id: &str,
        depth: u32,
    ) -> Result<DagSnapshot, String> {
        let nodes_sql = "
            WITH RECURSIVE prov(nid, d) AS (
                SELECT ?1, 0
                UNION
                SELECT CASE WHEN e.from_id = p.nid THEN e.to_id ELSE e.from_id END, p.d + 1
                FROM edges e
                JOIN prov p ON (e.from_id = p.nid OR e.to_id = p.nid)
                WHERE p.d < ?2
            )
            SELECT DISTINCT n.id, n.type, n.project_id, n.data, n.created_at, n.updated_at
            FROM nodes n
            JOIN prov p ON n.id = p.nid
        ";

        let depth_val = depth as i64;
        let mut stmt = self
            .conn
            .prepare(nodes_sql)
            .map_err(|e| format!("provenance nodes prepare: {}", e))?;

        let nodes: Vec<DagNode> = stmt
            .query_map(params![node_id, depth_val], |row| {
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
            .map_err(|e| format!("provenance nodes query: {}", e))?
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

        // Filter all project edges to those within the subgraph
        let node_id_set: std::collections::HashSet<&str> =
            nodes.iter().map(|n| n.id.as_str()).collect();
        let all_edges = self.list_edges(project_id)?;
        let edges = all_edges
            .into_iter()
            .filter(|e| {
                node_id_set.contains(e.from_id.as_str())
                    && node_id_set.contains(e.to_id.as_str())
            })
            .collect();

        Ok(DagSnapshot { nodes, edges })
    }

    // ── Queue item CRUD ────────────────────────────────────────────────────────

    pub fn create_queue_item(&self, input: NewQueueItem) -> Result<QueueItem, String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let options_str = serde_json::to_string(&input.options)
            .map_err(|e| format!("Failed to serialize options: {}", e))?;
        let context_str = serde_json::to_string(&input.context_snapshot)
            .map_err(|e| format!("Failed to serialize context_snapshot: {}", e))?;

        self.conn
            .execute(
                "INSERT INTO queue_items (id, project_id, agent_id, workflow_id, awakeable_id, question, options, context_snapshot, priority, status, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'pending', ?10)",
                params![id, input.project_id, input.agent_id, input.workflow_id, input.awakeable_id, input.question, options_str, context_str, input.priority, now],
            )
            .map_err(|e| format!("Failed to insert queue item: {}", e))?;

        Ok(QueueItem {
            id,
            project_id: input.project_id,
            agent_id: input.agent_id,
            workflow_id: input.workflow_id,
            awakeable_id: input.awakeable_id,
            question: input.question,
            options: input.options,
            context_snapshot: input.context_snapshot,
            priority: input.priority,
            status: "pending".to_string(),
            created_at: now,
            resolved_at: None,
            resolution: None,
        })
    }

    pub fn get_queue_item(&self, id: &str) -> Result<QueueItem, String> {
        self.conn
            .query_row(
                "SELECT id, project_id, agent_id, workflow_id, awakeable_id, question, options, context_snapshot, priority, status, created_at, resolved_at, resolution
                 FROM queue_items WHERE id = ?1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, i32>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, String>(10)?,
                        row.get::<_, Option<String>>(11)?,
                        row.get::<_, Option<String>>(12)?,
                    ))
                },
            )
            .map_err(|e| format!("Failed to get queue item {}: {}", id, e))
            .and_then(parse_queue_item_row)
    }

    pub fn list_queue_items(&self, project_id: &str) -> Result<Vec<QueueItem>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, project_id, agent_id, workflow_id, awakeable_id, question, options, context_snapshot, priority, status, created_at, resolved_at, resolution
                 FROM queue_items WHERE project_id = ?1 AND status = 'pending'
                 ORDER BY priority ASC, created_at ASC",
            )
            .map_err(|e| format!("Failed to prepare queue items query: {}", e))?;

        let items = stmt
            .query_map(params![project_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, i32>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, Option<String>>(11)?,
                    row.get::<_, Option<String>>(12)?,
                ))
            })
            .map_err(|e| format!("Failed to query queue items: {}", e))?
            .filter_map(|r| r.ok())
            .filter_map(|row| parse_queue_item_row(row).ok())
            .collect();

        Ok(items)
    }

    // ── Workflow CRUD ──────────────────────────────────────────────────────────

    pub fn create_workflow_record(&self, input: NewWorkflow) -> Result<WorkflowRecord, String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let config_str = serde_json::to_string(&input.config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        self.conn
            .execute(
                "INSERT INTO workflows (id, project_id, node_id, agent_id, workflow_type, status, config, started_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'pending', ?6, ?7, ?7)",
                params![id, input.project_id, input.node_id, input.agent_id, input.workflow_type, config_str, now],
            )
            .map_err(|e| format!("Failed to insert workflow: {}", e))?;

        Ok(WorkflowRecord {
            id,
            project_id: input.project_id,
            node_id: input.node_id,
            agent_id: input.agent_id,
            workflow_type: input.workflow_type,
            status: "pending".to_string(),
            current_step: None,
            config: input.config,
            started_at: now.clone(),
            updated_at: now,
            completed_at: None,
            error: None,
        })
    }

    pub fn get_workflow(&self, id: &str) -> Result<WorkflowRecord, String> {
        self.conn
            .query_row(
                "SELECT id, project_id, node_id, agent_id, workflow_type, status, current_step, config, started_at, updated_at, completed_at, error
                 FROM workflows WHERE id = ?1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, Option<String>>(10)?,
                        row.get::<_, Option<String>>(11)?,
                    ))
                },
            )
            .map_err(|e| format!("Failed to get workflow {}: {}", id, e))
            .and_then(|(id, project_id, node_id, agent_id, workflow_type, status, current_step, config_str, started_at, updated_at, completed_at, error)| {
                let config = serde_json::from_str(&config_str)
                    .map_err(|e| format!("Failed to parse workflow config: {}", e))?;
                Ok(WorkflowRecord { id, project_id, node_id, agent_id, workflow_type, status, current_step, config, started_at, updated_at, completed_at, error })
            })
    }

    pub fn list_workflows(&self, project_id: &str) -> Result<Vec<WorkflowRecord>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, project_id, node_id, agent_id, workflow_type, status, current_step, config, started_at, updated_at, completed_at, error
                 FROM workflows WHERE project_id = ?1 ORDER BY started_at DESC",
            )
            .map_err(|e| format!("Failed to prepare workflows query: {}", e))?;

        let records = stmt
            .query_map(params![project_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, Option<String>>(10)?,
                    row.get::<_, Option<String>>(11)?,
                ))
            })
            .map_err(|e| format!("Failed to query workflows: {}", e))?
            .filter_map(|r| r.ok())
            .filter_map(|(id, project_id, node_id, agent_id, workflow_type, status, current_step, config_str, started_at, updated_at, completed_at, error)| {
                serde_json::from_str(&config_str).ok().map(|config| WorkflowRecord {
                    id, project_id, node_id, agent_id, workflow_type, status, current_step, config, started_at, updated_at, completed_at, error,
                })
            })
            .collect();

        Ok(records)
    }

    pub fn update_workflow_status(
        &self,
        id: &str,
        status: &str,
        agent_id: Option<&str>,
        error: Option<&str>,
    ) -> Result<WorkflowRecord, String> {
        let now = Utc::now().to_rfc3339();
        let completed_at = if status == "completed" || status == "failed" || status == "cancelled" {
            Some(now.clone())
        } else {
            None
        };

        self.conn
            .execute(
                "UPDATE workflows SET status = ?1, agent_id = COALESCE(?2, agent_id), updated_at = ?3, completed_at = COALESCE(?4, completed_at), error = COALESCE(?5, error) WHERE id = ?6",
                params![status, agent_id, now, completed_at, error, id],
            )
            .map_err(|e| format!("Failed to update workflow status: {}", e))?;

        self.get_workflow(id)
    }

    pub fn update_workflow_step(
        &self,
        id: &str,
        current_step: &str,
    ) -> Result<WorkflowRecord, String> {
        let now = Utc::now().to_rfc3339();
        self.conn
            .execute(
                "UPDATE workflows SET current_step = ?1, updated_at = ?2 WHERE id = ?3",
                params![current_step, now, id],
            )
            .map_err(|e| format!("Failed to update workflow step: {}", e))?;

        self.get_workflow(id)
    }

    pub fn resolve_queue_item_in_db(
        &self,
        id: &str,
        resolution: serde_json::Value,
    ) -> Result<QueueItem, String> {
        let now = Utc::now().to_rfc3339();
        let resolution_str = serde_json::to_string(&resolution)
            .map_err(|e| format!("Failed to serialize resolution: {}", e))?;

        let rows = self
            .conn
            .execute(
                "UPDATE queue_items SET status = 'resolved', resolved_at = ?1, resolution = ?2 WHERE id = ?3 AND status = 'pending'",
                params![now, resolution_str, id],
            )
            .map_err(|e| format!("Failed to resolve queue item: {}", e))?;

        if rows == 0 {
            return Err(format!("Queue item '{}' not found or already resolved", id));
        }

        self.get_queue_item(id)
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
