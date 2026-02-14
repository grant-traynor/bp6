import { useState, useEffect } from 'react';
import { X, ExternalLink } from 'lucide-react';
import { SessionInfo, getPersonaIcon, formatSessionRuntime, createSessionWindow } from '../../api';
import { cn } from '../../utils';

interface SessionItemProps {
  session: SessionInfo;
  isActive: boolean;
  beadTitle: string;
  onSelect: (sessionId: string, beadId: string | null) => void;
  onTerminate: (sessionId: string) => void;
  className?: string;
}

export const SessionItem: React.FC<SessionItemProps> = ({
  session,
  isActive,
  beadTitle,
  onSelect,
  onTerminate,
  className
}) => {
  // Only render running sessions
  if (session.status !== 'running') {
    return null;
  }

  const [runtime, setRuntime] = useState(formatSessionRuntime(session.createdAt));

  // Update runtime every second
  useEffect(() => {
    const interval = setInterval(() => {
      setRuntime(formatSessionRuntime(session.createdAt));
    }, 1000);

    return () => clearInterval(interval);
  }, [session.createdAt]);

  const handleTerminate = (e: React.MouseEvent) => {
    e.stopPropagation(); // Don't trigger onSelect
    if (window.confirm(`Terminate session ${session.sessionId}? This will stop the agent.`)) {
      onTerminate(session.sessionId);
    }
  };

  const handleOpenInWindow = async (e: React.MouseEvent) => {
    e.stopPropagation(); // Don't trigger onSelect
    try {
      const windowLabel = await createSessionWindow(session.sessionId);
      console.log(`Opened session ${session.sessionId} in new window: ${windowLabel}`);
    } catch (error) {
      console.error('Failed to open session in new window:', error);
      alert(`Failed to open window: ${error}`);
    }
  };

  const handleSelect = () => {
    onSelect(session.sessionId, session.beadId);
  };

  const personaIcon = getPersonaIcon(session.persona);
  const personaLabel = session.persona.replace('_', ' ').toUpperCase();

  return (
    <div
      className={cn(
        'session-item',
        'group/session-item',
        'cursor-pointer',
        'transition-all',
        isActive && 'session-item-active',
        className
      )}
      onClick={handleSelect}
      role="button"
      tabIndex={0}
      aria-label={`Session for ${beadTitle} - ${personaLabel} - ${session.backendId}`}
      aria-current={isActive ? 'true' : 'false'}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          handleSelect();
        }
      }}
    >
      {/* Header: Persona icon + Bead title + Terminate button */}
      <div className="session-item-header">
        <div className="session-item-title">
          <span className="session-persona-icon" aria-hidden="true">
            {personaIcon}
          </span>
          <span className="session-persona-label">{personaLabel}</span>
          <span className="session-bead-separator">·</span>
          <span className="session-bead-title">{beadTitle}</span>
        </div>

        <div className="session-actions">
          <button
            className="session-window-btn"
            onClick={handleOpenInWindow}
            aria-label={`Open session ${session.sessionId} in new window`}
            title="Open in new window"
          >
            <ExternalLink size={14} />
          </button>
          <button
            className="session-terminate-btn"
            onClick={handleTerminate}
            aria-label={`Terminate session ${session.sessionId}`}
            title="Terminate session"
          >
            <X size={14} />
          </button>
        </div>
      </div>

      {/* Details: Backend + Runtime */}
      <div className="session-item-details">
        <span className="session-backend">{session.backendId}</span>
        <span className="session-runtime-separator">·</span>
        <span className="session-runtime">{runtime}</span>
      </div>

      {/* Status Badge */}
      <div className="session-item-status">
        <span className="session-status-badge">
          Running
        </span>
      </div>
    </div>
  );
};
