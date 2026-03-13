import type { Node } from '../types';

interface Props {
  nodes: Node[];
  onHandoverOpen: (nodeId: string) => void;
  onDebugOpen: (nodeId: string) => void;
}

interface TreeNode {
  node: Node;
  children: TreeNode[];
  depth: number;
}

function buildTree(nodes: Node[]): TreeNode[] {
  const children = new Map<string | null, Node[]>();

  for (const n of nodes) {
    const key = n.parentId ?? null;
    if (!children.has(key)) children.set(key, []);
    children.get(key)!.push(n);
  }

  function build(parentId: string | null, depth: number): TreeNode[] {
    const kids = children.get(parentId) ?? [];
    // Sort: epics > features > tasks/bugs/chores, then by title
    kids.sort((a, b) => {
      const order = { epic: 0, feature: 1, task: 2, bug: 2, chore: 2 };
      const ao = order[a.nodeType as keyof typeof order] ?? 3;
      const bo = order[b.nodeType as keyof typeof order] ?? 3;
      return ao !== bo ? ao - bo : a.title.localeCompare(b.title);
    });
    return kids.map(n => ({ node: n, children: build(n.id, depth + 1), depth }));
  }

  return build(null, 0);
}

function statusDot(status: Node['status']): { symbol: string; cls: string } {
  switch (status) {
    case 'running':  return { symbol: '●', cls: 'text-emerald-400' };
    case 'waiting':  return { symbol: '⏸', cls: 'text-amber-400' };
    case 'complete': return { symbol: '✓', cls: 'text-neutral-600' };
    case 'cancelled':return { symbol: '✕', cls: 'text-neutral-700' };
    case 'blocked':  return { symbol: '⊘', cls: 'text-red-600' };
    default:         return { symbol: '○', cls: 'text-neutral-600' };
  }
}

function NodeRow({ item, onHandoverOpen, onDebugOpen }: {
  item: TreeNode;
  onHandoverOpen: (id: string) => void;
  onDebugOpen: (id: string) => void;
}) {
  const { symbol, cls } = statusDot(item.node.status);
  const isRunning = item.node.status === 'running';
  const isWaiting = item.node.status === 'waiting';
  const typeLabel = item.node.nodeType === 'epic' ? 'EPIC'
                  : item.node.nodeType === 'feature' ? 'FEAT'
                  : item.node.nodeType === 'bug' ? 'BUG'
                  : null;

  return (
    <>
      <div
        className={`flex items-baseline gap-1.5 py-0.5 px-1 rounded text-[12px] leading-5 group ${
          isRunning
            ? 'bg-emerald-950/40 cursor-pointer hover:bg-emerald-950/60'
            : isWaiting
            ? 'cursor-pointer hover:bg-neutral-800/60'
            : ''
        }`}
        style={{ paddingLeft: `${4 + item.depth * 16}px` }}
        onClick={() => {
          if (isRunning) onDebugOpen(item.node.id);
          else if (isWaiting) onHandoverOpen(item.node.id);
        }}
        title={isRunning ? 'Click to view stream output' : isWaiting ? 'Click to open agent handover' : undefined}
      >
        {isRunning ? (
          <span className="shrink-0 w-3 flex items-center justify-center">
            <span className={`inline-block w-2 h-2 rounded-full border border-emerald-400 border-t-transparent animate-spin ${cls}`} />
          </span>
        ) : (
          <span className={`shrink-0 font-mono text-[11px] w-3 text-center ${cls}`}>{symbol}</span>
        )}
        {typeLabel && (
          <span className="shrink-0 text-[9px] font-bold text-neutral-600 tracking-wider">{typeLabel}</span>
        )}
        {item.node.requiresManualVerification && (
          <span className="shrink-0 text-[9px] font-bold text-amber-500/80 tracking-wider border border-amber-500/40 rounded px-0.5">Manual</span>
        )}
        <span className={`truncate ${item.node.status === 'complete' || item.node.status === 'cancelled' ? 'text-neutral-600' : 'text-neutral-300'}`}>
          {item.node.title}
        </span>
        {item.node.skillId && item.node.status === 'running' && (
          <span className="shrink-0 text-[10px] text-neutral-600 font-mono ml-auto pr-1">{item.node.skillId}</span>
        )}
        {item.node.skillId && item.node.status === 'waiting' && (
          <span className="shrink-0 text-[10px] text-amber-500/70 font-mono ml-auto pr-1">{item.node.skillId}</span>
        )}
      </div>
      {item.children.map(child => (
        <NodeRow key={child.node.id} item={child} onHandoverOpen={onHandoverOpen} onDebugOpen={onDebugOpen} />
      ))}
    </>
  );
}

export default function NodeTree({ nodes, onHandoverOpen, onDebugOpen }: Props) {
  const tree = buildTree(nodes);

  if (nodes.length === 0) return null;

  return (
    <div className="overflow-y-auto border-b border-neutral-800 py-1">
      {tree.map(item => (
        <NodeRow key={item.node.id} item={item} onHandoverOpen={onHandoverOpen} onDebugOpen={onDebugOpen} />
      ))}
    </div>
  );
}
