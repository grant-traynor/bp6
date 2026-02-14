import { useState, useEffect } from 'react';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { SessionInfo, listActiveSessions, onSessionListChanged, UnlistenFn } from '../../api';
import { SessionItem } from './SessionItem';
import { BeadNode } from '../../api';
import { cn } from '../../utils';

interface SessionListProps {
  activeSessionId: string | null;
  onSessionSelect: (sessionId: string) => void;
  onSessionTerminate: (sessionId: string) => void;
  beads?: Map<string, BeadNode>;  // For looking up bead titles
}

export const SessionList: React.FC<SessionListProps> = ({
  activeSessionId,
  onSessionSelect,
  onSessionTerminate,
  beads
}) => {
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [isCollapsed, setIsCollapsed] = useState(false);

  // Get bead title helper
  const getBeadTitle = (beadId: string): string => {
    return beads?.get(beadId)?.title || beadId;
  };

  // Load initial sessions and subscribe to updates
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    // Load initial session list
    listActiveSessions().then(initialSessions => {
      setSessions(initialSessions);
    });

    // Subscribe to session list changes (event-driven)
    onSessionListChanged((updatedSessions) => {
      setSessions(updatedSessions);
    }).then(fn => {
      unlisten = fn;
    });

    // Cleanup: unsubscribe on unmount
    return () => {
      unlisten?.();
    };
  }, []);

  const toggleCollapse = () => {
    setIsCollapsed(!isCollapsed);
  };

  return (
    <div className={cn('session-list', isCollapsed && 'session-list-collapsed')}>
      <div className="session-list-header">
        <span>Active Sessions ({sessions.length})</span>
        <button
          onClick={toggleCollapse}
          aria-label={isCollapsed ? 'Expand session list' : 'Collapse session list'}
          title={isCollapsed ? 'Expand' : 'Collapse'}
        >
          {isCollapsed ? <ChevronRight size={16} /> : <ChevronLeft size={16} />}
        </button>
      </div>
      <div className="session-list-body">
        {sessions.length === 0 ? (
          <div className="session-list-empty">No active sessions</div>
        ) : (
          sessions.map(session => (
            <SessionItem
              key={session.session_id}
              session={session}
              isActive={session.session_id === activeSessionId}
              beadTitle={getBeadTitle(session.bead_id)}
              onSelect={onSessionSelect}
              onTerminate={onSessionTerminate}
            />
          ))
        )}
      </div>
    </div>
  );
};
