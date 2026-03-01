import { useState, useCallback, useRef, useEffect, memo } from "react";
import { ArrowUp, ArrowDown, User, Clock, Calendar, Tag as TagIcon } from "lucide-react";
import { cn } from "../../utils";
import type { BeadNode, SessionInfo } from "../../api";
import { getChipStyles } from "../shared/Chip";
import { SessionIndicator } from "../shared/SessionIndicator";

const ROW_HEIGHT = 48;
const OVERSCAN = 5;
const COL_COUNT = 9;

function formatDate(dateStr?: string) {
  if (!dateStr) return "-";
  try {
    const date = new Date(dateStr);
    return date.toLocaleDateString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric'
    });
  } catch {
    return dateStr;
  }
}

interface ListViewRowProps {
  bead: BeadNode;
  isSelected: boolean;
  sessions: SessionInfo[] | undefined;
  onBeadClick: (bead: BeadNode) => void;
  onSessionClick?: (sessionId: string) => void;
}

const ListViewRow = memo(function ListViewRow({
  bead,
  isSelected,
  sessions,
  onBeadClick,
  onSessionClick,
}: ListViewRowProps) {
  return (
    <tr
      className={cn(
        "border-b border-[var(--border-primary)]/50 hover:bg-[var(--background-secondary)] transition-colors cursor-pointer group",
        isSelected && "bg-[var(--accent-primary)]/10"
      )}
      onClick={() => onBeadClick(bead)}
    >
      <td className="px-4 py-3">
        <span className={cn(
          "font-mono text-xs font-black px-2 py-0.5 rounded-md tracking-tighter border shadow-sm transition-all",
          isSelected ? "bg-[var(--accent-primary)] text-white border-[var(--accent-secondary)]" :
          bead.isCritical ? "bg-[var(--status-blocked)]/20 text-[var(--status-blocked)] border-[var(--status-blocked)]/40" : "bg-[var(--background-tertiary)] text-[var(--text-primary)] border-[var(--border-primary)]"
        )}>
          {bead.id.split('-').pop()}
        </span>
      </td>
      <td className="px-4 py-3">
        <div className="flex items-center gap-2">
          <span className={cn(
            "text-sm font-black tracking-tight",
            bead.status === 'closed' && "italic opacity-60 text-[var(--text-muted)]",
            isSelected ? "text-[var(--accent-primary)]" : "text-[var(--text-primary)] group-hover:text-indigo-600 dark:group-hover:text-indigo-400"
          )}>
            {bead.title}
          </span>
          <SessionIndicator sessions={sessions ?? []} className="shrink-0" onSessionClick={onSessionClick} />
        </div>
      </td>
      <td className="px-4 py-3">
        <span className={cn(
          "text-[10px] font-black px-2 py-0.5 rounded-full uppercase tracking-widest border",
          bead.status === 'open' ? "bg-slate-100 text-slate-600 border-slate-200 dark:bg-slate-800/40 dark:text-slate-400 dark:border-slate-700" :
          bead.status === 'in_progress' ? "bg-indigo-100 text-indigo-700 border-indigo-200 dark:bg-indigo-900/30 dark:text-indigo-300 dark:border-indigo-800" :
          bead.status === 'closed' ? "bg-emerald-100 text-emerald-700 border-emerald-200 dark:bg-emerald-900/30 dark:text-emerald-300 dark:border-emerald-800" :
          "bg-amber-100 text-amber-700 border-amber-200 dark:bg-amber-900/30 dark:text-amber-300 dark:border-amber-800"
        )}>
          {bead.status.replace('_', ' ')}
        </span>
      </td>
      <td className="px-4 py-3">
        <span className={cn(
          "font-mono text-xs font-black px-2 py-0.5 rounded border shadow-sm",
          bead.priority <= 1 ? "bg-rose-100 text-rose-700 border-rose-200 dark:bg-rose-900/30 dark:text-rose-300 dark:border-rose-800" :
          bead.priority === 2 ? "bg-indigo-100 text-indigo-700 border-indigo-200 dark:bg-indigo-900/30 dark:text-indigo-300 dark:border-indigo-800" :
          "bg-slate-100 text-slate-600 border-slate-200 dark:bg-slate-800/40 dark:text-slate-400 dark:border-slate-700"
        )}>
          P{bead.priority}
        </span>
      </td>
      <td className="px-4 py-3">
        <span className={cn(
          "text-[10px] font-black px-2 py-0.5 rounded-md uppercase tracking-widest border",
          getChipStyles(bead.issueType)
        )}>
          {bead.issueType}
        </span>
      </td>
      <td className="px-4 py-3">
        <span className="text-xs font-bold text-[var(--text-secondary)] truncate block">
          {bead.owner || "-"}
        </span>
      </td>
      <td className="px-4 py-3 text-center">
        <span className="text-xs font-mono font-black text-[var(--text-primary)]">
          {bead.estimate ? `${bead.estimate}m` : "-"}
        </span>
      </td>
      <td className="px-4 py-3">
        <span className="text-xs font-bold text-[var(--text-muted)]">
          {formatDate(bead.createdAt)}
        </span>
      </td>
      <td className="px-4 py-3">
        <div className="flex flex-wrap gap-1">
          {(bead.labels || []).slice(0, 3).map((label, idx) => (
            <span key={idx} className="text-[9px] font-black bg-indigo-500/5 text-indigo-600 dark:text-indigo-400 border border-indigo-500/10 px-1.5 py-0.5 rounded uppercase tracking-wider">
              {label}
            </span>
          ))}
          {(bead.labels || []).length > 3 && (
            <span className="text-[9px] font-black text-[var(--text-muted)] px-1">
              +{(bead.labels || []).length - 3}
            </span>
          )}
        </div>
      </td>
    </tr>
  );
});

interface ListViewProps {
  beads: BeadNode[];
  onBeadClick: (bead: BeadNode) => void;
  selectedBeadId?: string;
  sortBy: 'priority' | 'title' | 'type' | 'id' | 'none';
  sortOrder: 'asc' | 'desc' | 'none';
  onHeaderClick: (column: 'priority' | 'title' | 'type' | 'id') => void;
  sessionsByBead?: Record<string, SessionInfo[]>;
  onSessionClick?: (sessionId: string) => void;
}

export const ListView = memo(function ListView({
  beads,
  onBeadClick,
  selectedBeadId,
  sortBy,
  sortOrder,
  onHeaderClick,
  sessionsByBead = {},
  onSessionClick
}: ListViewProps) {
  const [scrollTop, setScrollTop] = useState(0);
  const [containerHeight, setContainerHeight] = useState(600);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    setContainerHeight(el.clientHeight);
    const ro = new ResizeObserver(() => setContainerHeight(el.clientHeight));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const handleScroll = useCallback((e: React.UIEvent<HTMLDivElement>) => {
    setScrollTop(e.currentTarget.scrollTop);
  }, []);

  const startIndex = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN);
  const visibleCount = Math.ceil(containerHeight / ROW_HEIGHT) + OVERSCAN * 2;
  const endIndex = Math.min(beads.length, startIndex + visibleCount);
  const visibleBeads = beads.slice(startIndex, endIndex);
  const paddingTop = startIndex * ROW_HEIGHT;
  const paddingBottom = (beads.length - endIndex) * ROW_HEIGHT;

  const renderSortIcon = (column: string) => {
    if (sortBy !== column || sortOrder === 'none') return null;
    return sortOrder === 'asc' ? <ArrowUp size={14} className="ml-1 stroke-[3]" /> : <ArrowDown size={14} className="ml-1 stroke-[3]" />;
  };

  return (
    <div ref={containerRef} className="flex-1 overflow-auto bg-[var(--background-primary)] custom-scrollbar" onScroll={handleScroll}>
      <table className="w-full border-collapse text-left min-w-[1000px]">
        <thead className="sticky top-0 z-20 bg-[var(--background-tertiary)] border-b-2 border-[var(--border-primary)] shadow-sm">
          <tr>
            <th
              className={cn(
                "px-4 py-3 text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.2em] cursor-pointer hover:text-indigo-500 transition-colors w-24",
                sortBy === 'id' && "text-indigo-500"
              )}
              onClick={() => onHeaderClick('id')}
            >
              <div className="flex items-center">ID {renderSortIcon('id')}</div>
            </th>
            <th
              className={cn(
                "px-4 py-3 text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.2em] cursor-pointer hover:text-indigo-500 transition-colors",
                sortBy === 'title' && "text-indigo-500"
              )}
              onClick={() => onHeaderClick('title')}
            >
              <div className="flex items-center">Title {renderSortIcon('title')}</div>
            </th>
            <th className="px-4 py-3 text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.2em] w-32">
              Status
            </th>
            <th
              className={cn(
                "px-4 py-3 text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.2em] cursor-pointer hover:text-indigo-500 transition-colors w-24",
                sortBy === 'priority' && "text-indigo-500"
              )}
              onClick={() => onHeaderClick('priority')}
            >
              <div className="flex items-center">Priority {renderSortIcon('priority')}</div>
            </th>
            <th
              className={cn(
                "px-4 py-3 text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.2em] cursor-pointer hover:text-indigo-500 transition-colors w-32",
                sortBy === 'type' && "text-indigo-500"
              )}
              onClick={() => onHeaderClick('type')}
            >
              <div className="flex items-center">Type {renderSortIcon('type')}</div>
            </th>
            <th className="px-4 py-3 text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.2em] w-40">
              <div className="flex items-center gap-2"><User size={14} /> Owner</div>
            </th>
            <th className="px-4 py-3 text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.2em] w-24">
              <div className="flex items-center gap-2"><Clock size={14} /> Est</div>
            </th>
            <th className="px-4 py-3 text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.2em] w-40">
              <div className="flex items-center gap-2"><Calendar size={14} /> Created</div>
            </th>
            <th className="px-4 py-3 text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">
              <div className="flex items-center gap-2"><TagIcon size={14} /> Labels</div>
            </th>
          </tr>
        </thead>
        <tbody>
          {paddingTop > 0 && (
            <tr><td colSpan={COL_COUNT} style={{ height: paddingTop, padding: 0 }} /></tr>
          )}
          {visibleBeads.map((bead) => (
            <ListViewRow
              key={bead.id}
              bead={bead}
              isSelected={selectedBeadId === bead.id}
              sessions={sessionsByBead[bead.id]}
              onBeadClick={onBeadClick}
              onSessionClick={onSessionClick}
            />
          ))}
          {paddingBottom > 0 && (
            <tr><td colSpan={COL_COUNT} style={{ height: paddingBottom, padding: 0 }} /></tr>
          )}
        </tbody>
      </table>
      {beads.length === 0 && (
        <div className="flex flex-col items-center justify-center p-20 text-[var(--text-muted)]">
          <TagIcon size={48} className="opacity-20 mb-4" />
          <p className="text-lg font-black uppercase tracking-widest">No tasks found</p>
          <p className="text-sm font-medium">Try adjusting your filters or search terms</p>
        </div>
      )}
    </div>
  );
});
