import type { TaskRow, PhaseRow } from '../poe-types';

interface TaskMatrixProps {
  tasks: TaskRow[];
  phases: PhaseRow[];
}

const STATUS_ORDER: TaskRow['status'][] = ['running', 'pending', 'waiting', 'done', 'cancelled'];

const STATUS_LABELS: Record<TaskRow['status'], string> = {
  running: 'Running',
  pending: 'Pending',
  waiting: 'Waiting',
  done: 'Done',
  cancelled: 'Cancelled',
};

const STATUS_CSS: Record<TaskRow['status'], string> = {
  running: 'status-running',
  pending: 'status-paused',
  waiting: 'status-blocked',
  done: 'status-completed',
  cancelled: 'status-cancelled',
};

export function TaskMatrix({ tasks, phases }: TaskMatrixProps) {
  const phaseMap = new Map(phases.map((p) => [p.id, p]));

  // Group tasks by status
  const byStatus = new Map<TaskRow['status'], TaskRow[]>();
  for (const status of STATUS_ORDER) {
    byStatus.set(status, []);
  }
  for (const task of tasks) {
    const bucket = byStatus.get(task.status);
    if (bucket) bucket.push(task);
  }

  const nonEmpty = STATUS_ORDER.filter((s) => (byStatus.get(s)?.length ?? 0) > 0);

  if (nonEmpty.length === 0) {
    return (
      <div style={{ padding: '24px 16px', textAlign: 'center' }}>
        <p style={{ fontSize: 12, color: 'var(--text-tertiary)', margin: 0 }}>No tasks yet</p>
      </div>
    );
  }

  return (
    <div style={{ padding: '8px 0' }}>
      {nonEmpty.map((status) => {
        const statusTasks = byStatus.get(status) ?? [];
        return (
          <div key={status} style={{ marginBottom: 4 }}>
            {/* Status header */}
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 8,
                padding: '4px 16px',
              }}
            >
              <span className={`status-badge ${STATUS_CSS[status]}`}>
                {STATUS_LABELS[status]}
              </span>
              <span style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>
                {statusTasks.length}
              </span>
            </div>

            {/* Task rows */}
            {statusTasks.map((task) => {
              const phase = task.phaseId ? phaseMap.get(task.phaseId) : null;
              return (
                <div
                  key={task.id}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 8,
                    padding: '4px 16px 4px 32px',
                    cursor: 'default',
                  }}
                  onMouseEnter={(e) =>
                    (e.currentTarget.style.background = 'var(--sidebar-hover-bg)')
                  }
                  onMouseLeave={(e) =>
                    (e.currentTarget.style.background = 'transparent')
                  }
                >
                  <span
                    style={{
                      flex: 1,
                      fontSize: 12,
                      color: 'var(--text-primary)',
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                      whiteSpace: 'nowrap',
                    }}
                  >
                    {task.title}
                  </span>
                  {phase && (
                    <span
                      style={{
                        fontSize: 10,
                        color: 'var(--text-tertiary)',
                        flexShrink: 0,
                      }}
                    >
                      {phase.name}
                    </span>
                  )}
                  {task.skill && (
                    <span
                      style={{
                        fontSize: 10,
                        color: 'var(--text-placeholder)',
                        flexShrink: 0,
                        fontFamily: 'ui-monospace, "SF Mono", Menlo, monospace',
                      }}
                    >
                      {task.skill}
                    </span>
                  )}
                </div>
              );
            })}
          </div>
        );
      })}
    </div>
  );
}
