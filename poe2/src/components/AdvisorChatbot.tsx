import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { AdvisorTurn } from '../types';

interface AdvisorTurnEvent {
  turnId: string;
  taskId: string;
  content: string;
}

interface Props {
  projectId: string;
  taskNodeId: string | null;
  decisionId?: string | null;
}

export default function AdvisorChatbot({ projectId, taskNodeId, decisionId = null }: Props) {
  const [turns, setTurns] = useState<AdvisorTurn[]>([]);
  const [inputValue, setInputValue] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [starting, setStarting] = useState(false);
  const [startError, setStartError] = useState<string | null>(null);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [sessionDone, setSessionDone] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);

  // Hydrate existing advisor turns when taskNodeId is known
  useEffect(() => {
    if (!taskNodeId) return;
    invoke<AdvisorTurn[]>('list_advisor_turns', { taskNodeId })
      .then(setTurns)
      .catch(console.error);
  }, [taskNodeId]);

  // Listen for new advisor-turn-added events
  useEffect(() => {
    if (!taskNodeId) return;
    let cancelled = false;
    const unlisten = listen<AdvisorTurnEvent>('poe-advisor-turn', ({ payload }) => {
      if (cancelled) return;
      if (payload.taskId !== taskNodeId) return;
      const newTurn: AdvisorTurn = {
        id: payload.turnId,
        taskId: payload.taskId,
        content: payload.content,
        response: null,
        createdAt: new Date().toISOString(),
        respondedAt: null,
      };
      setTurns(prev => {
        if (prev.some(t => t.id === newTurn.id)) return prev;
        return [...prev, newTurn];
      });
    });
    return () => {
      cancelled = true;
      void unlisten.then(u => u());
    };
  }, [taskNodeId]);

  // Listen for poe-task-done to detect session end
  useEffect(() => {
    if (!taskNodeId) return;
    let cancelled = false;
    const unlisten = listen<{ id: string; projectId: string; status: string }>(
      'poe-task-done',
      ({ payload }) => {
        if (cancelled) return;
        if (payload.projectId !== projectId) return;
        if (payload.id !== taskNodeId) return;
        setSessionDone(true);
      }
    );
    return () => {
      cancelled = true;
      void unlisten.then(u => u());
    };
  }, [projectId, taskNodeId]);

  // Scroll to bottom when turns update
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [turns]);

  const pendingTurn = [...turns].reverse().find(t => t.respondedAt === null) ?? null;

  async function handleStartSession() {
    setStarting(true);
    setStartError(null);
    try {
      await invoke('start_advisor_session', {
        projectId,
        decisionId: decisionId ?? null,
      });
    } catch (e) {
      setStartError(String(e));
    } finally {
      setStarting(false);
    }
  }

  async function handleSubmit() {
    if (!pendingTurn || !inputValue.trim() || submitting || !taskNodeId) return;
    setSubmitting(true);
    setSubmitError(null);
    try {
      await invoke('respond_to_advisor', {
        projectId,
        turnId: pendingTurn.id,
        response: inputValue.trim(),
      });
      setTurns(prev =>
        prev.map(t =>
          t.id === pendingTurn.id
            ? { ...t, response: inputValue.trim(), respondedAt: new Date().toISOString() }
            : t
        )
      );
      setInputValue('');
    } catch (e) {
      setSubmitError(String(e));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-1.5 border-b border-neutral-800 shrink-0">
        <span className="text-[10px] uppercase tracking-widest text-neutral-600">Advisor</span>
        {taskNodeId && (
          <span className="text-[9px] text-neutral-700 font-mono truncate ml-2">
            {taskNodeId.slice(0, 8)}…
          </span>
        )}
      </div>

      {/* No session yet */}
      {!taskNodeId && (
        <div className="flex-1 flex flex-col items-center justify-center p-3 gap-2">
          <p className="text-[11px] text-neutral-600 text-center">
            Ask the advisor to research a decision or explore project context.
          </p>
          {startError && (
            <p className="text-[10px] text-red-400 font-mono">{startError}</p>
          )}
          <button
            className="px-3 py-1.5 rounded text-[11px] bg-neutral-800 border border-neutral-700 text-neutral-300 hover:bg-neutral-700 hover:text-neutral-100 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            onClick={() => void handleStartSession()}
            disabled={starting}
          >
            {starting ? 'Starting…' : 'Ask Advisor'}
          </button>
        </div>
      )}

      {/* Turn list */}
      {taskNodeId && (
        <>
          <div className="flex-1 overflow-y-auto px-3 py-2 space-y-2">
            {turns.length === 0 ? (
              <p className="text-neutral-700 text-[11px] font-mono pt-2">
                {sessionDone ? 'Advisor session complete.' : 'Waiting for advisor…'}
              </p>
            ) : (
              turns.map(turn => (
                <div key={turn.id} className="space-y-1">
                  {/* Advisor message — left aligned */}
                  <div className="rounded px-2 py-1.5 bg-neutral-900 border border-neutral-800">
                    <p className="text-[10px] text-teal-600 font-mono mb-0.5">Advisor</p>
                    <p className="text-[12px] text-neutral-300 leading-5 whitespace-pre-wrap break-words">
                      {turn.content}
                    </p>
                  </div>
                  {/* User response — right aligned */}
                  {turn.response !== null && (
                    <div className="rounded px-2 py-1.5 bg-neutral-800 border border-neutral-700 ml-4">
                      <p className="text-[10px] text-neutral-500 font-mono mb-0.5">You</p>
                      <p className="text-[12px] text-neutral-200 leading-5 whitespace-pre-wrap break-words">
                        {turn.response}
                      </p>
                    </div>
                  )}
                </div>
              ))
            )}
            <div ref={bottomRef} />
          </div>

          {/* Input area */}
          {!sessionDone && (
            <div className="shrink-0 border-t border-neutral-800 p-2 space-y-1.5">
              {submitError && (
                <p className="text-red-400 text-[11px] font-mono">{submitError}</p>
              )}
              <textarea
                className="w-full bg-neutral-900 border border-neutral-700 rounded text-neutral-200 text-[12px] font-mono p-2 resize-none focus:outline-none focus:border-neutral-500 disabled:opacity-40 disabled:cursor-not-allowed placeholder:text-neutral-700"
                rows={2}
                placeholder={pendingTurn ? 'type response…' : 'waiting for advisor…'}
                value={inputValue}
                disabled={!pendingTurn || submitting}
                onChange={e => setInputValue(e.target.value)}
                onKeyDown={e => {
                  if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
                    void handleSubmit();
                  }
                }}
              />
              <div className="flex justify-end">
                <button
                  className="px-3 py-1 text-[11px] font-mono border-2 border-black bg-neutral-100 text-black hover:bg-white disabled:opacity-40 disabled:cursor-not-allowed rounded"
                  disabled={!pendingTurn || !inputValue.trim() || submitting}
                  onClick={() => void handleSubmit()}
                >
                  {submitting ? 'Sending…' : 'Send ↵'}
                </button>
              </div>
            </div>
          )}

          {sessionDone && (
            <div className="shrink-0 border-t border-neutral-800 px-3 py-2">
              <p className="text-[11px] text-neutral-700 font-mono">Advisor session complete.</p>
            </div>
          )}
        </>
      )}
    </div>
  );
}
