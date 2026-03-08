import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Phase, Artifact } from '../types';

interface Props {
  phase: Phase;
  projectId: string;
  artifacts: Artifact[];
  onArtifactOpen: (artifact: Artifact) => void;
  onAction: () => void; // called after any gate action to refresh
}

export default function StageGate({ phase, projectId, artifacts, onArtifactOpen, onAction }: Props) {
  const [working, setWorking] = useState(false);
  const [confirm, setConfirm] = useState<'advance' | 'revise' | 'rerun' | null>(null);

  const phaseArtifacts = artifacts.filter(a => a.phaseId === phase.id);

  async function doAction(action: 'advance' | 'revise' | 'rerun') {
    setWorking(true);
    try {
      const cmd = action === 'advance' ? 'advance_stage_gate'
                : action === 'revise'  ? 'revise_stage'
                : 'rerun_stage';
      await invoke(cmd, { phaseId: phase.id, projectId });
      onAction();
    } catch (e) {
      console.error(action, e);
    } finally {
      setWorking(false);
      setConfirm(null);
    }
  }

  return (
    <div className="border border-amber-800 rounded p-3 bg-amber-950/30 space-y-3">
      <div className="flex items-center gap-2">
        <span className="text-amber-400 text-[11px] font-semibold uppercase tracking-wide">
          ⬡ Stage Gate — {phase.title}
        </span>
        <span className="text-[10px] text-amber-600 font-mono">{phase.lifecycleStage}</span>
      </div>

      {phaseArtifacts.length > 0 && (
        <div>
          <p className="text-[10px] text-neutral-500 mb-1">Stage outputs</p>
          <div className="space-y-1">
            {phaseArtifacts.map(a => (
              <button
                key={a.id}
                className="block text-left text-[11px] text-green-400 hover:text-green-300 font-mono"
                onClick={() => onArtifactOpen(a)}
              >
                📄 {a.filename}
              </button>
            ))}
          </div>
        </div>
      )}

      {confirm ? (
        <div className="space-y-2">
          <p className="text-[12px] text-neutral-300">
            {confirm === 'advance' && 'Advance to the next stage?'}
            {confirm === 'revise' && 'Reset phase to Planning to allow revisions?'}
            {confirm === 'rerun' && 'Re-queue all tasks and re-run this stage?'}
          </p>
          <div className="flex gap-2">
            <button
              disabled={working}
              className="px-3 py-1 rounded text-[11px] bg-amber-700 hover:bg-amber-600 text-white disabled:opacity-40"
              onClick={() => void doAction(confirm)}
            >
              Confirm
            </button>
            <button
              className="px-3 py-1 rounded text-[11px] border border-neutral-700 text-neutral-400 hover:text-neutral-200"
              onClick={() => setConfirm(null)}
            >
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <div className="flex gap-2">
          <button
            className="px-3 py-1 rounded text-[11px] bg-emerald-800 hover:bg-emerald-700 text-white"
            onClick={() => setConfirm('advance')}
          >
            Advance →
          </button>
          <button
            className="px-3 py-1 rounded text-[11px] border border-neutral-700 text-neutral-400 hover:text-neutral-200"
            onClick={() => setConfirm('revise')}
          >
            Revise
          </button>
          <button
            className="px-3 py-1 rounded text-[11px] border border-neutral-700 text-neutral-400 hover:text-neutral-200"
            onClick={() => setConfirm('rerun')}
          >
            Re-run
          </button>
        </div>
      )}
    </div>
  );
}
