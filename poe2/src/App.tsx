import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type { Project, Node, Artifact } from './types';
import { usePoeProject } from './hooks/usePoeProject';
import ActivityFeed from './components/ActivityFeed';
import QueuePanel from './components/QueuePanel';
import ProjectCard from './components/ProjectCard';
import AgentHandover from './components/AgentHandover';
import ArtifactViewer from './components/ArtifactViewer';
import KnowledgePanel from './components/KnowledgePanel';
import StageGate from './components/StageGate';
import InterruptControls from './components/InterruptControls';

// ── CONOPS launcher ───────────────────────────────────────────────────────────

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
        <p className="text-xs text-neutral-500 uppercase tracking-widest">Run CONOPS</p>
        <textarea
          className="w-full h-40 bg-neutral-900 border border-neutral-700 rounded p-3 text-sm text-neutral-100 placeholder-neutral-600 resize-none focus:outline-none focus:border-neutral-500"
          placeholder="Describe the project. What are we building? Who is it for? What problem does it solve?"
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
    setQueueItems,
    addNode,
  } = usePoeProject(selectedId);

  // Modal state
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
      setOpenProjects(prev => {
        if (prev.some(p => p.id === project.id)) return prev;
        return [...prev, project];
      });
      setSelectedId(project.id);
    } catch (err) {
      console.error('Failed to open project:', err);
    }
  }

  async function handleCloseProject(projectId: string) {
    try {
      await invoke('close_project', { projectId });
    } catch (err) {
      console.error('Failed to close project:', err);
    }
    setOpenProjects(prev => prev.filter(p => p.id !== projectId));
    if (selectedId === projectId) setSelectedId(null);
  }

  // Active phase = first phase with lifecycleStage != 'complete'
  const activePhase = phases.find(p => p.lifecycleStage !== 'complete') ?? null;
  const runningAgentCount = nodes.filter(n => n.status === 'running').length;

  // Handover node lookup
  const handoverNode = handoverNodeId ? (nodes.find(n => n.id === handoverNodeId) ?? null) : null;

  return (
    <div className="flex h-screen bg-neutral-950 text-neutral-100 font-mono text-sm">
      {/* Sidebar */}
      <aside className="w-[220px] shrink-0 border-r border-neutral-800 flex flex-col">
        <div className="p-3 border-b border-neutral-800">
          <button
            onClick={() => void handleOpenProject()}
            className="w-full text-left px-2 py-1.5 rounded text-xs text-neutral-400 hover:bg-neutral-800 hover:text-neutral-100 transition-colors"
          >
            + Open Project…
          </button>
        </div>
        <div className="flex-1 overflow-y-auto p-2 space-y-1">
          {openProjects.map(p => (
            <ProjectCard
              key={p.id}
              project={p}
              nodes={selectedId === p.id ? nodes : []}
              phases={selectedId === p.id ? phases : []}
              queueCount={selectedId === p.id ? queueItems.length : 0}
              selected={selectedId === p.id}
              onSelect={() => setSelectedId(p.id)}
              onClose={() => void handleCloseProject(p.id)}
            />
          ))}
        </div>
        <div className="p-2 border-t border-neutral-800 text-xs text-neutral-600 text-center">
          POE2
        </div>
      </aside>

      {/* Main */}
      <main className="flex-1 flex flex-col overflow-hidden">
        {selectedId ? (
          <>
            {/* Project header with controls */}
            <div className="flex items-center justify-between px-3 py-1.5 border-b border-neutral-800 shrink-0">
              <div className="flex items-center gap-2">
                {activePhase && (
                  <span className="text-[11px] text-neutral-500 font-mono">
                    {activePhase.title}
                  </span>
                )}
              </div>
              <div className="flex items-center gap-2">
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

            {nodes.length === 0 ? (
              <ConopsLauncher
                projectId={selectedId}
                onLaunched={addNode}
              />
            ) : (
              <div className="flex-1 overflow-hidden flex flex-col">
                {/* Stage gate — shown when gate is held */}
                {activePhase?.gateHeld && (
                  <div className="px-3 py-2 border-b border-neutral-800 shrink-0">
                    <StageGate
                      phase={activePhase}
                      projectId={selectedId}
                      artifacts={artifacts}
                      onArtifactOpen={setArtifactViewer}
                      onAction={() => {
                        // Refresh will happen via event listeners naturally;
                        // nothing extra needed here
                      }}
                    />
                  </div>
                )}
                <div className="flex-1 overflow-hidden">
                  <ActivityFeed
                    items={feedItems}
                    nodes={nodes}
                    onHandoverOpen={setHandoverNodeId}
                  />
                </div>
              </div>
            )}
            <div className="h-[260px] shrink-0 border-t border-neutral-800">
              <QueuePanel
                items={queueItems}
                nodes={nodes}
                onResolve={async (itemId, resolution) => {
                  if (!selectedId) return;
                  await invoke('resolve_queue_item', {
                    itemId,
                    projectId: selectedId,
                    resolution,
                  });
                  setQueueItems(prev => prev.filter(q => q.id !== itemId));
                }}
              />
            </div>
          </>
        ) : (
          <div className="flex-1 flex items-center justify-center text-neutral-600">
            Open a project to get started
          </div>
        )}
      </main>

      {/* Modals */}
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
