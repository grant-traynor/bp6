import type { BeadNode, GanttItem } from "../../api";
import { GanttBar } from "./GanttBar";

const ROW_HEIGHT = 32;

interface GanttBarsProps {
  items: GanttItem[];
  selectedBead: BeadNode | null;
  onBeadClick: (bead: BeadNode) => void;
  onOpenChat: (persona: string, task?: string, beadId?: string, role?: string) => void;
  sessionsByBead: Record<string, string[]>;
  chainIds?: Set<string>;
}

export function GanttBars({ items, selectedBead, onBeadClick, chainIds }: GanttBarsProps) {
  const hasChain = chainIds && chainIds.size > 0;

  return (
    <>
      {items.map((item) => (
        <div
          key={item.bead.id}
          style={{
            position: 'absolute',
            top: item.row * ROW_HEIGHT,
            height: ROW_HEIGHT,
            left: 0,
            right: 0,
            opacity: hasChain && !chainIds!.has(item.bead.id) ? 0.55 : 1,
            transition: 'opacity 0.15s ease',
          }}
        >
          <GanttBar
            item={item}
            onClick={onBeadClick}
            isSelected={selectedBead?.id === item.bead.id}
            isInChain={hasChain ? chainIds!.has(item.bead.id) : false}
          />
        </div>
      ))}
    </>
  );
}
