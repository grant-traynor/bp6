import { invoke } from "@tauri-apps/api/core";

export interface Dependency {
  issue_id: string;
  depends_on_id: string;
  type: string;
  metadata?: Record<string, any>;
  [key: string]: any;
}

export interface Bead {
  id: string;
  title: string;
  description?: string;
  status: string;
  priority: number;
  issue_type: string;
  estimate?: number;
  dependencies: Dependency[];
  owner?: string;
  created_at?: string;
  created_by?: string;
  updated_at?: string;
  labels?: string[];
  acceptance_criteria?: string[];
  closed_at?: string;
  close_reason?: string;
  is_favorite?: boolean;
  parent?: string;
  external_reference?: string;
  // Unified field naming (matches Rust and JSONL)
  design?: string;
  notes?: string;
  // Legacy field names (for backward compatibility)
  design_notes?: string;
  working_notes?: string;
  [key: string]: any;
}

export interface WBSNode extends Bead {
  children: WBSNode[];
  parent?: string;
  isExpanded?: boolean;
  isCritical?: boolean;
  isBlocked?: boolean;
}

export interface Project {
  name: string;
  path: string;
  is_favorite?: boolean;
  last_opened?: string;
}

export async function fetchProjects(): Promise<Project[]> {
  try {
    return await invoke<Project[]>("get_projects");
  } catch (error) {
    console.error("Failed to fetch projects:", error);
    return [];
  }
}

export async function addProject(project: Project): Promise<void> {
  await invoke("add_project", { project });
}

export async function removeProject(path: string): Promise<void> {
  await invoke("remove_project", { path });
}

export async function toggleFavoriteProject(path: string): Promise<void> {
  await invoke("toggle_favorite", { path });
}

export async function openProject(path: string): Promise<void> {
  await invoke("open_project", { path });
}

export async function fetchBeads(): Promise<Bead[]> {
  try {
    return await invoke<Bead[]>("get_beads");
  } catch (error) {
    console.error("Failed to fetch beads:", error);
    throw error;
  }
}

export async function updateBead(bead: Bead): Promise<void> {
  try {
    await invoke("update_bead", { updatedBead: bead });
  } catch (error) {
    console.error("Failed to update bead:", error);
    throw error;
  }
}

export async function createBead(bead: Bead): Promise<string> {
  try {
    console.log('🔧 api.createBead: Invoking Tauri command with bead:', bead);
    const result = await invoke<string>("create_bead", { newBead: bead });
    console.log('🔧 api.createBead: Tauri returned ID:', result);
    return result;
  } catch (error) {
    console.error("🔧 api.createBead: Failed to create bead:", error);
    throw error;
  }
}

export async function closeBead(beadId: string, reason?: string): Promise<void> {
  try {
    await invoke("close_bead", { beadId, reason });
  } catch (error) {
    console.error("Failed to close bead:", error);
    throw error;
  }
}

export async function reopenBead(beadId: string): Promise<void> {
  try {
    await invoke("reopen_bead", { beadId });
  } catch (error) {
    console.error("Failed to reopen bead:", error);
    throw error;
  }
}

export async function claimBead(beadId: string): Promise<void> {
  try {
    await invoke("claim_bead", { beadId });
  } catch (error) {
    console.error("Failed to claim bead:", error);
    throw error;
  }
}

export async function fetchProcessedData(params: FilterParams): Promise<ProcessedData> {
  try {
    return await invoke<ProcessedData>("get_processed_data", { params });
  } catch (error) {
    console.error("Failed to fetch processed data:", error);
    throw error;
  }
}

/**
 * Fetch the unified ProjectViewModel from Rust backend.
 * This is the new API that replaces the dual-state system.
 */
export async function fetchProjectViewModel(params: FilterParams): Promise<ProjectViewModel> {
  try {
    return await invoke<ProjectViewModel>("get_project_view_model", { params });
  } catch (error) {
    console.error("Failed to fetch project view model:", error);
    throw error;
  }
}

// ============================================================================
// Agent API (bp6-5s4.2.5)
// ============================================================================

export interface SessionInfo {
  session_id: string;
  bead_id: string;
  persona: string;        // PersonaType as string
  backend_id: string;     // BackendId as string
  status: 'running' | 'paused';
  created_at: number;     // Unix timestamp
}

// Persona icon mapping (aligned with PersonaType enum from 6dk)
export const PERSONA_ICONS: Record<string, string> = {
  'product_manager': '📋',
  'qa_engineer': '🧪',
  'specialist': '⚡',
  // Future: architect, security, etc.
};

export interface AgentChunk {
  content: string;
  isDone: boolean;
}

/**
 * CLI backend type for agent sessions.
 * - 'gemini': Use Google Gemini CLI
 * - 'claude' | 'claude-code': Use Claude Code CLI
 */
export type CliBackend = 'gemini' | 'claude' | 'claude-code';

/**
 * Start a new agent session.
 * @param persona - The agent persona ('specialist', 'product-manager', 'qa-engineer')
 * @param task - Optional task type for the persona
 * @param beadId - Optional bead ID for context
 * @param cliBackend - Optional CLI backend to use (defaults to 'gemini' if not provided)
 * @returns The session ID (UUID) of the newly created session
 */
export async function startAgentSession(
  persona: string,
  task?: string,
  beadId?: string,
  cliBackend?: CliBackend
): Promise<string> {
  try {
    return await invoke<string>("start_agent_session", { persona, task, beadId, cliBackend });
  } catch (error) {
    console.error("Failed to start agent session:", error);
    throw error;
  }
}

/**
 * Send a message to a specific agent session.
 * @param sessionId - The session ID to send the message to
 * @param message - The message content
 */
export async function sendAgentMessage(sessionId: string, message: string): Promise<void> {
  try {
    await invoke("send_agent_message", { sessionId, message });
  } catch (error) {
    console.error("Failed to send agent message:", error);
    throw error;
  }
}

/**
 * Stop a specific agent session.
 * @param sessionId - The session ID to stop
 */
export async function stopAgentSession(sessionId: string): Promise<void> {
  try {
    await invoke("stop_agent_session", { sessionId });
  } catch (error) {
    console.error("Failed to stop agent session:", error);
    throw error;
  }
}

/**
 * Get the current CLI backend preference from persistent storage.
 * @returns The current CLI backend ('gemini', 'claude', or 'claude-code')
 * @throws Error if unable to read CLI preference from storage
 */
export async function getCliPreference(): Promise<string> {
  try {
    return await invoke<string>("get_cli_preference");
  } catch (error) {
    console.error("Failed to get CLI preference:", error);
    throw new Error(`Unable to retrieve CLI preference: ${error}`);
  }
}

/**
 * Set the CLI backend preference in persistent storage.
 * @param cliBackend - The CLI backend to use ('gemini', 'claude', or 'claude-code')
 * @throws Error if unable to save CLI preference to storage
 */
export async function setCliPreference(cliBackend: string): Promise<void> {
  try {
    await invoke("set_cli_preference", { cliBackend });
  } catch (error) {
    console.error("Failed to set CLI preference:", error);
    throw new Error(`Unable to save CLI preference: ${error}`);
  }
}

export async function approveSuggestion(command: string): Promise<string> {
  try {
    return await invoke<string>("approve_suggestion", { command });
  } catch (error) {
    console.error("Failed to approve suggestion:", error);
    throw error;
  }
}

// buildWBSTree function removed - now handled by Rust backend in get_processed_data

export interface GanttItem {
  bead: BeadNode;
  x: number;
  width: number;
  row: number;
  depth: number;
  isCritical: boolean;
  isBlocked: boolean;
}

export interface GanttConnector {
  from: { x: number; y: number };
  to: { x: number; y: number };
  isCritical: boolean;
}

export interface GanttLayout {
  items: GanttItem[];
  connectors: GanttConnector[];
  rowCount: number;
  rowDepths: number[];
}

export interface BucketDistribution {
  open: number;
  inProgress: number;
  blocked: number;
  closed: number;
}

export interface ProcessedData {
  tree: WBSNode[];
  layout: GanttLayout;
  distributions: BucketDistribution[];
}

// ============================================================================
// Unified View Model Types (bp6-75y.1)
// ============================================================================

/**
 * BeadNode is the unified node structure in the view model tree.
 * Contains all bead data, computed properties, hierarchical structure,
 * and logical positioning (NOT pixel coordinates).
 */
export interface BeadNode {
  // Core Bead Data
  id: string;
  title: string;
  description?: string;
  status: string;
  priority: number;
  issueType: string;
  estimate?: number;
  dependencies: Dependency[];

  // Metadata
  owner?: string;
  createdAt?: string;
  createdBy?: string;
  updatedAt?: string;
  labels?: string[];
  acceptanceCriteria?: string[];
  closedAt?: string;
  closeReason?: string;
  isFavorite?: boolean;
  parent?: string;
  externalReference?: string;

  // Unified Field Naming (design/notes, not design_notes/working_notes)
  design?: string;
  notes?: string;

  // Hierarchical Structure
  children: BeadNode[];

  // Computed Properties
  isBlocked: boolean;
  isCritical: boolean;
  blockingIds: string[];

  // Logical Positioning (NOT pixels - frontend converts)
  depth: number;
  cellOffset: number;  // Which grid cell column (0, 1, 2, ...)
  cellCount: number;   // How many cells wide (1, 2, 3, ...)

  // UI State
  isExpanded: boolean;
  isVisible: boolean;

  // Extra metadata
  [key: string]: any;
}

/**
 * ViewIndexes provides fast lookups into the view model tree.
 */
export interface ViewIndexes {
  idToIndex: Record<string, number>;
  idToParent: Record<string, string>;
  criticalPath: string[];
}

/**
 * ProjectMetadata contains aggregate statistics about the project.
 */
export interface ProjectMetadata {
  totalBeads: number;
  openCount: number;
  blockedCount: number;
  inProgressCount: number;
  closedCount: number;
  totalDuration: number;
  distributions: BucketDistribution[];
}

/**
 * ProjectViewModel is the single source of truth for all UI components.
 * Backend computes this once, frontend reactively renders it.
 */
export interface ProjectViewModel {
  tree: BeadNode[];
  metadata: ProjectMetadata;
  indexes: ViewIndexes;
}

export interface FilterParams {
  filter_text?: string;
  hide_closed?: boolean;
  closed_time_filter?: 'all' | '1h' | '6h' | '24h' | '7d' | '30d' | 'older_than_6h';
  include_hierarchy?: boolean;
  zoom?: number;
  collapsed_ids?: string[];
  sort_by?: 'priority' | 'title' | 'type' | 'id' | 'none';
  sort_order?: 'asc' | 'desc' | 'none';
}

/**
 * Convert BeadNode (camelCase, from view model) to Bead (snake_case, for API mutations).
 * Used when updating or creating beads.
 */
export function beadNodeToBead(node: Partial<BeadNode>): Partial<Bead> {
  return {
    id: node.id,
    title: node.title,
    description: node.description,
    status: node.status,
    priority: node.priority,
    issue_type: node.issueType,
    estimate: node.estimate,
    dependencies: node.dependencies,
    owner: node.owner,
    created_at: node.createdAt,
    created_by: node.createdBy,
    updated_at: node.updatedAt,
    labels: node.labels,
    acceptance_criteria: node.acceptanceCriteria,
    closed_at: node.closedAt,
    close_reason: node.closeReason,
    is_favorite: node.isFavorite,
    parent: node.parent,
    external_reference: node.externalReference,
    design: node.design,
    notes: node.notes,
  };
}

// calculateStateDistribution function removed - now handled by Rust backend in get_processed_data

// calculateGanttLayout function removed - now handled by Rust backend in get_processed_data
