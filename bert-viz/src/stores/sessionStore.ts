import { create } from 'zustand';
import { SessionInfo, listActiveSessions, onSessionListChanged, UnlistenFn } from '../api';

interface SessionStore {
  // State
  sessions: SessionInfo[];
  isInitialized: boolean;

  // Actions
  setSessions: (sessions: SessionInfo[]) => void;
  loadSessions: () => Promise<void>;
  initializeStore: () => Promise<UnlistenFn | undefined>;
  cleanup: () => void;
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

// Store instance
export const useSessionStore = create<SessionStore>((set, get) => ({
  // Initial state
  sessions: [],
  isInitialized: false,

  // Actions
  setSessions: (sessions) => set({ sessions }),

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

  // Initialize store: load sessions and set up event listener
  initializeStore: async () => {
    // Load initial sessions
    await get().loadSessions();

    // Set up event listener (single source of truth)
    const unlisten = await onSessionListChanged((sessionList) => {
      console.log('🎉 session-list-changed received in store:', sessionList);
      set({ sessions: sessionList || [] });
    });

    set({ isInitialized: true });
    console.log('✅ Session store initialized with event listener');

    return unlisten;
  },

  cleanup: () => {
    set({ sessions: [], isInitialized: false });
  }
}));
