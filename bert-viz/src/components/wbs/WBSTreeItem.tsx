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
    <div className="select-none h-[48px] flex flex-col justify-center border-b border-[var(--border-primary)] relative"
      style={{ backgroundColor: `var(--level-${Math.min(depth, 4)})` }}
    >
      {Array.from({ length: depth }).map((_, i) => (
        <div 
          key={i}
          className="absolute top-0 bottom-0 border-l border-[var(--border-primary)]/30"
          style={{ left: `${i * 1.5 + 0.75}rem` }}
        />
      ))}
      <div 
        className={cn(
          "flex items-center hover:bg-[var(--background-primary)] cursor-pointer group transition-all h-full",
          node.isCritical && "bg-rose-500/[0.08]"
        )}
        style={{ paddingLeft: `${depth * 1.5}rem` }}
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

        <div className="w-24 shrink-0 px-2 flex items-center h-full border-r border-[var(--border-primary)]/50">
           <span className={cn(
             "font-mono text-[10px] font-black px-2 py-0.5 rounded-md tracking-tighter border-2 shadow-sm",
             node.isCritical ? "bg-rose-500/20 text-rose-900 dark:text-rose-100 border-rose-500/40" : "bg-[var(--background-tertiary)] text-[var(--text-primary)] border-[var(--border-primary)]"
           )}>
             {node.id.split('-').pop()}
           </span>
        </div>

        <div 
          className="flex-1 px-4 flex items-center gap-3 truncate h-full"
        >
          <StatusIcon status={node.status} isBlocked={node.isBlocked} size={14} />
          <span className={cn(
            "text-[12px] truncate font-black tracking-tight",
            node.status === 'closed' ? "text-[var(--text-muted)] italic font-bold" : "text-[var(--text-primary)] group-hover:text-indigo-700 dark:group-hover:text-indigo-300"
          )}>
            {node.title}
          </span>
          {node.issue_type !== 'task' && (
            <span className={cn(
              "text-[8px] font-black px-1.5 py-0.5 rounded-md uppercase tracking-widest border border-[var(--border-primary)] shadow-sm",
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
