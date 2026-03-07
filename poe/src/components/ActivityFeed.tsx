import { useRef, useEffect } from 'react';
import type { ActivityItem, TaskRow } from '../poe-types';

interface ActivityFeedProps {
  items: ActivityItem[];
  tasks: TaskRow[];
}

type EventBadgeColor = {
  background: string;
  color: string;
  label: string;
};

function getBadgeStyle(eventType: string): EventBadgeColor {
  switch (eventType) {
    case 'brief':
      return { background: 'rgba(0, 122, 255, 0.12)', color: '#007AFF', label: 'brief' };
    case 'step':
      return { background: 'rgba(142, 142, 147, 0.12)', color: 'var(--text-secondary)', label: 'step' };
    case 'artifact':
      return { background: 'rgba(52, 199, 89, 0.12)', color: '#34C759', label: 'artifact' };
    case 'decision':
      return { background: 'rgba(255, 149, 0, 0.12)', color: '#FF9500', label: 'decision' };
    case 'done':
      return { background: 'rgba(52, 199, 89, 0.15)', color: '#30D158', label: 'done' };
    case 'knowledge':
      return { background: 'rgba(175, 82, 222, 0.12)', color: '#AF52DE', label: 'knowledge' };
    default:
      return { background: 'rgba(142, 142, 147, 0.08)', color: 'var(--text-tertiary)', label: eventType };
  }
}

function relativeTime(createdAt: number): string {
  const diffMs = Date.now() - createdAt;
  const diffSecs = Math.floor(diffMs / 1000);
  if (diffSecs < 60) return `${diffSecs}s ago`;
  const diffMins = Math.floor(diffSecs / 60);
  if (diffMins < 60) return `${diffMins}m ago`;
  const diffHours = Math.floor(diffMins / 60);
  return `${diffHours}h ago`;
}

function getPayloadSummary(eventType: string, payload: unknown): string | null {
  if (payload === null || payload === undefined) return null;
  const p = payload as Record<string, unknown>;

  switch (eventType) {
    case 'brief': {
      const content = typeof p['content'] === 'string' ? p['content'] : null;
      if (!content) return null;
      return content.length > 120 ? content.slice(0, 120) + '…' : content;
    }
    case 'step': {
      const name = typeof p['name'] === 'string' ? p['name'] : null;
      return name;
    }
    case 'artifact': {
      const name = typeof p['name'] === 'string' ? p['name'] : null;
      return name;
    }
    case 'decision': {
      const question = typeof p['question'] === 'string' ? p['question'] : null;
      return question;
    }
    case 'done': {
      const summary = typeof p['summary'] === 'string' ? p['summary'] : null;
      return summary ?? 'Completed';
    }
    default:
      return null;
  }
}

export function ActivityFeed({ items, tasks }: ActivityFeedProps) {
  const bottomRef = useRef<HTMLDivElement>(null);
  const taskMap = new Map(tasks.map((t) => [t.id, t]));

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [items.length]);

  return (
    <div
      style={{
        height: '100%',
        overflowY: 'auto',
        display: 'flex',
        flexDirection: 'column',
        padding: '8px 0',
      }}
    >
      {items.length === 0 ? (
        <div
          style={{
            flex: 1,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
          }}
        >
          <p style={{ fontSize: 12, color: 'var(--text-tertiary)', margin: 0 }}>
            Waiting for activity…
          </p>
        </div>
      ) : (
        <>
          {items.map((item) => {
            const badge = getBadgeStyle(item.eventType);
            const task = taskMap.get(item.taskId);
            const taskTitle = task?.title ?? item.taskId;
            const summary = getPayloadSummary(item.eventType, item.payload);

            return (
              <div
                key={item.id}
                style={{
                  display: 'flex',
                  alignItems: 'flex-start',
                  gap: 8,
                  padding: '6px 16px',
                  borderBottom: '1px solid var(--divider)',
                }}
                onMouseEnter={(e) =>
                  (e.currentTarget.style.background = 'var(--sidebar-hover-bg)')
                }
                onMouseLeave={(e) =>
                  (e.currentTarget.style.background = 'transparent')
                }
              >
                {/* Badge */}
                <span
                  style={{
                    display: 'inline-flex',
                    alignItems: 'center',
                    padding: '1px 6px',
                    borderRadius: 4,
                    fontSize: 10,
                    fontWeight: 600,
                    letterSpacing: '0.02em',
                    whiteSpace: 'nowrap',
                    flexShrink: 0,
                    marginTop: 1,
                    background: badge.background,
                    color: badge.color,
                  }}
                >
                  {badge.label}
                </span>

                {/* Content */}
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ display: 'flex', alignItems: 'baseline', gap: 6 }}>
                    <span
                      style={{
                        fontSize: 12,
                        fontWeight: 500,
                        color: 'var(--text-primary)',
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                        whiteSpace: 'nowrap',
                      }}
                    >
                      {taskTitle}
                    </span>
                    <span
                      style={{
                        fontSize: 10,
                        color: 'var(--text-tertiary)',
                        flexShrink: 0,
                      }}
                    >
                      {relativeTime(item.createdAt)}
                    </span>
                  </div>
                  {summary && (
                    <p
                      style={{
                        margin: '2px 0 0',
                        fontSize: 11,
                        color: 'var(--text-secondary)',
                        overflow: 'hidden',
                        textOverflow: 'ellipsis',
                        whiteSpace: 'nowrap',
                      }}
                    >
                      {summary}
                    </p>
                  )}
                </div>
              </div>
            );
          })}
          <div ref={bottomRef} />
        </>
      )}
    </div>
  );
}
