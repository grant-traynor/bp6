import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import '@xterm/xterm/css/xterm.css';
import type { Node } from '../types';

interface Props {
  nodeId: string;
  node: Node | null;
  onClose: () => void;
}

export default function AgentHandover({ nodeId, node, onClose }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const [status, setStatus] = useState<'connecting' | 'connected' | 'ended'>('connecting');
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    let cleanupResize: (() => void) | undefined;

    async function open() {
      try {
        const port = await invoke<number>('agent_handover_open', { nodeId });
        if (cancelled) return;

        const term = new Terminal({ cursorBlink: true, fontFamily: 'monospace', fontSize: 13, theme: { background: '#0a0a0a', foreground: '#d4d4d4' } });
        const fit = new FitAddon();
        term.loadAddon(fit);
        termRef.current = term;
        fitRef.current = fit;

        if (containerRef.current) {
          term.open(containerRef.current);
          fit.fit();
        }

        const ws = new WebSocket(`ws://127.0.0.1:${port}`);
        ws.binaryType = 'arraybuffer';
        wsRef.current = ws;

        ws.onopen = () => {
          if (cancelled) return;
          setStatus('connected');
          // Send initial resize
          const dims = fit.proposeDimensions();
          if (dims) {
            ws.send(JSON.stringify({ type: 'resize', cols: dims.cols, rows: dims.rows }));
          }
        };

        ws.onmessage = (e) => {
          if (e.data instanceof ArrayBuffer) {
            term.write(new Uint8Array(e.data));
          }
        };

        ws.onclose = () => {
          if (!cancelled) setStatus('ended');
          term.write('\r\n\x1b[90m[session ended]\x1b[0m\r\n');
        };

        ws.onerror = () => setError('WebSocket error');

        // Keyboard input → WebSocket
        term.onData((data) => {
          if (ws.readyState === WebSocket.OPEN) {
            ws.send(new TextEncoder().encode(data));
          }
        });

        // Resize handler
        const handleResize = () => {
          fit.fit();
          const dims = fit.proposeDimensions();
          if (dims && ws.readyState === WebSocket.OPEN) {
            ws.send(JSON.stringify({ type: 'resize', cols: dims.cols, rows: dims.rows }));
          }
        };
        window.addEventListener('resize', handleResize);
        cleanupResize = () => window.removeEventListener('resize', handleResize);
      } catch (e) {
        if (!cancelled) setError(String(e));
      }
    }

    void open();

    return () => {
      cancelled = true;
      cleanupResize?.();
      wsRef.current?.close();
      termRef.current?.dispose();
      invoke('agent_handover_close', { nodeId }).catch(() => {});
    };
  }, [nodeId]);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70">
      <div className="flex flex-col bg-neutral-950 border-2 border-neutral-700 rounded w-[900px] h-[600px] shadow-2xl">
        <div className="flex items-center justify-between px-3 py-2 border-b border-neutral-800 shrink-0">
          <div className="flex items-center gap-2 min-w-0">
            <span className="text-[11px] font-mono text-neutral-400 truncate">
              claude --resume · {node?.skillId ?? '—'} · {node?.title ?? nodeId}
            </span>
            {status === 'connecting' && (
              <span className="text-[10px] text-neutral-600">connecting…</span>
            )}
            {status === 'ended' && (
              <span className="text-[10px] text-neutral-600">[ended]</span>
            )}
          </div>
          <button
            className="text-neutral-500 hover:text-neutral-200 text-xs px-2 py-1 transition-colors"
            onClick={onClose}
          >
            ✕ close
          </button>
        </div>
        {error ? (
          <div className="flex-1 flex items-center justify-center text-red-400 text-sm">{error}</div>
        ) : (
          <div ref={containerRef} className="flex-1 min-h-0 p-1" />
        )}
      </div>
    </div>
  );
}
