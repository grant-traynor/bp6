import { ChevronDown, ChevronRight } from "lucide-react";
import { cn } from "../../utils";
import type { Bead, WBSNode } from "../../api";
import { StatusIcon } from "../shared/StatusIcon";
import { getChipStyles } from "../shared/Chip";

interface WBSTreeItemProps {
  node: WBSNode;
  depth?: number;
  onToggle: (id: string) => void;
  onClick: (bead: Bead) => void;
}

export const WBSTreeItem = ({ 
  node, 
  depth = 0, 
  onToggle,
  onClick
}: WBSTreeItemProps) => {
  const hasChildren = node.children.length > 0;

  return (
    <div className="select-none h-[48px] flex flex-col justify-center border-b border-[var(--border-primary)]">
      <div 
        className={cn(
          "flex items-center hover:bg-[var(--background-primary)] cursor-pointer group transition-all h-full",
          node.isCritical && "bg-rose-500/[0.08]"
        )}
        onClick={() => onClick(node as Bead)}
      >
        <div 
          className="w-10 shrink-0 flex items-center justify-center h-full text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors" 
          onClick={(e) => { e.stopPropagation(); onToggle(node.id); }}
        >
          {hasChildren ? (
            node.isExpanded ? <ChevronDown size={18} className="stroke-[3]" /> : <ChevronRight size={18} className="stroke-[3]" />
          ) : null}
        </div>

        <div className="w-28 shrink-0 px-2 flex items-center h-full border-r border-[var(--border-primary)]">
           <span className={cn(
             "font-mono text-[11px] font-black px-2.5 py-1 rounded-md tracking-tighter border-2 shadow-sm",
             node.isCritical ? "bg-rose-500/20 text-rose-900 dark:text-rose-100 border-rose-500/40" : "bg-[var(--background-tertiary)] text-[var(--text-primary)] border-[var(--border-primary)]"
           )}>
             {node.id.split('-').pop()}
           </span>
        </div>

        <div 
          className="flex-1 px-4 flex items-center gap-4 truncate h-full"
          style={{ paddingLeft: `${depth * 0.75 + 0.75}rem` }}
        >
          <StatusIcon status={node.status} size={16} />
          <span className={cn(
            "text-[13px] truncate font-black tracking-tight",
            node.status === 'closed' ? "text-[var(--text-muted)] line-through font-bold" : "text-[var(--text-primary)] group-hover:text-indigo-700 dark:group-hover:text-indigo-300"
          )}>
            {node.title}
          </span>
          {node.issue_type !== 'task' && (
            <span className={cn(
              "text-[8px] font-black px-2 py-0.5 rounded-md uppercase tracking-widest border-2 shadow-sm",
              getChipStyles(node.issue_type)
            )}>
              {node.issue_type}
            </span>
          )}
        </div>
      </div>
    </div>
  );
};

export const WBSTreeList = ({ nodes, depth = 0, onToggle, onClick }: { nodes: WBSNode[], depth?: number, onToggle: (id: string) => void, onClick: (bead: Bead) => void }) => {
  return (
    <>
      {nodes.map(node => (
        <div key={node.id}>
          <WBSTreeItem node={node} depth={depth} onToggle={onToggle} onClick={onClick} />
          {node.isExpanded && node.children.length > 0 && (
            <WBSTreeList nodes={node.children} depth={depth + 1} onToggle={onToggle} onClick={onClick} />
          )}
        </div>
      ))}
    </>
  );
};
