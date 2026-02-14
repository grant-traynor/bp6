import { cn } from "../../utils";
import { PERSONA_ICONS, type SessionInfo } from "../../api";

interface SessionIndicatorProps {
  sessions: SessionInfo[];
  className?: string;
}

export const SessionIndicator = ({ sessions, className }: SessionIndicatorProps) => {
  if (!sessions || sessions.length === 0) return null;

  const activeSessions = sessions.filter(s => s.status === 'running');
  if (activeSessions.length === 0) return null;

  // Use the first active session's persona for the icon
  const primarySession = activeSessions[0];
  const icon = PERSONA_ICONS[primarySession.persona] || '🤖';
  const count = activeSessions.length;

  const tooltipText = activeSessions
    .map(s => `${PERSONA_ICONS[s.persona] || '🤖'} ${s.persona.replace('_', ' ')}`)
    .join(', ');

  return (
    <div 
      className={cn("relative flex items-center justify-center group/session", className)}
      title={`${count} active session${count > 1 ? 's' : ''}: ${tooltipText}`}
    >
      {/* Pulsing Dot */}
      <div className="w-2 h-2 rounded-full bg-green-500 animate-pulse-slow shadow-[0_0_8px_rgba(34,197,94,0.6)]" />
      
      {/* Persona Icon Overlay */}
      <div className="absolute -top-1 -right-1 text-[10px] bg-[var(--background-secondary)] rounded-full w-4 h-4 flex items-center justify-center border border-[var(--border-primary)] shadow-sm">
        {icon}
      </div>

      {/* Count Badge (if > 1) */}
      {count > 1 && (
        <div className="absolute -bottom-1 -left-1 text-[8px] font-black bg-indigo-600 text-white rounded-full w-3.5 h-3.5 flex items-center justify-center border border-white dark:border-gray-900 shadow-sm">
          {count}
        </div>
      )}
    </div>
  );
};
