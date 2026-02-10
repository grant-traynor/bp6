import { Save, CheckCircle, RotateCcw, User, Undo2 } from "lucide-react";
import { cn } from "../../../utils";

interface SidebarActionsProps {
  isEditing: boolean;
  isCreating: boolean;
  isDirty?: boolean;
  beadStatus?: string;
  onEdit: () => void;
  onCancel: () => void;
  onSave: () => void;
  onUndo?: () => void;
  onClaim?: () => void;
  onClose?: () => void;
  onReopen?: () => void;
  isValid?: boolean;
}

export const SidebarActions = ({
  isEditing: _isEditing,
  isCreating,
  isDirty,
  beadStatus: _beadStatus,
  onEdit: _onEdit,
  onCancel,
  onSave,
  onUndo,
  onClaim,
  onClose,
  onReopen,
  isValid = true
}: SidebarActionsProps) => {
  // Creating mode
  if (isCreating) {
    return (
      <div className="p-6 bg-[var(--background-secondary)] border-t-[var(--border-thick)] border-[var(--border-primary)] shadow-[var(--shadow-panel)] flex gap-4">
        <button
          onClick={onCancel}
          className="flex-1 px-4 py-3 rounded-xl border-2 border-[var(--border-primary)] font-black text-xs uppercase tracking-widest text-[var(--text-secondary)] hover:bg-[var(--background-tertiary)] active:scale-95 transition-all"
        >
          Cancel
        </button>
        <button
          onClick={() => {
            console.log('💾 SidebarActions: Create button clicked', { isValid });
            onSave();
          }}
          disabled={!isValid}
          className={cn(
            "flex-1 px-4 py-3 rounded-xl font-black text-xs uppercase tracking-widest text-white shadow-lg active:scale-95 transition-all flex items-center justify-center gap-2",
            isValid ? "bg-indigo-600 hover:bg-indigo-700 shadow-indigo-500/20" : "bg-indigo-400 cursor-not-allowed opacity-50"
          )}
        >
          <Save size={16} strokeWidth={3} />
          Create
        </button>
      </div>
    );
  }

  // Unified view mode with dirty state
  if (isDirty) {
    return (
      <div className="p-6 bg-[var(--background-secondary)] border-t-[var(--border-thick)] border-[var(--border-primary)] shadow-[var(--shadow-panel)] flex gap-4">
        <button
          onClick={onUndo}
          className="flex-1 px-4 py-3 rounded-xl border-2 border-[var(--border-primary)] font-black text-xs uppercase tracking-widest text-[var(--text-secondary)] hover:bg-[var(--background-tertiary)] active:scale-95 transition-all flex items-center justify-center gap-2"
        >
          <Undo2 size={16} strokeWidth={3} />
          Undo
        </button>
        <button
          onClick={onSave}
          disabled={!isValid}
          className={cn(
            "flex-1 px-4 py-3 rounded-xl font-black text-xs uppercase tracking-widest text-white shadow-lg active:scale-95 transition-all flex items-center justify-center gap-2",
            isValid ? "bg-indigo-600 hover:bg-indigo-700 shadow-indigo-500/20" : "bg-indigo-400 cursor-not-allowed opacity-50"
          )}
        >
          <Save size={16} strokeWidth={3} />
          Save
        </button>
      </div>
    );
  }

  // Clean view mode - status transition buttons only
  return (
    <div className="p-6 bg-[var(--background-secondary)] border-t-[var(--border-thick)] border-[var(--border-primary)] shadow-[var(--shadow-panel)] flex gap-4">
      {onClaim && (
        <button
          onClick={onClaim}
          className="flex-1 px-6 py-4 border-2 border-[var(--border-primary)] text-indigo-600 dark:text-indigo-400 hover:bg-indigo-500/10 rounded-2xl transition-all active:scale-95 border-indigo-500/20 font-black text-xs uppercase tracking-[0.2em] flex items-center justify-center gap-2"
          title="Claim bead (mark as in progress)"
        >
          <User size={20} strokeWidth={2.5} />
          Claim
        </button>
      )}
      {onClose && (
        <button
          onClick={onClose}
          className="flex-1 px-6 py-4 border-2 border-[var(--border-primary)] text-emerald-600 dark:text-emerald-400 hover:bg-emerald-500/10 rounded-2xl transition-all active:scale-95 border-emerald-500/20 font-black text-xs uppercase tracking-[0.2em] flex items-center justify-center gap-2"
          title="Close bead (mark as complete)"
        >
          <CheckCircle size={20} strokeWidth={2.5} />
          Close
        </button>
      )}
      {onReopen && (
        <button
          onClick={onReopen}
          className="flex-1 px-6 py-4 border-2 border-[var(--border-primary)] text-amber-600 dark:text-amber-400 hover:bg-amber-500/10 rounded-2xl transition-all active:scale-95 border-amber-500/20 font-black text-xs uppercase tracking-[0.2em] flex items-center justify-center gap-2"
          title="Reopen bead"
        >
          <RotateCcw size={20} strokeWidth={2.5} />
          Reopen
        </button>
      )}
    </div>
  );
};
