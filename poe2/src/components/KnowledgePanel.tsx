import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { KnowledgeEntry } from '../types';

interface Props {
  entries: KnowledgeEntry[];
  projectId: string;
  onClose: () => void;
}

export default function KnowledgePanel({ entries, projectId, onClose }: Props) {
  const [search, setSearch] = useState('');
  const [expanded, setExpanded] = useState<string | null>(null);
  const [promoted, setPromoted] = useState<Set<string>>(new Set());

  const filtered = entries.filter(e =>
    e.key.toLowerCase().includes(search.toLowerCase()) ||
    e.value.toLowerCase().includes(search.toLowerCase())
  );

  async function flagPromotion(id: string) {
    try {
      await invoke('flag_knowledge_for_promotion', { id, projectId });
      setPromoted(prev => new Set([...prev, id]));
    } catch (e) {
      console.error('Failed to flag for promotion:', e);
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70">
      <div className="flex flex-col bg-neutral-950 border-2 border-neutral-700 rounded w-[700px] h-[560px] shadow-2xl">
        <div className="flex items-center justify-between px-3 py-2 border-b border-neutral-800 shrink-0">
          <span className="text-[11px] font-semibold tracking-widest text-neutral-400 uppercase">Knowledge Register</span>
          <button className="text-neutral-500 hover:text-neutral-200 text-xs px-2 py-1" onClick={onClose}>✕ close</button>
        </div>
        <div className="px-3 py-2 border-b border-neutral-800 shrink-0">
          <input
            type="text"
            placeholder="Search…"
            value={search}
            onChange={e => setSearch(e.target.value)}
            className="w-full bg-neutral-900 border border-neutral-700 rounded px-2 py-1 text-[12px] text-neutral-200 placeholder-neutral-600 focus:outline-none focus:border-neutral-500"
          />
        </div>
        <div className="flex-1 overflow-y-auto">
          {filtered.length === 0 ? (
            <div className="flex h-full items-center justify-center text-neutral-600 text-xs">No entries</div>
          ) : (
            filtered.map(entry => (
              <div key={entry.id} className="border-b border-neutral-900">
                <div
                  className="flex items-start justify-between gap-2 px-3 py-2 cursor-pointer hover:bg-neutral-900/50"
                  onClick={() => setExpanded(expanded === entry.id ? null : entry.id)}
                >
                  <div className="min-w-0">
                    <span className="text-[11px] font-mono text-teal-400">{entry.key}</span>
                    {expanded !== entry.id && (
                      <p className="text-[11px] text-neutral-500 truncate mt-0.5">{entry.value}</p>
                    )}
                  </div>
                  <div className="flex items-center gap-2 shrink-0">
                    {promoted.has(entry.id) ? (
                      <span className="text-[10px] text-teal-500">flagged ✓</span>
                    ) : (
                      <button
                        className="text-[10px] text-neutral-600 hover:text-teal-400 transition-colors"
                        onClick={e => { e.stopPropagation(); void flagPromotion(entry.id); }}
                      >
                        flag ↑
                      </button>
                    )}
                    <span className="text-neutral-700 text-[10px]">{expanded === entry.id ? '▲' : '▼'}</span>
                  </div>
                </div>
                {expanded === entry.id && (
                  <pre className="px-3 pb-3 text-[11px] font-mono text-neutral-300 whitespace-pre-wrap leading-5">
                    {entry.value}
                  </pre>
                )}
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
