import { ChevronDown, ChevronRight } from "lucide-react";
import { cn } from "../../utils";
import type { BeadNode } from "../../api";
import { getChipStyles } from "../shared/Chip";

interface WBSTreeItemProps {
  node: BeadNode;
  depth?: number;
  onToggle: (id: string) => void;
  onClick: (bead: BeadNode) => void;
  isSelected?: boolean;
}

export const WBSTreeItem = ({ 
  node, 
  depth = 0, 
  onToggle,
  onClick,
  isSelected
}: WBSTreeItemProps) => {
  const hasChildren = node.children.length > 0;

  return (
    <div
      data-bead-id={node.id}
      className={cn(
        "select-none h-[48px] flex flex-col justify-center border-b border-[var(--border-primary)]/50 relative transition-colors duration-200",
        isSelected ? "bg-[var(--accent-primary)]/10" : ""
      )}
      style={{ backgroundColor: !isSelected ? `var(--level-${Math.min(depth, 4)})` : undefined }}
    >
      {Array.from({ length: depth }).map((_, i) => (
        <div 
          key={i}
          className="absolute top-0 bottom-0 border-l border-transparent"
          style={{ left: `${i * 1.5 + 0.75}rem` }}
        />
      ))}
      <div 
        className={cn(
          "flex items-center hover:bg-[var(--background-primary)] cursor-pointer group transition-all h-full",
          node.isCritical && !isSelected && "bg-[var(--status-blocked)]/[0.08]",
          isSelected && "bg-[var(--accent-primary)]/5 shadow-[inset_4px_0_0_0_var(--accent-primary)]"
        )}
        style={{ paddingLeft: `${depth * 1.5}rem` }}
        onClick={() => onClick(node)}
      >
        <div
          className="w-10 shrink-0 flex items-center justify-center h-full text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors"
          onClick={(e) => { e.stopPropagation(); onToggle(node.id); }}
        >
          {hasChildren ? (
            node.isExpanded ? <ChevronDown size={18} className="stroke-[3]" /> : <ChevronRight size={18} className="stroke-[3]" />
          ) : null}
        </div>

        <div className="w-16 shrink-0 px-2 flex items-center justify-center h-full">
          <span className={cn(
            "shrink-0 font-mono text-xs font-black px-1.5 py-0.5 rounded border shadow-sm transition-colors",
            isSelected ? "bg-[var(--accent-primary)]/20 text-[var(--accent-primary)] border-[var(--accent-primary)]/40" :
            node.priority <= 1 ? "bg-[var(--status-blocked)]/20 text-[var(--status-blocked)] border-[var(--status-blocked)]/40" :
            node.priority === 2 ? "bg-[var(--status-active)]/20 text-[var(--status-active)] border-[var(--status-active)]/40" :
            "bg-[var(--background-tertiary)] text-[var(--text-muted)] border-[var(--border-primary)]"
          )}>
            P{node.priority}
          </span>
        </div>

        <div
          className="flex-1 px-4 flex items-center truncate h-full"
        >
          <span className={cn(
            "text-sm truncate font-black tracking-tight transition-colors",
            isSelected ? "text-[var(--accent-primary)]" :
            node.status === 'closed' ? "text-[var(--text-muted)] italic font-bold" : "text-[var(--text-primary)] group-hover:text-[var(--accent-primary)]"
          )}>
            {node.title}
          </span>
        </div>

        <div className="w-20 shrink-0 px-2 flex items-center justify-center h-full">
          <span className={cn(
            "text-xs font-black px-1.5 py-0.5 rounded-md uppercase tracking-widest border border-[var(--border-primary)] shadow-sm transition-all",
            isSelected ? "bg-[var(--accent-primary)]/20 border-[var(--accent-primary)]/40 text-[var(--accent-primary)] scale-95" : getChipStyles(node.issueType)
          )}>
            {node.issueType}
          </span>
        </div>

        <div className="w-24 shrink-0 px-2 flex items-center h-full">
          <span className={cn(
            "font-mono text-xs font-black px-2 py-0.5 rounded-md tracking-tighter border-2 shadow-sm transition-all",
            isSelected ? "bg-[var(--accent-primary)] text-white border-[var(--accent-secondary)] shadow-md scale-105" :
            node.isCritical ? "bg-[var(--status-blocked)]/20 text-[var(--status-blocked)] border-[var(--status-blocked)]/40" : "bg-[var(--background-tertiary)] text-[var(--text-primary)] border-[var(--border-primary)]"
          )}>
            {node.id.split('-').pop()}
          </span>
        </div>
      </div>
    </div>
  );
};

export const WBSTreeList = ({ nodes, depth = 0, onToggle, onClick, selectedId }: { nodes: BeadNode[], depth?: number, onToggle: (id: string) => void, onClick: (bead: BeadNode) => void, selectedId?: string }) => {
  return (
    <>
      {nodes.map(node => (
        <div key={node.id}>
          <WBSTreeItem node={node} depth={depth} onToggle={onToggle} onClick={onClick} isSelected={selectedId === node.id} />
          {node.isExpanded && node.children.length > 0 && (
            <WBSTreeList nodes={node.children} depth={depth + 1} onToggle={onToggle} onClick={onClick} selectedId={selectedId} />
          )}
        </div>
      ))}
    </>
  );
};
