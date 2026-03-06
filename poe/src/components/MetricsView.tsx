// bp6-ims.11: Per-stage metrics view
import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAtomValue } from "jotai";
import { projectAtom } from "../store/dag";

interface MetricRecord {
  id: string;
  projectId: string;
  stageNumber: number;
  step: number;
  metricKey: string;
  metricValue: number;
  recordedAt: string;
}

export function MetricsView() {
  const project = useAtomValue(projectAtom);
  const [stageNumber, setStageNumber] = useState(1);
  const [metrics, setMetrics] = useState<MetricRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!project) return;
    setLoading(true);
    setError(null);
    invoke<MetricRecord[]>("get_stage_metrics", {
      projectId: project.projectId,
      stageNumber,
    })
      .then((rows) => {
        setMetrics(rows);
      })
      .catch((err) => {
        setError(String(err));
      })
      .finally(() => setLoading(false));
  }, [project, stageNumber]);

  if (!project) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <p style={{ fontSize: 13, color: "var(--text-tertiary)" }}>No project open.</p>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col overflow-hidden" style={{ padding: 16 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 12, marginBottom: 16 }}>
        <span style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)" }}>
          Stage Metrics
        </span>
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <label style={{ fontSize: 12, color: "var(--text-secondary)" }}>Stage:</label>
          <input
            type="number"
            min={1}
            value={stageNumber}
            onChange={(e) => setStageNumber(Math.max(1, parseInt(e.target.value, 10) || 1))}
            style={{
              width: 60,
              padding: "3px 8px",
              fontSize: 12,
              border: "1px solid var(--border-strong)",
              borderRadius: 6,
              background: "var(--input-bg)",
              color: "var(--text-primary)",
            }}
          />
        </div>
      </div>

      {loading && (
        <p className="mono" style={{ fontSize: 12, color: "var(--text-tertiary)" }}>
          Loading…
        </p>
      )}
      {error && (
        <p style={{ fontSize: 12, color: "var(--status-error)" }}>Error: {error}</p>
      )}
      {!loading && !error && metrics.length === 0 && (
        <div
          style={{
            border: "1px dashed var(--border-strong)",
            borderRadius: 10,
            padding: "32px 24px",
            textAlign: "center",
          }}
        >
          <p style={{ fontSize: 13, color: "var(--text-tertiary)", margin: 0 }}>
            No metrics recorded for Stage {stageNumber}.
          </p>
          <p style={{ fontSize: 11, color: "var(--text-placeholder)", marginTop: 4, margin: "4px 0 0" }}>
            Metrics are recorded automatically when a stage completes execution.
          </p>
        </div>
      )}
      {!loading && metrics.length > 0 && (
        <div style={{ overflowX: "auto" }}>
          <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 12 }}>
            <thead>
              <tr style={{ borderBottom: "1px solid var(--border-strong)" }}>
                {["Stage", "Step", "Key", "Value", "Recorded At"].map((h) => (
                  <th
                    key={h}
                    style={{
                      textAlign: "left",
                      padding: "6px 10px",
                      color: "var(--text-secondary)",
                      fontWeight: 600,
                      whiteSpace: "nowrap",
                    }}
                  >
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {metrics.map((m) => (
                <tr
                  key={m.id}
                  style={{ borderBottom: "1px solid var(--divider)" }}
                >
                  <td style={{ padding: "6px 10px", color: "var(--text-primary)" }}>{m.stageNumber}</td>
                  <td style={{ padding: "6px 10px", color: "var(--text-primary)" }}>{m.step}</td>
                  <td className="mono" style={{ padding: "6px 10px", color: "var(--accent)" }}>{m.metricKey}</td>
                  <td className="mono" style={{ padding: "6px 10px", color: "var(--text-primary)" }}>{m.metricValue}</td>
                  <td className="mono" style={{ padding: "6px 10px", color: "var(--text-tertiary)", fontSize: 11 }}>
                    {new Date(m.recordedAt).toLocaleString()}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
