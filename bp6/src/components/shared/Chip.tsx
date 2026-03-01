import { cn } from "../../utils";

export const getChipStyles = (label: string | undefined) => {
  if (!label) return "bg-[var(--background-tertiary)] text-[var(--text-primary)] border-[var(--border-primary)]";
  const l = label.toLowerCase();
  if (l === 'epic') return "bg-[rgba(156,92,255,0.14)] text-[#9C5CFF] border-[rgba(156,92,255,0.35)]";
  if (l === 'bug') return "bg-[rgba(240,109,118,0.18)] text-[#F06D76] border-[rgba(240,109,118,0.35)]";
  if (l === 'feature') return "bg-[rgba(59,200,255,0.16)] text-[#3BC8FF] border-[rgba(59,200,255,0.35)]";
  if (l === 'task') return "bg-[rgba(15,139,255,0.12)] text-[#0F8BFF] border-[rgba(15,139,255,0.35)]";
  if (l.includes('infra')) return "bg-[rgba(242,169,59,0.16)] text-[#F2A93B] border-[rgba(242,169,59,0.35)]";
  if (l.includes('doc')) return "bg-[rgba(92,241,255,0.16)] text-[#5CF1FF] border-[rgba(92,241,255,0.35)]";
  return "bg-[var(--background-tertiary)] text-[var(--text-primary)] border-[var(--border-primary)]";
};

export const Chip = ({ label }: { label: string | undefined }) => (
  <span className={cn("px-2 py-0.5 rounded-md text-xs font-black uppercase tracking-widest border transition-all", getChipStyles(label))}>
    {label || 'unknown'}
  </span>
);
