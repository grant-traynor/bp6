import { useState } from 'react';
import type { QueueItem, Node } from '../types';
import AdvisorChatbot from './AdvisorChatbot';

interface Props {
  items: QueueItem[];   // all items for the project (resolved + pending)
  nodes: Node[];
  projectId: string;
  advisorTaskId: string | null;
  onResolve: (itemId: string, resolution: string) => Promise<void>;
}

interface StandardCardProps {
  item: QueueItem;
  nodes: Node[];
  onResolve: (itemId: string, resolution: string) => Promise<void>;
}

function parsedOptions(item: QueueItem): string[] {
  if (!item.options) return [];
  try {
    const parsed = JSON.parse(item.options);
    return Array.isArray(parsed) ? (parsed as string[]) : [];
  } catch {
    return [];
  }
}

function AnswerInput({
  item,
  onResolve,
}: {
  item: QueueItem;
  onResolve: (itemId: string, resolution: string) => Promise<void>;
}) {
  const [input, setInput] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const options = parsedOptions(item);

  async function submit() {
    const trimmed = input.trim();
    if (!trimmed || submitting) return;
    setSubmitting(true);
    try {
      await onResolve(item.id, trimmed);
      setInput('');
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <>
      {options.length > 0 && (
        <div className="flex flex-wrap gap-1 mt-1">
          {options.map((opt, i) => (
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
      <div className="flex gap-1.5 mt-1.5">
        <input
          type="text"
          className="flex-1 bg-neutral-800 border border-neutral-700 rounded px-2 py-1 text-[12px] text-neutral-100 placeholder-neutral-600 focus:outline-none focus:border-neutral-500"
          placeholder="Type a response…"
          value={input}
          onChange={e => setInput(e.target.value)}
          onKeyDown={e => { if (e.key === 'Enter') void submit(); }}
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
    </>
  );
}

// Standard single-question card (no prior history for this task).
function StandardCard({ item, nodes, onResolve }: StandardCardProps) {
  const taskNode = item.taskId ? nodes.find(n => n.id === item.taskId) : null;
  const taskLabel = taskNode ? taskNode.title : item.taskId ?? '—';
  const agentLabel = item.agentId ? item.agentId.slice(0, 12) + '…' : '—';

  return (
    <div className="border border-neutral-700 rounded p-2.5 space-y-2 bg-neutral-900">
      <p className="text-neutral-100 text-xs font-semibold leading-snug">{item.question}</p>
      <p className="text-[11px] text-neutral-500">
        <span className="text-neutral-400">{taskLabel}</span>
        {' · '}
        <span className="text-neutral-600">{agentLabel}</span>
      </p>
      <AnswerInput item={item} onResolve={onResolve} />
    </div>
  );
}

// Thread card: shows prior resolved Q+A pairs read-only, then the pending question.
function ThreadCard({
  resolved,
  pending,
  nodes,
  onResolve,
}: {
  resolved: QueueItem[];
  pending: QueueItem;
  nodes: Node[];
  onResolve: (itemId: string, resolution: string) => Promise<void>;
}) {
  const taskNode = pending.taskId ? nodes.find(n => n.id === pending.taskId) : null;
  const taskLabel = taskNode ? taskNode.title : pending.taskId ?? '—';

  return (
    <div className="border border-amber-700/60 rounded p-2.5 bg-neutral-900 space-y-2">
      <p className="text-[11px] font-semibold tracking-wide text-amber-400/80 uppercase">
        {taskLabel}
      </p>

      {/* Resolved pairs — read-only */}
      <div className="space-y-2">
        {resolved.map((r, i) => (
          <div key={r.id} className="space-y-0.5 opacity-60">
            <p className="text-[11px] text-neutral-400">
              <span className="text-neutral-600 mr-1">Q{i + 1}:</span>
              {r.question}
            </p>
            {r.resolution && (
              <p className="text-[11px] text-neutral-300 pl-4">
                <span className="text-neutral-600 mr-1">A:</span>
                {r.resolution}
              </p>
            )}
          </div>
        ))}
      </div>

      {/* Pending question — highlighted */}
      <div className="border-t border-neutral-700 pt-2 space-y-1">
        <p className="text-[11px] text-neutral-600">
          <span className="mr-1">Q{resolved.length + 1} (pending):</span>
        </p>
        <p className="text-neutral-100 text-xs font-semibold leading-snug">{pending.question}</p>
        <AnswerInput item={pending} onResolve={onResolve} />
      </div>
    </div>
  );
}

export default function QueuePanel({ items, nodes, projectId, advisorTaskId, onResolve }: Props) {
  // Group all items by taskId to detect thread mode.
  const taskGroups = new Map<string, { resolved: QueueItem[]; pending: QueueItem[] }>();

  for (const item of items) {
    const key = item.taskId ?? '__no_task__';
    if (!taskGroups.has(key)) taskGroups.set(key, { resolved: [], pending: [] });
    const group = taskGroups.get(key)!;
    if (item.resolvedAt === null) {
      group.pending.push(item);
    } else {
      group.resolved.push(item);
    }
  }

  // Only show groups that have at least one pending item.
  const pendingGroups = [...taskGroups.values()].filter(g => g.pending.length > 0);

  // Sort resolved within each group chronologically.
  for (const g of pendingGroups) {
    g.resolved.sort((a, b) => (a.createdAt ?? '').localeCompare(b.createdAt ?? ''));
  }

  const totalPending = pendingGroups.reduce((s, g) => s + g.pending.length, 0);

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center gap-2 px-3 py-2 border-b border-neutral-800 shrink-0">
        <span className="text-[11px] font-semibold tracking-widest text-neutral-400 uppercase">
          Decisions
        </span>
        {totalPending > 0 && (
          <span className="bg-amber-500 text-black text-[10px] font-bold px-1.5 py-0.5 rounded-full leading-none">
            {totalPending}
          </span>
        )}
      </div>

      <div className="flex-1 overflow-y-auto p-2 space-y-2 min-h-0">
        {pendingGroups.length === 0 ? (
          <div className="flex h-full items-center justify-center text-neutral-600 text-xs">
            No pending decisions
          </div>
        ) : (
          pendingGroups.map(group => {
            const pending = group.pending[0];  // one pending at a time per task
            if (group.resolved.length > 0) {
              // Thread mode: prior Q+A + current pending.
              return (
                <ThreadCard
                  key={pending.id}
                  resolved={group.resolved}
                  pending={pending}
                  nodes={nodes}
                  onResolve={onResolve}
                />
              );
            }
            return (
              <StandardCard
                key={pending.id}
                item={pending}
                nodes={nodes}
                onResolve={onResolve}
              />
            );
          })
        )}
      </div>

      {/* Advisor sub-panel */}
      <div className="shrink-0 border-t border-neutral-800 h-[260px] min-h-0 flex flex-col">
        <AdvisorChatbot
          projectId={projectId}
          taskNodeId={advisorTaskId}
        />
      </div>
    </div>
  );
}
