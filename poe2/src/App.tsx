import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type { Project, Node, Artifact } from './types';
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

// ── CONOPS launcher ───────────────────────────────────────────────────────────
// Shown when a project has no nodes yet. Bootstraps the first task.

function ConopsLauncher({ projectId, onLaunched }: { projectId: string; onLaunched: (node: Node) => void }) {
  const [brief, setBrief] = useState('');
  const [running, setRunning] = useState(false);

  async function handleSubmit() {
    const trimmed = brief.trim();
    if (!trimmed) return;
    setRunning(true);
    try {
      const node = await invoke<Node>('create_node', {
        input: {
          projectId,
          phaseId: null,
          parentId: null,
          nodeType: 'task',
          title: 'Develop CONOPS',
          description: trimmed,
          skillId: 'operational-analyst',
        },
      });
      onLaunched(node);
    } catch (err) {
      console.error('Failed to create CONOPS task:', err);
      setRunning(false);
    }
  }

  return (
    <div className="flex-1 flex flex-col items-center justify-center p-8 gap-4">
      <div className="w-full max-w-xl flex flex-col gap-3">
        <p className="text-[11px] text-neutral-500 uppercase tracking-widest">Start project — CONOPS</p>
        <p className="text-[11px] text-neutral-600">
          Describe the project. The operational-analyst will conduct a structured elicitation.
        </p>
        <textarea
          className="w-full h-40 bg-neutral-900 border border-neutral-700 rounded p-3 text-sm text-neutral-100 placeholder-neutral-600 resize-none focus:outline-none focus:border-neutral-500"
          placeholder="What are we building? Who is it for? What problem does it solve?"
          value={brief}
          onChange={e => setBrief(e.target.value)}
          disabled={running}
          autoFocus
        />
        <button
          onClick={() => void handleSubmit()}
          disabled={running || !brief.trim()}
          className="self-end px-4 py-2 bg-neutral-100 text-neutral-950 text-xs font-bold rounded hover:bg-white disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
        >
          {running ? 'Starting…' : 'Run CONOPS →'}
        </button>
      </div>
    </div>
  );
}

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
    addNode,
  } = usePoeProject(selectedId);

  const [artifactViewer, setArtifactViewer] = useState<Artifact | null>(null);
  const [showKnowledge, setShowKnowledge] = useState(false);

  useEffect(() => {
    invoke<Project[]>('list_projects')
      .then(projects => setOpenProjects(projects))
      .catch(console.error);
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
  const activePhase = phases.find(p => p.lifecycleStage !== 'complete') ?? null;
  const runningAgentCount = nodes.filter(n => n.status === 'running').length;
  const pendingQueueCount = queueItems.filter(q => q.resolvedAt === null).length;
  const handoverNode = handoverNodeId ? (nodes.find(n => n.id === handoverNodeId) ?? null) : null;

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

              {nodes.length === 0 ? (
                /* Empty project — show CONOPS launcher */
                <ConopsLauncher projectId={selectedId} onLaunched={addNode} />
              ) : (
                <>
                  {/* Pane 2a — Phase × Scope (WBS node tree) */}
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
                    />
                  </div>

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

        {/* Pane 3 — Queue + (Advisor placeholder) */}
        <aside className="w-[280px] shrink-0 border-l border-neutral-800 flex flex-col overflow-hidden">
          <div className="flex-1 overflow-hidden">
            <QueuePanel
              items={queueItems}
              nodes={nodes}
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
          </div>

          {/* Advisor placeholder (Phase 3) */}
          <div className="shrink-0 border-t border-neutral-800 p-3">
            <p className="text-[10px] uppercase tracking-widest text-neutral-700 mb-1">Advisor</p>
            <p className="text-[11px] text-neutral-700">Available in Phase 3</p>
          </div>
        </aside>

      </div>

      {/* ── Modals ─────────────────────────────────────────────────────────── */}
      {handoverNodeId && (
        <AgentHandover
          nodeId={handoverNodeId}
          node={handoverNode}
          onClose={() => setHandoverNodeId(null)}
        />
      )}
      {artifactViewer && selectedId && (
        <ArtifactViewer
          artifact={artifactViewer}
          projectId={selectedId}
          onClose={() => setArtifactViewer(null)}
        />
      )}
      {showKnowledge && selectedId && (
        <KnowledgePanel
          entries={knowledgeEntries}
          projectId={selectedId}
          onClose={() => setShowKnowledge(false)}
        />
      )}
    </div>
  );
}
