import { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { QueueItem, DecisionOption, Node } from '../types';
import AdvisorChatbot from './AdvisorChatbot';
import AdvisorPanel from './AdvisorPanel';

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
  processing: boolean;
  onResolve: (itemId: string, resolution: string) => Promise<void>;
}

function parsedOptions(item: QueueItem): DecisionOption[] {
  if (!item.options) return [];
  try {
    const parsed = JSON.parse(item.options);
    return Array.isArray(parsed) ? (parsed as DecisionOption[]) : [];
  } catch {
    return [];
  }
}

function AnswerInput({
  item,
  onResolve,
  onAfterResolve,
}: {
  item: QueueItem;
  onResolve: (itemId: string, resolution: string) => Promise<void>;
  onAfterResolve?: (taskId: string | null) => void;
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
      onAfterResolve?.(item.taskId ?? null);
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
                input === opt.id
                  ? 'border-amber-500 bg-amber-900 text-amber-200'
                  : 'border-neutral-600 text-neutral-400 hover:border-neutral-400 hover:text-neutral-200'
              }`}
              onClick={() => setInput(opt.id)}
            >
              {opt.label}
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
function StandardCard({ item, nodes, processing, onResolve, onAfterResolve }: StandardCardProps & { onAfterResolve?: (taskId: string | null) => void }) {
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
      {processing ? (
        <div className="flex items-center gap-1.5 text-[11px] text-neutral-500 py-1">
          <span className="inline-block w-3 h-3 border border-neutral-500 border-t-neutral-300 rounded-full animate-spin" />
          Processing…
        </div>
      ) : (
        <AnswerInput item={item} onResolve={onResolve} onAfterResolve={onAfterResolve} />
      )}
    </div>
  );
}

// Thread card: shows prior resolved Q+A pairs read-only, then the pending question.
function ThreadCard({
  resolved,
  pending,
  nodes,
  processing,
  onResolve,
  onAfterResolve,
}: {
  resolved: QueueItem[];
  pending: QueueItem;
  nodes: Node[];
  processing: boolean;
  onResolve: (itemId: string, resolution: string) => Promise<void>;
  onAfterResolve?: (taskId: string | null) => void;
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
        {processing ? (
          <div className="flex items-center gap-1.5 text-[11px] text-neutral-500 py-1">
            <span className="inline-block w-3 h-3 border border-neutral-500 border-t-neutral-300 rounded-full animate-spin" />
            Processing…
          </div>
        ) : (
          <AnswerInput item={pending} onResolve={onResolve} onAfterResolve={onAfterResolve} />
        )}
      </div>
    </div>
  );
}

export default function QueuePanel({ items, nodes, projectId, advisorTaskId, onResolve }: Props) {
  // processingTaskId: set after human submits a reply, cleared when agent responds.
  const [processingTaskId, setProcessingTaskId] = useState<string | null>(null);

  useEffect(() => {
    const unlisten = listen<{ taskId: string; agentId: string; type: string; content: string }>(
      'poe-agent-activity',
      ({ payload }) => {
        // Clear the processing indicator when the agent next speaks for that task.
        setProcessingTaskId(prev => (prev === payload.taskId ? null : prev));
      }
    );
    return () => { void unlisten.then(u => u()); };
  }, []);

  function handleAfterResolve(taskId: string | null) {
    if (taskId) setProcessingTaskId(taskId);
  }

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

  // Advisor nodes that have entered waiting or a terminal state — these drive AdvisorPanel.
  const advisorNodes = nodes.filter(
    n => n.nodeType === 'advisor' && (n.status === 'waiting' || n.status === 'complete' || n.status === 'cancelled')
  );

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
            const processing = processingTaskId === (pending.taskId ?? null);
            if (group.resolved.length > 0) {
              // Thread mode: prior Q+A + current pending.
              return (
                <ThreadCard
                  key={pending.id}
                  resolved={group.resolved}
                  pending={pending}
                  nodes={nodes}
                  processing={processing}
                  onResolve={onResolve}
                  onAfterResolve={handleAfterResolve}
                />
              );
            }
            return (
              <StandardCard
                key={pending.id}
                item={pending}
                nodes={nodes}
                processing={processing}
                onResolve={onResolve}
                onAfterResolve={handleAfterResolve}
              />
            );
          })
        )}
      </div>

      {/* Legacy advisor chatbot sub-panel */}
      <div className="shrink-0 border-t border-neutral-800 h-[260px] min-h-0 flex flex-col">
        <AdvisorChatbot
          projectId={projectId}
          taskNodeId={advisorTaskId}
        />
      </div>

      {/* Advisor interaction panel — renders when advisor nodes are active */}
      <AdvisorPanel advisorNodes={advisorNodes} />
    </div>
  );
}
