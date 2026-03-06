/**
 * bp6-ims.9: ArtifactsView
 *
 * Read-only panel showing all KnowledgeArtifact nodes for the project,
 * grouped by step number with expandable content preview.
 */
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAtomValue } from "jotai";
import { projectAtom } from "../store/dag";
import type { ArtifactRecord } from "../types";

function stepBadgeLabel(step: number, substep: string | null): string {
  if (step === 1) return "Step 1";
  if (step === 2) {
    if (substep === "1") return "Step 2.1";
    if (substep === "2") return "Step 2.2";
    if (substep === "3") return "Step 2.3";
    if (substep === "4") return "Step 2.4";
    if (substep === "review") return "Step 2.review";
    return "Step 2";
  }
  if (step === 3) return "Step 3";
  return `Step ${step}`;
}

function groupByStep(records: ArtifactRecord[]): Map<string, ArtifactRecord[]> {
  const map = new Map<string, ArtifactRecord[]>();
  for (const r of records) {
    const key = `${r.step}-${r.substep ?? ""}`;
    const existing = map.get(key) ?? [];
    existing.push(r);
    map.set(key, existing);
  }
  return map;
}

// ── ArtifactRow ───────────────────────────────────────────────────────────────

function ArtifactRow({ record }: { record: ArtifactRecord }) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div
      style={{
        borderBottom: "1px solid var(--divider)",
        padding: "10px 0",
      }}
    >
      <button
        onClick={() => setExpanded((v) => !v)}
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          width: "100%",
          background: "transparent",
          border: "none",
          cursor: "pointer",
          padding: 0,
          textAlign: "left",
        }}
      >
        <span
          style={{
            fontSize: 10,
            fontWeight: 600,
            color: "var(--accent)",
            background: "var(--content-secondary-bg)",
            border: "1px solid var(--border)",
            borderRadius: 4,
            padding: "1px 6px",
            fontFamily: "monospace",
            flexShrink: 0,
          }}
        >
          {stepBadgeLabel(record.step, record.substep)}
        </span>
        <span style={{ fontSize: 12, fontWeight: 500, color: "var(--text-primary)", flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {record.title}
        </span>
        <span style={{ fontSize: 11, color: "var(--text-tertiary)", flexShrink: 0 }}>
          {expanded ? "▲" : "▼"}
        </span>
      </button>

      {expanded && (
        <pre
          className="mono"
          style={{
            marginTop: 8,
            fontSize: 11,
            color: "var(--text-secondary)",
            background: "var(--content-secondary-bg)",
            border: "1px solid var(--border)",
            borderRadius: 6,
            padding: "10px 12px",
            overflow: "auto",
            maxHeight: 320,
            whiteSpace: "pre-wrap",
            wordBreak: "break-word",
            lineHeight: 1.6,
          }}
        >
          {record.content}
        </pre>
      )}
    </div>
  );
}

// ── ArtifactGroup ─────────────────────────────────────────────────────────────

function ArtifactGroup({ label, records }: { label: string; records: ArtifactRecord[] }) {
  return (
    <div style={{ marginBottom: 20 }}>
      <p className="mac-section-header" style={{ paddingTop: 0, paddingBottom: 4, marginBottom: 4 }}>
        {label}
      </p>
      {records.map((r) => (
        <ArtifactRow key={r.id} record={r} />
      ))}
    </div>
  );
}

// ── ArtifactsView ─────────────────────────────────────────────────────────────

export function ArtifactsView() {
  const project = useAtomValue(projectAtom);
  const [records, setRecords] = useState<ArtifactRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!project) return;
    setLoading(true);
    setError(null);
    // step=10 returns all artefacts from steps 1-9
    invoke<ArtifactRecord[]>("get_artefacts_for_step", { step: 10 })
      .then((data) => {
        setRecords(data);
      })
      .catch((err: unknown) => {
        setError(String(err));
      })
      .finally(() => setLoading(false));
  }, [project?.projectId]);

  if (!project) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <p style={{ fontSize: 13, color: "var(--text-tertiary)" }}>No project open.</p>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <p className="mono" style={{ fontSize: 12, color: "var(--text-tertiary)" }}>Loading artefacts…</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex-1 flex items-center justify-center" style={{ padding: 32 }}>
        <p className="mono" style={{ fontSize: 12, color: "var(--status-failed)" }}>
          Failed to load artefacts: {error}
        </p>
      </div>
    );
  }

  if (records.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div
          style={{
            border: "1px dashed var(--border-strong)",
            borderRadius: 10,
            padding: "48px 32px",
            textAlign: "center",
          }}
        >
          <p style={{ fontSize: 13, color: "var(--text-tertiary)", margin: 0 }}>
            No artefacts yet.
          </p>
          <p style={{ fontSize: 11, color: "var(--text-placeholder)", marginTop: 4 }}>
            Artefacts are created as lifecycle steps complete.
          </p>
        </div>
      </div>
    );
  }

  // Group by step+substep
  const grouped = groupByStep(records);
  const groupEntries = Array.from(grouped.entries()).sort(([a], [b]) => a.localeCompare(b));

  return (
    <div className="flex-1 overflow-auto" style={{ padding: "20px 24px" }}>
      <div style={{ maxWidth: 680, margin: "0 auto" }}>
        <div className="flex items-center justify-between" style={{ marginBottom: 20 }}>
          <h2 style={{ fontSize: 15, fontWeight: 600, color: "var(--text-primary)", margin: 0 }}>
            Project Artefacts
          </h2>
          <span className="mono" style={{ color: "var(--text-tertiary)", fontSize: 11 }}>
            {records.length} artefact{records.length !== 1 ? "s" : ""}
          </span>
        </div>

        {groupEntries.map(([key, groupRecords]) => {
          const first = groupRecords[0];
          const label = stepBadgeLabel(first.step, first.substep);
          return (
            <ArtifactGroup key={key} label={label} records={groupRecords} />
          );
        })}
      </div>
    </div>
  );
}
