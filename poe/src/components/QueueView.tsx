import { useState } from "react";
import { useAtomValue } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import { queueItemsListAtom, projectAtom } from "../store/dag";
import type { QueueItem } from "../types";

function QueueItemCard({ item }: { item: QueueItem }) {
  const [selectedOptionId, setSelectedOptionId] = useState<string | null>(null);
  const [userContext, setUserContext] = useState("");
  const [resolving, setResolving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const selectedOption = item.options.find((o) => o.id === selectedOptionId) ?? null;

  const handleConfirm = async () => {
    if (!selectedOptionId) return;
    setResolving(true);
    setError(null);
    try {
      await invoke("resolve_queue_item", {
        params: {
          itemId: item.id,
          chosenOptionId: selectedOptionId,
          userContext: userContext.trim() || null,
        },
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
      {/* Header */}
      <div className="flex items-start justify-between gap-4" style={{ marginBottom: 12 }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <p style={{ fontSize: 13, fontWeight: 500, color: "var(--text-primary)", lineHeight: 1.4 }}>
            {item.question}
          </p>
          <p className="mono" style={{ color: "var(--text-tertiary)", fontSize: 11, marginTop: 4 }}>
            {item.agentId.slice(0, 8)} · {item.id.slice(0, 8)}
          </p>
        </div>
        <span style={{ fontSize: 11, fontWeight: 600, color: priorityColor, whiteSpace: "nowrap", flexShrink: 0 }}>
          {priorityLabel}
        </span>
      </div>

      {/* Option list */}
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        {item.options.map((opt) => {
          const isSelected = selectedOptionId === opt.id;
          return (
            <button
              key={opt.id}
              onClick={() => setSelectedOptionId(isSelected ? null : opt.id)}
              disabled={resolving}
              className="mac-btn"
              style={{
                width: "100%", justifyContent: "flex-start", textAlign: "left",
                padding: "8px 12px", flexDirection: "column", alignItems: "flex-start", gap: 2,
                background: isSelected ? "var(--content-secondary-bg)" : undefined,
                border: isSelected ? "1px solid var(--border-strong)" : undefined,
              }}
            >
              <div style={{ display: "flex", alignItems: "center", gap: 6, width: "100%" }}>
                <span style={{
                  width: 14, height: 14, borderRadius: "50%", flexShrink: 0,
                  border: `2px solid ${isSelected ? "var(--status-running)" : "var(--border-strong)"}`,
                  background: isSelected ? "var(--status-running)" : "transparent",
                  display: "inline-block",
                }} />
                <span style={{ fontSize: 12, fontWeight: isSelected ? 600 : 500 }}>{opt.label}</span>
              </div>
              {opt.description && (
                <span style={{ fontSize: 11, color: "var(--text-secondary)", fontWeight: 400, paddingLeft: 20 }}>
                  {opt.description}
                </span>
              )}
            </button>
          );
        })}
      </div>

      {/* Context input + confirm — shown once an option is selected */}
      {selectedOption && (
        <div style={{ marginTop: 10, display: "flex", flexDirection: "column", gap: 6 }}>
          <textarea
            value={userContext}
            onChange={(e) => setUserContext(e.target.value)}
            placeholder={`Additional context for "${selectedOption.label}" (optional)…`}
            className="mac-input mono"
            style={{ fontSize: 11, height: 72, resize: "vertical" }}
            autoFocus
          />
          <div style={{ display: "flex", gap: 6 }}>
            <button
              onClick={handleConfirm}
              disabled={resolving}
              className="mac-btn mac-btn-primary"
              style={{ flex: 1, justifyContent: "center", fontSize: 11, padding: "5px 10px" }}
            >
              {resolving ? "Sending…" : `Confirm: ${selectedOption.label}`}
            </button>
            <button
              onClick={() => { setSelectedOptionId(null); setUserContext(""); }}
              disabled={resolving}
              className="mac-btn mac-btn-ghost"
              style={{ fontSize: 11, padding: "5px 10px" }}
            >
              Cancel
            </button>
          </div>
        </div>
      )}

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
