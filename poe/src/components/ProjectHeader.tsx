import { invoke } from "@tauri-apps/api/core";
import { useAtom, useAtomValue } from "jotai";
import { projectAtom, nodesListAtom, edgesListAtom } from "../store/dag";

export function ProjectHeader() {
  const [project, setProject] = useAtom(projectAtom);
  const nodes = useAtomValue(nodesListAtom);
  const edges = useAtomValue(edgesListAtom);

  async function handleClose() {
    try {
      await invoke("close_project");
      setProject(null);
    } catch (err) {
      console.error("Failed to close project:", err);
    }
  }

  if (!project) return null;

  return (
    <header className="border-b-4 border-black bg-white flex items-center justify-between px-4 py-2">
      <div className="flex items-center gap-4">
        <span className="font-black text-lg tracking-tight">{project.name}</span>
        <span className="font-mono text-xs text-stone-400 border border-stone-300 px-1">
          {nodes.length} nodes · {edges.length} edges
        </span>
      </div>

      <div className="flex items-center gap-2">
        <span className="font-mono text-xs text-stone-400 truncate max-w-xs">
          {project.projectDir}
        </span>
        <button
          onClick={handleClose}
          className="border-2 border-black font-mono text-xs px-2 py-1 hover:bg-black hover:text-white transition-colors"
        >
          Close
        </button>
      </div>
    </header>
  );
}
