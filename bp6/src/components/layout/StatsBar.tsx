interface StatsBarProps {
  stats: {
    total: number;
    open: number;
    inProgress: number;
    closed: number;
    blocked: number;
  };
}

export function StatsBar({ stats }: StatsBarProps) {
  return (
    <div className="flex items-center gap-10 mb-1">
      <div className="flex flex-col">
        <span className="text-xs font-black text-[var(--text-muted)] uppercase tracking-[0.25em] mb-1">Total</span>
        <span className="text-base font-black text-[var(--text-primary)]">{stats.total}</span>
      </div>
      <div className="flex flex-col border-l-2 border-[var(--border-primary)] pl-6">
        <span className="text-xs font-black text-[var(--status-open)] uppercase tracking-[0.25em] mb-1">Open</span>
        <span className="text-base font-black text-[var(--status-open)]">{stats.open}</span>
      </div>
      <div className="flex flex-col border-l-2 border-[var(--border-primary)] pl-6">
        <span className="text-xs font-black text-[var(--status-active)] uppercase tracking-[0.25em] mb-1">In Progress</span>
        <span className="text-base font-black text-[var(--status-active)]">{stats.inProgress}</span>
      </div>
      <div className="flex flex-col border-l-2 border-[var(--border-primary)] pl-6">
        <span className="text-xs font-black text-[var(--status-blocked)] uppercase tracking-[0.25em] mb-1">Blocked</span>
        <span className="text-base font-black text-[var(--status-blocked)]">{stats.blocked}</span>
      </div>
      <div className="flex flex-col border-l-2 border-[var(--border-primary)] pl-6">
        <span className="text-xs font-black text-[var(--status-done)] uppercase tracking-[0.25em] mb-1">Closed</span>
        <span className="text-base font-black text-[var(--status-done)]">{stats.closed}</span>
      </div>
    </div>
  );
}
