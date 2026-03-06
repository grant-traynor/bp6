import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { useSetAtom } from "jotai";
import { projectAtom } from "../store/dag";
import type { ProjectInfo } from "../types";

export function EmptyState() {
  const setProject = useSetAtom(projectAtom);

  async function handleOpenProject() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Open Project Directory",
    });

    if (!selected || typeof selected !== "string") return;

    try {
      const info = await invoke<ProjectInfo>("open_project", { dir: selected });
      setProject(info);
    } catch (err) {
      console.error("Failed to open project:", err);
      alert(`Failed to open project: ${err}`);
    }
  }

  return (
    <div className="flex flex-col items-center justify-center h-full gap-8 bg-stone-50">
      <div className="text-center space-y-2">
        <h1 className="text-4xl font-black tracking-tight border-b-4 border-black pb-2">
          POE
        </h1>
        <p className="text-stone-500 font-mono text-sm">
          Project Orchestration Engine
        </p>
      </div>

      <button
        onClick={handleOpenProject}
        className="border-4 border-black bg-black text-white font-mono font-bold px-8 py-4 text-sm uppercase tracking-widest hover:bg-white hover:text-black transition-colors"
      >
        Open Project
      </button>
    </div>
  );
}
