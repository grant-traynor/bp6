import { useRef, useEffect } from "react";
import { cn } from "../../utils";

export interface PersonaTask {
  id: string;
  label: string;
  icon?: React.ReactNode;
}

export interface PersonaOption {
  id: string;
  label: string;
  icon: string;
  color: string;
  borderColor: string;
  hoverBgColor: string;
  bgColor: string;
  tasks: PersonaTask[];
}

interface PersonaMenuProps {
  personas: PersonaOption[];
  onSelectTask: (personaId: string, taskId: string) => void;
  activePersona: string | null;
  setActivePersona: (id: string | null) => void;
}

export const PersonaMenu = ({
  personas,
  onSelectTask,
  activePersona,
  setActivePersona,
}: PersonaMenuProps) => {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        setActivePersona(null);
      }
    };

    if (activePersona) {
      document.addEventListener("mousedown", handleClickOutside);
    }

    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [activePersona, setActivePersona]);

  return (
    <div ref={menuRef} className="relative">
      {personas.map((persona) => (
        <div key={persona.id} className="relative">
          <button
            onClick={() =>
              setActivePersona(activePersona === persona.id ? null : persona.id)
            }
            className={cn(
              "w-full px-2 py-2.5 rounded-xl transition-all border-2 shadow-md hover:shadow-lg active:scale-95 flex flex-col items-center gap-0.5",
              activePersona === persona.id
                ? `${persona.bgColor} ${persona.color} border-[var(--accent-primary)]`
                : `bg-[var(--background-primary)] ${persona.color} ${persona.borderColor} hover:${persona.hoverBgColor}`
            )}
            title={persona.label}
          >
            <span className="text-xl leading-none">{persona.icon}</span>
            <span className="text-[8px] font-black uppercase tracking-wider">
              {persona.id.toUpperCase().slice(0, 3)}
            </span>
          </button>

          {activePersona === persona.id && (
            <div
              className={cn(
                "absolute left-full ml-2 top-0 w-48 bg-[var(--background-primary)] border-2 border-[var(--border-primary)] rounded-xl shadow-xl z-50 overflow-hidden"
              )}
            >
              <div className="px-3 py-2 border-b border-[var(--border-primary)]">
                <span className="text-xs font-black text-[var(--text-primary)] uppercase tracking-wider">
                  {persona.label}
                </span>
              </div>
              <div className="py-1">
                {persona.tasks.map((task) => (
                  <button
                    key={task.id}
                    onClick={() => {
                      onSelectTask(persona.id, task.id);
                      setActivePersona(null);
                    }}
                    className="w-full px-3 py-2 text-left text-sm font-medium text-[var(--text-primary)] hover:bg-[var(--background-secondary)] hover:text-[var(--accent-primary)] flex items-center gap-2 transition-colors"
                  >
                    {task.icon && <span>{task.icon}</span>}
                    {task.label}
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>
      ))}
    </div>
  );
};
