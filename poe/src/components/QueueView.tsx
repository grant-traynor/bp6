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
  const priorityColor = item.priority === 0 ? "var(--status-failed)" : item.priority === 1 ? "var(--status-paused)" : "var(--text-tertiary)";

  return (
    <div className="mac-card" style={{ padding: "16px 18px" }}>
      <div className="flex items-start justify-between gap-4" style={{ marginBottom: 12 }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <p style={{ fontSize: 13, fontWeight: 500, color: "var(--text-primary)", lineHeight: 1.4 }}>
            {item.question}
          </p>
          <p className="mono" style={{ color: "var(--text-tertiary)", fontSize: 11, marginTop: 4 }}>
            {item.agentId} · {item.id.slice(0, 8)}
          </p>
        </div>
        <span style={{ fontSize: 11, fontWeight: 600, color: priorityColor, whiteSpace: "nowrap", flexShrink: 0 }}>
          {priorityLabel}
        </span>
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        {item.options.map((opt) => (
          <button
            key={opt.id}
            onClick={() => handleResolve(opt.id)}
            disabled={resolving}
            className="mac-btn"
            style={{
              width: "100%", justifyContent: "flex-start", textAlign: "left",
              padding: "8px 12px", flexDirection: "column", alignItems: "flex-start", gap: 2,
            }}
          >
            <span style={{ fontSize: 12, fontWeight: 500 }}>{opt.label}</span>
            {opt.description && (
              <span style={{ fontSize: 11, color: "var(--text-secondary)", fontWeight: 400 }}>
                {opt.description}
              </span>
            )}
          </button>
        ))}
      </div>

      {error && (
        <p className="mono" style={{
          fontSize: 11, color: "var(--status-failed)", background: "var(--status-failed-bg)",
          padding: "5px 10px", borderRadius: 5, marginTop: 10,
        }}>
          {error}
        </p>
      )}

      {Object.keys(item.contextSnapshot).length > 0 && (
        <details style={{ marginTop: 10 }}>
          <summary className="mono" style={{ fontSize: 11, color: "var(--text-tertiary)", cursor: "pointer", userSelect: "none" }}>
            Context snapshot
          </summary>
          <pre className="mono" style={{
            fontSize: 11, color: "var(--text-secondary)", background: "var(--content-secondary-bg)",
            padding: "8px 10px", borderRadius: 5, overflow: "auto", maxHeight: 160, marginTop: 6,
            border: "1px solid var(--border)",
          }}>
            {JSON.stringify(item.contextSnapshot, null, 2)}
          </pre>
        </details>
      )}
    </div>
  );
}

export function QueueView() {
  const project = useAtomValue(projectAtom);
  const items = useAtomValue(queueItemsListAtom);

  return (
    <div className="flex-1 overflow-auto" style={{ padding: "20px 24px" }}>
      <div style={{ maxWidth: 640, margin: "0 auto" }}>
        {/* Header */}
        <div className="flex items-center justify-between" style={{ marginBottom: 16 }}>
          <h2 style={{ fontSize: 15, fontWeight: 600, color: "var(--text-primary)", margin: 0 }}>
            Decision Queue
          </h2>
          <span className="mono" style={{ color: "var(--text-tertiary)", fontSize: 11 }}>
            {items.length} pending
            {project && <> · {project.projectId.slice(0, 8)}</>}
          </span>
        </div>

        {items.length === 0 ? (
          <div style={{
            border: "1px dashed var(--border-strong)", borderRadius: 10,
            padding: "48px 24px", textAlign: "center",
          }}>
            <p style={{ fontSize: 13, color: "var(--text-tertiary)", margin: 0 }}>
              No decisions pending — agents are running.
            </p>
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
            {items.map((item) => (
              <QueueItemCard key={item.id} item={item} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
