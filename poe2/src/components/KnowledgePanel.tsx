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
  const [editing, setEditing] = useState<string | null>(null);
  const [editContent, setEditContent] = useState('');
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const filtered = entries.filter(e =>
    e.key.toLowerCase().includes(search.toLowerCase()) ||
    e.value.toLowerCase().includes(search.toLowerCase())
  );

  async function flagPromotion(id: string) {
    try {
      await invoke('promote_knowledge', { id, projectId });
      setPromoted(prev => new Set([...prev, id]));
    } catch (e) {
      console.error('Failed to promote knowledge:', e);
    }
  }

  function startEdit(entry: KnowledgeEntry) {
    setEditing(entry.id);
    setEditContent(entry.value);
    setSaveError(null);
  }

  async function saveEdit(id: string) {
    if (saving) return;
    setSaving(true);
    setSaveError(null);
    try {
      await invoke('update_knowledge', { id, content: editContent });
      setEditing(null);
    } catch (e) {
      setSaveError(String(e));
    } finally {
      setSaving(false);
    }
  }

  function cancelEdit() {
    setEditing(null);
    setSaveError(null);
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70">
      <div className="flex flex-col bg-neutral-950 border-2 border-neutral-700 rounded w-[780px] h-[580px] shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between px-3 py-2 border-b border-neutral-800 shrink-0">
          <span className="text-[11px] font-semibold tracking-widest text-neutral-400 uppercase">
            Knowledge Register
          </span>
          <button
            className="text-neutral-500 hover:text-neutral-200 text-xs px-2 py-1"
            onClick={onClose}
          >
            ✕ close
          </button>
        </div>

        {/* Search bar */}
        <div className="px-3 py-2 border-b border-neutral-800 shrink-0">
          <input
            type="text"
            placeholder="Search keys and values…"
            value={search}
            onChange={e => setSearch(e.target.value)}
            className="w-full bg-neutral-900 border border-neutral-700 rounded px-2 py-1 text-[12px] text-neutral-200 placeholder-neutral-600 focus:outline-none focus:border-neutral-500"
          />
        </div>

        {/* Entry list */}
        <div className="flex-1 overflow-y-auto">
          {filtered.length === 0 ? (
            <div className="flex h-full items-center justify-center text-neutral-600 text-xs">
              No entries
            </div>
          ) : (
            filtered.map(entry => (
              <div key={entry.id} className="border-b border-neutral-900">
                {/* Entry header row */}
                <div
                  className="flex items-start justify-between gap-2 px-3 py-2 cursor-pointer hover:bg-neutral-900/50"
                  onClick={() => setExpanded(expanded === entry.id ? null : entry.id)}
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="text-[11px] font-mono text-teal-400">{entry.key}</span>
                      {promoted.has(entry.id) && (
                        <span className="text-[9px] px-1 py-0 border border-teal-900 rounded text-teal-600">
                          promoted
                        </span>
                      )}
                      {entry.supersededId && (
                        <span className="text-[9px] px-1 py-0 border border-neutral-800 rounded text-neutral-700">
                          superseded
                        </span>
                      )}
                    </div>
                    {expanded !== entry.id && (
                      <p className="text-[11px] text-neutral-500 truncate mt-0.5">{entry.value}</p>
                    )}
                  </div>
                  <div className="flex items-center gap-2 shrink-0">
                    {!promoted.has(entry.id) && (
                      <button
                        className="text-[10px] text-neutral-600 hover:text-teal-400 transition-colors"
                        onClick={e => { e.stopPropagation(); void flagPromotion(entry.id); }}
                        title="Promote to user-level knowledge (~/.poe/knowledge/)"
                      >
                        promote ↑
                      </button>
                    )}
                    <button
                      className="text-[10px] text-neutral-600 hover:text-neutral-400 transition-colors"
                      onClick={e => { e.stopPropagation(); startEdit(entry); }}
                      title="Edit this entry"
                    >
                      edit
                    </button>
                    <span className="text-neutral-700 text-[10px]">
                      {expanded === entry.id ? '▲' : '▼'}
                    </span>
                  </div>
                </div>

                {/* Expanded view */}
                {expanded === entry.id && (
                  <div className="px-3 pb-3 space-y-2">
                    {editing === entry.id ? (
                      <div className="space-y-1.5">
                        <textarea
                          className="w-full bg-neutral-900 border border-neutral-600 rounded text-[12px] font-mono text-neutral-200 p-2 resize-none focus:outline-none focus:border-neutral-500"
                          rows={6}
                          value={editContent}
                          onChange={e => setEditContent(e.target.value)}
                          disabled={saving}
                        />
                        {saveError && (
                          <p className="text-[10px] text-red-400 font-mono">{saveError}</p>
                        )}
                        <div className="flex gap-2">
                          <button
                            className="px-2 py-1 rounded text-[11px] bg-neutral-700 hover:bg-neutral-600 text-neutral-200 disabled:opacity-40 transition-colors"
                            onClick={() => void saveEdit(entry.id)}
                            disabled={saving}
                          >
                            {saving ? 'Saving…' : 'Save'}
                          </button>
                          <button
                            className="px-2 py-1 rounded text-[11px] border border-neutral-700 text-neutral-500 hover:text-neutral-300 transition-colors"
                            onClick={cancelEdit}
                            disabled={saving}
                          >
                            Cancel
                          </button>
                        </div>
                      </div>
                    ) : (
                      <>
                        <pre className="text-[11px] font-mono text-neutral-300 whitespace-pre-wrap leading-5">
                          {entry.value}
                        </pre>
                        {entry.source && (
                          <p className="text-[10px] text-neutral-700 font-mono">
                            source: {entry.source}
                          </p>
                        )}
                        {entry.supersededId && (
                          <p className="text-[10px] text-neutral-700 font-mono">
                            supersedes: {entry.supersededId}
                          </p>
                        )}
                      </>
                    )}
                  </div>
                )}
              </div>
            ))
          )}
        </div>

        {/* Footer count */}
        <div className="px-3 py-1.5 border-t border-neutral-800 shrink-0">
          <span className="text-[10px] text-neutral-700">
            {filtered.length} entr{filtered.length !== 1 ? 'ies' : 'y'}
            {search && ` matching "${search}"`}
          </span>
        </div>
      </div>
    </div>
  );
}
