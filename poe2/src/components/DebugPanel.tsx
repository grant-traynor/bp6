import { useEffect, useRef } from 'react';
import type { Node } from '../types';

// A poe-agent-stream event payload from Rust.
// "event" is the raw stream-json JSON object emitted by claude on stdout.
interface StreamEvent {
  agentId: string;
  taskId: string;
  projectId: string;
  event: Record<string, unknown>;
}

interface Props {
  node: Node;
  // Raw stream-json events buffered for this task.
  streamEvents: StreamEvent[];
  onHandoverOpen: (nodeId: string) => void;
  onClose: () => void;
}

function eventLabel(ev: Record<string, unknown>): { label: string; cls: string } {
  const type = (ev['type'] as string) ?? 'unknown';
  const subtype = ev['subtype'] as string | undefined;

  if (type === 'system' && subtype === 'init') return { label: 'init', cls: 'text-sky-400' };
  if (type === 'system') return { label: `system:${subtype ?? '?'}`, cls: 'text-sky-600' };
  if (type === 'result') return { label: `result:${subtype ?? '?'}`, cls: 'text-emerald-400' };
  if (type === 'assistant') return { label: 'assistant', cls: 'text-neutral-300' };
  if (type === 'content_block_delta') {
    const deltaType = (ev['delta'] as Record<string, unknown> | undefined)?.['type'] as string | undefined;
    if (deltaType === 'text_delta') return { label: 'text_delta', cls: 'text-neutral-400' };
    return { label: `delta:${deltaType ?? '?'}`, cls: 'text-neutral-600' };
  }
  if (type === 'content_block_start') return { label: 'block_start', cls: 'text-neutral-700' };
  if (type === 'content_block_stop') return { label: 'block_stop', cls: 'text-neutral-700' };
  if (type === 'message_start') return { label: 'message_start', cls: 'text-neutral-700' };
  if (type === 'message_delta') return { label: 'message_delta', cls: 'text-neutral-700' };
  if (type === 'message_stop') return { label: 'message_stop', cls: 'text-neutral-700' };
  if (type === 'ping') return { label: 'ping', cls: 'text-neutral-800' };
  return { label: type, cls: 'text-neutral-500' };
}

function textPreview(ev: Record<string, unknown>): string | null {
  // Extract assistant text content for display alongside the event type.
  const type = ev['type'] as string;
  if (type === 'assistant') {
    const blocks = (ev['message'] as Record<string, unknown> | undefined)?.['content'] as unknown[] | undefined;
    const text = blocks
      ?.filter((b) => (b as Record<string, unknown>)['type'] === 'text')
      .map((b) => (b as Record<string, unknown>)['text'] as string)
      .join('');
    return text?.slice(0, 200) ?? null;
  }
  if (type === 'content_block_delta') {
    const delta = ev['delta'] as Record<string, unknown> | undefined;
    if (delta?.['type'] === 'text_delta') return (delta['text'] as string)?.slice(0, 200) ?? null;
  }
  return null;
}

export default function DebugPanel({ node, streamEvents, onHandoverOpen, onClose }: Props) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'instant' });
  }, [streamEvents]);

  const canHandover = node.status === 'waiting';
  const hasInit = streamEvents.some(e => e.event['type'] === 'system' && e.event['subtype'] === 'init');

  return (
    <div className="fixed inset-0 z-50 flex items-end justify-end bg-black/40">
      <div className="flex flex-col bg-neutral-950 border border-neutral-700 rounded-tl w-[760px] h-[540px] shadow-2xl">

        {/* Header */}
        <div className="flex items-center justify-between px-3 py-2 border-b border-neutral-800 shrink-0">
          <div className="flex items-center gap-3 min-w-0">
            <span className={`text-[11px] font-mono ${node.status === 'running' ? 'text-emerald-400' : 'text-amber-400'}`}>
              {node.status === 'running' ? '● running' : '? waiting'}
            </span>
            <span className="text-[12px] text-neutral-300 truncate">{node.title}</span>
            {node.skillId && (
              <span className="text-[10px] text-neutral-600 font-mono shrink-0">{node.skillId}</span>
            )}
          </div>
          <div className="flex items-center gap-2 shrink-0">
            {canHandover && (
              <button
                className="px-2 py-0.5 rounded text-[10px] border border-amber-800 text-amber-500 hover:border-amber-600 hover:text-amber-300 transition-colors"
                onClick={() => { onHandoverOpen(node.id); onClose(); }}
                title="Open claude --resume session"
              >
                Handover →
              </button>
            )}
            <button
              className="text-neutral-500 hover:text-neutral-200 text-xs px-2 py-1 transition-colors"
              onClick={onClose}
            >
              ✕
            </button>
          </div>
        </div>

        {/* Metadata bar */}
        <div className="flex items-center gap-4 px-3 py-1.5 border-b border-neutral-800/60 shrink-0 text-[10px] text-neutral-600 font-mono">
          <span>task: {node.id.slice(0, 16)}…</span>
          <span>type: {node.nodeType}</span>
          {streamEvents.length === 0
            ? <span className="text-neutral-700">waiting for stream-json events…</span>
            : hasInit
              ? <span className="text-sky-700">{streamEvents.length} events · claude confirmed started</span>
              : <span className="text-neutral-700">{streamEvents.length} events</span>
          }
        </div>

        {/* Stream event log */}
        <div className="flex-1 overflow-y-auto p-2 font-mono text-[11px] leading-5 bg-neutral-950">
          {streamEvents.length === 0 ? (
            <div className="flex h-full items-center justify-center text-neutral-700">
              <div className="text-center space-y-1">
                <p>No stream-json events received yet.</p>
                <p className="text-neutral-800">
                  Events arrive on claude's stdout pipe. If this stays empty, claude may not have started.
                </p>
              </div>
            </div>
          ) : (
            streamEvents.map((se, i) => {
              const { label, cls } = eventLabel(se.event);
              const preview = textPreview(se.event);
              return (
                <div key={i} className="flex items-baseline gap-2 py-0.5">
                  <span className={`shrink-0 w-28 text-right ${cls}`}>{label}</span>
                  {preview && (
                    <span className="text-neutral-500 truncate">{preview}</span>
                  )}
                </div>
              );
            })
          )}
          <div ref={bottomRef} />
        </div>

        {/* Footer */}
        <div className="px-3 py-1.5 border-t border-neutral-800/60 shrink-0 text-[10px] text-neutral-700">
          stream-json stdout events · sky = init · emerald = result · grey = content ·
          {canHandover ? ' agent is waiting — use Handover to resume' : ' agent is running'}
        </div>
      </div>
    </div>
  );
}
