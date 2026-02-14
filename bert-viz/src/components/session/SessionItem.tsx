import React, { useState, useEffect } from 'react';
import { X, ExternalLink } from 'lucide-react';
import { SessionInfo, getPersonaIcon, formatSessionRuntime, createSessionWindow } from '../../api';
import { cn } from '../../utils';

/**
 * Format timestamp as relative time (e.g., "2m ago", "just now")
 */
function formatRelativeTime(timestamp: number): string {
  const now = Date.now() / 1000; // Current time in seconds
  const diff = now - timestamp;

  if (diff < 10) return 'just now';
  if (diff < 60) return `${Math.floor(diff)}s ago`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  return `${Math.floor(diff / 86400)}d ago`;
}

interface SessionItemProps {
  session: SessionInfo;
  isActive: boolean;
  beadTitle: string;
  onSelect: (sessionId: string, beadId: string | null) => void;
  onTerminate: (sessionId: string) => void;
  className?: string;
}

export const SessionItem = React.memo<SessionItemProps>(({
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
  const [lastActivity, setLastActivity] = useState(formatRelativeTime(session.lastActivity));

  // Update runtime and last activity every 10 seconds (reduce flashing)
  useEffect(() => {
    const interval = setInterval(() => {
      setRuntime(formatSessionRuntime(session.createdAt));
      setLastActivity(formatRelativeTime(session.lastActivity));
    }, 10000); // 10 seconds instead of 1 second

    return () => clearInterval(interval);
  }, [session.createdAt, session.lastActivity]);

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
      {/* Header: Persona icon + Bead title + Activity indicator + Terminate button */}
      <div className="session-item-header">
        <div className="session-item-title">
          {/* Pulsing dot for unread indicator */}
          {session.hasUnread && (
            <span className="session-unread-dot" aria-label="Has unread messages" title="Unread activity">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-indigo-400 opacity-75"></span>
              <span className="relative inline-flex rounded-full h-2 w-2 bg-indigo-500"></span>
            </span>
          )}
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

      {/* Details: Backend + Runtime + Last Activity */}
      <div className="session-item-details">
        <span className="session-backend">{session.backendId}</span>
        <span className="session-runtime-separator">·</span>
        <span className="session-runtime">{runtime}</span>
        <span className="session-runtime-separator">·</span>
        <span className="session-last-activity" title={`Last activity: ${lastActivity}`}>
          {lastActivity}
        </span>
      </div>

      {/* Activity Badge: Message count */}
      {session.messageCount > 0 && (
        <div className="session-item-status">
          <span className="session-message-count-badge" title={`${session.messageCount} messages`}>
            {session.messageCount} {session.messageCount === 1 ? 'message' : 'messages'}
          </span>
        </div>
      )}
    </div>
  );
});
