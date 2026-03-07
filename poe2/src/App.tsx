import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type { Project } from './types';
import { usePoeProject } from './hooks/usePoeProject';
import ActivityFeed from './components/ActivityFeed';
import QueuePanel from './components/QueuePanel';
import ProjectCard from './components/ProjectCard';

export default function App() {
  const [openProjects, setOpenProjects] = useState<Project[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const { nodes, queueItems, feedItems, setQueueItems } = usePoeProject(selectedId);

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
            <div className="flex-1 overflow-hidden">
              <ActivityFeed items={feedItems} />
            </div>
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
    </div>
  );
}
