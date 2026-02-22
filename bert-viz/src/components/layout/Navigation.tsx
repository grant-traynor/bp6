import { useState } from "react";
import { TableProperties, Settings, GanttChart, Palette, MessageSquare, Sparkles, Plus, Terminal, Shield, Wrench, Construction, Layers, CheckSquare } from "lucide-react";
import brandLogo from "../../assets/brand_logo_1.svg";
import { cn } from "../../utils";
import { PersonaMenu, PersonaOption } from "./PersonaMenu";

export type ViewType = 'gantt' | 'list' | 'terminal' | 'settings';

interface NavigationProps {
  currentView: ViewType;
  onViewChange: (view: ViewType) => void;
  onOpenChat: (persona: string, task?: string, beadId?: string, role?: string) => void;
  onOpenPalettePreview: () => void;
  selectedBeadId?: string;
}

const PERSONAS: PersonaOption[] = [
  {
    id: "product-manager",
    label: "Product Manager",
    icon: "🤖",
    color: "text-[var(--accent-primary)]",
    borderColor: "border-[var(--accent-primary)]",
    hoverBgColor: "hover:bg-[var(--accent-primary)]",
    bgColor: "bg-[var(--accent-primary)] text-white",
    tasks: [
      { id: "chat", label: "Chat", icon: <MessageSquare size={14} /> },
      { id: "decompose", label: "Decompose", icon: <Sparkles size={14} /> },
      { id: "extend", label: "Extend", icon: <Plus size={14} /> },
      { id: "implement", label: "Implement", icon: <Terminal size={14} /> },
    ],
  },
  {
    id: "qa-engineer",
    label: "QA Engineer",
    icon: "🛡️",
    color: "text-emerald-600 dark:text-emerald-400",
    borderColor: "border-emerald-500/30",
    hoverBgColor: "hover:bg-emerald-500/10",
    bgColor: "bg-emerald-500/10 text-emerald-600",
    tasks: [
      { id: "chat", label: "Chat", icon: <MessageSquare size={14} /> },
      { id: "fix_dependencies", label: "Fix Dependencies", icon: <Wrench size={14} /> },
      { id: "process-improvement", label: "Process Improvement", icon: <Wrench size={14} /> },
    ],
  },
  {
    id: "qc-engineer",
    label: "QC Engineer",
    icon: "📊",
    color: "text-teal-600 dark:text-teal-400",
    borderColor: "border-teal-500/30",
    hoverBgColor: "hover:bg-teal-500/10",
    bgColor: "bg-teal-500/10 text-teal-600",
    tasks: [
      { id: "chat", label: "Chat", icon: <MessageSquare size={14} /> },
      { id: "collect-metrics", label: "Collect Metrics", icon: <Wrench size={14} /> },
      { id: "generate-report", label: "Generate Report", icon: <Wrench size={14} /> },
      { id: "guide-test", label: "Guide Test", icon: <CheckSquare size={14} /> },
    ],
  },
  {
    id: "architect",
    label: "Architect",
    icon: "🏗️",
    color: "text-purple-600 dark:text-purple-400",
    borderColor: "border-purple-500/30",
    hoverBgColor: "hover:bg-purple-500/10",
    bgColor: "bg-purple-500/10 text-purple-600",
    tasks: [
      { id: "chat", label: "Chat", icon: <MessageSquare size={14} /> },
      { id: "establish", label: "Establish Epic", icon: <Construction size={14} /> },
    ],
  },
  {
    id: "orchestrator",
    label: "Orchestrator",
    icon: "🎯",
    color: "text-indigo-600 dark:text-indigo-400",
    borderColor: "border-indigo-500/30",
    hoverBgColor: "hover:bg-indigo-500/10",
    bgColor: "bg-indigo-500/10 text-indigo-600",
    tasks: [
      { id: "chat", label: "Chat", icon: <MessageSquare size={14} /> },
      { id: "coordinate", label: "Coordinate", icon: <Layers size={14} /> },
    ],
  },
  {
    id: "customer",
    label: "Customer Voice",
    icon: "👤",
    color: "text-[var(--accent-violet)]",
    borderColor: "border-[var(--accent-violet)]",
    hoverBgColor: "hover:bg-[var(--accent-violet)]",
    bgColor: "bg-[var(--accent-violet)] text-white",
    tasks: [
      { id: "chat", label: "Chat", icon: <MessageSquare size={14} /> },
      { id: "bootstrap", label: "Bootstrap", icon: <Layers size={14} /> },
      { id: "refinement", label: "Refinement", icon: <Sparkles size={14} /> },
    ],
  },
  {
    id: "flutter",
    label: "Flutter Specialist",
    icon: "📱",
    color: "text-blue-600 dark:text-blue-400",
    borderColor: "border-blue-500/30",
    hoverBgColor: "hover:bg-blue-500/10",
    bgColor: "bg-blue-500/10 text-blue-600",
    tasks: [
      { id: "chat", label: "Chat", icon: <MessageSquare size={14} /> },
      { id: "implement", label: "Implement", icon: <Terminal size={14} /> },
      { id: "review", label: "Review", icon: <Shield size={14} /> },
    ],
  },
  {
    id: "tauri",
    label: "Tauri Specialist",
    icon: "🦀",
    color: "text-orange-600 dark:text-orange-400",
    borderColor: "border-orange-500/30",
    hoverBgColor: "hover:bg-orange-500/10",
    bgColor: "bg-orange-500/10 text-orange-600",
    tasks: [
      { id: "chat", label: "Chat", icon: <MessageSquare size={14} /> },
      { id: "implement", label: "Implement", icon: <Terminal size={14} /> },
      { id: "review", label: "Review", icon: <Shield size={14} /> },
    ],
  },
  {
    id: "supabase-db",
    label: "Supabase DB",
    icon: "🐘",
    color: "text-[var(--accent-violet)]",
    borderColor: "border-[var(--accent-violet)]",
    hoverBgColor: "hover:bg-[var(--accent-violet)]",
    bgColor: "bg-[var(--accent-violet)] text-white",
    tasks: [
      { id: "chat", label: "Chat", icon: <MessageSquare size={14} /> },
      { id: "implement", label: "Implement", icon: <Terminal size={14} /> },
      { id: "review", label: "Review", icon: <Shield size={14} /> },
    ],
  },
  {
    id: "supabase-edge",
    label: "Supabase Edge",
    icon: "⚡",
    color: "text-yellow-600 dark:text-yellow-400",
    borderColor: "border-yellow-500/30",
    hoverBgColor: "hover:bg-yellow-500/10",
    bgColor: "bg-yellow-500/10 text-yellow-600",
    tasks: [
      { id: "chat", label: "Chat", icon: <MessageSquare size={14} /> },
      { id: "implement", label: "Implement", icon: <Terminal size={14} /> },
      { id: "review", label: "Review", icon: <Shield size={14} /> },
    ],
  },
];

export const Navigation = ({ currentView, onViewChange, onOpenChat, onOpenPalettePreview, selectedBeadId }: NavigationProps) => {
  const [activePersona, setActivePersona] = useState<string | null>(null);

  const handleSelectTask = (personaId: string, taskId: string) => {
    // For specialist roles (flutter, tauri, supabase-db, supabase-edge),
    // we need to pass the role to onOpenChat
    const specialistRoles = ["flutter", "tauri", "supabase-db", "supabase-edge"];
    
    if (specialistRoles.includes(personaId)) {
      onOpenChat("specialist", taskId, selectedBeadId, personaId);
    } else {
      onOpenChat(personaId, taskId, selectedBeadId);
    }
  };

  return (
    <nav className="w-16 flex flex-col items-center py-6 border-r border-[var(--border-primary)] bg-[var(--background-secondary)] z-30 shadow-xl">
      <div className="flex flex-col gap-6 flex-1 items-center">
        <div className="w-12 h-12 p-2 rounded-xl border-2 border-[var(--border-primary)] bg-[var(--background-primary)] shadow-md flex items-center justify-center" title="Pairti">
          <img src={brandLogo} alt="Pairti" className="w-full h-full object-contain" style={{ filter: 'var(--logo-filter)' }} />
        </div>

        <button
          onClick={() => onViewChange('gantt')}
          className={cn(
            "p-3 rounded-xl transition-all border-2 shadow-md hover:shadow-lg active:scale-95",
            currentView === 'gantt'
              ? "bg-[var(--accent-primary)] text-white border-[var(--accent-primary)] shadow-[0_8px_30px_rgba(15,139,255,0.25)]"
              : "bg-[var(--background-primary)] text-[var(--text-muted)] border-[var(--border-primary)] hover:text-[var(--accent-primary)]"
          )}
          title="Gantt View"
        >
          <GanttChart size={24} strokeWidth={2.5} />
        </button>

        <button
          onClick={() => onViewChange('list')}
          className={cn(
            "p-3 rounded-xl transition-all border-2 shadow-md hover:shadow-lg active:scale-95",
            currentView === 'list'
              ? "bg-[var(--accent-primary)] text-white border-[var(--accent-primary)] shadow-[0_8px_30px_rgba(15,139,255,0.25)]"
              : "bg-[var(--background-primary)] text-[var(--text-muted)] border-[var(--border-primary)] hover:text-[var(--accent-primary)]"
          )}
          title="List View"
        >
          <TableProperties size={24} strokeWidth={2.5} />
        </button>

        <button
          onClick={() => onViewChange('terminal')}
          className={cn(
            "p-3 rounded-xl transition-all border-2 shadow-md hover:shadow-lg active:scale-95",
            currentView === 'terminal'
              ? "bg-[var(--accent-primary)] text-white border-[var(--accent-primary)] shadow-[0_8px_30px_rgba(15,139,255,0.25)]"
              : "bg-[var(--background-primary)] text-[var(--text-muted)] border-[var(--border-primary)] hover:text-[var(--accent-primary)]"
          )}
          title="Terminal View"
        >
          <Terminal size={24} strokeWidth={2.5} />
        </button>

        <div className="h-px w-8 bg-[var(--border-primary)] mx-auto" />

        {/* Persona Menu Section */}
        <div className="flex flex-col gap-3 items-center">
          <PersonaMenu
            personas={PERSONAS}
            onSelectTask={handleSelectTask}
            activePersona={activePersona}
            setActivePersona={setActivePersona}
          />
        </div>

        <div className="h-px w-8 bg-[var(--border-primary)] mx-auto" />
      </div>
      <div className="mt-auto border-t border-[var(--border-primary)] pt-4 flex flex-col items-center gap-3">
        <button
          onClick={onOpenPalettePreview}
          className="p-3 rounded-xl border-2 border-[var(--border-primary)] text-[var(--text-primary)] hover:text-[var(--accent-primary)] hover:border-[var(--accent-primary)] transition-colors shadow-sm"
          title="Open Palette Preview"
        >
          <Palette size={20} strokeWidth={2.5} />
        </button>
        <button
          onClick={() => onViewChange('settings')}
          className={cn(
            "p-3 rounded-xl transition-all border-2 shadow-md hover:shadow-lg active:scale-95",
            currentView === 'settings'
              ? "bg-[var(--accent-primary)] text-white border-[var(--accent-primary)] shadow-[0_8px_30px_rgba(15,139,255,0.25)]"
              : "bg-[var(--background-primary)] text-[var(--text-primary)] border-[var(--border-primary)] hover:text-[var(--accent-primary)]"
          )}
          title="Settings"
        >
          <Settings size={22} strokeWidth={2.5} />
        </button>
      </div>
    </nav>
  );
};
