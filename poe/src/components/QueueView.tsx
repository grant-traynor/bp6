import { useAtomValue } from "jotai";
import { nodesListAtom, edgesListAtom, projectAtom } from "../store/dag";

export function QueueView() {
  const project = useAtomValue(projectAtom);
  const nodes = useAtomValue(nodesListAtom);
  const edges = useAtomValue(edgesListAtom);

  const tasks = nodes.filter((n) => n.nodeType === "Task");

  return (
    <div className="flex-1 overflow-auto p-6">
      <div className="max-w-3xl mx-auto space-y-6">
        <div className="flex items-center justify-between">
          <h2 className="font-black text-xl tracking-tight border-b-4 border-black pb-1">
            Task Queue
          </h2>
          <span className="font-mono text-xs text-stone-400">
            project: {project?.projectId.slice(0, 8)}
          </span>
        </div>

        {tasks.length === 0 ? (
          <div className="border-4 border-dashed border-stone-300 p-12 text-center">
            <p className="font-mono text-stone-400 text-sm">No tasks yet.</p>
          </div>
        ) : (
          <ul className="space-y-2">
            {tasks.map((node) => (
              <li
                key={node.id}
                className="border-4 border-black p-4 flex items-center justify-between"
              >
                <div>
                  <p className="font-bold">
                    {String(node.data.title ?? node.id)}
                  </p>
                  <p className="font-mono text-xs text-stone-400">
                    {node.nodeType} · {node.id.slice(0, 8)}
                  </p>
                </div>
                <span className="font-mono text-xs border border-stone-300 px-2 py-1">
                  {String(node.data.status ?? "open")}
                </span>
              </li>
            ))}
          </ul>
        )}

        {/* DAG summary */}
        <details className="border-2 border-stone-200">
          <summary className="font-mono text-xs text-stone-400 px-4 py-2 cursor-pointer select-none">
            DAG summary ({nodes.length} nodes, {edges.length} edges)
          </summary>
          <pre className="font-mono text-xs p-4 overflow-auto max-h-64 text-stone-600">
            {JSON.stringify({ nodes: nodes.slice(0, 5), edges: edges.slice(0, 5) }, null, 2)}
          </pre>
        </details>
      </div>
    </div>
  );
}
