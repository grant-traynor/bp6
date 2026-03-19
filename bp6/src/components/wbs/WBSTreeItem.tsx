import { memo } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { cn } from "../../utils";
import type { BeadNode, SessionInfo } from "../../api";
import { getChipStyles } from "../shared/Chip";
import { SessionIndicator } from "../shared/SessionIndicator";

interface WBSTreeItemProps {
  node: BeadNode;
  depth?: number;
  onToggle: (id: string) => void;
  onClick: (bead: BeadNode) => void;
  isSelected?: boolean;
  sessions?: SessionInfo[];
  onStartSession?: (beadId: string, persona: string) => void;
  onSessionClick?: (sessionId: string) => void;
}

export const WBSTreeItem = memo(function WBSTreeItem({
  node,
  depth = 0,
  onToggle,
  onClick,
  isSelected,
  sessions = [],
  onStartSession,
  onSessionClick
}: WBSTreeItemProps) {
  const hasChildren = node.children.length > 0;

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    if (!onStartSession) return;

    // Show browser-native context menu with custom options
    const persona = window.prompt(
      'Start Agent Session\n\nChoose persona:\n1. product-manager\n2. qa-engineer\n3. specialist\n\nEnter 1, 2, or 3:',
      '1'
    );

    const personaMap: Record<string, string> = {
      '1': 'product-manager',
      '2': 'qa-engineer',
      '3': 'specialist'
    };

    const selectedPersona = personaMap[persona || '1'] || 'product-manager';
    onStartSession(node.id, selectedPersona);
  };

  return (
    <div
      data-bead-id={node.id}
      data-testid={`bead-item-${node.id}`}
      className={cn(
        "h-[32px] flex flex-col justify-center border-b border-[var(--border-primary)]/50 relative transition-colors duration-200",
        isSelected ? "bg-[var(--accent-primary)]/10" : ""
      )}
      style={{ backgroundColor: !isSelected ? `var(--level-${Math.min(depth, 4)})` : undefined }}
    >
      {Array.from({ length: depth }).map((_, i) => (
        <div
          key={i}
          className="absolute top-0 bottom-0 border-l border-transparent"
          style={{ left: `${i * 1.0 + 0.5}rem` }}
        />
      ))}
      <div
        className={cn(
          "flex items-center hover:bg-[var(--background-primary)] cursor-pointer group transition-all h-full",
          node.isCritical && !isSelected && "bg-[var(--status-blocked)]/[0.08]",
          isSelected && "bg-[var(--accent-primary)]/5 shadow-[inset_3px_0_0_0_var(--accent-primary)]"
        )}
        style={{ paddingLeft: `${depth * 1.0}rem` }}
        onClick={() => onClick(node)}
        onContextMenu={handleContextMenu}
      >
        {/* Chevron toggle */}
        <div
          className="w-8 shrink-0 flex items-center justify-center h-full text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors"
          onClick={(e) => { e.stopPropagation(); onToggle(node.id); }}
        >
          {hasChildren ? (
            node.isExpanded ? <ChevronDown size={14} className="stroke-[3]" /> : <ChevronRight size={14} className="stroke-[3]" />
          ) : null}
        </div>

        {/* ID */}
        <div className="w-20 shrink-0 px-1.5 flex items-center h-full">
          <span className={cn(
            "font-mono text-[11px] font-black px-1.5 py-px rounded tracking-tighter border-2 shadow-sm transition-all",
            isSelected ? "bg-[var(--accent-primary)] text-white border-[var(--accent-secondary)] shadow-md scale-105" :
            node.isCritical ? "bg-[var(--status-blocked)]/20 text-[var(--status-blocked)] border-[var(--status-blocked)]/40" : "bg-[var(--background-tertiary)] text-[var(--text-primary)] border-[var(--border-primary)]"
          )}>
            {node.id.split('-').pop()}
          </span>
        </div>

        {/* Name */}
        <div className="flex-1 px-2 flex items-center truncate h-full">
          <span className={cn(
            "text-[13px] truncate font-black tracking-tight transition-colors",
            node.status === 'closed' && "italic font-bold opacity-60",
            isSelected && node.status === 'closed' ? "text-[var(--accent-primary)]" :
            isSelected ? "text-[var(--accent-primary)]" :
            node.status === 'closed' ? "text-[var(--text-muted)]" : "text-[var(--text-primary)] group-hover:text-[var(--accent-primary)]"
          )}>
            {node.title}
          </span>
        </div>

        {/* Agent session icons */}
        <div className="w-16 shrink-0 flex items-center justify-end pr-1 h-full">
          {sessions && sessions.length > 0 && (
            <SessionIndicator sessions={sessions} onSessionClick={onSessionClick} />
          )}
        </div>

        {/* Assignee */}
        <div className="w-24 shrink-0 px-1.5 flex items-center h-full">
          {node.assignee ? (
            <span className="text-[11px] font-bold text-[var(--text-muted)] truncate" title={node.assignee}>
              {node.assignee}
            </span>
          ) : (
            <span className="text-[11px] text-[var(--text-muted)]/30">—</span>
          )}
        </div>

        {/* Type */}
        <div className="w-16 shrink-0 px-1 flex items-center justify-center h-full">
          <span className={cn(
            "text-[10px] font-black px-1 py-px rounded uppercase tracking-widest border border-[var(--border-primary)] shadow-sm transition-all",
            isSelected ? "bg-[var(--accent-primary)]/20 border-[var(--accent-primary)]/40 text-[var(--accent-primary)] scale-95" : getChipStyles(node.issueType)
          )}>
            {node.issueType}
          </span>
        </div>

        {/* Priority */}
        <div className="w-12 shrink-0 px-1 flex items-center justify-center h-full">
          <span className={cn(
            "shrink-0 font-mono text-[11px] font-black px-1 py-px rounded border shadow-sm transition-colors",
            isSelected ? "bg-[var(--accent-primary)]/20 text-[var(--accent-primary)] border-[var(--accent-primary)]/40" :
            node.priority <= 1 ? "bg-[var(--status-blocked)]/20 text-[var(--status-blocked)] border-[var(--status-blocked)]/40" :
            node.priority === 2 ? "bg-[var(--status-active)]/20 text-[var(--status-active)] border-[var(--status-active)]/40" :
            "bg-[var(--background-tertiary)] text-[var(--text-muted)] border-[var(--border-primary)]"
          )}>
            P{node.priority}
          </span>
        </div>
      </div>
    </div>
  );
});

export const WBSTreeList = memo(function WBSTreeList({
  nodes,
  depth = 0,
  onToggle,
  onClick,
  selectedId,
  sessionsByBead = {},
  onStartSession,
  onSessionClick
}: {
  nodes: BeadNode[],
  depth?: number,
  onToggle: (id: string) => void,
  onClick: (bead: BeadNode) => void,
  selectedId?: string,
  sessionsByBead?: Record<string, SessionInfo[]>,
  onStartSession?: (beadId: string, persona: string) => void,
  onSessionClick?: (sessionId: string) => void,
}) {
  return (
    <>
      {nodes.map(node => (
        <div key={node.id}>
          <WBSTreeItem
            node={node}
            depth={depth}
            onToggle={onToggle}
            onClick={onClick}
            isSelected={selectedId === node.id}
            sessions={sessionsByBead[node.id]}
            onStartSession={onStartSession}
            onSessionClick={onSessionClick}
          />
          {node.isExpanded && node.children.length > 0 && (
            <WBSTreeList
              nodes={node.children}
              depth={depth + 1}
              onToggle={onToggle}
              onClick={onClick}
              selectedId={selectedId}
              sessionsByBead={sessionsByBead}
              onStartSession={onStartSession}
              onSessionClick={onSessionClick}
            />
          )}
        </div>
      ))}
    </>
  );
});
