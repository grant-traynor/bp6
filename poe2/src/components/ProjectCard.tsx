import type { Project, Node } from '../types';

interface Props {
  project: Project;
  nodes: Node[];
  queueCount: number;
  selected: boolean;
  onSelect: () => void;
  onClose: () => void;
}

export default function ProjectCard({ project, nodes, queueCount, selected, onSelect, onClose }: Props) {
  const running = nodes.filter(n => n.status === 'running').length;
  const done = nodes.filter(n => n.status === 'complete').length;
  const pending = nodes.filter(n => n.status === 'pending').length;

  return (
    <div
      className={`relative rounded px-2.5 py-2 cursor-pointer transition-colors group ${
        selected
          ? 'bg-neutral-800 text-neutral-100'
          : 'text-neutral-400 hover:bg-neutral-800/60 hover:text-neutral-200'
      }`}
      onClick={onSelect}
    >
      <div className="flex items-center justify-between gap-1">
        <span className="text-xs font-semibold truncate flex-1">{project.name}</span>
        <div className="flex items-center gap-1 shrink-0">
          {queueCount > 0 && (
            <span className="bg-amber-500 text-black text-[9px] font-bold px-1 py-0.5 rounded-full leading-none">
              {queueCount}
            </span>
          )}
          <button
            className="opacity-0 group-hover:opacity-100 text-neutral-600 hover:text-neutral-300 transition-opacity text-xs leading-none p-0.5"
            onClick={e => {
              e.stopPropagation();
              onClose();
            }}
            title="Close project"
          >
            ×
          </button>
        </div>
      </div>
      {nodes.length > 0 && (
        <p className="text-[11px] text-neutral-600 mt-0.5">
          {running} running · {done} done · {pending} pending
        </p>
      )}
    </div>
  );
}
