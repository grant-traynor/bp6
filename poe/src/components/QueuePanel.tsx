import { useState, useEffect } from 'react';
import type { DecisionRow, TaskRow } from '../poe-types';

interface QueuePanelProps {
  decisions: DecisionRow[];
  tasks: TaskRow[];
  onResolve: (decisionId: number, resolution: string) => Promise<void>;
}

interface DecisionCardProps {
  decision: DecisionRow;
  task: TaskRow | null;
  onResolve: (decisionId: number, resolution: string) => Promise<void>;
}

// ── Time-waiting display ──────────────────────────────────────────────────────

function useElapsed(createdAt: number): string {
  const [elapsed, setElapsed] = useState(() => formatElapsed(createdAt));

  useEffect(() => {
    const id = setInterval(() => setElapsed(formatElapsed(createdAt)), 10_000);
    return () => clearInterval(id);
  }, [createdAt]);

  return elapsed;
}

function formatElapsed(createdAt: number): string {
  // createdAt is either a Unix timestamp (seconds, from DB) or ms (from live event).
  // Heuristic: if < 1e10 it's seconds, otherwise ms.
  const createdMs = createdAt < 1e10 ? createdAt * 1000 : createdAt;
  const diffSec = Math.max(0, Math.floor((Date.now() - createdMs) / 1000));
  if (diffSec < 60) return `waiting ${diffSec}s`;
  const m = Math.floor(diffSec / 60);
  const s = diffSec % 60;
  return `waiting ${m}m ${s}s`;
}

// ── Decision card ─────────────────────────────────────────────────────────────

function DecisionCard({ decision, task, onResolve }: DecisionCardProps) {
  const [input, setInput] = useState('');
  const [selectedOpt, setSelectedOpt] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const elapsed = useElapsed(decision.createdAt);

  const parsedOptions: string[] | null = (() => {
    if (!decision.options) return null;
    try {
      const parsed = JSON.parse(decision.options);
      if (Array.isArray(parsed)) return parsed as string[];
      return null;
    } catch {
      return null;
    }
  })();

  async function handleSubmit(resolution: string) {
    if (!resolution.trim()) return;
    setSubmitting(true);
    try {
      await onResolve(decision.id, resolution.trim());
      setInput('');
      setSelectedOpt(null);
    } catch (err) {
      console.error('resolve_decision failed:', err);
    } finally {
      setSubmitting(false);
    }
  }

  function handleOptionClick(opt: string) {
    setSelectedOpt(opt);
    setInput(opt);
  }

  const taskTitle = task?.title ?? decision.taskId;
  const taskSkill = task?.skill ?? null;

  return (
    <div
      className="mac-card"
      style={{
        margin: '6px 12px',
        padding: '10px 12px',
        display: 'flex',
        flexDirection: 'column',
        gap: 8,
      }}
    >
      {/* Header */}
      <div>
        <p
          style={{
            margin: 0,
            fontSize: 12,
            fontWeight: 600,
            color: 'var(--text-primary)',
            lineHeight: 1.4,
          }}
        >
          {decision.question}
        </p>

        {/* WBS context row */}
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 6,
            marginTop: 3,
            flexWrap: 'wrap',
          }}
        >
          <span style={{ fontSize: 10, color: 'var(--text-secondary)' }}>
            {taskTitle}
          </span>
          {taskSkill && (
            <>
              <span style={{ fontSize: 10, color: 'var(--text-placeholder)' }}>·</span>
              <span
                className="mono"
                style={{ fontSize: 10, color: 'var(--text-tertiary)' }}
              >
                {taskSkill}
              </span>
            </>
          )}
          <span style={{ fontSize: 10, color: 'var(--text-placeholder)' }}>·</span>
          <span style={{ fontSize: 10, color: 'var(--status-paused)' }}>
            {elapsed}
          </span>
        </div>
      </div>

      {/* Option buttons — click to select, not immediate submit */}
      {parsedOptions && parsedOptions.length > 0 && (
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
          {parsedOptions.map((opt) => {
            const isSelected = selectedOpt === opt;
            return (
              <button
                key={opt}
                onClick={() => handleOptionClick(opt)}
                disabled={submitting}
                className={`mac-btn${isSelected ? ' mac-btn-primary' : ''}`}
                style={{ fontSize: 11, padding: '3px 10px' }}
              >
                {opt}
              </button>
            );
          })}
        </div>
      )}

      {/* Free-text input */}
      <div style={{ display: 'flex', gap: 6 }}>
        <input
          className="mac-input"
          placeholder="Type a response…"
          value={input}
          onChange={(e) => {
            setInput(e.target.value);
            // Clear selection if user edits the text away from an option
            if (selectedOpt && e.target.value !== selectedOpt) {
              setSelectedOpt(null);
            }
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
              e.preventDefault();
              void handleSubmit(input);
            }
          }}
          disabled={submitting}
          style={{ flex: 1, fontSize: 11 }}
        />
        <button
          onClick={() => void handleSubmit(input)}
          disabled={submitting || !input.trim()}
          className="mac-btn mac-btn-primary"
          style={{ fontSize: 11, padding: '3px 10px', whiteSpace: 'nowrap' }}
        >
          Submit
        </button>
      </div>
    </div>
  );
}

export function QueuePanel({ decisions, tasks, onResolve }: QueuePanelProps) {
  const taskMap = new Map(tasks.map((t) => [t.id, t]));
  const count = decisions.length;

  return (
    <div style={{ height: '100%', display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
      {/* Panel header */}
      <div
        style={{
          padding: '0 16px',
          minHeight: 36,
          display: 'flex',
          alignItems: 'center',
          gap: 8,
          borderBottom: '1px solid var(--divider)',
          flexShrink: 0,
        }}
      >
        <span style={{ fontSize: 11, fontWeight: 600, color: 'var(--text-secondary)', textTransform: 'uppercase', letterSpacing: '0.04em' }}>
          Decisions
        </span>
        {count > 0 && (
          <span
            style={{
              display: 'inline-flex',
              alignItems: 'center',
              justifyContent: 'center',
              minWidth: 18,
              height: 18,
              padding: '0 5px',
              borderRadius: 9,
              background: 'var(--accent)',
              color: 'var(--accent-text)',
              fontSize: 10,
              fontWeight: 700,
            }}
          >
            {count}
          </span>
        )}
      </div>

      {/* Decision list */}
      <div style={{ flex: 1, overflowY: 'auto' }}>
        {decisions.length === 0 ? (
          <div
            style={{
              height: '100%',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
            }}
          >
            <p
              style={{
                fontSize: 12,
                color: 'var(--text-placeholder)',
                margin: 0,
                textAlign: 'center',
              }}
            >
              No pending decisions
            </p>
          </div>
        ) : (
          decisions.map((decision) => {
            const task = taskMap.get(decision.taskId) ?? null;
            return (
              <DecisionCard
                key={decision.id}
                decision={decision}
                task={task}
                onResolve={onResolve}
              />
            );
          })
        )}
      </div>
    </div>
  );
}
