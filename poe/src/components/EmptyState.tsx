import { useState, useRef, useEffect } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { useSetAtom } from "jotai";
import { projectAtom } from "../store/dag";
import { useRecentProjects, type RecentProject } from "../hooks/useRecentProjects";
import type { ProjectInfo } from "../types";

// ── Project row in dropdown ────────────────────────────────────────────────────

function ProjectRow({
  project,
  onOpen,
  onToggleFavourite,
  onRemove,
}: {
  project: RecentProject;
  onOpen: (p: RecentProject) => void;
  onToggleFavourite: (path: string) => void;
  onRemove: (path: string) => void;
}) {
  const [hovered, setHovered] = useState(false);

  return (
    <div
      className="flex items-center gap-2 px-3 py-2 hover:bg-stone-100 cursor-pointer"
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onClick={() => onOpen(project)}
    >
      <div className="flex-1 min-w-0">
        <p className="font-mono text-sm font-bold truncate">{project.name}</p>
        <p className="font-mono text-xs text-stone-400 truncate">{project.path}</p>
      </div>
      {hovered && (
        <div className="flex gap-1 shrink-0" onClick={(e) => e.stopPropagation()}>
          <button
            onClick={() => onToggleFavourite(project.path)}
            className={`text-xs px-1.5 py-0.5 border border-stone-300 hover:border-black transition-colors ${
              project.isFavourite ? "text-amber-500" : "text-stone-400"
            }`}
            title={project.isFavourite ? "Unpin" : "Pin"}
          >
            ★
          </button>
          <button
            onClick={() => onRemove(project.path)}
            className="text-xs px-1.5 py-0.5 border border-stone-300 text-stone-400 hover:border-red-400 hover:text-red-500 transition-colors"
            title="Remove"
          >
            ✕
          </button>
        </div>
      )}
    </div>
  );
}

// ── Main component ─────────────────────────────────────────────────────────────

export function EmptyState() {
  const setProject = useSetAtom(projectAtom);
  const { favourites, recentNonFav, recordOpen, toggleFavourite, remove } = useRecentProjects();
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const hasRecents = favourites.length > 0 || recentNonFav.length > 0;

  useEffect(() => {
    if (!dropdownOpen) return;
    function handler(e: MouseEvent) {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setDropdownOpen(false);
      }
    }
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [dropdownOpen]);

  async function openDir(dir: string) {
    setDropdownOpen(false);
    setError(null);
    try {
      const info = await invoke<ProjectInfo>("open_project", { dir });
      void recordOpen(info.projectDir, info.name);
      setProject(info);
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleBrowse() {
    setDropdownOpen(false);
    const selected = await openDialog({ directory: true, multiple: false, title: "Open Project Directory" });
    if (!selected || typeof selected !== "string") return;
    await openDir(selected);
  }

  function handleButtonClick() {
    if (hasRecents) {
      setDropdownOpen((v) => !v);
    } else {
      void handleBrowse();
    }
  }

  return (
    <div className="flex flex-col items-center justify-center h-full gap-8 bg-stone-50">
      <div className="text-center space-y-2">
        <h1 className="text-4xl font-black tracking-tight border-b-4 border-black pb-2">POE</h1>
        <p className="text-stone-500 font-mono text-sm">Project Orchestration Engine</p>
      </div>

      <div className="relative" ref={dropdownRef}>
        <button
          onClick={handleButtonClick}
          className="border-4 border-black bg-black text-white font-mono font-bold px-8 py-4 text-sm uppercase tracking-widest hover:bg-white hover:text-black transition-colors flex items-center gap-3"
        >
          Open Project
          {hasRecents && (
            <span className="text-xs opacity-60">{dropdownOpen ? "▲" : "▼"}</span>
          )}
        </button>

        {dropdownOpen && (
          <div className="absolute top-full left-0 right-0 mt-1 bg-white border-4 border-black z-50 min-w-72 shadow-lg">
            {favourites.length > 0 && (
              <>
                <div className="px-3 py-1.5 border-b-2 border-stone-200 bg-stone-50">
                  <span className="font-mono text-xs font-bold text-stone-400 uppercase tracking-wider">★ Pinned</span>
                </div>
                {favourites.map((p) => (
                  <ProjectRow key={p.path} project={p}
                    onOpen={(proj) => openDir(proj.path)}
                    onToggleFavourite={toggleFavourite}
                    onRemove={remove}
                  />
                ))}
              </>
            )}

            {recentNonFav.length > 0 && (
              <>
                <div className="px-3 py-1.5 border-b-2 border-stone-200 bg-stone-50">
                  <span className="font-mono text-xs font-bold text-stone-400 uppercase tracking-wider">Recent</span>
                </div>
                {recentNonFav.map((p) => (
                  <ProjectRow key={p.path} project={p}
                    onOpen={(proj) => openDir(proj.path)}
                    onToggleFavourite={toggleFavourite}
                    onRemove={remove}
                  />
                ))}
              </>
            )}

            <div className="border-t-2 border-stone-200">
              <button
                onClick={handleBrowse}
                className="w-full px-3 py-2.5 text-left font-mono text-sm hover:bg-stone-100 flex items-center gap-2 transition-colors"
              >
                <span className="text-stone-400">📁</span>
                <span>Browse…</span>
              </button>
            </div>
          </div>
        )}
      </div>

      {error && (
        <p className="font-mono text-xs text-red-600 border-2 border-red-400 px-3 py-2 max-w-sm text-center">
          {error}
        </p>
      )}
    </div>
  );
}
