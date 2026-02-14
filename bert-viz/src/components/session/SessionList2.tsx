import { useState, memo, useMemo } from 'react';
import { ChevronLeft, ChevronRight } from 'lucide-react';
import { SessionItem2 } from './SessionItem2';
import { BeadNode } from '../../api';
import { cn } from '../../utils';
import { useSessionStore } from '../../stores/sessionStore';

interface SessionList2Props {
  activeSessionId: string | null;
  onSessionSelect: (sessionId: string, beadId: string | null) => void;
  onSessionTerminate: (sessionId: string) => void;
  beads?: Map<string, BeadNode>;
}

/**
 * SessionList2 - Clean rewrite with proper architecture
 *
 * Features:
 * - Uses Zustand store (useSessionStore) for reactive session data
 * - Collapsible sidebar with chevron toggle
 * - Displays running sessions only
 * - Memoized to prevent unnecessary re-renders
 * - Clean event handlers (no inline functions)
 * - Proper TypeScript interfaces
 */
const SessionList2Component: React.FC<SessionList2Props> = ({
  activeSessionId,
  onSessionSelect,
  onSessionTerminate,
  beads
}) => {
  const sessions = useSessionStore(state => state.sessions);
  const [isCollapsed, setIsCollapsed] = useState(false);

  // Memoize bead title lookup to avoid recalculation on every render
  const getBeadTitle = useMemo(() => {
    return (beadId: string | null): string => {
      if (!beadId) return 'No Bead';
      return beads?.get(beadId)?.title || beadId;
    };
  }, [beads]);

  const handleToggleCollapse = () => {
    setIsCollapsed(prev => !prev);
  };

  // Filter running sessions only
  const runningSessions = useMemo(() => {
    return sessions.filter(s => s.status === 'running');
  }, [sessions]);

  return (
    <div className={cn('session-list', isCollapsed && 'session-list-collapsed')}>
      {/* Header */}
      <div className="session-list-header">
        <span>Active Sessions ({runningSessions.length})</span>
        <button
          onClick={handleToggleCollapse}
          aria-label={isCollapsed ? 'Expand session list' : 'Collapse session list'}
          title={isCollapsed ? 'Expand' : 'Collapse'}
        >
          {isCollapsed ? <ChevronRight size={16} /> : <ChevronLeft size={16} />}
        </button>
      </div>

      {/* Body - Session List */}
      <div className="session-list-body">
        {runningSessions.length === 0 ? (
          <div className="session-list-empty">No active sessions</div>
        ) : (
          runningSessions.map(session => (
            <SessionItem2
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

// Memoize to prevent unnecessary re-renders when parent updates
export const SessionList2 = memo(SessionList2Component);
