/**
 * Recent + favourite projects persisted to ~/.poe/app-state.json via Rust.
 * Capped at MAX_RECENTS entries, sorted by lastOpenedAt descending.
 */
import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

const MAX_RECENTS = 10;

export interface RecentProject {
  path: string;
  name: string;
  lastOpenedAt: string;
  isFavourite: boolean;
}

interface AppStateData {
  recentProjects: RecentProject[];
}

async function loadFromDisk(): Promise<RecentProject[]> {
  try {
    const data = await invoke<AppStateData>("load_app_state");
    return data.recentProjects ?? [];
  } catch {
    return [];
  }
}

async function saveToDisk(projects: RecentProject[]): Promise<void> {
  try {
    await invoke("save_app_state", { state: { recentProjects: projects } });
  } catch (e) {
    console.error("Failed to save app state:", e);
  }
}

export function useRecentProjects() {
  const [recents, setRecents] = useState<RecentProject[]>([]);

  // Load from ~/.poe/app-state.json on mount
  useEffect(() => {
    loadFromDisk().then(setRecents);
  }, []);

  const recordOpen = useCallback(async (path: string, name: string) => {
    setRecents((prev) => {
      const existing = prev.find((p) => p.path === path);
      const updated: RecentProject = {
        path,
        name,
        lastOpenedAt: new Date().toISOString(),
        isFavourite: existing?.isFavourite ?? false,
      };
      const next = [updated, ...prev.filter((p) => p.path !== path)].slice(0, MAX_RECENTS);
      void saveToDisk(next);
      return next;
    });
  }, []);

  const toggleFavourite = useCallback((path: string) => {
    setRecents((prev) => {
      const next = prev.map((p) =>
        p.path === path ? { ...p, isFavourite: !p.isFavourite } : p
      );
      void saveToDisk(next);
      return next;
    });
  }, []);

  const remove = useCallback((path: string) => {
    setRecents((prev) => {
      const next = prev.filter((p) => p.path !== path);
      void saveToDisk(next);
      return next;
    });
  }, []);

  const favourites = recents.filter((p) => p.isFavourite);
  const recentNonFav = recents.filter((p) => !p.isFavourite);

  return { recents, favourites, recentNonFav, recordOpen, toggleFavourite, remove };
}
