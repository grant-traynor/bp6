import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Phase, Artifact, PhaseGateEvent } from '../types';

interface Props {
  phase: Phase;
  projectId: string;
  artifacts: Artifact[];
  onArtifactOpen: (artifact: Artifact) => void;
  onAction: () => void; // called after any gate action to refresh
}

export default function StageGate({ phase, projectId, artifacts, onArtifactOpen, onAction }: Props) {
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState<'advance' | 'revise' | 'rerun' | null>(null);
  const [gateEvent, setGateEvent] = useState<PhaseGateEvent | null>(null);

  const phaseArtifacts = artifacts.filter(a => a.phaseId === phase.id);

  // Listen for phase-gate-reached events to get completion summary
  useEffect(() => {
    let cancelled = false;
    const unlisten = listen<PhaseGateEvent>('phase-gate-reached', ({ payload }) => {
      if (cancelled) return;
      if (payload.phaseId !== phase.id) return;
      setGateEvent(payload);
    });
    return () => {
      cancelled = true;
      void unlisten.then(u => u());
    };
  }, [phase.id]);

  async function doAction(action: 'advance' | 'revise' | 'rerun') {
    setWorking(true);
    setError(null);
    try {
      const cmd =
        action === 'advance' ? 'advance_phase' :
        action === 'revise'  ? 'revise_phase' :
        'rerun_phase';
      await invoke(cmd, { phaseId: phase.id, projectId });
      onAction();
    } catch (e) {
      console.error(action, e);
      setError(String(e));
    } finally {
      setWorking(false);
      setConfirm(null);
    }
  }

  return (
    <div className="border border-amber-800 rounded p-3 bg-amber-950/30 space-y-3">
      {/* Header */}
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <span className="text-amber-400 text-[11px] font-semibold uppercase tracking-wide">
            ⬡ Stage Gate — {phase.title}
          </span>
          <span className="text-[10px] text-amber-600 font-mono">{phase.lifecycleStage}</span>
        </div>
        {gateEvent && (
          <span className="text-[10px] text-neutral-500 font-mono shrink-0">
            {gateEvent.completedCount}/{gateEvent.totalCount} tasks complete
          </span>
        )}
      </div>

      {/* Completion summary */}
      {gateEvent && (
        <div className="text-[11px] text-neutral-500">
          <span className="text-neutral-400">{gateEvent.phaseTitle}</span> phase reached the gate.{' '}
          {gateEvent.completedCount === gateEvent.totalCount
            ? 'All tasks complete.'
            : `${gateEvent.totalCount - gateEvent.completedCount} task(s) did not complete.`}
        </div>
      )}

      {/* Artifacts produced by this stage */}
      {phaseArtifacts.length > 0 && (
        <div>
          <p className="text-[10px] text-neutral-500 mb-1">Stage outputs</p>
          <div className="space-y-1">
            {phaseArtifacts.map(a => (
              <button
                key={a.id}
                className="block text-left text-[11px] text-emerald-500 hover:text-emerald-300 font-mono transition-colors"
                onClick={() => onArtifactOpen(a)}
              >
                📄 {a.filename}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Error message */}
      {error && (
        <p className="text-[11px] text-red-400 font-mono">{error}</p>
      )}

      {/* Confirmation dialog */}
      {confirm ? (
        <div className="space-y-2 border border-amber-900/60 rounded p-2 bg-amber-950/20">
          <p className="text-[12px] text-neutral-300">
            {confirm === 'advance' && 'Advance to the next stage? This will activate the next phase.'}
            {confirm === 'revise' && 'Reset this phase to allow revisions? Selected tasks will be re-queued.'}
            {confirm === 'rerun' && 'Re-run this entire phase? All tasks will be reset to pending.'}
          </p>
          <div className="flex gap-2">
            <button
              disabled={working}
              className="px-3 py-1 rounded text-[11px] bg-amber-700 hover:bg-amber-600 text-white disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
              onClick={() => void doAction(confirm)}
            >
              {working ? 'Working…' : 'Confirm'}
            </button>
            <button
              disabled={working}
              className="px-3 py-1 rounded text-[11px] border border-neutral-700 text-neutral-400 hover:text-neutral-200 disabled:opacity-40 transition-colors"
              onClick={() => setConfirm(null)}
            >
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <div className="flex gap-2">
          <button
            className="px-3 py-1 rounded text-[11px] bg-emerald-800 hover:bg-emerald-700 text-white transition-colors"
            onClick={() => setConfirm('advance')}
          >
            Advance →
          </button>
          <button
            className="px-3 py-1 rounded text-[11px] border border-neutral-700 text-neutral-400 hover:text-neutral-200 transition-colors"
            onClick={() => setConfirm('revise')}
          >
            Revise
          </button>
          <button
            className="px-3 py-1 rounded text-[11px] border border-neutral-700 text-neutral-400 hover:text-neutral-200 transition-colors"
            onClick={() => setConfirm('rerun')}
          >
            Re-run
          </button>
        </div>
      )}
    </div>
  );
}
