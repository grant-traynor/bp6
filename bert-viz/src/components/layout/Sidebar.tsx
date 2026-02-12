import { useState, useEffect, useMemo, useRef } from "react";
import { User, Tag, Clock, Star, Trash2, Plus, ArrowRight } from "lucide-react";
import type { BeadNode } from "../../api";

// Subcomponents
import { CollapsibleSection } from "./sidebar/CollapsibleSection";
import { SidebarHeader } from "./sidebar/SidebarHeader";
import { SidebarProperty } from "./sidebar/SidebarProperty";
import { SidebarActions } from "./sidebar/SidebarActions";

interface SidebarProps {
  selectedBead: BeadNode | null;
  isCreating: boolean;
  isEditing: boolean;
  editForm: Partial<BeadNode>;
  beads: BeadNode[];
  setIsEditing: (editing: boolean) => void;
  setIsCreating: (creating: boolean) => void;
  setSelectedBead: (bead: BeadNode | null) => void;
  setEditForm: (form: Partial<BeadNode>) => void;
  handleSaveEdit: () => Promise<void>;
  handleSaveCreate: () => Promise<void>;
  handleStartEdit: () => void;
  handleCloseBead: (beadId: string) => Promise<void>;
  handleReopenBead: (beadId: string) => Promise<void>;
  handleClaimBead: (beadId: string) => Promise<void>;
  toggleFavorite: (bead: BeadNode) => Promise<void>;
}

export const Sidebar = ({
  selectedBead,
  isCreating,
  beads,
  setIsCreating,
  setSelectedBead,
  setEditForm,
  editForm,
  handleSaveEdit,
  handleSaveCreate,
  handleCloseBead,
  handleReopenBead,
  handleClaimBead,
  toggleFavorite,
}: SidebarProps) => {
  // Collapsible section state
  const [collapsedSections, setCollapsedSections] = useState<Record<string, boolean>>(() => {
    const saved = localStorage.getItem("sidebar-collapsed-sections");
    return saved ? JSON.parse(saved) : {};
  });

  // Local form state for unified always-editable fields
  const [formData, setFormData] = useState<Partial<BeadNode>>(() => {
    if (isCreating) return editForm;
    if (selectedBead) return selectedBead;
    return {};
  });

  // Initialize form data when selectedBead changes or when creating
  useEffect(() => {
    if (isCreating) {
      setFormData(editForm);
    } else if (selectedBead) {
      setFormData(selectedBead);
    }
  }, [selectedBead, isCreating, editForm]);

  // Detect if form has changes (dirty state)
  const isDirty = useMemo(() => {
    if (isCreating) return false; // Creating mode doesn't have dirty state
    if (!selectedBead || !formData) return false;

    const original = selectedBead;
    const current = formData;

    // Compare key fields with safe null checks
    return (
      (original.title || '') !== (current.title || '') ||
      (original.description || '') !== (current.description || '') ||
      (original.owner || '') !== (current.owner || '') ||
      (original.priority || 0) !== (current.priority || 0) ||
      (original.estimate || 0) !== (current.estimate || 0) ||
      (original.issueType || '') !== (current.issueType || '') ||
      (original.parent || '') !== (current.parent || '') ||
      (original.design || '') !== (current.design || '') ||
      (original.notes || '') !== (current.notes || '') ||
      (original.externalReference || '') !== (current.externalReference || '') ||
      JSON.stringify(original.labels || []) !== JSON.stringify(current.labels || []) ||
      JSON.stringify(original.acceptance_criteria || []) !== JSON.stringify(current.acceptance_criteria || [])
    );
  }, [formData, selectedBead, isCreating]);

  const toggleSection = (key: string) => {
    setCollapsedSections(prev => {
      const next = { ...prev, [key]: !prev[key] };
      localStorage.setItem("sidebar-collapsed-sections", JSON.stringify(next));
      return next;
    });
  };

  const pendingSaveRef = useRef<'create' | 'edit' | null>(null);

  // Effect to handle saves after editForm state updates
  useEffect(() => {
    if (pendingSaveRef.current === 'create') {
      pendingSaveRef.current = null;
      handleSaveCreate();
    } else if (pendingSaveRef.current === 'edit') {
      pendingSaveRef.current = null;
      handleSaveEdit();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [editForm]);

  const handleSave = () => {
    setEditForm(formData);
    if (isCreating) {
      pendingSaveRef.current = 'create';
    } else {
      pendingSaveRef.current = 'edit';
    }
  };

  const handleUndo = () => {
    if (selectedBead) {
      setFormData(selectedBead);
    }
  };

  const handleCancel = () => {
    setIsCreating(false);
    if (!selectedBead) setSelectedBead(null);
  };

  const displayBead = formData;

  if (!selectedBead && !isCreating) {
    return (
      <div className="w-[624px] h-full bg-[var(--background-primary)] border-l-[var(--border-thick)] border-[var(--border-primary)] shadow-[var(--shadow-panel)] flex flex-col backdrop-blur-2xl shrink-0 items-center justify-center p-12 text-center">
        <div className="w-20 h-20 bg-[var(--background-secondary)] rounded-3xl flex items-center justify-center mb-6 border-[var(--border-thick)] border-[var(--border-primary)] shadow-[var(--shadow-sm)]">
          <Tag size={32} className="text-indigo-500 opacity-20" />
        </div>
        <h3 className="text-xl font-black text-[var(--text-primary)] mb-2 uppercase tracking-tight">No Bead Selected</h3>
        <p className="text-sm text-[var(--text-muted)] font-medium leading-relaxed max-w-[280px]">
          Select a bead from the WBS tree or Gantt chart to view and edit its details.
        </p>
      </div>
    );
  }

  if (!formData || Object.keys(formData).length === 0) return null;

  return (
    <div className="w-[624px] h-full bg-[var(--background-primary)] border-l-[var(--border-thick)] border-[var(--border-primary)] shadow-[var(--shadow-panel)] flex flex-col backdrop-blur-2xl shrink-0">
      {/* Header */}
      <SidebarHeader
        bead={displayBead as BeadNode}
        onClose={() => { setSelectedBead(null); setIsCreating(false); }}
        onToggleFavorite={toggleFavorite as any}
      />

      <div className="flex-1 overflow-y-auto p-10 flex flex-col gap-12 custom-scrollbar">
        {/* Title & Description - Always Editable, Styled Like Read-Only */}
        <section className="flex flex-col gap-6">
          <div className="flex gap-4">
            <div className="flex-1 flex flex-col gap-2.5">
              <span className="text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.25em] pl-1">Type</span>
              <select
                value={formData.issueType || "task"}
                onChange={e => setFormData({...formData, issueType: e.target.value})}
                className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-sm font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm"
              >
                <option value="task">Task</option>
                <option value="feature">Feature</option>
                <option value="bug">Bug</option>
                <option value="epic">Epic</option>
              </select>
            </div>
            <div className="flex-1 flex flex-col gap-2.5 min-w-0">
              <span className="text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.25em] pl-1">Parent</span>
              <select
                value={formData.parent || ""}
                onChange={e => setFormData({...formData, parent: e.target.value})}
                className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-sm font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm max-w-full truncate"
                title="Only Epics and Features can be parents"
              >
                <option value="">None</option>
                {beads
                  .filter(b => b.issueType === 'epic' || b.issueType === 'feature')
                  .filter(b => !isCreating || b.id !== formData.id) // Don't show self as parent
                  .map(b => (
                    <option key={b.id} value={b.id}>
                      {b.id}: {b.title.length > 30 ? b.title.substring(0, 30) + '...' : b.title}
                    </option>
                  ))}
              </select>
            </div>
          </div>

          <div className="flex flex-col gap-2.5">
            <span className="text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.25em] pl-1">Title</span>
            <input
              className="bg-transparent border-none p-0 text-2xl font-black w-full focus:ring-0 focus:outline-none text-[var(--text-primary)] placeholder:text-[var(--text-muted)]"
              value={formData.title || ""}
              onChange={e => setFormData({...formData, title: e.target.value})}
              placeholder="Enter title..."
            />
          </div>

          <div className="flex flex-col gap-2.5">
            <span className="text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.25em] pl-1">Description</span>
            <textarea
              className="bg-transparent border-none p-0 text-base font-bold min-h-[160px] w-full focus:ring-0 focus:outline-none resize-none text-[var(--text-secondary)] leading-relaxed placeholder:text-[var(--text-muted)]"
              value={formData.description || ""}
              onChange={e => setFormData({...formData, description: e.target.value})}
              placeholder="Enter description..."
            />
          </div>
        </section>

        {/* Properties Grid */}
        <div className="grid grid-cols-2 gap-4">
          <SidebarProperty
            label="Owner"
            icon={User}
            iconColor="text-indigo-600 dark:text-indigo-400"
            value={
              <input
                type="text"
                value={formData.owner || ""}
                onChange={(e) => setFormData({...formData, owner: e.target.value})}
                className="bg-transparent border-none p-0 focus:ring-0 w-full placeholder:text-[var(--text-muted)] text-sm font-black text-[var(--text-primary)]"
                placeholder="Unassigned"
              />
            }
          />
          <SidebarProperty
            label="Priority"
            icon={Star}
            iconColor="text-amber-500"
            value={
              <select
                value={formData.priority || 2}
                onChange={(e) => setFormData({...formData, priority: Number(e.target.value)})}
                className="bg-transparent border-none p-0 focus:ring-0 w-full text-sm font-black text-[var(--text-primary)]"
              >
                {[0,1,2,3,4].map(p => (
                  <option key={p} value={p}>P{p} - {['Critical', 'High', 'Medium', 'Low', 'Trivial'][p]}</option>
                ))}
              </select>
            }
          />
          <SidebarProperty
            label="Estimate"
            icon={Clock}
            iconColor="text-indigo-600 dark:text-indigo-400"
            value={
              <div className="flex items-center gap-1">
                <input
                  type="number"
                  value={formData.estimate || ""}
                  onChange={(e) => setFormData({...formData, estimate: parseInt(e.target.value) || 0})}
                  className="bg-transparent border-none p-0 focus:ring-0 w-16 text-sm font-black text-[var(--text-primary)]"
                  placeholder="0"
                />
                <span className="text-[10px] font-black text-[var(--text-muted)] mt-0.5">MIN</span>
              </div>
            }
          />
          <SidebarProperty
            label="Status"
            icon={Tag}
            iconColor="text-indigo-600 dark:text-indigo-400"
            value={(formData.status || "open").replace('_', ' ').toUpperCase()}
          />
        </div>

        <CollapsibleSection
          title="Labels"
          isCollapsed={!!collapsedSections['labels']}
          onToggle={() => toggleSection('labels')}
        >
          <div className="flex flex-wrap gap-3">
            <input
              className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-sm font-bold text-[var(--text-primary)] w-full focus:border-indigo-500 outline-none shadow-sm"
              value={(formData.labels || []).join(", ")}
              onChange={e => setFormData({...formData, labels: e.target.value.split(",").map(l => l.trim()).filter(l => l)})}
              placeholder="Add labels (comma separated)..."
            />
          </div>
        </CollapsibleSection>

        <CollapsibleSection
          title="Notes & References"
          isCollapsed={!!collapsedSections['notes']}
          onToggle={() => toggleSection('notes')}
        >
          <div className="flex flex-col gap-8">
            <div className="flex flex-col gap-2.5">
              <span className="text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">Design Notes</span>
              <textarea
                className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-4 text-sm font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none min-h-[100px] resize-none shadow-sm leading-relaxed"
                value={formData.design || ""}
                onChange={e => setFormData({...formData, design: e.target.value})}
                placeholder="Architectural decisions..."
              />
            </div>
            <div className="flex flex-col gap-2.5">
              <span className="text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">Working Notes</span>
              <textarea
                className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-4 text-sm font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none min-h-[100px] resize-none shadow-sm leading-relaxed"
                value={formData.notes || ""}
                onChange={e => setFormData({...formData, notes: e.target.value})}
                placeholder="Progress observations..."
              />
            </div>
            <div className="flex flex-col gap-2.5">
              <span className="text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">External Reference</span>
              <input
                className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-sm font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm"
                value={formData.externalReference || ""}
                onChange={e => setFormData({...formData, externalReference: e.target.value})}
                placeholder="URLs, IDs, or paths..."
              />
            </div>
          </div>
        </CollapsibleSection>

        <CollapsibleSection
          title="Acceptance Criteria"
          isCollapsed={!!collapsedSections['ac']}
          onToggle={() => toggleSection('ac')}
        >
          <div className="flex flex-col gap-4">
            {(formData.acceptance_criteria || []).map((ac: string, i: number) => (
              <div key={i} className="flex gap-3">
                <input
                  className="flex-1 bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-sm font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm"
                  value={ac}
                  onChange={e => {
                    const newAC = [...(formData.acceptance_criteria || [])];
                    newAC[i] = e.target.value;
                    setFormData({...formData, acceptance_criteria: newAC});
                  }}
                />
                <button
                  onClick={() => {
                    const newAC = (formData.acceptance_criteria || []).filter((_: string, idx: number) => idx !== i);
                    setFormData({...formData, acceptance_criteria: newAC});
                  }}
                  className="text-rose-600 dark:text-rose-400 hover:bg-rose-500/10 p-3 rounded-xl transition-colors border-2 border-transparent hover:border-rose-500/20 active:scale-90"
                >
                  <Trash2 size={20} strokeWidth={3} />
                </button>
              </div>
            ))}
            <button
              onClick={() => setFormData({...formData, acceptance_criteria: [...(formData.acceptance_criteria || []), ""]})}
              className="text-sm font-black text-indigo-700 dark:text-indigo-400 hover:text-indigo-800 self-start flex items-center gap-3 bg-indigo-500/10 px-5 py-3 rounded-2xl border-2 border-indigo-500/20 transition-all hover:bg-indigo-500/10 active:scale-95 uppercase tracking-widest"
            >
              <Plus size={16} strokeWidth={3} /> ADD CRITERION
            </button>
          </div>
        </CollapsibleSection>

        <CollapsibleSection
          title="Dependencies"
          isCollapsed={!!collapsedSections['deps']}
          onToggle={() => toggleSection('deps')}
          rightElement={
            <button
              onClick={(e) => {
                e.stopPropagation();
                const target = prompt("Enter Target Bead ID:");
                if (target) {
                  const newDeps = [...(formData.dependencies || []), { issue_id: formData.id!, depends_on_id: target, type: "blocks" }];
                  setFormData({...formData, dependencies: newDeps as any});
                }
              }}
              className="text-sm font-black text-indigo-700 dark:text-indigo-400 hover:text-indigo-800 flex items-center gap-3 bg-indigo-500/10 px-5 py-3 rounded-2xl border-2 border-indigo-500/20 transition-all hover:bg-indigo-500/20 active:scale-95 uppercase tracking-widest"
            >
              <Plus size={16} strokeWidth={3} /> ADD
            </button>
          }
        >
          <div className="flex flex-col gap-4">
            {formData.dependencies?.map((d, i) => (
              <div key={i} className="flex items-center justify-between p-4 rounded-xl bg-[var(--background-secondary)] border border-[var(--border-primary)]/40 group hover:border-indigo-500 shadow-sm transition-all">
                <div className="flex flex-col gap-1.5">
                  <div className="flex items-center gap-3">
                    <span className="text-xs font-black font-mono text-indigo-700 dark:text-indigo-400 bg-indigo-500/10 px-2 py-0.5 rounded border border-indigo-500/20 shadow-sm">{d.depends_on_id}</span>
                    <ArrowRight size={12} className="text-[var(--text-muted)] mt-0.5" />
                    <span className="text-[10px] uppercase font-black text-[var(--text-primary)] tracking-[0.15em] opacity-80">{d.type}</span>
                  </div>
                  {d.metadata && Object.keys(d.metadata).length > 0 && (
                    <div className="flex flex-wrap gap-1.5 mt-0.5 pl-0.5">
                      {Object.entries(d.metadata).map(([k, v]) => (
                        <span key={k} className="text-[9px] font-black bg-[var(--background-tertiary)] px-2 py-0.5 rounded-md text-[var(--text-muted)] border border-[var(--border-primary)]/30 font-mono tracking-tight">
                          {k}: {String(v)}
                        </span>
                      ))}
                    </div>
                  )}
                </div>
                <button
                  onClick={() => {
                    const currentDeps = formData.dependencies || [];
                    const newDeps = currentDeps.filter((_, index) => index !== i);
                    setFormData({...formData, dependencies: newDeps});
                  }}
                  className="text-rose-600 dark:text-rose-400 hover:bg-rose-500/10 p-2 rounded-lg transition-colors border border-transparent hover:border-rose-500/20 active:scale-90"
                >
                  <Trash2 size={16} strokeWidth={3} />
                </button>
              </div>
            )) || <div className="text-sm text-[var(--text-muted)] font-black italic pl-1">None</div>}
          </div>
        </CollapsibleSection>
      </div>

      {/* Actions */}
      <SidebarActions
        isEditing={false}
        isCreating={isCreating}
        isDirty={isDirty}
        beadStatus={selectedBead?.status}
        isValid={!!formData.title}
        onEdit={() => {}} // No longer needed
        onCancel={handleCancel}
        onSave={handleSave}
        onUndo={handleUndo}
        onClaim={selectedBead && selectedBead.status === 'open' ? () => handleClaimBead(selectedBead.id) : undefined}
        onClose={selectedBead && selectedBead.status === 'in_progress' ? () => handleCloseBead(selectedBead.id) : undefined}
        onReopen={selectedBead && selectedBead.status === 'closed' ? () => handleReopenBead(selectedBead.id) : undefined}
      />
    </div>
  );
};
