import { ArrowUp, ArrowDown, ChevronsDown, ChevronsUp } from "lucide-react";
import { cn } from "../../utils";

type SortColumn = 'priority' | 'title' | 'type' | 'id';
type SortOrder = 'asc' | 'desc' | 'none';

interface WBSHeaderProps {
  sortBy: string;
  sortOrder: SortOrder;
  onHeaderClick: (column: SortColumn) => void;
  beadCount: number;
  onExpandAll: () => void;
  onCollapseAll: () => void;
}

export function WBSHeader({
  sortBy,
  sortOrder,
  onHeaderClick,
  onExpandAll,
  onCollapseAll,
}: WBSHeaderProps) {
  const sortIndicator = (col: SortColumn) => {
    if (sortBy !== col || sortOrder === 'none') return null;
    return sortOrder === 'asc'
      ? <ArrowUp size={12} strokeWidth={3} />
      : <ArrowDown size={12} strokeWidth={3} />;
  };

  return (
    <div className="flex items-center px-4 py-2 bg-[var(--background-tertiary)] text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.3em]">
      <div className="w-10 shrink-0" />
      <div
        className={cn(
          "w-16 shrink-0 px-2 border-r-2 border-[var(--border-primary)]/50 cursor-pointer hover:text-indigo-500 transition-colors flex items-center justify-between group",
          sortBy === 'priority' && sortOrder !== 'none' && "text-indigo-500"
        )}
        onClick={() => onHeaderClick('priority')}
      >
        <span>P</span>
        {sortIndicator('priority')}
      </div>
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
      <div
        className={cn(
          "w-24 shrink-0 px-2 cursor-pointer hover:text-indigo-500 transition-colors flex items-center justify-between group",
          sortBy === 'id' && sortOrder !== 'none' && "text-indigo-500"
        )}
        onClick={() => onHeaderClick('id')}
      >
        <span>ID</span>
        {sortIndicator('id')}
      </div>
      <div className="flex items-center gap-1 ml-2">
        <button
          onClick={onExpandAll}
          title="Expand All"
          className="p-1.5 hover:bg-[var(--background-tertiary)] rounded-md text-[var(--text-muted)] hover:text-indigo-500 transition-colors"
        >
          <ChevronsDown size={16} strokeWidth={2.5} />
        </button>
        <button
          onClick={onCollapseAll}
          title="Collapse All"
          className="p-1.5 hover:bg-[var(--background-tertiary)] rounded-md text-[var(--text-muted)] hover:text-indigo-500 transition-colors"
        >
          <ChevronsUp size={16} strokeWidth={2.5} />
        </button>
      </div>
    </div>
  );
}

export type { SortColumn, SortOrder };
