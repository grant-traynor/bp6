import { invoke } from "@tauri-apps/api/core";
import { useAtom, useAtomValue } from "jotai";
import { X } from "lucide-react";
import { projectAtom, nodesListAtom, edgesListAtom } from "../store/dag";

export function ProjectHeader() {
  const [project, setProject] = useAtom(projectAtom);
  const nodes = useAtomValue(nodesListAtom);
  const edges = useAtomValue(edgesListAtom);

  if (!project) return null;

  async function handleClose() {
    try {
      await invoke("close_project");
      setProject(null);
    } catch (err) {
      console.error("Failed to close project:", err);
    }
  }

  return (
    <div className="mac-toolbar flex items-center justify-between px-4 shrink-0" style={{ height: 36, minHeight: 36 }}>
      <div className="flex items-center gap-3">
        <span className="mono" style={{ color: "var(--text-tertiary)", fontSize: 11 }}>
          {nodes.length} nodes · {edges.length} edges
        </span>
      </div>
      <div className="flex items-center gap-2">
        <span className="mono truncate" style={{ color: "var(--text-tertiary)", fontSize: 11, maxWidth: 280 }}>
          {project.projectDir}
        </span>
        <button onClick={handleClose} className="mac-btn mac-btn-ghost" style={{ padding: "2px 6px" }} title="Close project">
          <X size={12} />
        </button>
      </div>
    </div>
  );
}
