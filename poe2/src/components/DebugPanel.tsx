import { useEffect, useRef } from 'react';
import type { Node } from '../types';

interface Props {
  node: Node;
  lines: string[];   // raw poe-pty-line output for this task
  onHandoverOpen: (nodeId: string) => void;
  onClose: () => void;
}

export default function DebugPanel({ node, lines, onHandoverOpen, onClose }: Props) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'instant' });
  }, [lines]);

  const canHandover = node.status === 'waiting';

  return (
    <div className="fixed inset-0 z-50 flex items-end justify-end bg-black/40">
      <div className="flex flex-col bg-neutral-950 border border-neutral-700 rounded-tl w-[720px] h-[520px] shadow-2xl">

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
          <span>id: {node.id.slice(0, 16)}…</span>
          <span>type: {node.nodeType}</span>
          <span>started: {new Date(node.updatedAt).toLocaleTimeString()}</span>
          <span className="text-neutral-700">
            {lines.length === 0 ? 'no output yet' : `${lines.length} line${lines.length !== 1 ? 's' : ''} received`}
          </span>
        </div>

        {/* Raw output */}
        <div className="flex-1 overflow-y-auto p-2 font-mono text-[11px] leading-4 bg-neutral-950">
          {lines.length === 0 ? (
            <div className="flex h-full items-center justify-center text-neutral-700">
              <div className="text-center space-y-1">
                <p>No raw output received yet.</p>
                <p className="text-neutral-800">
                  poe-pty-line events will appear here as the agent writes output.
                </p>
              </div>
            </div>
          ) : (
            lines.map((line, i) => {
              // Colour-code poe: events vs prose
              const isPoeEvent = /^\s*\{"poe":/.test(line);
              const isError = /error|Error|panic|SIGTERM/i.test(line) && !isPoeEvent;
              const cls = isPoeEvent
                ? 'text-emerald-400'
                : isError
                  ? 'text-red-400'
                  : 'text-neutral-400';
              return (
                <div key={i} className={`whitespace-pre-wrap break-all ${cls}`}>
                  {line}
                </div>
              );
            })
          )}
          <div ref={bottomRef} />
        </div>

        {/* Footer hint */}
        <div className="px-3 py-1.5 border-t border-neutral-800/60 shrink-0 text-[10px] text-neutral-700">
          Raw poe-pty-line stream · green = poe: event · red = error ·
          {canHandover ? ' agent is waiting — use Handover to resume' : ' agent is running — Handover available when waiting'}
        </div>
      </div>
    </div>
  );
}
