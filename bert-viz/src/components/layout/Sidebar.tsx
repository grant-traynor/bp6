import { useState } from "react";
import { User, Tag, Clock, Star, Trash2, Plus, CheckCircle2, ArrowRight } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { Bead } from "../../api";
import { Chip } from "../shared/Chip";

// Subcomponents
import { CollapsibleSection } from "./sidebar/CollapsibleSection";
import { SidebarHeader } from "./sidebar/SidebarHeader";
import { SidebarProperty } from "./sidebar/SidebarProperty";
import { SidebarActions } from "./sidebar/SidebarActions";

interface SidebarProps {
  selectedBead: Bead | null;
  isCreating: boolean;
  isEditing: boolean;
  editForm: Partial<Bead>;
  beads: Bead[];
  setIsEditing: (editing: boolean) => void;
  setIsCreating: (creating: boolean) => void;
  setSelectedBead: (bead: Bead | null) => void;
  setEditForm: (form: Partial<Bead>) => void;
  handleSaveEdit: () => Promise<void>;
  handleSaveCreate: () => Promise<void>;
  handleStartEdit: () => void;
  handleCloseBead: (beadId: string) => Promise<void>;
  handleReopenBead: (beadId: string) => Promise<void>;
  handleClaimBead: (beadId: string) => Promise<void>;
  toggleFavorite: (bead: Bead) => Promise<void>;
}

export const Sidebar = ({
  selectedBead,
  isCreating,
  isEditing,
  editForm,
  beads,
  setIsEditing,
  setIsCreating,
  setSelectedBead,
  setEditForm,
  handleSaveEdit,
  handleSaveCreate,
  handleStartEdit,
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

  const toggleSection = (key: string) => {
    setCollapsedSections(prev => {
      const next = { ...prev, [key]: !prev[key] };
      localStorage.setItem("sidebar-collapsed-sections", JSON.stringify(next));
      return next;
    });
  };

  if (!selectedBead && !isCreating) return null;

  return (
    <div className="absolute right-0 top-0 bottom-0 w-[480px] bg-[var(--background-primary)] border-l-[var(--border-thick)] border-[var(--border-primary)] shadow-[var(--shadow-panel)] z-50 flex flex-col animate-in slide-in-from-right duration-300 backdrop-blur-2xl">
      {/* Header */}
      <SidebarHeader 
        bead={(isCreating || isEditing) ? (editForm as Bead) : (selectedBead as Bead)} 
        onClose={() => { setSelectedBead(null); setIsCreating(false); }} 
        onToggleFavorite={toggleFavorite}
      />
      
      <div className="flex-1 overflow-y-auto p-10 flex flex-col gap-12 custom-scrollbar">
        <section className="flex flex-col gap-6">
          {(isEditing || isCreating) ? (
            <div className="flex flex-col gap-6">
              <div className="flex gap-4">
                 <div className="flex-1 flex flex-col gap-2.5">
                    <span className="text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.25em] pl-1">Type</span>
                    <select 
                      value={editForm.issue_type} 
                      onChange={e => setEditForm({...editForm, issue_type: e.target.value})}
                      className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-sm font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm"
                    >
                      <option value="task">Task</option>
                      <option value="feature">Feature</option>
                      <option value="bug">Bug</option>
                      <option value="epic">Epic</option>
                    </select>
                 </div>
                 <div className="flex-1 flex flex-col gap-2.5">
                    <span className="text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.25em] pl-1">Parent</span>
                    <select 
                      value={editForm.parent || ""} 
                      onChange={e => setEditForm({...editForm, parent: e.target.value})}
                      className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-sm font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm"
                    >
                      <option value="">None</option>
                      {beads.filter(b => b.issue_type === 'epic' || b.issue_type === 'feature').map(b => (
                        <option key={b.id} value={b.id}>{b.id}: {b.title}</option>
                      ))}
                    </select>
                 </div>
              </div>
              
              <div className="flex flex-col gap-2.5">
                <span className="text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.25em] pl-1">Title</span>
                <input 
                  className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-4 text-lg font-black w-full focus:border-indigo-500 outline-none text-[var(--text-primary)] shadow-sm placeholder:text-[var(--text-muted)]"
                  value={editForm.title}
                  onChange={e => setEditForm({...editForm, title: e.target.value})}
                  placeholder="Enter title..."
                />
              </div>

              <div className="flex flex-col gap-2.5">
                <span className="text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.25em] pl-1">Description</span>
                <textarea 
                  className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-4 text-base font-bold min-h-[160px] w-full focus:border-indigo-500 outline-none resize-none text-[var(--text-primary)] shadow-sm leading-relaxed placeholder:text-[var(--text-muted)]"
                  value={editForm.description || ""}
                  onChange={e => setEditForm({...editForm, description: e.target.value})}
                  placeholder="Enter description..."
                />
              </div>
            </div>
          ) : selectedBead && (
            <>
              <h2 className="text-2xl font-black text-[var(--text-primary)] leading-tight tracking-tight drop-shadow-sm">{selectedBead.title}</h2>
              <div className="text-base font-bold text-[var(--text-secondary)] leading-relaxed prose prose-sm dark:prose-invert max-w-none prose-p:my-2 prose-ul:my-2 prose-li:my-1 prose-headings:font-black prose-headings:text-[var(--text-primary)] prose-a:text-indigo-600 dark:prose-a:text-indigo-400 prose-code:text-indigo-700 dark:prose-code:text-indigo-300 prose-code:bg-indigo-500/10 prose-code:px-1.5 prose-code:py-0.5 prose-code:rounded">
                <ReactMarkdown remarkPlugins={[remarkGfm]}>
                  {selectedBead.description || "No description provided."}
                </ReactMarkdown>
              </div>
            </>
          )}
        </section>

        {/* Properties Grid */}
        <div className="grid grid-cols-2 gap-4">
          <SidebarProperty 
            label="Owner"
            icon={User}
            iconColor="text-indigo-600 dark:text-indigo-400"
            value={isEditing || isCreating ? (
              <input 
                type="text"
                value={editForm.owner || ""}
                onChange={(e) => setEditForm({...editForm, owner: e.target.value})}
                className="bg-transparent border-none p-0 focus:ring-0 w-full placeholder:text-[var(--text-muted)] text-sm font-black text-[var(--text-primary)]"
                placeholder="Unassigned"
              />
            ) : selectedBead?.owner || "Unassigned"}
          />
          <SidebarProperty 
            label="Priority"
            icon={Star}
            iconColor="text-amber-500"
            value={isEditing || isCreating ? (
              <select 
                value={editForm.priority}
                onChange={(e) => setEditForm({...editForm, priority: Number(e.target.value)})}
                className="bg-transparent border-none p-0 focus:ring-0 w-full text-sm font-black text-[var(--text-primary)]"
              >
                {[0,1,2,3,4].map(p => (
                  <option key={p} value={p}>P{p} - {['Critical', 'High', 'Medium', 'Low', 'Trivial'][p]}</option>
                ))}
              </select>
            ) : `P${selectedBead?.priority}`}
          />
          <SidebarProperty 
            label="Estimate"
            icon={Clock}
            iconColor="text-indigo-600 dark:text-indigo-400"
            value={isEditing || isCreating ? (
              <div className="flex items-center gap-1">
                <input 
                  type="number"
                  value={editForm.estimate || ""}
                  onChange={(e) => setEditForm({...editForm, estimate: parseInt(e.target.value) || 0})}
                  className="bg-transparent border-none p-0 focus:ring-0 w-16 text-sm font-black text-[var(--text-primary)]"
                  placeholder="0"
                />
                <span className="text-[10px] font-black text-[var(--text-muted)] mt-0.5">MIN</span>
              </div>
            ) : `${selectedBead?.estimate || 0}m`}
          />
          <SidebarProperty 
            label="Status"
            icon={Tag}
            iconColor="text-indigo-600 dark:text-indigo-400"
            value={isEditing || isCreating ? (
              <select 
                value={editForm.status}
                onChange={(e) => {
                  const newStatus = e.target.value;
                  let extra = {};
                  if (newStatus === 'closed' && editForm.status !== 'closed') {
                    const reason = prompt("Enter closure reason:");
                    extra = {
                      closed_at: new Date().toISOString(),
                      close_reason: reason || "Completed"
                    };
                  } else if (newStatus !== 'closed') {
                    extra = {
                      closed_at: undefined,
                      close_reason: undefined
                    };
                  }
                  setEditForm({...editForm, status: newStatus, ...extra});
                }}
                className="bg-transparent border-none p-0 focus:ring-0 w-full text-sm font-black uppercase text-[var(--text-primary)]"
              >
                <option value="open">Open</option>
                <option value="in_progress">In Progress</option>
                <option value="closed">Closed</option>
              </select>
            ) : (selectedBead?.status || "").replace('_', ' ').toUpperCase()}
          />
        </div>

        <CollapsibleSection 
          title="Labels" 
          isCollapsed={!!collapsedSections['labels']} 
          onToggle={() => toggleSection('labels')}
        >
          <div className="flex flex-wrap gap-3">
            {(isEditing || isCreating) ? (
              <input 
                className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-sm font-bold text-[var(--text-primary)] w-full focus:border-indigo-500 outline-none shadow-sm"
                value={(editForm.labels || []).join(", ")}
                onChange={e => setEditForm({...editForm, labels: e.target.value.split(",").map(l => l.trim()).filter(l => l)})}
                placeholder="Add labels (comma separated)..."
              />
            ) : (
              <>
                {selectedBead && <Chip label={selectedBead.issue_type} />}
                {selectedBead?.labels?.map(l => (
                  <Chip key={l} label={l} />
                )) || (!selectedBead?.issue_type && <span className="text-sm text-[var(--text-muted)] font-black italic">No labels</span>)}
              </>
            )}
          </div>
        </CollapsibleSection>

        <CollapsibleSection 
          title="Notes & References" 
          isCollapsed={!!collapsedSections['notes']} 
          onToggle={() => toggleSection('notes')}
        >
          <div className="flex flex-col gap-8">
            {(isEditing || isCreating) ? (
              <>
                <div className="flex flex-col gap-2.5">
                  <span className="text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">Design Notes</span>
                  <textarea 
                    className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-4 text-sm font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none min-h-[100px] resize-none shadow-sm leading-relaxed"
                    value={editForm.design_notes || ""}
                    onChange={e => setEditForm({...editForm, design_notes: e.target.value})}
                    placeholder="Architectural decisions..."
                  />
                </div>
                <div className="flex flex-col gap-2.5">
                  <span className="text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">Working Notes</span>
                  <textarea 
                    className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-4 text-sm font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none min-h-[100px] resize-none shadow-sm leading-relaxed"
                    value={editForm.working_notes || ""}
                    onChange={e => setEditForm({...editForm, working_notes: e.target.value})}
                    placeholder="Progress observations..."
                  />
                </div>
                <div className="flex flex-col gap-2.5">
                  <span className="text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">External Reference</span>
                  <input 
                    className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-sm font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm"
                    value={editForm.external_reference || ""}
                    onChange={e => setEditForm({...editForm, external_reference: e.target.value})}
                    placeholder="URLs, IDs, or paths..."
                  />
                </div>
              </>
            ) : (
              <>
                {selectedBead?.design_notes && (
                  <div className="flex flex-col gap-3">
                     <span className="text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">Design Notes</span>
                     <div className="text-base font-bold text-[var(--text-secondary)] leading-relaxed bg-[var(--background-secondary)] p-4 rounded-2xl border-2 border-[var(--border-primary)] shadow-sm prose prose-sm dark:prose-invert max-w-none prose-p:my-2 prose-ul:my-2 prose-li:my-1 prose-headings:font-black prose-headings:text-[var(--text-primary)] prose-a:text-indigo-600 dark:prose-a:text-indigo-400 prose-code:text-indigo-700 dark:prose-code:text-indigo-300 prose-code:bg-indigo-500/10 prose-code:px-1.5 prose-code:py-0.5 prose-code:rounded">
                       <ReactMarkdown remarkPlugins={[remarkGfm]}>
                         {selectedBead.design_notes}
                       </ReactMarkdown>
                     </div>
                  </div>
                )}
                {selectedBead?.working_notes && (
                  <div className="flex flex-col gap-3">
                     <span className="text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">Working Notes</span>
                     <div className="text-base font-bold text-[var(--text-secondary)] leading-relaxed bg-[var(--background-secondary)] p-4 rounded-2xl border-2 border-[var(--border-primary)] shadow-sm prose prose-sm dark:prose-invert max-w-none prose-p:my-2 prose-ul:my-2 prose-li:my-1 prose-headings:font-black prose-headings:text-[var(--text-primary)] prose-a:text-indigo-600 dark:prose-a:text-indigo-400 prose-code:text-indigo-700 dark:prose-code:text-indigo-300 prose-code:bg-indigo-500/10 prose-code:px-1.5 prose-code:py-0.5 prose-code:rounded">
                       <ReactMarkdown remarkPlugins={[remarkGfm]}>
                         {selectedBead.working_notes}
                       </ReactMarkdown>
                     </div>
                  </div>
                )}
                {selectedBead?.external_reference && (
                  <div className="flex flex-col gap-3">
                     <span className="text-xs font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">External Reference</span>
                     <span className="text-sm text-indigo-700 dark:text-indigo-400 font-black font-mono bg-indigo-500/10 p-3 rounded-xl border-2 border-indigo-500/20 shadow-sm">{selectedBead.external_reference}</span>
                  </div>
                )}
              </>
            )}
          </div>
        </CollapsibleSection>

        <CollapsibleSection 
          title="Acceptance Criteria" 
          isCollapsed={!!collapsedSections['ac']} 
          onToggle={() => toggleSection('ac')}
        >
          <div className="flex flex-col gap-4">
            {(isEditing || isCreating) ? (
              <>
                {(editForm.acceptance_criteria || []).map((ac, i) => (
                  <div key={i} className="flex gap-3">
                    <input 
                      className="flex-1 bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-sm font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm"
                      value={ac}
                      onChange={e => {
                        const newAC = [...(editForm.acceptance_criteria || [])];
                        newAC[i] = e.target.value;
                        setEditForm({...editForm, acceptance_criteria: newAC});
                      }}
                    />
                    <button 
                      onClick={() => {
                        const newAC = (editForm.acceptance_criteria || []).filter((_, idx) => idx !== i);
                        setEditForm({...editForm, acceptance_criteria: newAC});
                      }}
                      className="text-rose-600 dark:text-rose-400 hover:bg-rose-500/10 p-3 rounded-xl transition-colors border-2 border-transparent hover:border-rose-500/20 active:scale-90"
                    >
                      <Trash2 size={20} strokeWidth={3} />
                    </button>
                  </div>
                ))}
                <button 
                  onClick={() => setEditForm({...editForm, acceptance_criteria: [...(editForm.acceptance_criteria || []), ""]})}
                  className="text-sm font-black text-indigo-700 dark:text-indigo-400 hover:text-indigo-800 self-start flex items-center gap-3 bg-indigo-500/10 px-5 py-3 rounded-2xl border-2 border-indigo-500/20 transition-all hover:bg-indigo-500/10 active:scale-95 uppercase tracking-widest"
                >
                  <Plus size={16} strokeWidth={3} /> ADD CRITERION
                </button>
              </>
            ) : (
              (selectedBead?.acceptance_criteria || []).map((ac, i) => (
                <div key={i} className="flex gap-3 items-start bg-[var(--background-secondary)] p-3 rounded-xl border border-[var(--border-primary)]/40 shadow-sm transition-all hover:border-[var(--border-primary)]">
                  <CheckCircle2 size={16} className="text-emerald-600 dark:text-emerald-400 shrink-0 mt-0.5 stroke-[3]" />
                  <span className="text-sm font-bold text-[var(--text-secondary)] leading-tight">{ac}</span>
                </div>
              )) || <span className="text-xs text-[var(--text-muted)] font-black italic pl-1">None specified</span>
            )}
          </div>
        </CollapsibleSection>


        <CollapsibleSection 
          title="Dependencies" 
          isCollapsed={!!collapsedSections['deps']} 
          onToggle={() => toggleSection('deps')}
          rightElement={(isEditing || isCreating) ? (
            <button 
              onClick={(e) => {
                e.stopPropagation();
                const target = prompt("Enter Target Bead ID:");
                if (target) {
                  const newDeps = [...(editForm.dependencies || []), { issue_id: isCreating ? editForm.id! : selectedBead!.id, depends_on_id: target, type: "blocks" }];
                  setEditForm({...editForm, dependencies: newDeps as any});
                }
              }}
              className="text-sm font-black text-indigo-700 dark:text-indigo-400 hover:text-indigo-800 flex items-center gap-3 bg-indigo-500/10 px-5 py-3 rounded-2xl border-2 border-indigo-500/20 transition-all hover:bg-indigo-500/20 active:scale-95 uppercase tracking-widest"
            >
              <Plus size={16} strokeWidth={3} /> ADD
            </button>
          ) : undefined}
        >
          <div className="flex flex-col gap-4">
            {(isCreating ? editForm.dependencies : selectedBead?.dependencies)?.map((d, i) => (
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
                {(isEditing || isCreating) && (
                  <button 
                    onClick={() => {
                      const currentDeps = editForm.dependencies || [];
                      const newDeps = currentDeps.filter((_, index) => index !== i);
                      setEditForm({...editForm, dependencies: newDeps});
                    }}
                    className="text-rose-600 dark:text-rose-400 hover:bg-rose-500/10 p-2 rounded-lg transition-colors border border-transparent hover:border-rose-500/20 active:scale-90"
                  >
                    <Trash2 size={16} strokeWidth={3} />
                  </button>
                )}
              </div>
            )) || <div className="text-sm text-[var(--text-muted)] font-black italic pl-1">None</div>}
          </div>
        </CollapsibleSection>
      </div>
      
      {/* Actions */}
      <SidebarActions
        isEditing={isEditing}
        isCreating={isCreating}
        beadStatus={selectedBead?.status}
        isValid={!!editForm.title}
        onEdit={handleStartEdit}
        onCancel={() => {
          setIsEditing(false);
          setIsCreating(false);
          if (!selectedBead) setSelectedBead(null);
        }}
        onSave={isCreating ? handleSaveCreate : handleSaveEdit}
        onClaim={selectedBead && !isCreating && selectedBead.status === 'open' ? () => handleClaimBead(selectedBead.id) : undefined}
        onClose={selectedBead && !isCreating && selectedBead.status === 'in_progress' ? () => handleCloseBead(selectedBead.id) : undefined}
        onReopen={selectedBead && !isCreating && selectedBead.status === 'closed' ? () => handleReopenBead(selectedBead.id) : undefined}
      />
    </div>
  );
};
