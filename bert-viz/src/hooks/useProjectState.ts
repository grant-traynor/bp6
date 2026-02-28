import { useState, useCallback, useMemo, startTransition } from "react";
import {
  fetchProjects,
  openProject,
  removeProject,
  toggleFavoriteProject,
  killProjectShell,
  type Project,
} from "../api";

const PROJECT_SHELL_ID = "project-shell";

export interface UseProjectStateReturn {
  currentProjectPath: string;
  setCurrentProjectPath: React.Dispatch<React.SetStateAction<string>>;
  hasProject: boolean;
  setHasProject: React.Dispatch<React.SetStateAction<boolean>>;
  projects: Project[];
  setProjects: React.Dispatch<React.SetStateAction<Project[]>>;
  projectMenuOpen: boolean;
  setProjectMenuOpen: React.Dispatch<React.SetStateAction<boolean>>;
  showPalettePreview: boolean;
  setShowPalettePreview: React.Dispatch<React.SetStateAction<boolean>>;
  favoriteProjects: Project[];
  recentProjects: Project[];
  handleOpenProject: (path: string) => Promise<void>;
  handleRemoveProject: (path: string) => Promise<void>;
  handleToggleFavoriteProject: (path: string) => Promise<void>;
  handleSelectProject: () => Promise<void>;
  setLoading: React.Dispatch<React.SetStateAction<boolean>>;
  setProjectShellKey: React.Dispatch<React.SetStateAction<number>>;
}

export function useProjectState(
  loadData: () => void,
  setLoadingExternal: React.Dispatch<React.SetStateAction<boolean>>,
  setProjectShellKeyExternal: React.Dispatch<React.SetStateAction<number>>
): UseProjectStateReturn {
  const [currentProjectPath, setCurrentProjectPath] = useState<string>("");
  const [hasProject, setHasProject] = useState(false);
  const [projects, setProjects] = useState<Project[]>([]);
  const [projectMenuOpen, setProjectMenuOpen] = useState(false);
  const [showPalettePreview, setShowPalettePreview] = useState(false);

  const loadProjects = useCallback(async () => {
    const data = await fetchProjects();
    setProjects(data);
  }, []);

  const handleOpenProject = useCallback(
    async (path: string) => {
      try {
        setLoadingExternal(true);
        await openProject(path);

        // Kill any existing project shell so Terminal spawns a fresh one
        try {
          await killProjectShell(PROJECT_SHELL_ID);
        } catch (_) {
          /* ignore */
        }
        setProjectShellKeyExternal((k) => k + 1);

        setCurrentProjectPath(path);
        setHasProject(true);

        startTransition(() => {
          // Trigger refetch via loadData
          loadData();
          loadProjects().then(() => {
            setLoadingExternal(false);
          });
        });
      } catch (error) {
        alert(`Failed to open project: ${error}`);
        setLoadingExternal(false);
      }
    },
    [loadData, loadProjects, setLoadingExternal, setProjectShellKeyExternal]
  );

  const handleRemoveProject = useCallback(
    async (path: string) => {
      try {
        await removeProject(path);
        await loadProjects();
      } catch (error) {
        alert(`Failed to remove project: ${error}`);
      }
    },
    [loadProjects]
  );

  const handleToggleFavoriteProject = useCallback(
    async (path: string) => {
      try {
        await toggleFavoriteProject(path);
        await loadProjects();
      } catch (error) {
        alert(`Failed to toggle favorite: ${error}`);
      }
    },
    [loadProjects]
  );

  const handleSelectProject = useCallback(async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({
        directory: true,
        multiple: false,
        title: "Select BERT Project Directory",
      });
      if (selected && typeof selected === "string") {
        await handleOpenProject(selected);
      }
    } catch (error) {
      alert(`Failed to select project: ${error}`);
    }
  }, [handleOpenProject]);

  const favoriteProjects = useMemo(
    () => projects.filter((p) => p.is_favorite),
    [projects]
  );

  const recentProjects = useMemo(
    () =>
      projects
        .filter((p) => !p.is_favorite)
        .sort((a, b) =>
          (b.last_opened || "").localeCompare(a.last_opened || "")
        ),
    [projects]
  );

  return {
    currentProjectPath,
    setCurrentProjectPath,
    hasProject,
    setHasProject,
    projects,
    setProjects,
    projectMenuOpen,
    setProjectMenuOpen,
    showPalettePreview,
    setShowPalettePreview,
    favoriteProjects,
    recentProjects,
    handleOpenProject,
    handleRemoveProject,
    handleToggleFavoriteProject,
    handleSelectProject,
    setLoading: setLoadingExternal,
    setProjectShellKey: setProjectShellKeyExternal,
  };
}
