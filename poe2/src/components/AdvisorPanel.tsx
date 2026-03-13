import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { Node } from '../types';

interface AdvisorTurnPayload {
  turnId: string;
  taskId: string;
  content: string;
}

interface ActivityPayload {
  taskId: string;
  agentId: string;
  type: 'step' | 'brief';
  content: string;
}

interface AdvisorEntry {
  id: string;
  kind: 'advisor' | 'step' | 'brief';
  content: string;
  ts: string;
}

interface AdvisorSession {
  nodeId: string;
  skillId: string | null;
  title: string;
  entries: AdvisorEntry[];
  dismissed: boolean;
  complete: boolean;
}

interface Props {
  advisorNodes: Node[];   // all advisor nodes (waiting + complete)
}

function SessionCard({ session, onDismiss }: { session: AdvisorSession; onDismiss: () => void }) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [session.entries.length]);

  return (
    <div className="border border-teal-800/50 rounded bg-neutral-900 flex flex-col min-h-0">
      {/* Session header */}
      <div className="flex items-center justify-between px-2.5 py-1.5 border-b border-neutral-800 shrink-0">
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-[10px] font-semibold tracking-wide text-teal-500 uppercase shrink-0">
            Advisor
          </span>
          {session.skillId && (
            <span className="text-[10px] text-neutral-500 font-mono truncate">
              {session.skillId}
            </span>
          )}
          <span className="text-[10px] text-neutral-400 truncate" title={session.title}>
            {session.title}
          </span>
        </div>
        <div className="flex items-center gap-1.5 shrink-0 ml-2">
          {session.complete && (
            <span className="text-[9px] text-neutral-600 font-mono">done</span>
          )}
          {!session.complete && (
            <span className="inline-block w-1.5 h-1.5 rounded-full bg-teal-500 animate-pulse" />
          )}
          <button
            className="px-2 py-0.5 rounded text-[10px] border border-neutral-700 text-neutral-500 hover:border-neutral-500 hover:text-neutral-300 transition-colors"
            onClick={onDismiss}
          >
            Dismiss
          </button>
        </div>
      </div>

      {/* Entry stream */}
      <div className="flex-1 overflow-y-auto px-2.5 py-2 space-y-1.5 min-h-0 max-h-48">
        {session.entries.length === 0 ? (
          <p className="text-neutral-700 text-[11px] font-mono">
            {session.complete ? 'Advisor session complete.' : 'Waiting for advisor output…'}
          </p>
        ) : (
          session.entries.map(entry => (
            <div key={entry.id} className={`rounded px-2 py-1 ${
              entry.kind === 'advisor'
                ? 'bg-neutral-800 border border-teal-900/50'
                : 'bg-neutral-900/50 border border-neutral-800'
            }`}>
              {entry.kind !== 'advisor' && (
                <p className="text-[9px] text-neutral-600 font-mono mb-0.5 uppercase">
                  {entry.kind}
                </p>
              )}
              <p className="text-[11px] text-neutral-300 leading-5 whitespace-pre-wrap break-words">
                {entry.content}
              </p>
            </div>
          ))
        )}
        <div ref={bottomRef} />
      </div>
    </div>
  );
}

export default function AdvisorPanel({ advisorNodes }: Props) {
  const [sessions, setSessions] = useState<Map<string, AdvisorSession>>(new Map());

  // Ensure a session exists for each advisor node in waiting/complete state.
  useEffect(() => {
    setSessions(prev => {
      let changed = false;
      const next = new Map(prev);
      for (const node of advisorNodes) {
        if (!next.has(node.id)) {
          next.set(node.id, {
            nodeId: node.id,
            skillId: node.skillId,
            title: node.title,
            entries: [],
            dismissed: false,
            complete: node.status === 'complete' || node.status === 'cancelled',
          });
          changed = true;
        } else {
          // Update complete flag if node transitioned
          const existing = next.get(node.id)!;
          const isComplete = node.status === 'complete' || node.status === 'cancelled';
          if (existing.complete !== isComplete) {
            next.set(node.id, { ...existing, complete: isComplete });
            changed = true;
          }
        }
      }
      return changed ? next : prev;
    });
  }, [advisorNodes]);

  // Subscribe to poe-advisor-turn to capture advisor output.
  useEffect(() => {
    let cancelled = false;
    const unlisten = listen<AdvisorTurnPayload>('poe-advisor-turn', ({ payload }) => {
      if (cancelled) return;
      setSessions(prev => {
        const session = prev.get(payload.taskId);
        if (!session) return prev;
        const entry: AdvisorEntry = {
          id: `advisor-${payload.turnId}`,
          kind: 'advisor',
          content: payload.content,
          ts: new Date().toISOString(),
        };
        // Deduplicate by id.
        if (session.entries.some(e => e.id === entry.id)) return prev;
        const next = new Map(prev);
        next.set(payload.taskId, { ...session, entries: [...session.entries, entry] });
        return next;
      });
    });
    return () => {
      cancelled = true;
      void unlisten.then(u => u());
    };
  }, []);

  // Subscribe to poe-agent-activity to capture step/brief progress for advisor agents.
  useEffect(() => {
    let cancelled = false;
    const unlisten = listen<ActivityPayload>('poe-agent-activity', ({ payload }) => {
      if (cancelled) return;
      setSessions(prev => {
        const session = prev.get(payload.taskId);
        if (!session) return prev;
        const entry: AdvisorEntry = {
          id: `activity-${payload.agentId}-${payload.type}-${Date.now()}-${Math.random()}`,
          kind: payload.type,
          content: payload.content,
          ts: new Date().toISOString(),
        };
        const next = new Map(prev);
        next.set(payload.taskId, { ...session, entries: [...session.entries, entry] });
        return next;
      });
    });
    return () => {
      cancelled = true;
      void unlisten.then(u => u());
    };
  }, []);

  function dismissSession(nodeId: string) {
    setSessions(prev => {
      const session = prev.get(nodeId);
      if (!session) return prev;
      const next = new Map(prev);
      next.set(nodeId, { ...session, dismissed: true });
      return next;
    });
  }

  const visibleSessions = [...sessions.values()].filter(s => !s.dismissed);

  if (visibleSessions.length === 0) return null;

  return (
    <div className="shrink-0 border-t border-neutral-800 flex flex-col gap-2 p-2 min-h-0">
      <div className="flex items-center gap-2 px-0.5">
        <span className="text-[10px] font-semibold tracking-widest text-neutral-500 uppercase">
          Advisors
        </span>
        <span className="bg-teal-700 text-white text-[9px] font-bold px-1.5 py-0.5 rounded-full leading-none">
          {visibleSessions.length}
        </span>
      </div>
      {visibleSessions.map(session => (
        <SessionCard
          key={session.nodeId}
          session={session}
          onDismiss={() => dismissSession(session.nodeId)}
        />
      ))}
    </div>
  );
}
