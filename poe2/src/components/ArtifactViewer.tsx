import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Artifact, Node } from '../types';

interface ChatTurn {
  id: string;
  taskId: string;
  content: string;
  response: string | null;
  createdAt: string;
  respondedAt: string | null;
}

interface ChatTurnEvent {
  turnId: string;
  taskId: string;
  content: string;
}

interface PoeEventPayload {
  eventType: string;
  projectId: string;
  agentId: string | null;
  taskId: string | null;
  payload: string;
}

interface Props {
  artifact?: Artifact | null;
  projectId: string;
  taskId?: string | null;
  onClose: () => void;
}

export default function ArtifactViewer({ artifact, projectId, taskId, onClose }: Props) {
  const [content, setContent] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // localTaskId: starts from prop, overwritten when human-initiated dispatch creates a new node
  const [localTaskId, setLocalTaskId] = useState<string | null>(taskId ?? null);

  // Chat panel state
  const [chatTurns, setChatTurns] = useState<ChatTurn[]>([]);
  const [chatActive, setChatActive] = useState(false);
  const [inputValue, setInputValue] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [taskDone, setTaskDone] = useState(false);

  // Human-initiated dispatch state
  const [dispatching, setDispatching] = useState(false);
  const [dispatchError, setDispatchError] = useState<string | null>(null);

  const chatBottomRef = useRef<HTMLDivElement>(null);

  // Load artifact content (only when an artifact is provided)
  useEffect(() => {
    if (!artifact) return;
    const art = artifact;
    setContent(null);
    setError(null);
    invoke<string>('read_artifact_content', { artifactId: art.id, projectId })
      .then(setContent)
      .catch((e) => setError(String(e)));
  }, [artifact?.id, projectId]);

  // Hydrate existing chat turns and open panel if turns exist
  useEffect(() => {
    if (!localTaskId) return;
    invoke<ChatTurn[]>('get_chat_turns', { projectId, taskId: localTaskId })
      .then((turns) => {
        setChatTurns(turns);
        if (turns.length > 0) {
          setChatActive(true);
        }
      })
      .catch((e) => console.error('[ArtifactViewer] get_chat_turns error:', e));
  }, [projectId, localTaskId]);

  // Scroll chat to bottom when turns change
  useEffect(() => {
    chatBottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [chatTurns]);

  // Listen for new poe-chat-turn events
  useEffect(() => {
    if (!localTaskId) return;
    let cancelled = false;

    const unlistenPromise = listen<ChatTurnEvent>('poe-chat-turn', ({ payload }) => {
      if (cancelled) return;
      if (payload.taskId !== localTaskId) return;
      const newTurn: ChatTurn = {
        id: payload.turnId,
        taskId: payload.taskId,
        content: payload.content,
        response: null,
        createdAt: new Date().toISOString(),
        respondedAt: null,
      };
      setChatTurns((prev) => {
        if (prev.some((t) => t.id === newTurn.id)) return prev;
        return [...prev, newTurn];
      });
      setChatActive(true);
    });

    return () => {
      cancelled = true;
      void unlistenPromise.then((u) => u());
    };
  }, [localTaskId]);

  // Listen for poe-event to detect poe:done for this task
  useEffect(() => {
    if (!localTaskId) return;
    let cancelled = false;

    const unlistenPromise = listen<PoeEventPayload>('poe-event', ({ payload }) => {
      if (cancelled) return;
      if (payload.projectId !== projectId) return;
      if (payload.taskId !== localTaskId) return;
      if (payload.eventType === 'poe:done') {
        setTaskDone(true);
      }
    });

    return () => {
      cancelled = true;
      void unlistenPromise.then((u) => u());
    };
  }, [projectId, localTaskId]);

  // Also detect task done via poe-task-done (fires with node payload)
  useEffect(() => {
    if (!localTaskId) return;
    let cancelled = false;

    const unlistenPromise = listen<{ id: string; projectId: string; status: string }>(
      'poe-task-done',
      ({ payload }) => {
        if (cancelled) return;
        if (payload.projectId !== projectId) return;
        if (payload.id !== localTaskId) return;
        setTaskDone(true);
      }
    );

    return () => {
      cancelled = true;
      void unlistenPromise.then((u) => u());
    };
  }, [projectId, localTaskId]);

  const pendingTurn =
    [...chatTurns].reverse().find((t: ChatTurn) => t.respondedAt === null) ?? null;

  // Human-initiated: create a new interactive task referencing the artifact
  async function handleDispatchChat() {
    if (!artifact || !content || dispatching) return;
    setDispatching(true);
    setDispatchError(null);
    try {
      const node = await invoke<Node>('create_node', {
        input: {
          projectId,
          phaseId: null,
          parentId: null,
          nodeType: 'task',
          title: `Discuss: ${artifact.filename}`,
          description: `The user wants to discuss the following artifact.\n\n---\n\n${content}`,
          skillId: 'operational-analyst',
          initialStatus: 'pending',
        },
      });
      setLocalTaskId(node.id);
      setChatActive(true);
    } catch (e) {
      setDispatchError(String(e));
    } finally {
      setDispatching(false);
    }
  }

  async function handleSubmit() {
    if (!pendingTurn || !inputValue.trim() || submitting) return;
    setSubmitting(true);
    setSubmitError(null);
    try {
      await invoke<void>('respond_to_chat', {
        projectId,
        turnId: pendingTurn.id,
        response: inputValue.trim(),
      });
      // Mark the turn as responded locally
      setChatTurns((prev) =>
        prev.map((t) =>
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

  function handleKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      void handleSubmit();
    }
  }

  const canSubmit = !!pendingTurn && !!inputValue.trim() && !submitting && !taskDone;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70">
      <div
        className={`flex flex-col bg-neutral-950 border-2 border-neutral-700 rounded shadow-2xl transition-all duration-200 ${
          chatActive ? 'w-[1100px]' : 'w-[800px]'
        } h-[620px]`}
      >
        {/* Toolbar */}
        <div className="flex items-center justify-between px-3 py-2 border-b border-neutral-800 shrink-0">
          <div className="min-w-0 flex items-center gap-3">
            <span className="text-xs font-semibold text-neutral-200">{artifact?.filename ?? 'Chat session'}</span>
            {artifact?.artifactType && <span className="text-[10px] text-neutral-500 font-mono">{artifact.artifactType}</span>}
            {taskDone && (
              <span className="inline-block px-1.5 py-0.5 rounded text-[10px] font-mono bg-emerald-900 text-emerald-300">
                task complete
              </span>
            )}
          </div>
          <div className="flex items-center gap-2">
            {/* Human-initiated: artifact loaded, no active session yet */}
            {artifact && content && !localTaskId && !chatActive && (
              <button
                className="text-neutral-400 hover:text-neutral-100 text-[11px] font-mono border border-neutral-700 hover:border-neutral-500 px-2 py-0.5 rounded disabled:opacity-40 disabled:cursor-not-allowed"
                disabled={dispatching}
                onClick={() => void handleDispatchChat()}
              >
                {dispatching ? 'Opening…' : 'Chat about this'}
              </button>
            )}
            {dispatchError && (
              <span className="text-red-400 text-[10px] font-mono">{dispatchError}</span>
            )}
            {/* Agent-initiated session done: re-show chat panel */}
            {localTaskId && taskDone && !chatActive && chatTurns.length > 0 && (
              <button
                className="text-neutral-400 hover:text-neutral-100 text-[11px] font-mono border border-neutral-700 hover:border-neutral-500 px-2 py-0.5 rounded"
                onClick={() => setChatActive(true)}
              >
                Chat about this
              </button>
            )}
            {chatActive && (
              <button
                className="text-neutral-500 hover:text-neutral-300 text-[11px] font-mono"
                onClick={() => setChatActive(false)}
              >
                hide chat
              </button>
            )}
            <button
              className="text-neutral-500 hover:text-neutral-200 text-xs px-2 py-1"
              onClick={onClose}
            >
              ✕ close
            </button>
          </div>
        </div>

        {/* Body */}
        <div className="flex flex-1 overflow-hidden">
          {/* Artifact pane — hidden in chat-only mode (no artifact yet) */}
          {(!chatActive || artifact) && (
            <div className={`flex flex-col overflow-hidden ${chatActive ? 'flex-1 border-r border-neutral-800' : 'w-full'}`}>
              <div className="flex-1 overflow-y-auto p-4">
                {!artifact ? (
                  <p className="text-neutral-700 text-[12px] font-mono">
                    Artifact will appear here once the analyst writes it.
                  </p>
                ) : error ? (
                  <p className="text-red-400 text-sm">{error}</p>
                ) : content === null ? (
                  <p className="text-neutral-600 text-sm">Loading…</p>
                ) : (
                  <pre className="text-neutral-300 text-[12px] font-mono whitespace-pre-wrap leading-5">
                    {content}
                  </pre>
                )}
              </div>
            </div>
          )}

          {/* Chat pane */}
          {chatActive && (
            <div className="w-[340px] shrink-0 flex flex-col overflow-hidden">
              {/* Chat header */}
              <div className="px-3 py-1.5 border-b border-neutral-800 shrink-0">
                <span className="text-[10px] uppercase tracking-widest text-neutral-600">Chat</span>
                {taskDone && (
                  <span className="ml-2 text-[10px] text-emerald-700 font-mono">· session closed</span>
                )}
              </div>

              {/* Turn list */}
              <div className="flex-1 overflow-y-auto px-3 py-2 space-y-2">
                {chatTurns.length === 0 ? (
                  <p className="text-neutral-700 text-[11px] font-mono pt-2">
                    {taskDone
                      ? 'No chat turns for this task.'
                      : 'Waiting for analyst questions…'}
                  </p>
                ) : (
                  chatTurns.map((turn) => (
                    <div key={turn.id} className="space-y-1">
                      {/* Agent turn */}
                      <div className="rounded px-2 py-1.5 bg-neutral-900 border border-neutral-800">
                        <p className="text-[10px] text-neutral-600 font-mono mb-0.5">Analyst</p>
                        <p className="text-[12px] text-neutral-300 leading-5 whitespace-pre-wrap break-words">
                          {turn.content}
                        </p>
                      </div>
                      {/* Human response */}
                      {turn.response !== null && (
                        <div className="rounded px-2 py-1.5 bg-neutral-800 border border-neutral-700 ml-3">
                          <p className="text-[10px] text-neutral-500 font-mono mb-0.5">You</p>
                          <p className="text-[12px] text-neutral-200 leading-5 whitespace-pre-wrap break-words">
                            {turn.response}
                          </p>
                        </div>
                      )}
                    </div>
                  ))
                )}
                <div ref={chatBottomRef} />
              </div>

              {/* Input area */}
              {!taskDone && (
                <div className="shrink-0 border-t border-neutral-800 p-2 space-y-1.5">
                  {submitError && (
                    <p className="text-red-400 text-[11px] font-mono">{submitError}</p>
                  )}
                  <textarea
                    className="w-full bg-neutral-900 border border-neutral-700 rounded text-neutral-200 text-[12px] font-mono p-2 resize-none focus:outline-none focus:border-neutral-500 disabled:opacity-40 disabled:cursor-not-allowed placeholder:text-neutral-700"
                    rows={3}
                    placeholder={pendingTurn ? 'type response…' : 'waiting for analyst…'}
                    value={inputValue}
                    disabled={!pendingTurn || submitting}
                    onChange={(e) => setInputValue(e.target.value)}
                    onKeyDown={handleKeyDown}
                  />
                  <div className="flex justify-end">
                    <button
                      className="px-3 py-1 text-[11px] font-mono border-2 border-black bg-neutral-100 text-black hover:bg-white disabled:opacity-40 disabled:cursor-not-allowed rounded"
                      disabled={!canSubmit}
                      onClick={() => void handleSubmit()}
                    >
                      {submitting ? 'Sending…' : 'Send ↵'}
                    </button>
                  </div>
                </div>
              )}
              {taskDone && (
                <div className="shrink-0 border-t border-neutral-800 px-3 py-2">
                  <p className="text-[11px] text-neutral-700 font-mono">Task complete — chat session closed.</p>
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
