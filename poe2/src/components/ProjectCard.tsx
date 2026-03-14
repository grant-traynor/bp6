import type { Project, Node, Phase } from '../types';

interface Props {
  project: Project;
  nodes: Node[];
  phases: Phase[];
  queueCount: number;
  selected: boolean;
  onSelect: () => void;
  onClose: () => void;
}

export default function ProjectCard({ project, nodes, phases, queueCount, selected, onSelect, onClose }: Props) {
  const running = nodes.filter(n => n.status === 'running').length;
  const waiting = nodes.filter(n => n.status === 'waiting').length;
  const pending = nodes.filter(n => n.status === 'pending').length;

  const activePhase = phases.find(p => p.status !== 'complete');
  const gateHeld = activePhase?.status === 'gate' || activePhase?.status === 'paused';

  return (
    <div
      className={`relative rounded px-2.5 py-2 cursor-pointer transition-colors group ${
        selected ? 'bg-neutral-800 text-neutral-100' : 'text-neutral-400 hover:bg-neutral-800/60 hover:text-neutral-200'
      }`}
      onClick={onSelect}
    >
      <div className="flex items-center justify-between gap-1">
        <span className="text-xs font-semibold truncate flex-1">{project.name}</span>
        <div className="flex items-center gap-1 shrink-0">
          {gateHeld && (
            <span className="bg-amber-600 text-black text-[9px] font-bold px-1 py-0.5 rounded leading-none">GATE</span>
          )}
          {queueCount > 0 && (
            <span className="bg-amber-500 text-black text-[9px] font-bold px-1 py-0.5 rounded-full leading-none">{queueCount}</span>
          )}
          <button
            className="opacity-0 group-hover:opacity-100 text-neutral-600 hover:text-neutral-300 transition-opacity text-xs leading-none p-0.5"
            onClick={e => { e.stopPropagation(); onClose(); }}
            title="Close project"
          >×</button>
        </div>
      </div>
      <p className="text-[11px] text-neutral-600 mt-0.5">
        {activePhase && <span className="text-neutral-500">{activePhase.title} · </span>}
        {running > 0 && <span className="text-emerald-600">{running} running</span>}
        {waiting > 0 && <span className="text-amber-600"> · {waiting} waiting</span>}
        {pending > 0 && <span> · {pending} pending</span>}
      </p>
    </div>
  );
}
