import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface Props {
  projectId: string;
  activePhaseId: string | null;
  runningAgentCount: number;
}

export default function InterruptControls({ projectId, activePhaseId, runningAgentCount }: Props) {
  const [confirm, setConfirm] = useState<'pause' | 'abort' | null>(null);
  const [working, setWorking] = useState(false);

  async function pauseStage() {
    if (!activePhaseId) return;
    setWorking(true);
    try {
      await invoke('pause_stage', { phaseId: activePhaseId, projectId });
    } catch (e) {
      console.error('pause_stage', e);
    } finally {
      setWorking(false);
      setConfirm(null);
    }
  }

  async function abortProject() {
    setWorking(true);
    try {
      await invoke('abort_project', { projectId });
    } catch (e) {
      console.error('abort_project', e);
    } finally {
      setWorking(false);
      setConfirm(null);
    }
  }

  if (runningAgentCount === 0) return null;

  return (
    <div className="flex items-center gap-2">
      {confirm === 'pause' ? (
        <>
          <span className="text-[11px] text-neutral-400">
            Pause stage? {runningAgentCount} agent{runningAgentCount !== 1 ? 's' : ''} will be stopped.
          </span>
          <button
            disabled={working}
            className="px-2 py-0.5 rounded text-[10px] bg-amber-700 hover:bg-amber-600 text-white disabled:opacity-40"
            onClick={() => void pauseStage()}
          >
            Confirm
          </button>
          <button
            className="px-2 py-0.5 rounded text-[10px] text-neutral-500 hover:text-neutral-200"
            onClick={() => setConfirm(null)}
          >
            Cancel
          </button>
        </>
      ) : confirm === 'abort' ? (
        <>
          <span className="text-[11px] text-red-400">
            Abort project? All {runningAgentCount} agents will be stopped.
          </span>
          <button
            disabled={working}
            className="px-2 py-0.5 rounded text-[10px] bg-red-800 hover:bg-red-700 text-white disabled:opacity-40"
            onClick={() => void abortProject()}
          >
            Abort
          </button>
          <button
            className="px-2 py-0.5 rounded text-[10px] text-neutral-500 hover:text-neutral-200"
            onClick={() => setConfirm(null)}
          >
            Cancel
          </button>
        </>
      ) : (
        <>
          {activePhaseId && (
            <button
              className="px-2 py-0.5 rounded text-[10px] border border-amber-800 text-amber-500 hover:border-amber-600 hover:text-amber-300 transition-colors"
              onClick={() => setConfirm('pause')}
            >
              Pause stage
            </button>
          )}
          <button
            className="px-2 py-0.5 rounded text-[10px] border border-red-900 text-red-600 hover:border-red-700 hover:text-red-400 transition-colors"
            onClick={() => setConfirm('abort')}
          >
            Abort
          </button>
        </>
      )}
    </div>
  );
}
