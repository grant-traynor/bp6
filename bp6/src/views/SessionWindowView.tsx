import { useEffect, useState, useCallback, useRef } from "react";
import { saveWindowState, PERSONA_ICONS, startAgentSession, createSessionWindow, terminateSession, fetchBeads, updateBead, closeBead, reopenBead, claimBead, beadNodeToBead, type CliBackend, type Bead, type BeadNode } from "../api";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useSessionStore } from "../stores/sessionStore";
import { BeadDetailModal } from "../components/shared/BeadDetailModal";
import { FileText } from "lucide-react";
import { Terminal } from "../components/terminal/Terminal";

const DEFAULT_PANEL_WIDTH = 420;
const MIN_PANEL_WIDTH = 280;
const MAX_PANEL_WIDTH = 800;

interface SessionWindowViewProps {
  sessionId: string;
}

export function SessionWindowView({ sessionId }: SessionWindowViewProps) {
  const sessions = useSessionStore((state) => state.sessions);
  const sessionMeta = sessions.find((s) => s.sessionId === sessionId);

  const sessionPersona = sessionMeta?.persona || "specialist";
  const sessionBeadId = sessionMeta?.beadId || null;
  const sessionBackendId = sessionMeta?.backendId || "claude";
  const sessionTask = sessionMeta?.task || null;
  const sessionProjectPath = sessionMeta?.projectPath || null;
  const sessionBeadTitle = sessionMeta?.beadTitle || null;
  const sessionBeadDescriptionPreview = sessionMeta?.beadDescriptionPreview || null;
  const projectName = sessionProjectPath
    ? sessionProjectPath.split("/").filter(Boolean).pop() ?? null
    : null;

  const personaIcon = PERSONA_ICONS[sessionPersona] || PERSONA_ICONS[sessionMeta?.role || ""] || "🤖";
  const sessionRole = sessionMeta?.role || null;
  const personaLabel = sessionPersona === "specialist" && sessionRole
    ? `${sessionRole} specialist`
    : sessionPersona;

  const [isRestarting, setIsRestarting] = useState(false);
  const [showBeadDetail, setShowBeadDetail] = useState(false);
  const [beadNode, setBeadNode] = useState<BeadNode | null>(null);
  const [editForm, setEditForm] = useState<Partial<BeadNode>>({});

  // Panel width — persisted to localStorage
  const [panelWidth, setPanelWidth] = useState<number>(() => {
    try { return parseInt(localStorage.getItem("sessionBeadPanelWidth") || "") || DEFAULT_PANEL_WIDTH; } catch { return DEFAULT_PANEL_WIDTH; }
  });
  const panelWidthRef = useRef(panelWidth);
  useEffect(() => { panelWidthRef.current = panelWidth; }, [panelWidth]);
  useEffect(() => {
    try { localStorage.setItem("sessionBeadPanelWidth", String(panelWidth)); } catch { /* ignore */ }
  }, [panelWidth]);

  const handlePanelResizeStart = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    const startX = e.clientX;
    const startW = panelWidthRef.current;
    const onMove = (ev: MouseEvent) => {
      const delta = startX - ev.clientX; // drag left = wider panel
      setPanelWidth(Math.max(MIN_PANEL_WIDTH, Math.min(MAX_PANEL_WIDTH, startW + delta)));
    };
    const onUp = () => {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  }, []);

  const beadToBeadNode = useCallback((b: Bead): BeadNode => ({
    id: b.id, title: b.title, description: b.description, status: b.status,
    priority: b.priority, issueType: b.issue_type, estimate: b.estimate,
    dependencies: b.dependencies, owner: b.owner, assignee: b.assignee,
    createdAt: b.created_at, createdBy: b.created_by, updatedAt: b.updated_at,
    labels: b.labels, acceptanceCriteria: b.acceptance_criteria,
    closedAt: b.closed_at, closeReason: b.close_reason, isFavorite: b.is_favorite,
    parent: b.parent, externalReference: b.external_reference,
    design: (b as any).design, notes: (b as any).notes,
    children: [], isBlocked: false, isCritical: false, blockingIds: [],
    depth: 0, cellOffset: 0, cellCount: 1, isExpanded: true, isVisible: true,
  }), []);

  const refreshBead = useCallback(async () => {
    if (!sessionBeadId) return;
    try {
      const all = await fetchBeads();
      const found = all.find(b => b.id === sessionBeadId);
      if (found) setBeadNode(beadToBeadNode(found));
    } catch { /* non-critical */ }
  }, [sessionBeadId, beadToBeadNode]);

  useEffect(() => { refreshBead(); }, [refreshBead]);

  const handleSaveEdit = useCallback(async () => {
    if (!editForm?.id) return;
    await updateBead(beadNodeToBead(editForm) as Parameters<typeof updateBead>[0]);
    await refreshBead();
  }, [editForm, refreshBead]);

  const handleCloseBead = useCallback(async (id: string) => {
    await closeBead(id);
    await refreshBead();
  }, [refreshBead]);

  const handleReopenBead = useCallback(async (id: string) => {
    await reopenBead(id);
    await refreshBead();
  }, [refreshBead]);

  const handleClaimBead = useCallback(async (id: string) => {
    await claimBead(id);
    await refreshBead();
  }, [refreshBead]);

  const handleNewSession = async () => {
    if (isRestarting) return;
    setIsRestarting(true);
    try {
      const newSessionId = await startAgentSession(
        sessionPersona,
        sessionTask || undefined,
        sessionBeadId || undefined,
        sessionBackendId as CliBackend,
        sessionMeta?.role || undefined,
        sessionProjectPath || undefined,
        true
      );
      await useSessionStore.getState().refreshSessions();
      await createSessionWindow(newSessionId, getCurrentWindow().label);
      await terminateSession(sessionId);
    } catch (error) {
      console.error("Failed to create new session:", error);
      setIsRestarting(false);
    }
  };

  useEffect(() => {
    const title = [sessionBeadId || "Untracked", personaLabel, sessionTask, sessionBackendId]
      .filter(Boolean).join(" · ");
    getCurrentWindow().setTitle(title).catch(() => {});
  }, [sessionBeadId, sessionPersona, sessionTask, sessionBackendId]);

  // Window state persistence
  useEffect(() => {
    const saveCurrentState = async () => {
      try {
        const win = getCurrentWindow();
        const scaleFactor = await win.scaleFactor();
        const physPos = await win.outerPosition();
        const physSize = await win.outerSize();
        const position = physPos.toLogical(scaleFactor);
        const size = physSize.toLogical(scaleFactor);
        const isMaximized = await win.isMaximized();
        await saveWindowState(sessionId, Math.round(position.x), Math.round(position.y),
          Math.round(size.width), Math.round(size.height), isMaximized);
      } catch { /* ignore */ }
    };
    let saveTimeout: ReturnType<typeof setTimeout> | null = null;
    const debouncedSave = () => { if (saveTimeout) clearTimeout(saveTimeout); saveTimeout = setTimeout(saveCurrentState, 500); };
    let unlistenResize: (() => void) | null = null;
    let unlistenMove: (() => void) | null = null;
    const setupListeners = async () => {
      const win = getCurrentWindow();
      unlistenResize = await win.onResized(debouncedSave);
      unlistenMove = await win.onMoved(debouncedSave);
    };
    setupListeners();
    return () => {
      if (saveTimeout) clearTimeout(saveTimeout);
      if (unlistenResize) unlistenResize();
      if (unlistenMove) unlistenMove();
      saveCurrentState();
    };
  }, [sessionId]);

  const hasBead = !!sessionBeadId;

  return (
    <div className="flex flex-col h-screen w-screen overflow-hidden bg-[var(--background-primary)]">
      {/* ── Header ── */}
      <div className="flex-shrink-0 border-b border-[var(--border-primary)] bg-[var(--background-secondary)] px-3 py-2 text-xs">
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-base leading-none">{personaIcon}</span>
          <span className="font-semibold text-[var(--text-primary)] capitalize whitespace-nowrap">{personaLabel}</span>
          {sessionTask && (
            <>
              <span className="text-[var(--text-muted)]">·</span>
              <span className="text-[var(--text-secondary)] truncate">{sessionTask}</span>
            </>
          )}
          <div className="ml-auto flex items-center gap-2 text-[var(--text-muted)] whitespace-nowrap">
            {projectName && <span className="text-[var(--text-secondary)]">📁 {projectName}</span>}
            <span>{sessionBackendId}</span>
          </div>
        </div>

        {/* Bead context row + toggle button */}
        {hasBead && (
          <div className="flex items-center justify-between mt-1 pl-6">
            <button
              onClick={() => { if (!showBeadDetail) refreshBead(); setShowBeadDetail(p => !p); }}
              title={showBeadDetail ? "Hide bead detail" : "Show bead detail"}
              className={`flex items-center gap-1.5 px-2 py-1 rounded-lg border transition-all text-xs ${
                showBeadDetail
                  ? "bg-[var(--accent-primary)]/10 border-[var(--accent-primary)]/30 text-[var(--accent-primary)]"
                  : "bg-[var(--background-tertiary)] border-[var(--border-primary)] text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:border-[var(--border-primary)]"
              }`}
            >
              <FileText size={11} strokeWidth={2.5} />
              <span className="font-mono font-bold">{sessionBeadId}</span>
              {sessionBeadTitle && <span className="text-[var(--text-secondary)] truncate max-w-[200px]">{sessionBeadTitle}</span>}
            </button>
            <button
              onClick={handleNewSession}
              disabled={isRestarting}
              className="px-2 py-0.5 rounded text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-[var(--background-tertiary)] disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            >
              {isRestarting ? "↻ Starting…" : "+ New Session"}
            </button>
          </div>
        )}

        {/* No bead: just the new session button */}
        {!hasBead && (
          <div className="flex justify-end mt-1">
            <button
              onClick={handleNewSession}
              disabled={isRestarting}
              className="px-2 py-0.5 rounded text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-[var(--background-tertiary)] disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            >
              {isRestarting ? "↻ Starting…" : "+ New Session"}
            </button>
          </div>
        )}

        {/* Description preview (when panel closed) */}
        {sessionBeadDescriptionPreview && !showBeadDetail && (
          <div className="mt-0.5 pl-6 text-[10px] leading-snug text-[var(--text-muted)] opacity-70 line-clamp-2">
            {sessionBeadDescriptionPreview}
          </div>
        )}
      </div>

      {/* ── Body: terminal + optional side panel ── */}
      <div className="flex-1 flex flex-row min-h-0 overflow-hidden">
        {/* Terminal */}
        <div className="flex-1 min-w-0">
          <Terminal sessionId={sessionId} projectPath={sessionProjectPath || undefined} />
        </div>

        {/* Drag handle + bead panel */}
        {showBeadDetail && beadNode && (
          <>
            {/* Drag handle */}
            <div
              onMouseDown={handlePanelResizeStart}
              className="w-1 cursor-col-resize bg-[var(--border-primary)] hover:bg-[var(--accent-primary)] transition-colors shrink-0"
              title="Drag to resize"
            />
            {/* Bead detail panel */}
            <div className="shrink-0 overflow-hidden" style={{ width: panelWidth }}>
              <BeadDetailModal
                panel
                bead={beadNode}
                open={true}
                onClose={() => setShowBeadDetail(false)}
                isCreating={false}
                editForm={editForm}
                beads={beadNode ? [beadNode] : []}
                setIsCreating={() => {}}
                setSelectedBead={setBeadNode}
                setEditForm={setEditForm}
                handleSaveEdit={handleSaveEdit}
                handleSaveCreate={async () => {}}
                handleCloseBead={handleCloseBead}
                handleReopenBead={handleReopenBead}
                handleClaimBead={handleClaimBead}
                toggleFavorite={async () => {}}
                onOpenChat={() => {}}
              />
            </div>
          </>
        )}
      </div>
    </div>
  );
}
