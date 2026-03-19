import { ArrowUp, ArrowDown } from "lucide-react";
import { cn } from "../../utils";

type SortColumn = 'priority' | 'title' | 'type' | 'id';
type SortOrder = 'asc' | 'desc' | 'none';

interface WBSHeaderProps {
  sortBy: string;
  sortOrder: SortOrder;
  onHeaderClick: (column: SortColumn) => void;
}

export function WBSHeader({
  sortBy,
  sortOrder,
  onHeaderClick,
}: WBSHeaderProps) {
  const sortIndicator = (col: SortColumn) => {
    if (sortBy !== col || sortOrder === 'none') return null;
    return sortOrder === 'asc'
      ? <ArrowUp size={12} strokeWidth={3} />
      : <ArrowDown size={12} strokeWidth={3} />;
  };

  return (
    <div className="flex items-center px-4 py-2 bg-[var(--background-tertiary)] text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.3em]">
      {/* chevron spacer */}
      <div className="w-10 shrink-0" />
      {/* ID */}
      <div
        className={cn(
          "w-24 shrink-0 px-2 border-r-2 border-[var(--border-primary)]/50 cursor-pointer hover:text-indigo-500 transition-colors flex items-center justify-between group",
          sortBy === 'id' && sortOrder !== 'none' && "text-indigo-500"
        )}
        onClick={() => onHeaderClick('id')}
      >
        <span>ID</span>
        {sortIndicator('id')}
      </div>
      {/* Name */}
      <div
        className={cn(
          "flex-1 px-4 border-r-2 border-[var(--border-primary)]/50 cursor-pointer hover:text-indigo-500 transition-colors flex items-center justify-between group",
          sortBy === 'title' && sortOrder !== 'none' && "text-indigo-500"
        )}
        onClick={() => onHeaderClick('title')}
      >
        <span>Name</span>
        {sortIndicator('title')}
      </div>
      {/* Agents column — no sort */}
      <div className="w-20 shrink-0 border-r-2 border-[var(--border-primary)]/50" />
      {/* Assignee */}
      <div className="w-28 shrink-0 px-2 border-r-2 border-[var(--border-primary)]/50">
        <span>Assignee</span>
      </div>
      {/* Type */}
      <div
        className={cn(
          "w-20 shrink-0 px-2 border-r-2 border-[var(--border-primary)]/50 cursor-pointer hover:text-indigo-500 transition-colors flex items-center justify-between group",
          sortBy === 'type' && sortOrder !== 'none' && "text-indigo-500"
        )}
        onClick={() => onHeaderClick('type')}
      >
        <span>Type</span>
        {sortIndicator('type')}
      </div>
      {/* Priority */}
      <div
        className={cn(
          "w-16 shrink-0 px-2 cursor-pointer hover:text-indigo-500 transition-colors flex items-center justify-between group",
          sortBy === 'priority' && sortOrder !== 'none' && "text-indigo-500"
        )}
        onClick={() => onHeaderClick('priority')}
      >
        <span>P</span>
        {sortIndicator('priority')}
      </div>
    </div>
  );
}

export type { SortColumn, SortOrder };
