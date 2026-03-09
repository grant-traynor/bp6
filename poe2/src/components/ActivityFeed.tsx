import { useEffect, useRef } from 'react';
import type { FeedItem, Node } from '../types';
import { getAncestry } from '../hooks/usePoeProject';

interface Props {
  items: FeedItem[];
  nodes: Node[];
  onHandoverOpen: (nodeId: string) => void;
  // TODO(bp6-881.14): onChatOpen will be added when the dispatch mechanism is wired up.
  // onChatOpen?: (taskId: string) => void;
}

function formatTime(ts: string): string {
  try {
    const d = new Date(ts);
    return d.toLocaleTimeString('en-GB', { hour12: false });
  } catch {
    return ts;
  }
}

interface BadgeProps {
  eventType: string | undefined;
  itemType: FeedItem['type'];
  success?: boolean;
}

function Badge({ eventType, itemType, success }: BadgeProps) {
  let label = eventType ?? itemType;
  let cls = 'bg-neutral-700 text-neutral-300';

  if (itemType === 'agent-start') {
    cls = 'bg-purple-900 text-purple-300';
    label = 'agent-start';
  } else if (itemType === 'agent-exit') {
    cls = success !== false ? 'bg-slate-700 text-slate-300' : 'bg-red-900 text-red-300';
    label = 'agent-exit';
  } else if (itemType === 'node-created') {
    cls = 'bg-blue-900 text-blue-300';
    label = 'task-created';
  } else if (eventType === 'poe:brief' || eventType === 'poe-brief') {
    cls = 'bg-blue-900 text-blue-300';
  } else if (eventType === 'poe:step' || eventType === 'poe-step') {
    cls = 'bg-neutral-700 text-neutral-400';
  } else if (eventType === 'poe:artifact' || eventType === 'poe-artifact') {
    cls = 'bg-green-900 text-green-300';
  } else if (eventType === 'poe:knowledge' || eventType === 'poe-knowledge') {
    cls = 'bg-teal-900 text-teal-300';
  } else if (
    eventType === 'poe:done' ||
    eventType === 'poe-done' ||
    eventType === 'poe-task-done'
  ) {
    cls = 'bg-emerald-900 text-emerald-300';
  } else if (eventType === 'poe:decision' || eventType === 'poe-decision') {
    cls = 'bg-amber-900 text-amber-300';
  } else if (eventType === 'poe:chat') {
    cls = 'bg-sky-900 text-sky-300';
  }

  return (
    <span className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-mono shrink-0 ${cls}`}>
      {label}
    </span>
  );
}

export default function ActivityFeed({ items, nodes, onHandoverOpen }: Props) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [items]);

  if (items.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-neutral-600 text-sm">
        Waiting for agent output…
      </div>
    );
  }

  return (
    <div className="h-full overflow-y-auto px-3 py-2 space-y-1">
      {items.map(item => {
        const ancestry = getAncestry(item.taskId, nodes);
        const ancestryLabel = ancestry.length > 1
          ? ancestry
              .slice()
              .reverse()
              .map(n => n.title)
              .join(' › ')
          : null;

        const isChatTurn = item.eventType === 'poe:chat';
        // Navigation for poe:chat turns: opening the ArtifactViewer in chat-active mode
        // requires a hook from the parent that isn't yet wired (see bp6-881.14). For now
        // clicking a poe:chat entry falls through to onHandoverOpen so the task panel
        // opens, giving the user context. The TODO comment below marks the upgrade point.
        // TODO(bp6-881.14): replace onClick with onChatOpen(item.taskId) once wired.
        const displayMessage = isChatTurn
          ? `Chat turn — ${item.message.slice(0, 60)}${item.message.length > 60 ? '…' : ''}`
          : item.message;

        return (
          <div
            key={item.id}
            className={`flex flex-col min-w-0 rounded px-1 py-0.5 cursor-pointer transition-colors ${
              isChatTurn
                ? 'hover:bg-sky-950/30 border-l-2 border-sky-900/50'
                : 'hover:bg-neutral-800/40'
            }`}
            onClick={() => item.taskId && onHandoverOpen(item.taskId)}
          >
            <div className="flex items-start gap-2 min-w-0">
              <span className="shrink-0 text-neutral-600 text-[11px] tabular-nums pt-0.5">
                {formatTime(item.ts)}
              </span>
              <Badge
                eventType={item.eventType}
                itemType={item.type}
                success={
                  item.type === 'agent-exit'
                    ? !item.message.includes('failed')
                    : undefined
                }
              />
              {item.model && (
                <span className="shrink-0 inline-block px-1.5 py-0.5 rounded text-[10px] font-mono bg-violet-900 text-violet-300">
                  {item.model}
                </span>
              )}
              <span className={`text-[12px] break-words min-w-0 leading-5 ${isChatTurn ? 'text-sky-300/80' : 'text-neutral-300'}`}>
                {displayMessage}
              </span>
            </div>
            {ancestryLabel && (
              <p className="text-[10px] text-neutral-600 pl-[72px] truncate leading-4">
                {ancestryLabel}
              </p>
            )}
          </div>
        );
      })}
      <div ref={bottomRef} />
    </div>
  );
}
