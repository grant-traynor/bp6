import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type { UnlistenFn };

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

// NOTE: Rust uses #[serde(rename_all = "camelCase")], so fields are camelCase in JSON
export interface SessionInfo {
  sessionId: string;
  beadId: string | null;  // Optional bead ID
  persona: string;        // PersonaType as string
  backendId: string;      // BackendId as string
  status: 'running' | 'paused';
  createdAt: number;      // Unix timestamp in seconds (Rust u64)
  cliSessionId?: string | null;  // CLI session ID for resume capability
  lastActivity: number;   // Unix timestamp of last activity (Rust u64)
  hasUnread: boolean;     // Whether session has unread messages
  messageCount: number;   // Number of messages in session
}

// Persona icon mapping (aligned with PersonaType enum - uses hyphens as per Rust backend)
export const PERSONA_ICONS: Record<string, string> = {
  'product-manager': '📋',
  'qa-engineer': '🧪',
  'specialist': '⚡',
  // Future: architect, security, etc.
};

/**
 * Format session runtime from created_at timestamp to "Xm Ys" format.
 * @param createdAt Unix timestamp in seconds (Rust u64)
 * @returns Formatted runtime string (e.g., "5m 32s")
 */
export function formatSessionRuntime(createdAt: number): string {
  const nowSeconds = Math.floor(Date.now() / 1000);
  const elapsed = nowSeconds - createdAt;
  const minutes = Math.floor(elapsed / 60);
  const remainingSeconds = elapsed % 60;
  return `${minutes}m ${remainingSeconds}s`;
}

/**
 * Get persona icon for a given persona type.
 * @param persona PersonaType as string (with hyphens, e.g., "product-manager")
 * @returns Emoji icon for the persona, or ❓ if unknown
 */
export function getPersonaIcon(persona: string): string {
  return PERSONA_ICONS[persona] || '❓';
}

export interface AgentChunk {
  content: string;
  isDone: boolean;
  sessionId?: string;  // Session ID for multi-window routing
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
 * Interrupt a specific agent session without terminating it.
 * Sends SIGINT to stop the current streaming response but keeps the session alive.
 * @param sessionId - The session ID to interrupt
 */
export async function interruptAgentSession(sessionId: string): Promise<void> {
  try {
    await invoke("interrupt_agent_session", { sessionId });
  } catch (error) {
    console.error("Failed to interrupt agent session:", error);
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

/**
 * List all active agent sessions.
 * @returns Array of SessionInfo for all currently active sessions
 */
export async function listActiveSessions(): Promise<SessionInfo[]> {
  try {
    return await invoke<SessionInfo[]>('list_active_sessions');
  } catch (error) {
    console.error('Failed to list active sessions:', error);
    return [];
  }
}

/**
 * Switch the active session to a different session.
 * @param sessionId - The session ID to switch to
 */
export async function switchActiveSession(sessionId: string): Promise<void> {
  await invoke('switch_active_session', { sessionId });
}

/**
 * Get the currently active session ID.
 * @returns The active session ID, or null if no session is active
 */
export async function getActiveSessionId(): Promise<string | null> {
  return await invoke<string | null>('get_active_session_id');
}

/**
 * Terminate a specific agent session.
 * @param sessionId - The session ID to terminate
 */
export async function terminateSession(sessionId: string): Promise<void> {
  await invoke('terminate_session', { sessionId });
}

/**
 * Mark a session as read (clear the unread indicator).
 * @param sessionId - The session ID to mark as read
 */
export async function markSessionRead(sessionId: string): Promise<void> {
  await invoke('mark_session_read', { sessionId });
}

/**
 * Create a new window for a specific session (multi-window support).
 * @param sessionId - The session ID to open in a new window
 * @returns The window label (e.g., "agent-session-{uuid}")
 */
export async function createSessionWindow(sessionId: string): Promise<string> {
  try {
    return await invoke<string>('create_session_window', { sessionId });
  } catch (error) {
    console.error('Failed to create session window:', error);
    throw error;
  }
}

// ============================================================================
// Window State Persistence (bp6-643.005.5)
// ============================================================================

export interface WindowState {
  sessionId: string;
  x: number;
  y: number;
  width: number;
  height: number;
  isMaximized: boolean;
  lastUpdated: number;
}

// ============================================================================
// Startup State Persistence (bp6-j33p.2)
// ============================================================================

export interface MainWindowState {
  width: number;
  height: number;
  x?: number;
  y?: number;
  isMaximized: boolean;
}

export interface FilterStateData {
  filterText: string;
  hideClosed: boolean;
  closedTimeFilter: string;
  includeHierarchy: boolean;
}

export interface SortStateData {
  sortBy: string;
  sortOrder: string;
}

export interface UiStateData {
  zoom: number;
  collapsedIds: string[];
}

export interface StartupState {
  window: MainWindowState;
  filters: FilterStateData;
  sort: SortStateData;
  ui: UiStateData;
}

/**
 * Load startup state from ~/.bp6/startup.json
 */
export async function loadStartupState(): Promise<StartupState | null> {
  try {
    return await invoke<StartupState | null>('load_startup_state');
  } catch (error) {
    console.error('Failed to load startup state:', error);
    return null;
  }
}

/**
 * Save startup state to ~/.bp6/startup.json
 */
export async function saveStartupState(state: StartupState): Promise<void> {
  try {
    await invoke('save_startup_state', { state });
  } catch (error) {
    console.error('Failed to save startup state:', error);
    throw error;
  }
}

/**
 * Save window state for a session
 */
export async function saveWindowState(
  sessionId: string,
  x: number,
  y: number,
  width: number,
  height: number,
  isMaximized: boolean
): Promise<void> {
  try {
    await invoke('save_window_state', { sessionId, x, y, width, height, isMaximized });
  } catch (error) {
    console.error('Failed to save window state:', error);
    throw error;
  }
}

/**
 * Load window state for a session
 */
export async function loadWindowState(sessionId: string): Promise<WindowState | null> {
  try {
    return await invoke<WindowState | null>('load_window_state', { sessionId });
  } catch (error) {
    console.error('Failed to load window state:', error);
    return null;
  }
}

// ============================================================================
// Session History API (bp6-643.004.5)
// ============================================================================

/**
 * Log event structure from JSONL files.
 * Matches the Rust LogEvent structure in session.rs.
 */
export interface LogEvent {
  timestamp: string;
  session_id: string;
  bead_id: string | null;
  persona: string;
  backend: string;
  event_type: 'sessionstart' | 'message' | 'chunk' | 'sessionend';
  content: string;
  metadata?: Record<string, any>;
}

/**
 * Message structure for conversation history.
 * Simplified from LogEvent for UI display.
 */
export interface ConversationMessage {
  role: 'user' | 'assistant';
  content: string;
  timestamp: string;
}

/**
 * Load session conversation history from JSONL log files.
 * Calls the backend get_session_history Tauri command.
 *
 * @param sessionId - The session UUID to load history for
 * @param beadId - The bead ID (used for directory organization, can be null for untracked sessions)
 * @returns Array of conversation messages (user and assistant)
 */
export async function loadSessionHistory(
  sessionId: string,
  beadId: string | null
): Promise<ConversationMessage[]> {
  try {
    return await invoke<ConversationMessage[]>('get_session_history', {
      sessionId,
      beadId
    });
  } catch (error) {
    console.error('Failed to load session history:', error);
    return [];
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

// ============================================================================
// Event Listeners (bp6-b8n)
// ============================================================================

/**
 * Listen for session list changes from the backend.
 * @param callback - Function to call with the updated session list
 * @returns A promise that resolves to an unlisten function for cleanup
 */
export async function onSessionListChanged(
  callback: (sessions: SessionInfo[]) => void
): Promise<UnlistenFn> {
  console.log('🎧 Setting up session-list-changed event listener');
  return listen<{ sessions: SessionInfo[] }>("session-list-changed", (event) => {
    console.log('📨 RAW session-list-changed event received:', event);
    callback(event.payload.sessions);
  });
}

/**
 * Listen for bead update events from the backend.
 * @param callback - Function to call when beads are updated
 * @returns A promise that resolves to an unlisten function for cleanup
 */
export async function onBeadsUpdated(callback: () => void): Promise<UnlistenFn> {
  return listen("beads-updated", () => {
    callback();
  });
}

/**
 * Listen for project list update events from the backend.
 * @param callback - Function to call when projects are updated
 * @returns A promise that resolves to an unlisten function for cleanup
 */
export async function onProjectsUpdated(callback: () => void): Promise<UnlistenFn> {
  return listen("projects-updated", () => {
    callback();
  });
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
