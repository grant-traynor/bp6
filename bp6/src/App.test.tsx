import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import App from './App';
import * as api from './api';
import { listen } from '@tauri-apps/api/event';

// Mock Tauri APIs
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}));

// Mock api functions
vi.mock('./api', async () => {
  const actual = await vi.importActual('./api');
  return {
    ...actual,
    fetchProjects: vi.fn().mockResolvedValue([{ name: 'Test Project', path: '/test', last_opened: '2026-01-01' }]),
    fetchProjectViewModel: vi.fn(),
    openProject: vi.fn().mockResolvedValue(undefined),
    getCliPreference: vi.fn().mockResolvedValue('gemini'),
  };
});

describe('App Integration - Session Indicators', () => {
  const mockBeadId = 'bp6-643.003';
  
  const mockViewModel = {
    tree: [
      {
        id: mockBeadId,
        title: 'WBS Tree UI Indicators',
        status: 'open',
        priority: 1,
        issueType: 'feature',
        children: [],
        dependencies: [],
        cellOffset: 0,
        cellCount: 1,
        depth: 0,
        isExpanded: true,
        isVisible: true,
        isBlocked: false,
        isCritical: false,
      }
    ],
    metadata: {
      distributions: [],
      totalBeads: 1,
      openCount: 1,
      blockedCount: 0,
      inProgressCount: 0,
      closedCount: 0,
      totalDuration: 0,
    },
    indexes: {
      idToIndex: { [mockBeadId]: 0 },
      idToParent: {},
      criticalPath: [],
    }
  };

  beforeEach(() => {
    vi.clearAllMocks();
    (api.fetchProjectViewModel as any).mockResolvedValue(mockViewModel);
  });

  it('updates session indicators when session-list-changed event is received', async () => {
    let eventHandler: any;
    (listen as any).mockImplementation((event: string, handler: any) => {
      if (event === 'session-list-changed') {
        eventHandler = handler;
      }
      return Promise.resolve(() => {});
    });

    await act(async () => {
      render(<App />);
    });

    // Verify bead is rendered using data-bead-id
    const wbsItem = await screen.findByTestId(`bead-item-${mockBeadId}`);
    expect(wbsItem).toBeDefined();
    expect(wbsItem.textContent).toContain('WBS Tree UI Indicators');
    
    // Initially no session indicator
    expect(screen.queryByTitle(/active session/)).toBeNull();

    // Trigger the event (payload uses camelCase as serde transforms it)
    await act(async () => {
      eventHandler({
        payload: {
          sessions: [
            {
              sessionId: 'sess-1',
              beadId: mockBeadId,
              persona: 'product-manager',
              backendId: 'gemini',
              status: 'running',
              createdAt: Date.now(),
              lastActivity: Date.now(),
              hasUnread: false,
              messageCount: 0
            }
          ]
        }
      });
    });

    // Now session indicator should be visible
    const indicator = await screen.findByTitle(/1 active session/);
    expect(indicator).toBeDefined();
    expect(indicator.textContent).toContain('📋');
  });
});
