import { useState, useEffect, useMemo, useRef, useCallback } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  X, Pencil, Eye, User, Star, Clock, Tag, Trash2, Plus, ArrowRight, Link2, CheckSquare,
} from "lucide-react";
import type { BeadNode } from "../../api";
import { cn } from "../../utils";

const PRIORITY_LABELS = ["P0 — Critical", "P1 — High", "P2 — Medium", "P3 — Low", "P4 — Backlog"];

// ─── bead → markdown (mirrors Rust Bead::to_markdown) ─────────────────────────
function beadToMarkdown(bead: BeadNode): string {
  const priorityLabel = PRIORITY_LABELS[bead.priority] ?? "Unknown";
  let md = `# ${bead.id} — ${bead.title}\n\n`;
  md += `**Status**: ${bead.status} | **Priority**: ${priorityLabel} | **Type**: ${bead.issueType}`;
  if (bead.owner) md += ` | **Owner**: ${bead.owner}`;
  if (bead.parent) md += ` | **Parent**: ${bead.parent}`;
  md += "\n\n";
  if (bead.labels?.length) md += `**Labels**: ${bead.labels.join(", ")}\n\n`;
  if (bead.description?.trim()) md += `## Description\n\n${bead.description.trim()}\n\n`;
  if (bead.acceptanceCriteria?.length) {
    md += "## Acceptance Criteria\n\n";
    for (const item of bead.acceptanceCriteria) {
      const line = item.trim();
      md += line.startsWith("- [ ]") || line.startsWith("- [x]") ? `${line}\n` : `- [ ] ${line}\n`;
    }
    md += "\n";
  }
  if (bead.design?.trim()) md += `## Design\n\n${bead.design.trim()}\n\n`;
  if (bead.notes?.trim()) md += `## Notes\n\n${bead.notes.trim()}\n\n`;
  return md;
}

// ─── Custom markdown renderers — reliable styling without prose plugin ─────────
const mdComponents = {
  h1: ({ children }: any) => (
    <h1 className="text-lg font-black text-[var(--text-primary)] mb-3 pb-2 border-b-2 border-[var(--border-primary)]">
      {children}
    </h1>
  ),
  h2: ({ children }: any) => (
    <h2 className="text-[10px] font-black uppercase tracking-[0.25em] text-[var(--text-muted)] mt-6 mb-2">
      {children}
    </h2>
  ),
  p: ({ children }: any) => (
    <p className="text-sm text-[var(--text-secondary)] leading-relaxed mb-3">{children}</p>
  ),
  strong: ({ children }: any) => (
    <strong className="font-black text-[var(--text-primary)]">{children}</strong>
  ),
  ul: ({ children }: any) => <ul className="list-none pl-0 mb-3 space-y-1">{children}</ul>,
  li: ({ children }: any) => (
    <li className="flex items-start gap-2 text-sm text-[var(--text-secondary)]">
      {children}
    </li>
  ),
  input: ({ checked }: any) => (
    <input type="checkbox" checked={!!checked} readOnly
      className="mt-0.5 shrink-0 accent-indigo-500 cursor-default" />
  ),
};

// ─── Props ────────────────────────────────────────────────────────────────────
interface BeadDetailModalProps {
  bead: BeadNode | null;
  open: boolean;
  onClose: () => void;
  isCreating: boolean;
  editForm: Partial<BeadNode>;
  beads: BeadNode[];
  setIsCreating: (b: boolean) => void;
  setSelectedBead: (b: BeadNode | null) => void;
  setEditForm: (f: Partial<BeadNode>) => void;
  handleSaveEdit: () => Promise<void>;
  handleSaveCreate: () => Promise<void>;
  handleCloseBead: (id: string) => Promise<void>;
  handleReopenBead: (id: string) => Promise<void>;
  handleClaimBead: (id: string) => Promise<void>;
  toggleFavorite: (b: BeadNode) => Promise<void>;
  onOpenChat: (persona: string, task?: string, beadId?: string, role?: string) => void;
}

// ─── Component ────────────────────────────────────────────────────────────────
export const BeadDetailModal = ({
  bead,
  open,
  onClose,
  isCreating,
  editForm,
  beads,
  setIsCreating,
  setSelectedBead,
  setEditForm,
  handleSaveEdit,
  handleSaveCreate,
  handleCloseBead,
  handleReopenBead,
  handleClaimBead,
}: BeadDetailModalProps) => {
  const [mode, setMode] = useState<"view" | "edit">("view");
  const [formData, setFormData] = useState<Partial<BeadNode>>({});

  // Drag state
  const [pos, setPos] = useState({ x: 0, y: 0 });
  const [initialised, setInitialised] = useState(false);
  const dragging = useRef(false);
  const dragOffset = useRef({ x: 0, y: 0 });
  const panelRef = useRef<HTMLDivElement>(null);

  // Set initial position once on first open
  useEffect(() => {
    if (open && !initialised) {
      setPos({ x: Math.max(0, window.innerWidth - 660), y: 80 });
      setInitialised(true);
    }
  }, [open, initialised]);

  // Reset form & mode when bead changes
  useEffect(() => {
    if (isCreating) {
      setFormData(editForm);
      setMode("edit");
    } else if (bead) {
      setFormData(bead);
      setMode("view");
    }
  }, [bead?.id, isCreating]); // eslint-disable-line react-hooks/exhaustive-deps

  const isDirty = useMemo(() => {
    if (isCreating || !bead || !formData) return false;
    return (
      (bead.title || "") !== (formData.title || "") ||
      (bead.description || "") !== (formData.description || "") ||
      (bead.owner || "") !== (formData.owner || "") ||
      (bead.priority ?? 2) !== (formData.priority ?? 2) ||
      (bead.estimate ?? 0) !== (formData.estimate ?? 0) ||
      (bead.issueType || "") !== (formData.issueType || "") ||
      (bead.parent || "") !== (formData.parent || "") ||
      (bead.design || "") !== (formData.design || "") ||
      (bead.notes || "") !== (formData.notes || "") ||
      (bead.externalReference || "") !== (formData.externalReference || "") ||
      JSON.stringify(bead.labels || []) !== JSON.stringify(formData.labels || []) ||
      JSON.stringify(bead.acceptanceCriteria || []) !== JSON.stringify(formData.acceptanceCriteria || [])
    );
  }, [bead, formData, isCreating]);

  const pendingSaveRef = useRef<"create" | "edit" | null>(null);
  useEffect(() => {
    if (pendingSaveRef.current === "create") { pendingSaveRef.current = null; handleSaveCreate(); }
    else if (pendingSaveRef.current === "edit") { pendingSaveRef.current = null; handleSaveEdit(); }
  }, [editForm]); // eslint-disable-line react-hooks/exhaustive-deps

  const handleSave = () => {
    setEditForm(formData);
    pendingSaveRef.current = isCreating ? "create" : "edit";
  };
  const handleUndo = () => { if (bead) setFormData(bead); };
  const handleCancel = () => { setIsCreating(false); if (!bead) setSelectedBead(null); setMode("view"); };

  // ── Drag handlers ────────────────────────────────────────────────────────────
  const onDragStart = useCallback((e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest("button")) return;
    dragging.current = true;
    dragOffset.current = { x: e.clientX - pos.x, y: e.clientY - pos.y };
    e.preventDefault();
  }, [pos]);

  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      if (!dragging.current) return;
      setPos({ x: e.clientX - dragOffset.current.x, y: e.clientY - dragOffset.current.y });
    };
    const onUp = () => { dragging.current = false; };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => { window.removeEventListener("mousemove", onMove); window.removeEventListener("mouseup", onUp); };
  }, []);

  if (!open) return null;
  if (!bead && !isCreating) return null;
  if (!formData || Object.keys(formData).length === 0) return null;

  return (
    // Non-modal: pointer-events only on the panel itself, app remains interactive
    <div
      ref={panelRef}
      className="fixed z-50 flex flex-col bg-[var(--background-primary)] border-2 border-[var(--border-primary)] rounded-3xl shadow-2xl overflow-hidden"
      style={{
        left: pos.x,
        top: pos.y,
        width: 600,
        minWidth: 400,
        minHeight: 300,
        maxHeight: "80vh",
        resize: "both",
        overflow: "hidden",
      }}
    >
      {/* ── Header (drag handle) ─────────────────────────────────────────── */}
      <div
        onMouseDown={onDragStart}
        className="flex items-center gap-3 px-5 py-3 border-b-2 border-[var(--border-primary)] bg-[var(--background-secondary)] shrink-0 cursor-grab active:cursor-grabbing select-none"
      >
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="text-xs font-black font-mono text-[var(--text-muted)] uppercase tracking-widest">
              {formData.id ?? "New"}
            </span>
            <span className="text-xs font-black uppercase tracking-widest"
              style={{ color: formData.status === "open" ? "var(--accent-primary)" : formData.status === "in_progress" ? "#f59e0b" : "#6b7280" }}>
              · {formData.status ?? "open"}
            </span>
          </div>
          <h2 className="text-sm font-black text-[var(--text-primary)] truncate leading-tight mt-0.5">
            {formData.title ?? "New Bead"}
          </h2>
        </div>

        {/* View / Edit toggle */}
        {!isCreating && (
          <div className="flex items-center gap-1 bg-[var(--background-tertiary)] p-1 rounded-xl border border-[var(--border-primary)]">
            <button onClick={() => setMode("view")}
              className={cn("flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-[10px] font-black uppercase tracking-widest transition-all",
                mode === "view" ? "bg-[var(--background-primary)] text-[var(--accent-primary)] shadow-sm" : "text-[var(--text-muted)] hover:text-[var(--text-primary)]")}>
              <Eye size={12} strokeWidth={2.5} /> View
            </button>
            <button onClick={() => setMode("edit")}
              className={cn("flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-[10px] font-black uppercase tracking-widest transition-all",
                mode === "edit" ? "bg-[var(--background-primary)] text-[var(--accent-primary)] shadow-sm" : "text-[var(--text-muted)] hover:text-[var(--text-primary)]")}>
              <Pencil size={12} strokeWidth={2.5} /> Edit
            </button>
          </div>
        )}

        <button onClick={onClose}
          className="p-1.5 hover:bg-[var(--background-tertiary)] rounded-xl text-[var(--text-muted)] hover:text-[var(--text-primary)] transition-all active:scale-90 shrink-0">
          <X size={16} strokeWidth={2.5} />
        </button>
      </div>

      {/* ── Scrollable content ───────────────────────────────────────────── */}
      <div className="flex-1 overflow-y-auto custom-scrollbar" style={{ overflowY: "auto" }}>

        {/* ── VIEW MODE ─────────────────────────────────────────────────── */}
        {mode === "view" && bead && (
          <div className="px-6 py-5">
            <ReactMarkdown remarkPlugins={[remarkGfm]} components={mdComponents}>
              {beadToMarkdown(bead)}
            </ReactMarkdown>
          </div>
        )}

        {/* ── EDIT MODE ─────────────────────────────────────────────────── */}
        {(mode === "edit" || isCreating) && (
          <div className="px-6 py-5 flex flex-col gap-6">
            {/* Type + Parent */}
            <div className="flex gap-3">
              <div className="flex-1 flex flex-col gap-1.5">
                <span className="text-[10px] font-black uppercase tracking-[0.25em] text-[var(--text-muted)]">Type</span>
                <select value={formData.issueType ?? "task"}
                  onChange={e => setFormData({ ...formData, issueType: e.target.value })}
                  className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-xl p-2.5 text-sm font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none">
                  {["task","feature","bug","epic"].map(t => <option key={t} value={t}>{t}</option>)}
                </select>
              </div>
              <div className="flex-1 flex flex-col gap-1.5 min-w-0">
                <span className="text-[10px] font-black uppercase tracking-[0.25em] text-[var(--text-muted)]">Parent</span>
                <select value={formData.parent ?? ""}
                  onChange={e => setFormData({ ...formData, parent: e.target.value })}
                  className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-xl p-2.5 text-sm font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none truncate">
                  <option value="">None</option>
                  {beads.filter(b => b.issueType === "epic" || b.issueType === "feature")
                    .map(b => <option key={b.id} value={b.id}>{b.id}: {b.title.length > 28 ? b.title.slice(0,28)+"…" : b.title}</option>)}
                </select>
              </div>
            </div>

            {/* Title */}
            <div className="flex flex-col gap-1.5">
              <span className="text-[10px] font-black uppercase tracking-[0.25em] text-[var(--text-muted)]">Title</span>
              <input className="bg-transparent border-none p-0 text-lg font-black w-full focus:ring-0 focus:outline-none text-[var(--text-primary)] placeholder:text-[var(--text-muted)]"
                value={formData.title ?? ""} onChange={e => setFormData({ ...formData, title: e.target.value })} placeholder="Enter title…" />
            </div>

            {/* Description */}
            <div className="flex flex-col gap-1.5">
              <span className="text-[10px] font-black uppercase tracking-[0.25em] text-[var(--text-muted)]">Description</span>
              <textarea className="bg-transparent border-none p-0 text-sm font-medium min-h-[80px] w-full focus:ring-0 focus:outline-none resize-none text-[var(--text-secondary)] leading-relaxed placeholder:text-[var(--text-muted)]"
                value={formData.description ?? ""} onChange={e => setFormData({ ...formData, description: e.target.value })} placeholder="Description… (markdown supported)" />
            </div>

            {/* Priority + Owner + Estimate */}
            <div className="grid grid-cols-3 gap-3">
              <div className="flex flex-col gap-1.5">
                <span className="text-[10px] font-black uppercase tracking-[0.25em] text-[var(--text-muted)] flex items-center gap-1"><Star size={10} /> Priority</span>
                <select value={formData.priority ?? 2} onChange={e => setFormData({ ...formData, priority: Number(e.target.value) })}
                  className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-xl p-2 text-sm font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none">
                  {[0,1,2,3,4].map(p => <option key={p} value={p}>P{p} — {["Critical","High","Medium","Low","Trivial"][p]}</option>)}
                </select>
              </div>
              <div className="flex flex-col gap-1.5">
                <span className="text-[10px] font-black uppercase tracking-[0.25em] text-[var(--text-muted)] flex items-center gap-1"><User size={10} /> Owner</span>
                <input type="text" value={formData.owner ?? ""} onChange={e => setFormData({ ...formData, owner: e.target.value })}
                  className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-xl p-2 text-sm font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none" placeholder="Unassigned" />
              </div>
              <div className="flex flex-col gap-1.5">
                <span className="text-[10px] font-black uppercase tracking-[0.25em] text-[var(--text-muted)] flex items-center gap-1"><Clock size={10} /> Estimate</span>
                <div className="flex items-center gap-1 bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-xl px-2.5 py-2">
                  <input type="number" value={formData.estimate ?? ""} onChange={e => setFormData({ ...formData, estimate: parseInt(e.target.value) || 0 })}
                    className="bg-transparent border-none p-0 focus:ring-0 w-10 text-sm font-black text-[var(--text-primary)]" placeholder="0" />
                  <span className="text-[9px] font-black text-[var(--text-muted)] uppercase">min</span>
                </div>
              </div>
            </div>

            {/* Labels */}
            <div className="flex flex-col gap-1.5">
              <span className="text-[10px] font-black uppercase tracking-[0.25em] text-[var(--text-muted)] flex items-center gap-1"><Tag size={10} /> Labels</span>
              <input className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-xl p-2.5 text-sm font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none"
                value={(formData.labels ?? []).join(", ")} onChange={e => setFormData({ ...formData, labels: e.target.value.split(",").map(l => l.trim()).filter(Boolean) })} placeholder="label-a, label-b…" />
            </div>

            {/* Acceptance Criteria */}
            <div className="flex flex-col gap-2">
              <span className="text-[10px] font-black uppercase tracking-[0.25em] text-[var(--text-muted)] flex items-center gap-1"><CheckSquare size={10} /> Acceptance Criteria</span>
              {(formData.acceptanceCriteria ?? []).map((ac: string, i: number) => (
                <div key={i} className="flex items-center gap-2 group">
                  <CheckSquare size={13} className="text-emerald-500 shrink-0" strokeWidth={2.5} />
                  <input className="flex-1 bg-[var(--background-secondary)] border border-[var(--border-primary)] rounded-lg px-2.5 py-1.5 text-sm font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none"
                    value={ac} onChange={e => { const n = [...(formData.acceptanceCriteria ?? [])]; n[i] = e.target.value; setFormData({ ...formData, acceptanceCriteria: n }); }} placeholder={`Criterion ${i + 1}…`} />
                  <button onClick={() => setFormData({ ...formData, acceptanceCriteria: (formData.acceptanceCriteria ?? []).filter((_: string, idx: number) => idx !== i) })}
                    className="text-rose-500 hover:bg-rose-500/10 p-1 rounded-lg opacity-0 group-hover:opacity-100 transition-all active:scale-90">
                    <Trash2 size={13} strokeWidth={2.5} />
                  </button>
                </div>
              ))}
              <button onClick={() => setFormData({ ...formData, acceptanceCriteria: [...(formData.acceptanceCriteria ?? []), ""] })}
                className="self-start flex items-center gap-1.5 text-[10px] font-black text-indigo-600 dark:text-indigo-400 bg-indigo-500/10 px-3 py-1.5 rounded-xl border border-indigo-500/20 uppercase tracking-widest transition-all hover:bg-indigo-500/20 active:scale-95">
                <Plus size={11} strokeWidth={3} /> Add Criterion
              </button>
            </div>

            {/* Design Notes */}
            <div className="flex flex-col gap-1.5">
              <span className="text-[10px] font-black uppercase tracking-[0.25em] text-[var(--text-muted)]">Design Notes</span>
              <textarea className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-xl p-2.5 text-sm font-medium text-[var(--text-primary)] focus:border-indigo-500 outline-none min-h-[70px] resize-none leading-relaxed"
                value={formData.design ?? ""} onChange={e => setFormData({ ...formData, design: e.target.value })} placeholder="Architectural decisions… (markdown supported)" />
            </div>

            {/* Working Notes */}
            <div className="flex flex-col gap-1.5">
              <span className="text-[10px] font-black uppercase tracking-[0.25em] text-[var(--text-muted)]">Working Notes</span>
              <textarea className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-xl p-2.5 text-sm font-medium text-[var(--text-primary)] focus:border-indigo-500 outline-none min-h-[70px] resize-none leading-relaxed"
                value={formData.notes ?? ""} onChange={e => setFormData({ ...formData, notes: e.target.value })} placeholder="Progress observations… (markdown supported)" />
            </div>

            {/* External Reference */}
            <div className="flex flex-col gap-1.5">
              <span className="text-[10px] font-black uppercase tracking-[0.25em] text-[var(--text-muted)]">External Reference</span>
              <input className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-xl p-2.5 text-sm font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none"
                value={formData.externalReference ?? ""} onChange={e => setFormData({ ...formData, externalReference: e.target.value })} placeholder="URLs, IDs, or paths…" />
            </div>

            {/* Dependencies */}
            <div className="flex flex-col gap-2">
              <div className="flex items-center justify-between">
                <span className="text-[10px] font-black uppercase tracking-[0.25em] text-[var(--text-muted)] flex items-center gap-1"><Link2 size={10} /> Dependencies</span>
                <button onClick={() => { const t = prompt("Enter Target Bead ID:"); if (t) setFormData({ ...formData, dependencies: [...(formData.dependencies ?? []), { issue_id: formData.id!, depends_on_id: t, type: "blocks" }] }); }}
                  className="flex items-center gap-1 text-[10px] font-black text-indigo-600 dark:text-indigo-400 bg-indigo-500/10 px-2.5 py-1 rounded-xl border border-indigo-500/20 uppercase tracking-widest transition-all hover:bg-indigo-500/20 active:scale-95">
                  <Plus size={11} strokeWidth={3} /> Add
                </button>
              </div>
              {!(formData.dependencies ?? []).length
                ? <div className="text-xs text-[var(--text-muted)] font-bold italic">None</div>
                : (formData.dependencies ?? []).map((d, i) => (
                  <div key={i} className="flex items-center justify-between px-2.5 py-1.5 rounded-lg bg-[var(--background-secondary)] border border-[var(--border-primary)]/40 group hover:border-indigo-500 transition-all">
                    <div className="flex items-center gap-2 flex-1 min-w-0">
                      <Link2 size={12} className="text-indigo-500 shrink-0" strokeWidth={2.5} />
                      <span className="text-[10px] font-black font-mono text-indigo-600 dark:text-indigo-400 bg-indigo-500/10 px-1.5 py-0.5 rounded border border-indigo-500/20">{d.depends_on_id}</span>
                      <ArrowRight size={9} className="text-[var(--text-muted)] shrink-0" strokeWidth={2.5} />
                      <span className="text-[9px] uppercase font-black text-[var(--text-primary)] tracking-[0.15em]">{d.type}</span>
                    </div>
                    <button onClick={() => setFormData({ ...formData, dependencies: (formData.dependencies ?? []).filter((_, idx) => idx !== i) })}
                      className="text-rose-500 hover:bg-rose-500/10 p-1 rounded-lg opacity-0 group-hover:opacity-100 transition-all active:scale-90">
                      <Trash2 size={12} strokeWidth={2.5} />
                    </button>
                  </div>
                ))}
            </div>
          </div>
        )}
      </div>

      {/* ── Footer ──────────────────────────────────────────────────────── */}
      <div className="flex items-center justify-between gap-2 px-5 py-3 border-t-2 border-[var(--border-primary)] bg-[var(--background-secondary)] shrink-0">
        <div className="flex items-center gap-2">
          {bead?.status === "open" && (
            <button onClick={() => handleClaimBead(bead.id)}
              className="px-3 py-1.5 rounded-xl text-[10px] font-black uppercase tracking-widest bg-indigo-500/10 text-indigo-600 dark:text-indigo-400 border border-indigo-500/20 hover:bg-indigo-500/20 transition-all active:scale-95">
              Claim
            </button>
          )}
          {bead?.status === "in_progress" && (
            <button onClick={() => handleCloseBead(bead.id)}
              className="px-3 py-1.5 rounded-xl text-[10px] font-black uppercase tracking-widest bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border border-emerald-500/20 hover:bg-emerald-500/20 transition-all active:scale-95">
              Close
            </button>
          )}
          {bead?.status === "closed" && (
            <button onClick={() => handleReopenBead(bead.id)}
              className="px-3 py-1.5 rounded-xl text-[10px] font-black uppercase tracking-widest bg-amber-500/10 text-amber-600 dark:text-amber-400 border border-amber-500/20 hover:bg-amber-500/20 transition-all active:scale-95">
              Reopen
            </button>
          )}
        </div>
        <div className="flex items-center gap-2">
          {mode === "edit" && isDirty && (
            <button onClick={handleUndo}
              className="px-3 py-1.5 rounded-xl text-[10px] font-black uppercase tracking-widest text-[var(--text-muted)] hover:bg-[var(--background-tertiary)] transition-all active:scale-95">
              Undo
            </button>
          )}
          {(mode === "edit" || isCreating) && (
            <>
              <button onClick={handleCancel}
                className="px-3 py-1.5 rounded-xl text-[10px] font-black uppercase tracking-widest text-[var(--text-muted)] hover:bg-[var(--background-tertiary)] border border-[var(--border-primary)] transition-all active:scale-95">
                Cancel
              </button>
              <button onClick={handleSave} disabled={!formData.title}
                className={cn("px-5 py-1.5 rounded-xl text-[10px] font-black uppercase tracking-widest transition-all active:scale-95 shadow",
                  formData.title ? "bg-indigo-600 hover:bg-indigo-700 text-white" : "bg-[var(--background-tertiary)] text-[var(--text-muted)] cursor-not-allowed")}>
                {isCreating ? "Create" : "Save"}
              </button>
            </>
          )}
        </div>
      </div>

      {/* Resize hint indicator in bottom-right */}
      <div className="absolute bottom-1 right-1 w-3 h-3 pointer-events-none opacity-30"
        style={{ borderRight: "2px solid var(--text-muted)", borderBottom: "2px solid var(--text-muted)", borderRadius: "0 0 4px 0" }} />
    </div>
  );
};
