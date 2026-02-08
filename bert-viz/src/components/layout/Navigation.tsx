import { ListTree, Settings } from "lucide-react";

export const Navigation = () => (
  <nav className="w-16 flex flex-col items-center py-6 border-r border-[var(--border-primary)] bg-[var(--background-secondary)] z-30 shadow-xl">
    <div className="flex flex-col gap-4 flex-1">
      <button className="p-3 rounded-xl bg-[var(--background-primary)] text-indigo-700 dark:text-indigo-400 transition-all border-2 border-[var(--border-primary)] shadow-md hover:shadow-lg active:scale-95"><ListTree size={24} strokeWidth={2.5} /></button>
      <div className="h-px w-8 bg-[var(--border-primary)] mx-auto" />
    </div>
    <div className="mt-auto border-t border-[var(--border-primary)] pt-4">
      <button className="p-3 text-[var(--text-primary)] hover:text-indigo-600 transition-colors">
        <Settings size={22} strokeWidth={2.5} />
      </button>
    </div>
  </nav>
);
