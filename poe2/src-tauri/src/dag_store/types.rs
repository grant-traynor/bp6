use serde::{Deserialize, Serialize};

// ── Node types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Project,
    Phase,
    Epic,
    Feature,
    Task,
    Bug,
    Chore,
    Subtask,
    /// Reviewer node spawned by SF-4 yield-handling when reason='review'.
    /// Was previously represented as NodeType::Task as a workaround.
    PlanReview,
    /// Advisor node for the Queue Advisor chatbot. Uses poe:advisor events.
    Advisor,
}

impl std::fmt::Display for NodeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeType::Project => write!(f, "project"),
            NodeType::Phase => write!(f, "phase"),
            NodeType::Epic => write!(f, "epic"),
            NodeType::Feature => write!(f, "feature"),
            NodeType::Task => write!(f, "task"),
            NodeType::Bug => write!(f, "bug"),
            NodeType::Chore => write!(f, "chore"),
            NodeType::Subtask => write!(f, "subtask"),
            NodeType::PlanReview => write!(f, "plan_review"),
            NodeType::Advisor => write!(f, "advisor"),
        }
    }
}

impl std::str::FromStr for NodeType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "project" => Ok(NodeType::Project),
            "phase" => Ok(NodeType::Phase),
            "epic" => Ok(NodeType::Epic),
            "feature" => Ok(NodeType::Feature),
            "task" => Ok(NodeType::Task),
            "bug" => Ok(NodeType::Bug),
            "chore" => Ok(NodeType::Chore),
            "subtask" => Ok(NodeType::Subtask),
            "plan_review" => Ok(NodeType::PlanReview),
            "advisor" => Ok(NodeType::Advisor),
            _ => Err(format!("unknown node type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Pending,
    Running,
    Waiting,
    Blocked,
    Complete,
    Cancelled,
}

impl std::fmt::Display for NodeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeStatus::Pending => write!(f, "pending"),
            NodeStatus::Running => write!(f, "running"),
            NodeStatus::Waiting => write!(f, "waiting"),
            NodeStatus::Blocked => write!(f, "blocked"),
            NodeStatus::Complete => write!(f, "complete"),
            NodeStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for NodeStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(NodeStatus::Pending),
            "running" => Ok(NodeStatus::Running),
            "waiting" => Ok(NodeStatus::Waiting),
            "blocked" => Ok(NodeStatus::Blocked),
            "complete" => Ok(NodeStatus::Complete),
            "cancelled" => Ok(NodeStatus::Cancelled),
            _ => Err(format!("unknown node status: {}", s)),
        }
    }
}

// ── Edge types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    DependsOn,
    RelatesTo,
}

impl std::fmt::Display for EdgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeType::DependsOn => write!(f, "depends_on"),
            EdgeType::RelatesTo => write!(f, "relates_to"),
        }
    }
}

impl std::str::FromStr for EdgeType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "depends_on" => Ok(EdgeType::DependsOn),
            "relates_to" => Ok(EdgeType::RelatesTo),
            _ => Err(format!("unknown edge type: {}", s)),
        }
    }
}

// ── Phase lifecycle ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PhaseLifecycleStage {
    Planning,
    Execution,
    Retrospective,
    Complete,
}

impl std::fmt::Display for PhaseLifecycleStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PhaseLifecycleStage::Planning => write!(f, "planning"),
            PhaseLifecycleStage::Execution => write!(f, "execution"),
            PhaseLifecycleStage::Retrospective => write!(f, "retrospective"),
            PhaseLifecycleStage::Complete => write!(f, "complete"),
        }
    }
}

impl std::str::FromStr for PhaseLifecycleStage {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "planning" => Ok(PhaseLifecycleStage::Planning),
            "execution" => Ok(PhaseLifecycleStage::Execution),
            "retrospective" => Ok(PhaseLifecycleStage::Retrospective),
            "complete" => Ok(PhaseLifecycleStage::Complete),
            _ => Err(format!("unknown phase lifecycle stage: {}", s)),
        }
    }
}

// ── Core structs ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub conops_ref: Option<String>,
    pub active_phase_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Phase {
    pub id: String,
    pub project_id: String,
    pub number: i64,
    pub title: String,
    pub lifecycle_stage: PhaseLifecycleStage,
    pub gate_held: bool,
    /// Stage type e.g. 'execution' | 'conops' | 'guardrails' | etc. Added in Phase 3.
    pub stage_type: String,
    /// Phase status: 'pending' | 'running' | 'gate' | 'complete'. Added in Phase 3.
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: String,
    pub project_id: String,
    pub phase_id: Option<String>,
    pub parent_id: Option<String>,
    pub node_type: NodeType,
    pub title: String,
    pub description: Option<String>,
    pub status: NodeStatus,
    pub skill_id: Option<String>,
    pub assignee: Option<String>,
    /// 'review' | 'decision' | NULL — set when status=waiting. Used by recovery.
    pub yield_reason: Option<String>,
    /// Claude --resume handle. Canonical location per Protocol.md §1.
    pub session_id: Option<String>,
    /// Reviewer tasks only: back-reference to the task that requested this review.
    pub requesting_task_id: Option<String>,
    /// The 'id' from the originating poe:review event. NULL for non-reviewer tasks.
    pub review_id: Option<String>,
    /// Watchdog retry counter for reviewer tasks.
    pub retry_count: i64,
    /// Matrix drag-to-reorder position. NULL = render in creation order.
    pub sort_order: Option<i64>,
    /// JSON array of modes declared in skill frontmatter. NULL = treat as '["autonomous"]'.
    pub skill_modes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    pub id: String,
    pub from_id: String,
    pub to_id: String,
    pub edge_type: EdgeType,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub id: String,
    pub project_id: String,
    pub phase_id: Option<String>,
    pub artifact_type: String,
    pub filename: String,
    pub produced_by_stage: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeEntry {
    pub id: String,
    pub project_id: String,
    pub key: String,
    pub value: String,
    pub source: Option<String>,
    pub supersedes_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRecord {
    pub id: String,
    pub project_id: String,
    pub agent_id: Option<String>,
    pub task_id: Option<String>,
    pub event_type: String,
    pub payload: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRecord {
    pub id: String,
    pub project_id: String,
    pub skill_id: String,
    pub task_id: String,
    pub status: String,
    pub session_id: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueItem {
    pub id: String,
    pub project_id: String,
    pub agent_id: Option<String>,
    pub task_id: Option<String>,
    pub question: String,
    pub options: Option<String>,
    pub resolution: Option<String>,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatTurn {
    pub id: String,
    pub task_id: String,
    pub content: String,
    pub response: Option<String>,
    pub created_at: String,
    pub responded_at: Option<String>,
}

/// Structurally identical to ChatTurn but routes to Pane 3 (advisor panel).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvisorTurn {
    pub id: String,
    pub task_id: String,
    pub content: String,
    pub response: Option<String>,
    pub created_at: String,
    pub responded_at: Option<String>,
}

// ── Input types for commands ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateNodeInput {
    pub project_id: String,
    /// Agent-supplied stable ID (e.g. "task-005"). When present, used as the
    /// node's primary key so that subsequent poe:task:update and poe:edge events
    /// can reference it by the same ID. Falls back to a UUID when absent.
    pub id: Option<String>,
    pub phase_id: Option<String>,
    pub parent_id: Option<String>,
    pub node_type: NodeType,
    pub title: String,
    pub description: Option<String>,
    pub skill_id: Option<String>,
    /// Initial status — defaults to `pending` if absent.
    /// Use `waiting` to prevent autonomous dispatch immediately on creation.
    pub initial_status: Option<NodeStatus>,
    /// Reviewer tasks only: back-reference to the task that requested this review.
    pub requesting_task_id: Option<String>,
    /// The 'id' from the originating poe:review event. NULL for non-reviewer tasks.
    pub review_id: Option<String>,
    /// Watchdog retry counter initial value. Defaults to 0.
    pub retry_count: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNodeInput {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<NodeStatus>,
    pub skill_id: Option<String>,
    pub assignee: Option<String>,
    pub yield_reason: Option<String>,
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateArtifactInput {
    pub project_id: String,
    pub phase_id: Option<String>,
    pub artifact_type: String,
    pub filename: String,
    pub produced_by_stage: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateKnowledgeInput {
    pub project_id: String,
    pub key: String,
    pub value: String,
    pub source: Option<String>,
    pub supersedes_id: Option<String>,
}
