/**
 * Diagnostic utilities for session store debugging
 */

import { listActiveSessions } from '../api';
import { useSessionStore } from './sessionStore';

export async function diagnoseSessions() {
  console.log('=== SESSION DIAGNOSTICS ===');

  // 1. Check store state
  const storeState = useSessionStore.getState();
  console.log('Store initialized:', storeState.isInitialized);
  console.log('Store sessions:', storeState.sessions);

  // 2. Direct API call to backend
  try {
    const backendSessions = await listActiveSessions();
    console.log('Backend sessions (direct call):', backendSessions);
  } catch (error) {
    console.error('Failed to fetch backend sessions:', error);
  }

  console.log('=== END DIAGNOSTICS ===');
}

// Expose to window for manual testing
if (typeof window !== 'undefined') {
  (window as any).diagnoseSessions = diagnoseSessions;
}
