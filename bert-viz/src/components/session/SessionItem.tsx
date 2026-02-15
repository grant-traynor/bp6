import { useState, useEffect, memo, useCallback } from 'react';
import { X, ExternalLink, Hand } from 'lucide-react';
import { SessionInfo, getPersonaIcon, formatSessionRuntime, createSessionWindow, handoverToInteractive } from '../../api';
import { cn } from '../../utils';

/**
 * Format timestamp as relative time (e.g., "2m ago", "just now")
 */
function formatRelativeTime(timestamp: number): string {
  const now = Date.now() / 1000;
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

/**
 * SessionItem - Clean rewrite with proper architecture
 *
 * Features:
 * - Displays session metadata (persona, bead, runtime, last activity)
 * - Session selection and termination with confirmation
 * - Open in new window support
 * - Unread indicator with animation
 * - Auto-updating runtime and last activity (10s interval)
 * - Proper event bubbling control
 * - Clean TypeScript interfaces
 */
export const SessionItem = memo<SessionItemProps>(({
  session,
  isActive,
  beadTitle,
  onSelect,
  onTerminate,
  className
}) => {
  const [runtime, setRuntime] = useState(formatSessionRuntime(session.createdAt));
  const [lastActivity, setLastActivity] = useState(formatRelativeTime(session.lastActivity));
  const [isHandingOver, setIsHandingOver] = useState(false);

  // Update runtime and last activity every 10 seconds
  useEffect(() => {
    const interval = setInterval(() => {
      setRuntime(formatSessionRuntime(session.createdAt));
      setLastActivity(formatRelativeTime(session.lastActivity));
    }, 10000);

    return () => clearInterval(interval);
  }, [session.createdAt, session.lastActivity]);

  // Event handlers - memoized to prevent recreation
  const handleSelect = useCallback(() => {
    onSelect(session.sessionId, session.beadId);
  }, [onSelect, session.sessionId, session.beadId]);

  const handleTerminate = useCallback((e: React.MouseEvent) => {
    e.stopPropagation(); // Don't trigger onSelect
    if (window.confirm(`Terminate session ${session.sessionId}? This will stop the agent.`)) {
      onTerminate(session.sessionId);
    }
  }, [onTerminate, session.sessionId]);

  const handleOpenInWindow = useCallback(async (e: React.MouseEvent) => {
    e.stopPropagation(); // Don't trigger onSelect
    try {
      const windowLabel = await createSessionWindow(session.sessionId);
      console.log(`Opened session ${session.sessionId} in new window: ${windowLabel}`);
    } catch (error) {
      console.error('Failed to open session in new window:', error);
      alert(`Failed to open window: ${error}`);
    }
  }, [session.sessionId]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      handleSelect();
    }
  }, [handleSelect]);

  const handleTakeOver = useCallback(async (e: React.MouseEvent) => {
    e.stopPropagation(); // Don't trigger onSelect
    if (!window.confirm('Take over this headless session? This will clear the command queue and switch to interactive mode.')) {
      return;
    }

    setIsHandingOver(true);
    try {
      await handoverToInteractive(session.sessionId);
      console.log(`Handed over session ${session.sessionId} to interactive mode`);
    } catch (error) {
      console.error('Failed to handover session:', error);
      alert(`Failed to take over session: ${error}`);
    } finally {
      setIsHandingOver(false);
    }
  }, [session.sessionId]);

  const personaIcon = getPersonaIcon(session.persona);
  const personaLabel = session.persona.replace(/-/g, ' ').toUpperCase();
  const isHeadless = session.executionMode === 'headless';
  const commandsRemaining = session.commandsRemaining ?? 0;
  const commandsCompleted = session.totalCommands && session.commandsRemaining !== undefined && session.commandsRemaining !== null
    ? session.totalCommands - commandsRemaining
    : 0;
  const totalCommands = session.totalCommands || 0;

  return (
    <div
      className={cn(
        'session-item',
        'group/session-item',
        'cursor-pointer',
        'transition-colors duration-150',
        isActive && 'session-item-active',
        className
      )}
      onClick={handleSelect}
      onKeyDown={handleKeyDown}
      role="button"
      tabIndex={0}
      aria-label={`Session for ${beadTitle} - ${personaLabel} - ${session.backendId}`}
      aria-current={isActive ? 'true' : 'false'}
    >
      {/* Header: Persona icon + Activity indicator + Actions */}
      <div className="session-item-header">
        <div className="session-item-title">
          {/* Unread indicator with pulsing animation */}
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

          {/* Execution Mode Badge */}
          <span
            className={cn(
              "text-[10px] font-bold px-2 py-0.5 rounded",
              isHeadless
                ? "bg-amber-500 text-white"
                : "bg-emerald-500 text-white"
            )}
            title={isHeadless ? "Running in headless mode" : "Interactive mode"}
          >
            {isHeadless ? '🤖' : '💬'}
          </span>
        </div>

        {/* Action buttons */}
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

      {/* Bead ID and Title */}
      {session.beadId && (
        <div className="flex items-center gap-2 text-xs px-3 py-1">
          <span className="font-mono font-bold text-indigo-400">{session.beadId}</span>
          {beadTitle && beadTitle !== session.beadId && (
            <>
              <span className="text-slate-400">·</span>
              <span className="text-slate-300 truncate">{beadTitle}</span>
            </>
          )}
        </div>
      )}

      {/* Command Progress for Headless Sessions */}
      {isHeadless && totalCommands > 0 && (
        <div className="px-3 py-2 space-y-1">
          <div className="flex items-center justify-between text-xs">
            <span className="text-slate-400 font-bold">
              Command {commandsCompleted}/{totalCommands}
            </span>
            <span className="text-amber-400 font-bold">
              {session.commandsRemaining || 0} remaining
            </span>
          </div>
          <div className="w-full bg-slate-700 rounded-full h-1.5">
            <div
              className="bg-amber-500 h-1.5 rounded-full transition-all duration-300"
              style={{ width: `${(commandsCompleted / totalCommands) * 100}%` }}
            />
          </div>
        </div>
      )}

      {/* Take Over Button for Headless Sessions */}
      {isHeadless && (
        <div className="px-3 pb-2">
          <button
            onClick={handleTakeOver}
            disabled={isHandingOver}
            className={cn(
              "w-full px-3 py-1.5 rounded text-xs font-bold transition-all",
              "flex items-center justify-center gap-2",
              isHandingOver
                ? "bg-slate-600 text-slate-400 cursor-not-allowed"
                : "bg-indigo-600 hover:bg-indigo-700 text-white active:scale-95"
            )}
            title="Take control of this headless session"
          >
            <Hand size={12} />
            {isHandingOver ? 'Taking Over...' : 'Take Over'}
          </button>
        </div>
      )}

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

      {/* Message count badge */}
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

SessionItem.displayName = 'SessionItem';
