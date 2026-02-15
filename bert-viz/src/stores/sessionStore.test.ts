import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useSessionStore, groupSessionsByBead } from './sessionStore';
import type { SessionInfo } from '../api';

// Mock the API functions
vi.mock('../api', () => ({
  listActiveSessions: vi.fn(),
  onSessionListChanged: vi.fn(),
}));

describe('sessionStore', () => {
  beforeEach(() => {
    // Reset store before each test
    useSessionStore.getState().cleanup();
  });

  it('should initialize with empty sessions', () => {
    const { sessions } = useSessionStore.getState();
    expect(sessions).toEqual([]);
  });

  it('should update sessions when setSessions is called', () => {
    const mockSessions: SessionInfo[] = [
      {
        sessionId: 'session-1',
        beadId: 'bp6-123',
        persona: 'product-manager',
        backendId: 'claude',
        status: 'running',
          executionMode: 'interactive',
        createdAt: Date.now(),
        lastActivity: Date.now(),
        hasUnread: false,
        messageCount: 0,
      },
      {
        sessionId: 'session-2',
        beadId: 'bp6-456',
        persona: 'engineer',
        backendId: 'claude',
        status: 'running',
          executionMode: 'interactive',
        createdAt: Date.now(),
        lastActivity: Date.now(),
        hasUnread: false,
        messageCount: 0,
      },
    ];

    useSessionStore.getState().setSessions(mockSessions);

    const { sessions } = useSessionStore.getState();
    expect(sessions).toEqual(mockSessions);
    expect(sessions).toHaveLength(2);
  });

  it('should group sessions by bead ID correctly', () => {
    const mockSessions: SessionInfo[] = [
      {
        sessionId: 'session-1',
        beadId: 'bp6-123',
        persona: 'product-manager',
        backendId: 'claude',
        status: 'running',
          executionMode: 'interactive',
        createdAt: Date.now(),
        lastActivity: Date.now(),
        hasUnread: false,
        messageCount: 0,
      },
      {
        sessionId: 'session-2',
        beadId: 'bp6-123',
        persona: 'engineer',
        backendId: 'claude',
        status: 'running',
          executionMode: 'interactive',
        createdAt: Date.now(),
        lastActivity: Date.now(),
        hasUnread: false,
        messageCount: 0,
      },
      {
        sessionId: 'session-3',
        beadId: 'bp6-456',
        persona: 'designer',
        backendId: 'claude',
        status: 'running',
          executionMode: 'interactive',
        createdAt: Date.now(),
        lastActivity: Date.now(),
        hasUnread: false,
        messageCount: 0,
      },
      {
        sessionId: 'session-4',
        beadId: null,
        persona: 'researcher',
        backendId: 'claude',
        status: 'running',
          executionMode: 'interactive',
        createdAt: Date.now(),
        lastActivity: Date.now(),
        hasUnread: false,
        messageCount: 0,
      },
    ];

    useSessionStore.getState().setSessions(mockSessions);

    const sessionsByBead = groupSessionsByBead(useSessionStore.getState().sessions);

    // bp6-123 should have 2 sessions
    expect(sessionsByBead['bp6-123']).toHaveLength(2);
    expect(sessionsByBead['bp6-123'][0].sessionId).toBe('session-1');
    expect(sessionsByBead['bp6-123'][1].sessionId).toBe('session-2');

    // bp6-456 should have 1 session
    expect(sessionsByBead['bp6-456']).toHaveLength(1);
    expect(sessionsByBead['bp6-456'][0].sessionId).toBe('session-3');

    // Untracked (null beadId) should have 1 session
    expect(sessionsByBead['untracked']).toHaveLength(1);
    expect(sessionsByBead['untracked'][0].sessionId).toBe('session-4');
  });

  it('should handle empty sessions in sessionsByBead', () => {
    const sessionsByBead = groupSessionsByBead(useSessionStore.getState().sessions);
    expect(sessionsByBead).toEqual({});
  });

  it('should cleanup and reset state', () => {
    const mockSessions: SessionInfo[] = [
      {
        sessionId: 'session-1',
        beadId: 'bp6-123',
        persona: 'product-manager',
        backendId: 'claude',
        status: 'running',
          executionMode: 'interactive',
        createdAt: Date.now(),
        lastActivity: Date.now(),
        hasUnread: false,
        messageCount: 0,
      },
    ];

    useSessionStore.getState().setSessions(mockSessions);
    expect(useSessionStore.getState().sessions).toHaveLength(1);

    useSessionStore.getState().cleanup();

    expect(useSessionStore.getState().sessions).toEqual([]);
    expect(useSessionStore.getState().isInitialized).toBe(false);
  });
});
