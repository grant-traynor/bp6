import { useState } from "react";
import { useAtomValue } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import { queueItemsListAtom, projectAtom } from "../store/dag";
import type { QueueItem } from "../types";

function QueueItemCard({ item }: { item: QueueItem }) {
  const [resolving, setResolving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleResolve = async (optionId: string) => {
    setResolving(true);
    setError(null);
    try {
      await invoke("resolve_queue_item", {
        params: { itemId: item.id, chosenOptionId: optionId },
      });
    } catch (err) {
      setError(err as string);
      setResolving(false);
    }
  };

  const priorityLabel = ["P0", "P1", "P2", "P3"][item.priority] ?? `P${item.priority}`;

  return (
    <li className="border-4 border-black p-5 space-y-4">
      <div className="flex items-start justify-between gap-4">
        <div className="space-y-1">
          <p className="font-black text-base leading-snug">{item.question}</p>
          <p className="font-mono text-xs text-stone-400">
            {item.agentId} · {item.id.slice(0, 8)} · {priorityLabel}
          </p>
        </div>
        <span className="font-mono text-xs border-2 border-black px-2 py-0.5 whitespace-nowrap">
          pending
        </span>
      </div>

      <ul className="grid gap-2">
        {item.options.map((opt) => (
          <li key={opt.id}>
            <button
              onClick={() => handleResolve(opt.id)}
              disabled={resolving}
              className="w-full text-left border-2 border-black px-4 py-2 font-mono text-sm hover:bg-black hover:text-white transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
            >
              <span className="font-bold">{opt.label}</span>
              {opt.description && (
                <span className="block text-xs text-stone-500 mt-0.5">{opt.description}</span>
              )}
            </button>
          </li>
        ))}
      </ul>

      {error && (
        <p className="font-mono text-xs text-red-600 border-2 border-red-600 px-3 py-1">
          {error}
        </p>
      )}

      {Object.keys(item.contextSnapshot).length > 0 && (
        <details className="border border-stone-200">
          <summary className="font-mono text-xs text-stone-400 px-3 py-1.5 cursor-pointer select-none">
            context snapshot
          </summary>
          <pre className="font-mono text-xs p-3 overflow-auto max-h-48 text-stone-600">
            {JSON.stringify(item.contextSnapshot, null, 2)}
          </pre>
        </details>
      )}
    </li>
  );
}

export function QueueView() {
  const project = useAtomValue(projectAtom);
  const items = useAtomValue(queueItemsListAtom);

  return (
    <div className="flex-1 overflow-auto p-6">
      <div className="max-w-3xl mx-auto space-y-6">
        <div className="flex items-center justify-between">
          <h2 className="font-black text-xl tracking-tight border-b-4 border-black pb-1">
            Decision Queue
          </h2>
          <span className="font-mono text-xs text-stone-400">
            {items.length} pending · {project?.projectId.slice(0, 8)}
          </span>
        </div>

        {items.length === 0 ? (
          <div className="border-4 border-dashed border-stone-300 p-12 text-center">
            <p className="font-mono text-stone-400 text-sm">
              No decisions pending — agents are running.
            </p>
          </div>
        ) : (
          <ul className="space-y-4">
            {items.map((item) => (
              <QueueItemCard key={item.id} item={item} />
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
