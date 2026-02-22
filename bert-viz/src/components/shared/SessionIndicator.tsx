import { useState } from "react";
import { cn } from "../../utils";
import { PERSONA_ICONS, type SessionInfo } from "../../api";

interface SessionIndicatorProps {
  sessions: SessionInfo[];
  className?: string;
  onSessionClick?: (sessionId: string) => void;
}

export const SessionIndicator = ({ sessions, className, onSessionClick }: SessionIndicatorProps) => {
  const [isHovered, setIsHovered] = useState(false);

  if (!sessions || sessions.length === 0) return null;

  const activeSessions = sessions.filter(s => s.status === 'running');
  if (activeSessions.length === 0) return null;

  // Use the first active session's persona for the icon
  const primarySession = activeSessions[0];
  const icon = PERSONA_ICONS[primarySession.role || primarySession.persona] || '🤖';
  const count = activeSessions.length;

  const handleClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (count === 1 && onSessionClick) {
      onSessionClick(activeSessions[0].sessionId);
    }
    // For multiple sessions, clicking just shows/keeps the popover visible
  };

  const handleSessionRowClick = (e: React.MouseEvent, sessionId: string) => {
    e.stopPropagation();
    if (onSessionClick) {
      onSessionClick(sessionId);
    }
  };

  return (
    <div
      className={cn("relative flex items-center justify-center group/session", className)}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      onClick={handleClick}
      style={{ cursor: onSessionClick ? 'pointer' : 'default' }}
    >
      {/* Pulsing Dot */}
      <div className="w-2 h-2 rounded-full bg-green-500 animate-pulse-slow shadow-[0_0_8px_rgba(34,197,94,0.6)]" />

      {/* Persona Icon Overlay */}
      <div className="absolute -top-1 -right-1 text-[10px] bg-[var(--background-secondary)] rounded-full w-4 h-4 flex items-center justify-center border border-[var(--border-primary)] shadow-sm">
        {icon}
      </div>

      {/* Count Badge (if > 1) */}
      {count > 1 && (
        <div className="absolute -bottom-1 -left-1 text-[8px] font-black bg-[var(--accent-primary)] text-white rounded-full w-3.5 h-3.5 flex items-center justify-center border border-white dark:border-gray-900 shadow-sm">
          {count}
        </div>
      )}

      {/* Hover Popover */}
      {isHovered && (
        <div
          className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 z-50 min-w-[160px] max-w-[240px]"
          onClick={e => e.stopPropagation()}
        >
          <div className="bg-[var(--background-secondary)] border border-[var(--border-primary)] rounded-lg shadow-lg p-2 text-left">
            {activeSessions.map(session => {
              const sessionIcon = PERSONA_ICONS[session.role || session.persona] || '🤖';
              const personaLabel = (session.role || session.persona || '').replace(/-/g, ' ');
              const taskLabel = session.task || '';
              return (
                <div
                  key={session.sessionId}
                  className={cn(
                    "flex flex-col gap-0.5 px-2 py-1.5 rounded-md",
                    count > 1 && onSessionClick
                      ? "hover:bg-[var(--background-tertiary)] cursor-pointer transition-colors"
                      : ""
                  )}
                  onClick={count > 1 ? (e) => handleSessionRowClick(e, session.sessionId) : undefined}
                >
                  <div className="flex items-center gap-1.5">
                    <span className="text-[11px]">{sessionIcon}</span>
                    <span className="text-xs font-semibold text-[var(--text-primary)] capitalize truncate">
                      {personaLabel}
                    </span>
                  </div>
                  {taskLabel && (
                    <span className="text-[10px] text-[var(--text-secondary)] truncate pl-5">
                      {taskLabel}
                    </span>
                  )}
                </div>
              );
            })}
            <div className="mt-1 pt-1 border-t border-[var(--border-primary)]/50 px-2">
              <span className="text-[9px] text-[var(--text-muted)] uppercase tracking-wider">
                {count === 1 ? "Click to open chat" : "Click row to open chat"}
              </span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
