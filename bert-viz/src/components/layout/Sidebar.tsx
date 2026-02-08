import { X, Star, User, Tag, Clock, Save, Edit3, Trash2, Plus, CheckCircle2 } from "lucide-react";
import { cn } from "../../utils";
import type { Bead } from "../../api";
import { StatusIcon } from "../shared/StatusIcon";
import { Chip } from "../shared/Chip";

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
  toggleFavorite,
}: SidebarProps) => {
  if (!selectedBead && !isCreating) return null;

  return (
    <div className="absolute right-0 top-0 bottom-0 w-[480px] bg-[var(--background-primary)] border-l-2 border-[var(--border-primary)] shadow-[0_0_60px_rgba(0,0,0,0.2)] z-50 flex flex-col animate-in slide-in-from-right duration-300 backdrop-blur-2xl">
      <div className="p-6 border-b-2 border-[var(--border-primary)] flex items-center justify-between bg-[var(--background-secondary)]">
        <div className="flex items-center gap-4">
          <span className="font-mono text-[11px] px-3 py-2 rounded-xl bg-indigo-700 dark:bg-indigo-500 text-white font-black border-2 border-indigo-800 shadow-md tracking-widest uppercase">
            {isCreating ? "NEW BEAD" : selectedBead?.id}
          </span>
          {selectedBead && !isEditing && !isCreating && (
            <button 
              onClick={() => toggleFavorite(selectedBead)}
              className={cn(
                "p-2.5 rounded-xl hover:bg-[var(--background-tertiary)] border-2 border-transparent hover:border-[var(--border-primary)] transition-all active:scale-90",
                selectedBead.is_favorite ? "text-amber-500 bg-amber-500/5 border-amber-500/20" : "text-[var(--text-secondary)]"
              )}
            >
              <Star size={20} className={cn(selectedBead.is_favorite && "fill-current")} />
            </button>
          )}
          {(isEditing || isCreating) && (
            <select 
              value={editForm.status} 
              onChange={e => {
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
              className="bg-[var(--background-primary)] text-[11px] font-black text-[var(--text-primary)] border-2 border-[var(--border-primary)] rounded-xl px-4 py-2.5 focus:ring-4 focus:ring-indigo-500/20 focus:border-indigo-500 outline-none uppercase tracking-[0.2em] shadow-sm"
            >
              <option value="open">Open</option>
              <option value="in_progress">In Progress</option>
              <option value="closed">Closed</option>
            </select>
          )}
        </div>
        <button onClick={() => { setSelectedBead(null); setIsCreating(false); }} className="p-3 hover:bg-rose-500/10 rounded-2xl text-[var(--text-secondary)] hover:text-rose-600 transition-all active:scale-90 border-2 border-transparent hover:border-rose-500/20">
          <X size={24} strokeWidth={3} />
        </button>
      </div>
      
      <div className="flex-1 overflow-y-auto p-10 flex flex-col gap-12 custom-scrollbar">
        <section className="flex flex-col gap-6">
          {(isEditing || isCreating) ? (
            <div className="flex flex-col gap-6">
              <div className="flex gap-4">
                 <div className="flex-1 flex flex-col gap-2.5">
                    <span className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.25em] pl-1">Type</span>
                    <select 
                      value={editForm.issue_type} 
                      onChange={e => setEditForm({...editForm, issue_type: e.target.value})}
                      className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-[12px] font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm"
                    >
                      <option value="task">Task</option>
                      <option value="feature">Feature</option>
                      <option value="bug">Bug</option>
                      <option value="epic">Epic</option>
                    </select>
                 </div>
                 <div className="flex-1 flex flex-col gap-2.5">
                    <span className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.25em] pl-1">Parent</span>
                    <select 
                      value={editForm.parent || ""} 
                      onChange={e => setEditForm({...editForm, parent: e.target.value})}
                      className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-[12px] font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm"
                    >
                      <option value="">None</option>
                      {beads.filter(b => b.issue_type === 'epic' || b.issue_type === 'feature').map(b => (
                        <option key={b.id} value={b.id}>{b.id}: {b.title}</option>
                      ))}
                    </select>
                 </div>
              </div>
              
              <div className="flex flex-col gap-2.5">
                <span className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.25em] pl-1">Title</span>
                <input 
                  className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-4 text-lg font-black w-full focus:border-indigo-500 outline-none text-[var(--text-primary)] shadow-sm placeholder:text-[var(--text-muted)]"
                  value={editForm.title}
                  onChange={e => setEditForm({...editForm, title: e.target.value})}
                  placeholder="Enter title..."
                />
              </div>

              <div className="flex flex-col gap-2.5">
                <span className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.25em] pl-1">Description</span>
                <textarea 
                  className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-4 text-[13px] font-bold min-h-[160px] w-full focus:border-indigo-500 outline-none resize-none text-[var(--text-primary)] shadow-sm leading-relaxed placeholder:text-[var(--text-muted)]"
                  value={editForm.description || ""}
                  onChange={e => setEditForm({...editForm, description: e.target.value})}
                  placeholder="Enter description..."
                />
              </div>
            </div>
          ) : selectedBead && (
            <>
              <h2 className="text-2xl font-black text-[var(--text-primary)] leading-tight tracking-tight drop-shadow-sm">{selectedBead.title}</h2>
              <p className="text-[13px] font-bold text-[var(--text-secondary)] leading-relaxed">{selectedBead.description || "No description provided."}</p>
            </>
          )}
        </section>

        <div className="grid grid-cols-2 gap-10">
          <div className="flex flex-col gap-3">
            <span className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.25em] flex items-center gap-2"><User size={14} className="text-indigo-600 dark:text-indigo-400 stroke-[3]" /> Owner</span>
            {(isEditing || isCreating) ? (
              <input 
                className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-[12px] font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm"
                value={editForm.owner || ""}
                onChange={e => setEditForm({...editForm, owner: e.target.value})}
                placeholder="Assignee"
              />
            ) : (
              <span className="text-[13px] text-[var(--text-primary)] font-black tracking-tight bg-[var(--background-tertiary)] px-4 py-2.5 rounded-xl border-2 border-[var(--border-primary)]/50">{selectedBead?.owner || "Unassigned"}</span>
            )}
          </div>
          <div className="flex flex-col gap-3">
            <span className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.25em] flex items-center gap-2"><Tag size={14} className="text-indigo-600 dark:text-indigo-400 stroke-[3]" /> Priority</span>
            {(isEditing || isCreating) ? (
              <select 
                value={editForm.priority} 
                onChange={e => setEditForm({...editForm, priority: parseInt(e.target.value)})}
                className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-[12px] font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm"
              >
                {[0,1,2,3,4].map(p => <option key={p} value={p}>P{p} - {['Critical', 'High', 'Medium', 'Low', 'Trivial'][p]}</option>)}
              </select>
            ) : (
              <span className="text-[13px] text-[var(--text-primary)] font-black tracking-tight bg-[var(--background-tertiary)] px-4 py-2.5 rounded-xl border-2 border-[var(--border-primary)]/50 text-center">P{selectedBead?.priority}</span>
            )}
          </div>
        </div>

        <section className="flex flex-col gap-5">
          <h3 className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.3em] pl-1">Labels</h3>
          <div className="flex flex-wrap gap-3">
            {(isEditing || isCreating) ? (
              <input 
                className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-[12px] font-bold text-[var(--text-primary)] w-full focus:border-indigo-500 outline-none shadow-sm"
                value={(editForm.labels || []).join(", ")}
                onChange={e => setEditForm({...editForm, labels: e.target.value.split(",").map(l => l.trim()).filter(l => l)})}
                placeholder="Add labels (comma separated)..."
              />
            ) : (
              <>
                {selectedBead && <Chip label={selectedBead.issue_type} />}
                {selectedBead?.labels?.map(l => (
                  <Chip key={l} label={l} />
                )) || (!selectedBead?.issue_type && <span className="text-[12px] text-[var(--text-muted)] font-black italic">No labels</span>)}
              </>
            )}
          </div>
        </section>

        <section className="flex flex-col gap-5">
          <h3 className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.3em] pl-1">Notes & References</h3>
          <div className="flex flex-col gap-8">
            {(isEditing || isCreating) ? (
              <>
                <div className="flex flex-col gap-2.5">
                  <span className="text-[9px] font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">Design Notes</span>
                  <textarea 
                    className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-4 text-[12px] font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none min-h-[100px] resize-none shadow-sm leading-relaxed"
                    value={editForm.design_notes || ""}
                    onChange={e => setEditForm({...editForm, design_notes: e.target.value})}
                    placeholder="Architectural decisions..."
                  />
                </div>
                <div className="flex flex-col gap-2.5">
                  <span className="text-[9px] font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">Working Notes</span>
                  <textarea 
                    className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-4 text-[12px] font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none min-h-[100px] resize-none shadow-sm leading-relaxed"
                    value={editForm.working_notes || ""}
                    onChange={e => setEditForm({...editForm, working_notes: e.target.value})}
                    placeholder="Progress observations..."
                  />
                </div>
                <div className="flex flex-col gap-2.5">
                  <span className="text-[9px] font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">External Reference</span>
                  <input 
                    className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-[12px] font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm"
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
                     <span className="text-[9px] font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">Design Notes</span>
                     <p className="text-[13px] font-bold text-[var(--text-secondary)] leading-relaxed whitespace-pre-wrap bg-[var(--background-secondary)] p-4 rounded-2xl border-2 border-[var(--border-primary)] shadow-sm">{selectedBead.design_notes}</p>
                  </div>
                )}
                {selectedBead?.working_notes && (
                  <div className="flex flex-col gap-3">
                     <span className="text-[9px] font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">Working Notes</span>
                     <p className="text-[13px] font-bold text-[var(--text-secondary)] leading-relaxed whitespace-pre-wrap bg-[var(--background-secondary)] p-4 rounded-2xl border-2 border-[var(--border-primary)] shadow-sm">{selectedBead.working_notes}</p>
                  </div>
                )}
                {selectedBead?.external_reference && (
                  <div className="flex flex-col gap-3">
                     <span className="text-[9px] font-black text-[var(--text-primary)] uppercase tracking-[0.2em]">External Reference</span>
                     <span className="text-[12px] text-indigo-700 dark:text-indigo-400 font-black font-mono bg-indigo-500/10 p-3 rounded-xl border-2 border-indigo-500/20 shadow-sm">{selectedBead.external_reference}</span>
                  </div>
                )}
              </>
            )}
          </div>
        </section>

        <section className="flex flex-col gap-6">
          <h3 className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.3em] pl-1">Acceptance Criteria</h3>
          <div className="flex flex-col gap-4">
            {(isEditing || isCreating) ? (
              <>
                {(editForm.acceptance_criteria || []).map((ac, i) => (
                  <div key={i} className="flex gap-3">
                    <input 
                      className="flex-1 bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-[12px] font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm"
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
                  className="text-[11px] font-black text-indigo-700 dark:text-indigo-400 hover:text-indigo-800 self-start flex items-center gap-3 bg-indigo-500/10 px-5 py-3 rounded-2xl border-2 border-indigo-500/20 transition-all hover:bg-indigo-500/10 active:scale-95 uppercase tracking-widest"
                >
                  <Plus size={16} strokeWidth={3} /> ADD CRITERION
                </button>
              </>
            ) : (
              (selectedBead?.acceptance_criteria || []).map((ac, i) => (
                <div key={i} className="flex gap-4 items-start bg-[var(--background-secondary)] p-4 rounded-2xl border-2 border-[var(--border-primary)] shadow-sm">
                  <CheckCircle2 size={18} className="text-emerald-600 dark:text-emerald-400 shrink-0 mt-0.5 stroke-[3]" />
                  <span className="text-[13px] font-bold text-[var(--text-secondary)] leading-relaxed">{ac}</span>
                </div>
              )) || <span className="text-[12px] text-[var(--text-muted)] font-black italic pl-1">None specified</span>
            )}
          </div>
        </section>

        <div className="grid grid-cols-2 gap-10">
          <div className="flex flex-col gap-3">
            <span className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.25em] flex items-center gap-2"><Clock size={14} className="text-indigo-600 dark:text-indigo-400 stroke-[3]" /> Estimate</span>
            {(isEditing || isCreating) ? (
              <input 
                type="number"
                className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-[12px] font-bold text-[var(--text-primary)] focus:border-indigo-500 outline-none shadow-sm"
                value={editForm.estimate || ""}
                onChange={e => setEditForm({...editForm, estimate: parseInt(e.target.value) || 0})}
                placeholder="Minutes"
              />
            ) : (
              <span className="text-[13px] text-[var(--text-primary)] font-black tracking-tight bg-[var(--background-tertiary)] px-4 py-2.5 rounded-xl border-2 border-[var(--border-primary)]/50 text-center">{selectedBead?.estimate || 0}m</span>
            )}
          </div>
          <div className="flex flex-col gap-3">
            <span className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.25em] flex items-center gap-2">Status</span>
            {(isEditing || isCreating) ? (
               <select 
                value={editForm.status} 
                onChange={e => {
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
                className="bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-2xl p-3.5 text-[12px] font-black text-[var(--text-primary)] focus:border-indigo-500 outline-none uppercase tracking-[0.2em] shadow-sm"
              >
                <option value="open">Open</option>
                <option value="in_progress">In Progress</option>
                <option value="closed">Closed</option>
              </select>
            ) : (
              <div className="flex items-center gap-3 bg-indigo-700 dark:bg-indigo-500 px-4 py-2.5 rounded-xl border-2 border-indigo-800 dark:border-indigo-400 self-start shadow-md">
                <StatusIcon status={selectedBead?.status || ""} size={16} className="text-white" />
                <span className="text-[11px] text-white font-black uppercase tracking-widest">{selectedBead?.status.replace('_', ' ')}</span>
              </div>
            )}
          </div>
        </div>

        <section className="flex flex-col gap-6 pb-6">
          <div className="flex items-center justify-between">
            <h3 className="text-[10px] font-black text-[var(--text-primary)] uppercase tracking-[0.3em] pl-1">Dependencies</h3>
            {(isEditing || isCreating) && (
              <button 
                onClick={() => {
                  const target = prompt("Enter Target Bead ID:");
                  if (target) {
                    const newDeps = [...(editForm.dependencies || []), { issue_id: isCreating ? editForm.id! : selectedBead!.id, depends_on_id: target, type: "blocks" }];
                    setEditForm({...editForm, dependencies: newDeps as any});
                  }
                }}
                className="text-[11px] font-black text-indigo-700 dark:text-indigo-400 hover:text-indigo-800 flex items-center gap-3 bg-indigo-500/10 px-5 py-3 rounded-2xl border-2 border-indigo-500/20 transition-all hover:bg-indigo-500/20 active:scale-95 uppercase tracking-widest"
              >
                <Plus size={16} strokeWidth={3} /> ADD
              </button>
            )}
          </div>
          <div className="flex flex-col gap-4">
            {(isCreating ? editForm.dependencies : selectedBead?.dependencies)?.map((d, i) => (
              <div key={i} className="flex items-center justify-between p-5 rounded-2xl bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] group hover:border-indigo-500 shadow-sm transition-all">
                <div className="flex flex-col gap-2">
                  <div className="flex items-center gap-4">
                     <span className="text-[11px] font-black font-mono text-indigo-700 dark:text-indigo-400 bg-indigo-500/10 px-3 py-1 rounded-lg border-2 border-indigo-500/20 shadow-sm">{d.depends_on_id}</span>
                     <span className="text-[10px] uppercase font-black text-[var(--text-primary)] tracking-[0.2em]">{d.type}</span>
                  </div>
                  {d.metadata && Object.keys(d.metadata).length > 0 && (
                    <div className="flex flex-wrap gap-2 mt-1 pl-1">
                      {Object.entries(d.metadata).map(([k, v]) => (
                        <span key={k} className="text-[9px] font-black bg-[var(--background-tertiary)] px-2.5 py-1 rounded-lg text-[var(--text-muted)] border-2 border-[var(--border-primary)]/50 font-mono tracking-tight">
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
                    className="text-rose-600 dark:text-rose-400 hover:bg-rose-500/10 p-3 rounded-xl transition-colors border-2 border-transparent hover:border-rose-500/20 active:scale-90"
                  >
                    <Trash2 size={20} strokeWidth={3} />
                  </button>
                )}
              </div>
            )) || <div className="text-[13px] text-[var(--text-muted)] font-black italic pl-1">None</div>}
          </div>
        </section>
      </div>
      
      <div className="p-8 border-t-2 border-[var(--border-primary)] bg-[var(--background-secondary)] flex gap-5 shadow-2xl backdrop-blur-md">
        {(isEditing || isCreating) ? (
          <>
            <button onClick={() => { setIsEditing(false); setIsCreating(false); }} className="flex-1 py-4 rounded-2xl bg-[var(--background-tertiary)] hover:bg-[var(--border-primary)] text-[var(--text-primary)] text-xs font-black transition-all border-2 border-[var(--border-primary)] active:scale-95 uppercase tracking-widest">Cancel</button>
            <button onClick={isCreating ? handleSaveCreate : handleSaveEdit} className="flex-1 py-4 rounded-2xl bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-black transition-all shadow-xl shadow-emerald-600/30 flex items-center justify-center gap-3 active:scale-95 border-2 border-emerald-700 uppercase tracking-widest"><Save size={20} strokeWidth={3} /> {isCreating ? "Create Bead" : "Save Changes"}</button>
          </>
        ) : (
          <button onClick={handleStartEdit} className="w-full py-4 rounded-2xl bg-indigo-700 hover:bg-indigo-600 text-white text-xs font-black transition-all shadow-xl shadow-indigo-700/30 flex items-center justify-center gap-3 active:scale-95 border-2 border-indigo-800 uppercase tracking-widest"><Edit3 size={20} strokeWidth={3} /> Edit Bead</button>
        )}
      </div>
    </div>
  );
};
