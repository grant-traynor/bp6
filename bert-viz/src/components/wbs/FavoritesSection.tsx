import { Star } from "lucide-react";
import type { BeadNode, Project } from "../../api";

interface FavoritesSectionProps {
  favoriteBeads: BeadNode[];
  favoriteProjects: Project[];
  onBeadClick: (bead: BeadNode) => void;
  onOpenChat: (persona: string, task?: string, beadId?: string, role?: string) => void;
  onOpenProject: (path: string) => void;
  sessionsByBead: Record<string, string[]>;
}

export function FavoritesSection({
  favoriteBeads,
  onBeadClick,
}: FavoritesSectionProps) {
  if (favoriteBeads.length === 0) {
    return null;
  }

  return (
    <div className="px-6 py-4 border-b-2 border-[var(--border-primary)]/50 bg-indigo-500/10">
      <h2 className="text-xs font-black text-indigo-800 dark:text-indigo-300 uppercase tracking-[0.2em] flex items-center gap-2 mb-3">
        <Star size={12} className="fill-current" /> Favorites
      </h2>
      <div className="flex flex-wrap gap-2">
        {favoriteBeads.map(f => (
          <div
            key={f.id}
            onClick={() => onBeadClick(f)}
            className="flex items-center gap-2 px-2.5 py-1.5 rounded-xl bg-[var(--background-primary)] border-[var(--border-thick)] border-[var(--border-primary)] hover:border-indigo-500 shadow-[var(--shadow-sm)] cursor-pointer transition-all active:scale-95 hover-lift"
          >
            <span className="font-mono text-xs font-black text-indigo-700 dark:text-indigo-400">{f.id}</span>
            <span className="text-sm font-black text-[var(--text-primary)] truncate max-w-[140px]">{f.title}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
