import { useState } from 'react';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { SessionItem } from './SessionItem';
import { BeadNode } from '../../api';
import { cn } from '../../utils';
import { useSessionStore } from '../../stores/sessionStore';

interface SessionListProps {
  activeSessionId: string | null;
  onSessionSelect: (sessionId: string, beadId: string | null) => void;
  onSessionTerminate: (sessionId: string) => void;
  beads?: Map<string, BeadNode>;  // For looking up bead titles
}

export const SessionList: React.FC<SessionListProps> = ({
  activeSessionId,
  onSessionSelect,
  onSessionTerminate,
  beads
}) => {
  // Get sessions from Zustand store (no local state or event listeners needed)
  const sessions = useSessionStore(state => state.sessions);
  const [isCollapsed, setIsCollapsed] = useState(false);

  // Get bead title helper
  const getBeadTitle = (beadId: string | null): string => {
    if (!beadId) return 'No Bead';
    return beads?.get(beadId)?.title || beadId;
  };

  const toggleCollapse = () => {
    setIsCollapsed(!isCollapsed);
  };

  // Safety check: ensure sessions is always an array
  const safeSessions = sessions || [];

  return (
    <div className={cn('session-list', isCollapsed && 'session-list-collapsed')}>
      <div className="session-list-header">
        <span>Active Sessions ({safeSessions.length})</span>
        <button
          onClick={toggleCollapse}
          aria-label={isCollapsed ? 'Expand session list' : 'Collapse session list'}
          title={isCollapsed ? 'Expand' : 'Collapse'}
        >
          {isCollapsed ? <ChevronRight size={16} /> : <ChevronLeft size={16} />}
        </button>
      </div>
      <div className="session-list-body">
        {safeSessions.length === 0 ? (
          <div className="session-list-empty">No active sessions</div>
        ) : (
          safeSessions.map(session => (
            <SessionItem
              key={session.sessionId}
              session={session}
              isActive={session.sessionId === activeSessionId}
              beadTitle={getBeadTitle(session.beadId)}
              onSelect={onSessionSelect}
              onTerminate={onSessionTerminate}
            />
          ))
        )}
      </div>
    </div>
  );
};
