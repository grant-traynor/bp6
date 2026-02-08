import { cn } from "../../utils";

export const getChipStyles = (label: string) => {
  const l = label.toLowerCase();
  if (l === 'epic') return "bg-purple-500/20 text-purple-800 dark:text-purple-300 border-purple-500/40";
  if (l === 'bug') return "bg-rose-500/20 text-rose-800 dark:text-rose-300 border-rose-500/40";
  if (l === 'feature') return "bg-emerald-500/20 text-emerald-800 dark:text-emerald-300 border-emerald-500/40";
  if (l === 'task') return "bg-indigo-500/20 text-indigo-800 dark:text-indigo-300 border-indigo-500/40";
  if (l.includes('infra')) return "bg-amber-500/20 text-amber-800 dark:text-amber-300 border-amber-500/40";
  if (l.includes('doc')) return "bg-cyan-500/20 text-cyan-800 dark:text-cyan-300 border-cyan-500/40";
  return "bg-[var(--background-tertiary)] text-[var(--text-primary)] border-[var(--border-primary)]";
};

export const Chip = ({ label }: { label: string }) => (
  <span className={cn("px-2 py-0.5 rounded-md text-[10px] font-black uppercase tracking-widest border transition-all", getChipStyles(label))}>
    {label}
  </span>
);
