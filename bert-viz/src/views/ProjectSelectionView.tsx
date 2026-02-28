import { useState } from "react";
import { type Project } from "../api";

interface ProjectSelectionViewProps {
  projects: Project[];
  favoriteProjects: Project[];
  recentProjects: Project[];
  onOpenProject: (path: string) => void;
  onRemoveProject: (path: string) => void;
  onToggleFavorite: (path: string) => void;
  loading: boolean;
}

export function ProjectSelectionView({
  loading,
  onOpenProject,
}: ProjectSelectionViewProps) {
  const [_projectMenuOpen, _setProjectMenuOpen] = useState(false);

  const handleSelectProject = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select BERT Project Directory",
      });
      if (selected && typeof selected === "string") onOpenProject(selected);
    } catch (error) {
      alert(`Failed to select project: ${error}`);
    }
  };

  return (
    <div className="flex-1 flex items-center justify-center">
      {loading ? (
        <div className="text-center">
          <div className="inline-block h-12 w-12 animate-spin rounded-full border-4 border-solid border-indigo-600 border-r-transparent mb-4"></div>
          <p className="text-lg text-[var(--text-muted)] font-medium">
            Loading project...
          </p>
        </div>
      ) : (
        <div className="text-center max-w-md px-8">
          <h1 className="text-4xl font-black text-[var(--text-primary)] mb-4 tracking-tight">
            Welcome to BERT
          </h1>
          <p className="text-lg text-[var(--text-muted)] mb-8 font-medium">
            Get started by loading a project directory with a .beads folder
          </p>
          <button
            onClick={handleSelectProject}
            className="px-8 py-4 bg-indigo-600 hover:bg-indigo-700 text-white font-black rounded-xl shadow-lg hover:shadow-xl transition-all active:scale-95 text-lg uppercase tracking-wider"
          >
            Load Project
          </button>
        </div>
      )}
    </div>
  );
}
