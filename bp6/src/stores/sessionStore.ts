import { create } from 'zustand';
import { listen } from '@tauri-apps/api/event';
import { SessionInfo, listActiveSessions, onSessionListChanged, onSessionsDiscovered, UnlistenFn } from '../api';

interface SessionStore {
  // State
  sessions: SessionInfo[];
  discoveredSessions: SessionInfo[];
  isInitialized: boolean;
  // bp6-ldgi: Track which sessions have had their first agent message
  agentReadySessions: Set<string>;

  // Actions
  setSessions: (sessions: SessionInfo[]) => void;
  setDiscoveredSessions: (sessions: SessionInfo[]) => void;
  loadSessions: () => Promise<void>;
  refreshSessions: () => Promise<void>;  // Manual refresh
  initializeStore: () => Promise<UnlistenFn | undefined>;
  cleanup: () => void;
  // bp6-ldgi: Mark a session as agent-ready
  setAgentReady: (sessionId: string) => void;
  isAgentReady: (sessionId: string) => boolean;
}

// Helper function to group sessions by bead ID (use in components with useMemo)
export const groupSessionsByBead = (sessions: SessionInfo[]): Record<string, SessionInfo[]> => {
  const grouped: Record<string, SessionInfo[]> = {};
  sessions.forEach(s => {
    const key = s.beadId || 'untracked';
    if (!grouped[key]) grouped[key] = [];
    grouped[key].push(s);
  });
  return grouped;
};

/**
 * Merge running sessions and discovered historical sessions.
 * Running sessions take precedence — if a discovered session has the same
 * sessionId as a running session, the running one wins (dedup by sessionId).
 */
export const getAllSessions = (
  running: SessionInfo[],
  discovered: SessionInfo[]
): SessionInfo[] => {
  const runningIds = new Set(running.map(s => s.sessionId));
  const historical = discovered.filter(s => !runningIds.has(s.sessionId));
  return [...running, ...historical];
};

// Store instance
export const useSessionStore = create<SessionStore>((set, get) => ({
  // Initial state
  sessions: [],
  discoveredSessions: [],
  isInitialized: false,
  agentReadySessions: new Set<string>(),

  // Actions
  setSessions: (sessions) => {
    console.log('📝 setSessions called with:', sessions);
    set({ sessions });
  },

  setDiscoveredSessions: (sessions) => {
    set({ discoveredSessions: sessions });
  },

  loadSessions: async () => {
    try {
      const sessionList = await listActiveSessions();
      console.log('📊 Sessions loaded into store:', sessionList);
      set({ sessions: sessionList || [] });
    } catch (error) {
      console.error('Failed to load sessions:', error);
      set({ sessions: [] });
    }
  },

  refreshSessions: async () => {
    console.log('🔄 Manually refreshing sessions...');
    await get().loadSessions();
  },

  // Initialize store: load sessions and set up event listeners
  initializeStore: async () => {
    // Load initial sessions
    await get().loadSessions();

    // Set up event listener for running sessions (single source of truth)
    const unlisten = await onSessionListChanged((sessionList) => {
      console.log('🎉 session-list-changed received in store:', sessionList);
      set({ sessions: sessionList || [] });
    });

    // Listen for startup-discovered historical sessions
    onSessionsDiscovered((discovered) => {
      console.log('🔍 sessions-discovered received:', discovered.length, 'sessions');
      set({ discoveredSessions: discovered || [] });
    });

    // bp6-ldgi: Listen for agent-session-ready events — refresh sessions so messageCount is current
    listen<{ session_id: string }>('agent-session-ready', (event) => {
      const sessionId = event.payload.session_id;
      console.log('🟢 agent-session-ready received for session:', sessionId);
      get().setAgentReady(sessionId);
      get().refreshSessions();
    });

    set({ isInitialized: true });
    console.log('✅ Session store initialized with event listener');

    return unlisten;
  },

  cleanup: () => {
    set({ sessions: [], discoveredSessions: [], isInitialized: false, agentReadySessions: new Set<string>() });
  },

  // bp6-ldgi: Mark a session as agent-ready
  setAgentReady: (sessionId: string) => {
    set((state) => {
      const next = new Set(state.agentReadySessions);
      next.add(sessionId);
      return { agentReadySessions: next };
    });
  },

  // bp6-ldgi: Check if a session is agent-ready
  isAgentReady: (sessionId: string) => {
    return get().agentReadySessions.has(sessionId);
  },
}));
