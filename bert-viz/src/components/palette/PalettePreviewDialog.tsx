import { X } from "lucide-react";
import type { CSSProperties } from "react";

type PaletteItem = {
  key: string;
  label: string;
  note?: string;
  kind?: 'stroke';
};

const paletteValues = {
  dark: {
    bgPrimary: "#05070D",
    bgPanel: "#0E1624",
    bgRaised: "#141E2D",
    textPrimary: "#E9F1FF",
    textSecondary: "#B6C4D9",
    textMuted: "#7D8AA3",
    border: "#1F2A3A",
    accentPrimary: "#3BA3FF",
    accentSecondary: "#5CF1FF",
    accentViolet: "#9C5CFF",
    statusOpen: "#2FC6A0",
    statusActive: "#3BA3FF",
    statusBlocked: "#F06D76",
    statusDone: "#6B768C",
    ganttConnector: "#6B7A90",
    ganttConnectorCritical: "#FF6B6B",
    ganttGridline: "rgba(255,255,255,0.06)",
    ganttSummary: "#94A9C5",
    ganttMilestone: "#5CF1FF",
    focus: "rgba(92,241,255,0.35)",
    glow: "rgba(59,163,255,0.20)",
  },
  light: {
    bgPrimary: "#F6F9FF",
    bgPanel: "#EDF2FA",
    bgRaised: "#E5EBF5",
    textPrimary: "#0C1A2A",
    textSecondary: "#24344D",
    textMuted: "#55657D",
    border: "#C8D3E5",
    accentPrimary: "#0F8BFF",
    accentSecondary: "#3BC8FF",
    accentViolet: "#9C5CFF",
    statusOpen: "#2FC6A0",
    statusActive: "#0F8BFF",
    statusBlocked: "#F06D76",
    statusDone: "#6B768C",
    ganttConnector: "#6B7A90",
    ganttConnectorCritical: "#FF6B6B",
    ganttGridline: "rgba(0,23,46,0.08)",
    ganttSummary: "#4C6B8C",
    ganttMilestone: "#0F8BFF",
    focus: "rgba(15,139,255,0.25)",
    glow: "rgba(59,163,255,0.18)",
  }
};

const groups: PaletteItem[] = [
  { key: "bgPrimary", label: "BG Primary" },
  { key: "bgPanel", label: "BG Panel" },
  { key: "bgRaised", label: "BG Raised" },
  { key: "textPrimary", label: "Text Primary" },
  { key: "textSecondary", label: "Text Secondary" },
  { key: "textMuted", label: "Text Muted" },
  { key: "border", label: "Border" },
  { key: "accentPrimary", label: "Accent Primary" },
  { key: "accentSecondary", label: "Accent Cyan" },
  { key: "accentViolet", label: "Accent Violet" },
  { key: "statusOpen", label: "Status Open" },
  { key: "statusActive", label: "Status Active" },
  { key: "statusBlocked", label: "Status Blocked" },
  { key: "statusDone", label: "Status Done" },
  { key: "ganttConnector", label: "Gantt Connector" },
  { key: "ganttConnectorCritical", label: "Gantt Critical" },
  { key: "ganttGridline", label: "Gantt Gridline" },
  { key: "ganttSummary", label: "Gantt Summary" },
  { key: "ganttMilestone", label: "Gantt Milestone", kind: "stroke" },
  { key: "focus", label: "Focus" },
  { key: "glow", label: "Glow" },
];

const Swatch = ({ label, value, kind }: { label: string; value: string; kind?: 'stroke'; }) => {
  const isGradient = value.startsWith("linear-gradient") || value.startsWith("radial-gradient") || value.startsWith("rgba(");
  const style: CSSProperties = kind === 'stroke'
    ? { background: "transparent", boxShadow: `inset 0 0 0 3px ${value}`, borderRadius: 12 }
    : { background: isGradient ? value : value, borderRadius: 12 };

  return (
    <div
      className="flex flex-col gap-2 rounded-xl p-3 shadow-[var(--shadow-sm)]"
      style={{ backgroundColor: "var(--background-primary)", border: `1px solid var(--border-primary)` }}
    >
      <div className="h-16 w-full rounded-lg" style={style} />
      <div className="text-[11px] font-black uppercase tracking-[0.08em] text-[var(--text-secondary)]">{label}</div>
      <div className="text-[12px] font-mono text-[var(--text-muted)] break-all">{value}</div>
    </div>
  );
};

const Column = ({ title, theme }: { title: string; theme: keyof typeof paletteValues; }) => (
  <div
    className="flex-1 min-w-[280px] rounded-xl p-4 flex flex-col gap-3"
    style={{ backgroundColor: "var(--background-tertiary)", border: "1px solid var(--border-primary)" }}
  >
    <div className="text-sm font-black uppercase tracking-[0.14em] text-[var(--text-secondary)]">{title}</div>
    <div className="grid grid-cols-2 gap-3">
      {groups.map(({ key, label, kind }) => (
        <Swatch key={`${theme}-${key}`} label={label} value={(paletteValues as any)[theme][key]} kind={kind} />
      ))}
    </div>
  </div>
);

export const PalettePreviewDialog = ({ onClose }: { onClose: () => void; }) => {
  return (
    <div className="fixed inset-0 z-[200] flex items-center justify-center bg-black/70 px-4">
      <div className="bg-[var(--background-secondary)] border border-[var(--border-primary)] rounded-2xl shadow-[var(--shadow-xl)] w-full max-w-6xl max-h-[90vh] overflow-hidden">
        <div className="flex items-center justify-between px-5 py-4 border-b border-[var(--border-primary)] bg-[var(--background-tertiary)]/70">
          <div>
            <div className="text-xs uppercase tracking-[0.18em] text-[var(--text-muted)] font-black">Palette Preview</div>
            <div className="text-lg font-black text-[var(--text-primary)] leading-tight">Pairti Palette · Dark & Light</div>
          </div>
          <button
            onClick={onClose}
            className="p-2 rounded-lg border border-[var(--border-primary)] text-[var(--text-primary)] hover:border-[var(--accent-primary)] hover:text-[var(--accent-primary)] transition-colors"
            aria-label="Close palette preview"
          >
            <X size={18} />
          </button>
        </div>
        <div className="p-5 overflow-auto max-h-[calc(90vh-96px)]">
          <div className="flex flex-col lg:flex-row gap-4">
            <Column title="Dark" theme="dark" />
            <Column title="Light" theme="light" />
          </div>
        </div>
      </div>
    </div>
  );
};
