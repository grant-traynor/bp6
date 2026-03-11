import { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type { Project, Node, Artifact, Edge, AdvisorTurn } from './types';
import { usePoeProject } from './hooks/usePoeProject';
import ActivityFeed from './components/ActivityFeed';
import QueuePanel from './components/QueuePanel';
import ProjectCard from './components/ProjectCard';
import NodeTree from './components/NodeTree';
import AgentHandover from './components/AgentHandover';
import ArtifactViewer from './components/ArtifactViewer';
import KnowledgePanel from './components/KnowledgePanel';
import StageGate from './components/StageGate';
import InterruptControls from './components/InterruptControls';
import DebugPanel from './components/DebugPanel';
import PhaseMatrix from './components/PhaseMatrix';
import TaskDetailPanel from './components/TaskDetailPanel';
import PlanComposer from './components/PlanComposer';


// ── App ───────────────────────────────────────────────────────────────────────

export default function App() {
  const [openProjects, setOpenProjects] = useState<Project[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const {
    nodes,
    queueItems,
    feedItems,
    phases,
    artifacts,
    knowledgeEntries,
    handoverNodeId,
    setHandoverNodeId,
    streamEventMap,
  } = usePoeProject(selectedId);

  const [debugNodeId, setDebugNodeId] = useState<string | null>(null);

  const [artifactViewer, setArtifactViewer] = useState<Artifact | null>(null);
  // taskId of an active chat session — opens ArtifactViewer in chat-only mode
  const [chatTaskId, setChatTaskId] = useState<string | null>(null);
  const [showKnowledge, setShowKnowledge] = useState(false);

  // Phase 3: edges, task detail panel, advisor session
  const [edges, setEdges] = useState<Edge[]>([]);
  const [taskDetailNode, setTaskDetailNode] = useState<Node | null>(null);
  const [advisorTaskId, setAdvisorTaskId] = useState<string | null>(null);

  useEffect(() => {
    invoke<Project[]>('list_projects')
      .then(projects => setOpenProjects(projects))
      .catch(console.error);
  }, []);

  // Load edges when project changes
  useEffect(() => {
    if (!selectedId) { setEdges([]); return; }
    invoke<Edge[]>('list_edges', { projectId: selectedId })
      .then(setEdges)
      .catch(console.error);
  }, [selectedId]);

  // Listen for poe-advisor-turn to know the active advisor task ID
  useEffect(() => {
    const unlisten = listen<{ turnId: string; taskId: string; content: string }>('poe-advisor-turn', ({ payload }) => {
      setAdvisorTaskId(payload.taskId);
    });
    return () => { void unlisten.then(u => u()); };
  }, []);

  // Auto-open ArtifactViewer in chat mode when agent emits poe:chat
  useEffect(() => {
    const unlisten = listen<{ turnId: string; taskId: string; content: string }>(
      'poe-chat-turn',
      ({ payload }) => {
        // Only open if not already viewing this task
        setChatTaskId(prev => prev === payload.taskId ? prev : payload.taskId);
      }
    );
    return () => { void unlisten.then(u => u()); };
  }, []);

  async function handleOpenProject() {
    const selected = await open({ directory: true, multiple: false });
    if (!selected || typeof selected !== 'string') return;
    try {
      const project = await invoke<Project>('open_project', { path: selected });
      setOpenProjects(prev => prev.some(p => p.id === project.id) ? prev : [...prev, project]);
      setSelectedId(project.id);
    } catch (err) {
      console.error('Failed to open project:', err);
    }
  }

  async function handleCloseProject(projectId: string) {
    try { await invoke('close_project', { projectId }); } catch { /* ignore */ }
    setOpenProjects(prev => prev.filter(p => p.id !== projectId));
    if (selectedId === projectId) setSelectedId(null);
  }

  // Derived state
  // Prioritise: gated phase first (needs human action), then any non-complete
  const activePhase =
    phases.find(p => p.gateHeld) ??
    phases.find(p => p.lifecycleStage !== 'complete') ??
    null;
  const runningAgentCount = nodes.filter(n => n.status === 'running').length;
  const pendingQueueCount = queueItems.filter(q => q.resolvedAt === null).length;
  const handoverNode = handoverNodeId ? (nodes.find(n => n.id === handoverNodeId) ?? null) : null;
  const debugNode = debugNodeId ? (nodes.find(n => n.id === debugNodeId) ?? null) : null;
  const debugStreamEvents = debugNodeId ? (streamEventMap.get(debugNodeId) ?? []) : [];

  // Global stats across all open projects: for now use selected project's data.
  // Phase 3 will track all projects simultaneously.
  const globalRunning = runningAgentCount;
  const globalQueue = pendingQueueCount;

  return (
    <div className="flex flex-col h-screen bg-neutral-950 text-neutral-100 font-mono text-sm overflow-hidden">

      {/* ── Global bar ─────────────────────────────────────────────────────── */}
      <header className="flex items-center justify-between px-4 py-1.5 border-b border-neutral-800 shrink-0 bg-neutral-900/60">
        <span className="text-[11px] font-bold tracking-widest text-neutral-500 uppercase">POE</span>
        <div className="flex items-center gap-4 text-[11px] text-neutral-500">
          {globalRunning > 0 ? (
            <span>
              <span className="text-emerald-400 font-bold">{globalRunning}</span>
              <span className="text-neutral-600"> agent{globalRunning !== 1 ? 's' : ''} running</span>
            </span>
          ) : (
            <span className="text-neutral-700">no agents running</span>
          )}
          {globalQueue > 0 ? (
            <span>
              <span className="text-amber-400 font-bold">{globalQueue}</span>
              <span className="text-neutral-600"> decision{globalQueue !== 1 ? 's' : ''} pending</span>
            </span>
          ) : null}
          <span className="text-neutral-700">{openProjects.length} project{openProjects.length !== 1 ? 's' : ''}</span>
        </div>
      </header>

      {/* ── Three-column body ───────────────────────────────────────────────── */}
      <div className="flex flex-1 overflow-hidden">

        {/* Pane 1 — Projects sidebar */}
        <aside className="w-[220px] shrink-0 border-r border-neutral-800 flex flex-col">
          <div className="p-2 border-b border-neutral-800">
            <button
              onClick={() => void handleOpenProject()}
              className="w-full text-left px-2 py-1.5 rounded text-xs text-neutral-500 hover:bg-neutral-800 hover:text-neutral-100 transition-colors"
            >
              + Open Project…
            </button>
          </div>
          <div className="flex-1 overflow-y-auto p-2 space-y-1">
            {openProjects.length === 0 && (
              <p className="text-[11px] text-neutral-700 px-2 py-4 text-center">No projects open</p>
            )}
            {openProjects.map(p => (
              <ProjectCard
                key={p.id}
                project={p}
                nodes={selectedId === p.id ? nodes : []}
                phases={selectedId === p.id ? phases : []}
                queueCount={selectedId === p.id ? pendingQueueCount : 0}
                selected={selectedId === p.id}
                onSelect={() => setSelectedId(p.id)}
                onClose={() => void handleCloseProject(p.id)}
              />
            ))}
          </div>
        </aside>

        {/* Pane 2 — Selected project: matrix + feed */}
        <main className="flex-1 flex flex-col overflow-hidden">
          {!selectedId ? (
            <div className="flex-1 flex items-center justify-center text-neutral-700 text-xs">
              Select a project
            </div>
          ) : (
            <>
              {/* Project header */}
              <div className="flex items-center justify-between px-3 py-1.5 border-b border-neutral-800 shrink-0">
                <div className="flex items-center gap-2 text-[11px] text-neutral-500 min-w-0">
                  {activePhase ? (
                    <>
                      <span className="text-neutral-400 font-semibold truncate">{activePhase.title}</span>
                      <span className="text-neutral-700">·</span>
                      <span className="text-neutral-600">{activePhase.lifecycleStage}</span>
                    </>
                  ) : (
                    <span className="text-neutral-700">No active phase</span>
                  )}
                </div>
                <div className="flex items-center gap-2 shrink-0">
                  <button
                    className="px-2 py-0.5 rounded text-[10px] border border-teal-900 text-teal-600 hover:border-teal-700 hover:text-teal-400 transition-colors"
                    onClick={() => setShowKnowledge(true)}
                  >
                    Knowledge
                  </button>
                  <InterruptControls
                    projectId={selectedId}
                    activePhaseId={activePhase?.id ?? null}
                    runningAgentCount={runningAgentCount}
                  />
                </div>
              </div>

              {/* Stage gate banner — shown when phase gate is held */}
              {activePhase?.gateHeld && (
                <div className="px-3 py-2 border-b border-amber-900/40 shrink-0">
                  <StageGate
                    phase={activePhase}
                    projectId={selectedId}
                    artifacts={artifacts}
                    onArtifactOpen={setArtifactViewer}
                    onAction={() => { /* event listeners handle refresh */ }}
                  />
                </div>
              )}

              {nodes.length === 0 && phases.length === 0 ? (
                /* Empty project — show Plan Composer to design phase DAG */
                <PlanComposer projectId={selectedId} onComplete={() => { /* phases reload via poe-phase-update */ }} />
              ) : (
                <>
                  {/* Pane 2a — Phase × Scope matrix (Phase 3) or WBS tree */}
                  {phases.length > 0 ? (
                    <div className="shrink-0 max-h-[45%] overflow-hidden border-b border-neutral-800 flex flex-col">
                      <div className="px-3 py-1 border-b border-neutral-800/60 flex items-center justify-between shrink-0">
                        <span className="text-[10px] uppercase tracking-widest text-neutral-600">Phase × Scope</span>
                        <span className="text-[10px] text-neutral-700">
                          {runningAgentCount > 0 && (
                            <span className="text-emerald-600">{runningAgentCount} running · </span>
                          )}
                          {nodes.filter(n => n.status === 'pending').length} pending ·{' '}
                          {nodes.filter(n => n.status === 'complete').length} done
                        </span>
                      </div>
                      <div className="flex-1 overflow-hidden">
                        <PhaseMatrix
                          phases={phases}
                          nodes={nodes}
                          edges={edges}
                          onTaskClick={setTaskDetailNode}
                        />
                      </div>
                    </div>
                  ) : (
                    <div className="shrink-0 max-h-[40%] overflow-y-auto border-b border-neutral-800">
                      <div className="px-3 py-1 border-b border-neutral-800/60 flex items-center justify-between">
                        <span className="text-[10px] uppercase tracking-widest text-neutral-600">Scope</span>
                        <span className="text-[10px] text-neutral-700">
                          {runningAgentCount > 0 && (
                            <span className="text-emerald-600">{runningAgentCount} running · </span>
                          )}
                          {nodes.filter(n => n.status === 'pending').length} pending ·{' '}
                          {nodes.filter(n => n.status === 'complete').length} done
                        </span>
                      </div>
                      <NodeTree
                        nodes={nodes}
                        onHandoverOpen={setHandoverNodeId}
                        onDebugOpen={setDebugNodeId}
                      />
                    </div>
                  )}

                  {/* Pane 2b — Activity feed */}
                  <div className="flex-1 overflow-hidden flex flex-col">
                    <div className="px-3 py-1 border-b border-neutral-800/60 shrink-0">
                      <span className="text-[10px] uppercase tracking-widest text-neutral-600">Activity</span>
                    </div>
                    <div className="flex-1 overflow-hidden">
                      <ActivityFeed
                        items={feedItems}
                        nodes={nodes}
                        onHandoverOpen={setHandoverNodeId}
                      />
                    </div>
                  </div>
                </>
              )}
            </>
          )}
        </main>

        {/* Pane 3 — Queue + Advisor */}
        <aside className="w-[280px] shrink-0 border-l border-neutral-800 flex flex-col overflow-hidden">
          <QueuePanel
            items={queueItems}
            nodes={nodes}
            projectId={selectedId ?? ''}
            advisorTaskId={advisorTaskId}
            onResolve={async (itemId, resolution) => {
              if (!selectedId) return;
              // Fire and forget — poe-decision-resolved event listener
              // updates state in-place (keeping resolved items for thread history).
              await invoke('resolve_queue_item', {
                itemId,
                projectId: selectedId,
                resolution,
              });
            }}
          />
        </aside>

      </div>

      {/* ── Modals ─────────────────────────────────────────────────────────── */}
      {debugNodeId && debugNode && (
        <DebugPanel
          node={debugNode}
          streamEvents={debugStreamEvents}
          onHandoverOpen={setHandoverNodeId}
          onClose={() => setDebugNodeId(null)}
        />
      )}
      {handoverNodeId && (
        <AgentHandover
          nodeId={handoverNodeId}
          node={handoverNode}
          onClose={() => setHandoverNodeId(null)}
        />
      )}
      {(artifactViewer || chatTaskId) && selectedId && (
        <ArtifactViewer
          artifact={artifactViewer ?? undefined}
          projectId={selectedId}
          taskId={chatTaskId}
          onClose={() => { setArtifactViewer(null); setChatTaskId(null); }}
        />
      )}
      {showKnowledge && selectedId && (
        <KnowledgePanel
          entries={knowledgeEntries}
          projectId={selectedId}
          onClose={() => setShowKnowledge(false)}
        />
      )}
      {taskDetailNode && selectedId && (
        <TaskDetailPanel
          node={taskDetailNode}
          projectId={selectedId}
          nodes={nodes}
          artifacts={artifacts}
          edges={edges}
          onClose={() => setTaskDetailNode(null)}
          onArtifactOpen={setArtifactViewer}
        />
      )}
    </div>
  );
}
