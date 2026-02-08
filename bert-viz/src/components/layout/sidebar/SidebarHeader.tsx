import { X, Star } from "lucide-react";
import { cn } from "../../../utils";
import { StatusIcon } from "../../shared/StatusIcon";
import type { Bead } from "../../../api";

interface SidebarHeaderProps {
  bead: Bead;
  onClose: () => void;
  onToggleFavorite?: (bead: Bead) => void;
}

export const SidebarHeader = ({ bead, onClose, onToggleFavorite }: SidebarHeaderProps) => {
  return (
    <div className="flex items-center justify-between p-6 pb-2">
      <div className="flex items-center gap-4">
        <div className="flex flex-col">
          <div className="flex items-center gap-2">
            <span className="text-[10px] font-black font-mono text-indigo-700 dark:text-indigo-400 bg-indigo-500/10 px-2.5 py-1 rounded-lg border-2 border-indigo-500/20 shadow-sm uppercase tracking-tighter">
              {bead.id}
            </span>
            <div className="flex items-center gap-1.5 px-3 py-1 bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-full shadow-sm hover:shadow-md transition-all cursor-default group">
              <StatusIcon status={bead.status} className="w-3 h-3 group-hover:scale-110 transition-transform" />
              <span className="text-[10px] uppercase font-black tracking-widest text-[var(--text-primary)]">
                {bead.status.replace('_', ' ')}
              </span>
            </div>
          </div>
        </div>
      </div>
      <div className="flex items-center gap-2">
        <button 
          onClick={() => onToggleFavorite?.(bead)}
          className={cn(
            "p-2.5 rounded-xl transition-all border-2 border-transparent active:scale-95",
            bead.is_favorite 
              ? "text-amber-500 bg-amber-500/10 border-amber-500/20" 
              : "text-[var(--text-muted)] hover:text-amber-500 hover:bg-amber-500/10 hover:border-amber-500/20"
          )}
        >
          <Star size={18} className={bead.is_favorite ? "fill-current" : ""} />
        </button>
        <button 
          onClick={onClose}
          className="text-[var(--text-muted)] hover:text-rose-500 p-2.5 rounded-xl hover:bg-rose-500/10 transition-all border-2 border-transparent hover:border-rose-500/20 active:scale-95"
        >
          <X size={18} strokeWidth={3} />
        </button>
      </div>
    </div>
  );
};
