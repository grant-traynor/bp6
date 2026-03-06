import { useState, useRef, useEffect } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { useSetAtom } from "jotai";
import { ChevronDown, Star, X, FolderOpen } from "lucide-react";
import { projectAtom } from "../store/dag";
import { useRecentProjects, type RecentProject } from "../hooks/useRecentProjects";
import type { ProjectInfo } from "../types";

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
      className="flex items-center gap-2 cursor-pointer"
      style={{
        padding: "7px 12px",
        background: hovered ? "var(--sidebar-hover-bg)" : "transparent",
        transition: "background 0.1s",
      }}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onClick={() => onOpen(project)}
    >
      <div className="flex-1 min-w-0">
        <p style={{ fontSize: 13, fontWeight: 500, color: "var(--text-primary)" }} className="truncate">
          {project.name}
        </p>
        <p className="mono truncate" style={{ color: "var(--text-tertiary)", fontSize: 11 }}>
          {project.path}
        </p>
      </div>
      {hovered && (
        <div className="flex gap-1 shrink-0" onClick={(e) => e.stopPropagation()}>
          <button
            onClick={() => onToggleFavourite(project.path)}
            className="mac-btn mac-btn-ghost"
            style={{ padding: "2px 5px", color: project.isFavourite ? "var(--status-paused)" : "var(--text-tertiary)" }}
            title={project.isFavourite ? "Unpin" : "Pin"}
          >
            <Star size={12} fill={project.isFavourite ? "currentColor" : "none"} />
          </button>
          <button
            onClick={() => onRemove(project.path)}
            className="mac-btn mac-btn-ghost"
            style={{ padding: "2px 5px", color: "var(--text-tertiary)" }}
            title="Remove"
          >
            <X size={12} />
          </button>
        </div>
      )}
    </div>
  );
}

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
    <div
      className="flex flex-col items-center justify-center h-full gap-8"
      style={{ background: "var(--content-secondary-bg)" }}
    >
      {/* Logo */}
      <div className="text-center" style={{ userSelect: "none" }}>
        <h1 style={{ fontSize: 40, fontWeight: 700, letterSpacing: "-0.03em", color: "var(--text-primary)", marginBottom: 6 }}>
          POE
        </h1>
        <p style={{ fontSize: 14, color: "var(--text-secondary)" }}>Project Orchestration Engine</p>
      </div>

      {/* Dropdown button */}
      <div className="relative" ref={dropdownRef}>
        <button
          onClick={handleButtonClick}
          className="mac-btn mac-btn-primary"
          style={{ padding: "8px 20px", fontSize: 14, gap: 8, borderRadius: 8 }}
        >
          <FolderOpen size={15} />
          Open Project
          {hasRecents && <ChevronDown size={13} style={{ opacity: 0.8, transform: dropdownOpen ? "rotate(180deg)" : "none", transition: "transform 0.15s" }} />}
        </button>

        {dropdownOpen && (
          <div
            className="mac-card absolute mt-1 z-50 overflow-hidden"
            style={{ top: "100%", left: 0, right: 0, minWidth: 320, borderRadius: 10 }}
          >
            {favourites.length > 0 && (
              <>
                <p className="mac-section-header" style={{ paddingTop: 8 }}>Pinned</p>
                {favourites.map((p) => (
                  <ProjectRow key={p.path} project={p} onOpen={(proj) => openDir(proj.path)} onToggleFavourite={toggleFavourite} onRemove={remove} />
                ))}
                <div className="mac-divider" />
              </>
            )}

            {recentNonFav.length > 0 && (
              <>
                <p className="mac-section-header" style={{ paddingTop: favourites.length ? 4 : 8 }}>Recent</p>
                {recentNonFav.map((p) => (
                  <ProjectRow key={p.path} project={p} onOpen={(proj) => openDir(proj.path)} onToggleFavourite={toggleFavourite} onRemove={remove} />
                ))}
                <div className="mac-divider" />
              </>
            )}

            <button
              onClick={handleBrowse}
              className="flex items-center gap-2 w-full"
              style={{
                padding: "8px 12px", fontSize: 13,
                color: "var(--accent)", background: "transparent",
                border: "none", cursor: "pointer", fontFamily: "inherit",
              }}
            >
              <FolderOpen size={13} />
              Browse…
            </button>
          </div>
        )}
      </div>

      {error && (
        <p style={{ fontSize: 12, color: "var(--status-failed)", background: "var(--status-failed-bg)", padding: "6px 12px", borderRadius: 6, maxWidth: 360 }}>
          {error}
        </p>
      )}
    </div>
  );
}
