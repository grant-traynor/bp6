import { useState } from 'react';
import type { QueueItem, Node } from '../types';

interface Props {
  items: QueueItem[];
  nodes: Node[];
  onResolve: (itemId: string, resolution: string) => Promise<void>;
}

interface CardProps {
  item: QueueItem;
  nodes: Node[];
  onResolve: (itemId: string, resolution: string) => Promise<void>;
}

function QueueCard({ item, nodes, onResolve }: CardProps) {
  const [input, setInput] = useState('');
  const [submitting, setSubmitting] = useState(false);

  const taskNode = item.taskId ? nodes.find(n => n.id === item.taskId) : null;
  const taskLabel = taskNode ? taskNode.title : item.taskId ?? '—';
  const agentLabel = item.agentId ? item.agentId.slice(0, 12) + '…' : '—';

  const parsedOptions: string[] = (() => {
    if (!item.options) return [];
    try {
      const parsed = JSON.parse(item.options);
      if (Array.isArray(parsed)) return parsed as string[];
      return [];
    } catch {
      return [];
    }
  })();

  async function submit() {
    const trimmed = input.trim();
    if (!trimmed || submitting) return;
    setSubmitting(true);
    try {
      await onResolve(item.id, trimmed);
    } finally {
      setSubmitting(false);
    }
  }

  function handleKey(e: React.KeyboardEvent<HTMLInputElement>) {
    if (e.key === 'Enter') void submit();
  }

  return (
    <div className="border border-neutral-700 rounded p-2.5 space-y-2 bg-neutral-900">
      <p className="text-neutral-100 text-xs font-semibold leading-snug">{item.question}</p>
      <p className="text-[11px] text-neutral-500">
        <span className="text-neutral-400">{taskLabel}</span>
        {' · '}
        <span className="text-neutral-600">{agentLabel}</span>
      </p>

      {parsedOptions.length > 0 && (
        <div className="flex flex-wrap gap-1">
          {parsedOptions.map((opt, i) => (
            <button
              key={i}
              className={`px-2 py-0.5 rounded text-[11px] border transition-colors ${
                input === opt
                  ? 'border-amber-500 bg-amber-900 text-amber-200'
                  : 'border-neutral-600 text-neutral-400 hover:border-neutral-400 hover:text-neutral-200'
              }`}
              onClick={() => setInput(opt)}
            >
              {opt}
            </button>
          ))}
        </div>
      )}

      <div className="flex gap-1.5">
        <input
          type="text"
          className="flex-1 bg-neutral-800 border border-neutral-700 rounded px-2 py-1 text-[12px] text-neutral-100 placeholder-neutral-600 focus:outline-none focus:border-neutral-500"
          placeholder="Type a response…"
          value={input}
          onChange={e => setInput(e.target.value)}
          onKeyDown={handleKey}
          disabled={submitting}
        />
        <button
          className="px-2.5 py-1 rounded bg-amber-600 hover:bg-amber-500 text-white text-[12px] disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
          onClick={() => void submit()}
          disabled={!input.trim() || submitting}
        >
          Submit
        </button>
      </div>
    </div>
  );
}

export default function QueuePanel({ items, nodes, onResolve }: Props) {
  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center gap-2 px-3 py-2 border-b border-neutral-800 shrink-0">
        <span className="text-[11px] font-semibold tracking-widest text-neutral-400 uppercase">
          Decisions
        </span>
        {items.length > 0 && (
          <span className="bg-amber-500 text-black text-[10px] font-bold px-1.5 py-0.5 rounded-full leading-none">
            {items.length}
          </span>
        )}
      </div>

      <div className="flex-1 overflow-y-auto p-2 space-y-2">
        {items.length === 0 ? (
          <div className="flex h-full items-center justify-center text-neutral-600 text-xs">
            No pending decisions
          </div>
        ) : (
          items.map(item => (
            <QueueCard key={item.id} item={item} nodes={nodes} onResolve={onResolve} />
          ))
        )}
      </div>
    </div>
  );
}
