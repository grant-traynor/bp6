import { useRef } from "react";

type ClosedTimeFilter =
  | 'all'
  | '1h'
  | '6h'
  | '24h'
  | '7d'
  | '30d'
  | 'older_than_6h';

interface WBSFiltersProps {
  filterText: string;
  onFilterTextChange: (value: string) => void;
  hideClosed: boolean;
  onHideClosedChange: (value: boolean) => void;
  closedTimeFilter: ClosedTimeFilter;
  onClosedTimeFilterChange: (value: ClosedTimeFilter) => void;
  includeHierarchy: boolean;
  onIncludeHierarchyChange: (value: boolean) => void;
  searchInputRef?: React.RefObject<HTMLInputElement | null>;
}

export function WBSFilters({
  filterText,
  onFilterTextChange,
  hideClosed,
  onHideClosedChange,
  closedTimeFilter,
  onClosedTimeFilterChange,
  includeHierarchy,
  onIncludeHierarchyChange,
  searchInputRef,
}: WBSFiltersProps) {
  const internalRef = useRef<HTMLInputElement>(null);
  const inputRef = searchInputRef ?? internalRef;

  return (
    <div className="px-4 py-3 bg-[var(--background-primary)]">
      <div className="relative group mb-3">
        <input
          ref={inputRef}
          type="text"
          placeholder="Search by title, ID, or label..."
          className="w-full bg-[var(--background-secondary)] border-[var(--border-thick)] border-[var(--border-primary)] rounded-xl px-4 py-2.5 text-sm font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none transition-all placeholder:text-[var(--text-muted)] shadow-[var(--shadow-inset)]"
          value={filterText}
          onChange={e => onFilterTextChange(e.target.value)}
        />
      </div>
      <div className="flex items-center gap-4 px-1 flex-wrap">
        <label className="flex items-center gap-2 cursor-pointer group">
          <input
            type="checkbox"
            checked={hideClosed}
            onChange={e => onHideClosedChange(e.target.checked)}
            className="rounded border-2 border-[var(--border-primary)] text-indigo-600 focus:ring-indigo-500 h-3.5 w-3.5 bg-[var(--background-secondary)]"
          />
          <span className="text-xs font-black text-[var(--text-muted)] group-hover:text-[var(--text-primary)] uppercase tracking-wider">Hide Closed</span>
        </label>
        <label className="flex items-center gap-2 cursor-pointer group">
          <input
            type="checkbox"
            checked={includeHierarchy}
            onChange={e => onIncludeHierarchyChange(e.target.checked)}
            className="rounded border-2 border-[var(--border-primary)] text-indigo-600 focus:ring-indigo-500 h-3.5 w-3.5 bg-[var(--background-secondary)]"
          />
          <span className="text-xs font-black text-[var(--text-muted)] group-hover:text-[var(--text-primary)] uppercase tracking-wider">Hierarchy</span>
        </label>
        <label className="flex items-center gap-2 group">
          <span className="text-xs font-black text-[var(--text-muted)] uppercase tracking-wider">Closed:</span>
          <select
            value={closedTimeFilter}
            onChange={e => onClosedTimeFilterChange(e.target.value as ClosedTimeFilter)}
            className="text-xs font-black bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-lg px-2 py-1 text-[var(--text-primary)] focus:border-indigo-500 outline-none uppercase tracking-wider"
          >
            <option value="all">All Time</option>
            <option value="1h">Last Hour</option>
            <option value="6h">Last 6 Hours</option>
            <option value="24h">Last 24 Hours</option>
            <option value="7d">Last 7 Days</option>
            <option value="30d">Last 30 Days</option>
            <option value="older_than_6h">Older Than 6h</option>
          </select>
        </label>
      </div>
    </div>
  );
}

export type { ClosedTimeFilter };
