import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook, waitFor, act } from '@testing-library/react';

import '../test/mocks/tauri'; // activates vi.mock calls
import { mockInvoke, mockListen, setInvokeResponse } from '../test/mocks/tauri';

import { usePoeProject } from '../hooks/usePoeProject';
import { makeNode, makePhase, makeArtifact, makeQueueItem } from './factories';

// ── Group 1: invoke calls on mount ───────────────────────────────────────────

describe('usePoeProject — invoke calls on mount', () => {
  beforeEach(() => {
    mockInvoke.mockResolvedValue([]);
    mockListen.mockResolvedValue(() => {});
  });

  it('calls list_nodes with project_id on mount', async () => {
    const { unmount } = renderHook(() => usePoeProject('project-1'));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('list_nodes', { projectId: 'project-1', phaseId: null });
    });
    unmount();
  });

  it('calls list_phases with project_id on mount', async () => {
    const { unmount } = renderHook(() => usePoeProject('project-1'));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('list_phases', { projectId: 'project-1' });
    });
    unmount();
  });

  it('calls list_artifacts with project_id on mount', async () => {
    const { unmount } = renderHook(() => usePoeProject('project-1'));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('list_artifacts', { projectId: 'project-1' });
    });
    unmount();
  });

  it('calls list_events with project_id on mount', async () => {
    // NOTE: The hook does NOT call list_knowledge. It calls list_events instead.
    // The bead spec mentioned list_knowledge but the source uses list_events.
    const { unmount } = renderHook(() => usePoeProject('project-1'));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('list_events', { projectId: 'project-1', since: null });
    });
    unmount();
  });

  it('calls list_queue_items with project_id on mount', async () => {
    const { unmount } = renderHook(() => usePoeProject('project-1'));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('list_queue_items', { projectId: 'project-1' });
    });
    unmount();
  });

  it('returns empty arrays when project_id is null', () => {
    mockInvoke.mockClear();
    const { result, unmount } = renderHook(() => usePoeProject(null));
    // No invokes should be made
    expect(mockInvoke).not.toHaveBeenCalled();
    expect(result.current.nodes).toEqual([]);
    expect(result.current.phases).toEqual([]);
    expect(result.current.artifacts).toEqual([]);
    expect(result.current.queueItems).toEqual([]);
    expect(result.current.feedItems).toEqual([]);
    unmount();
  });
});

// ── Group 2: listen() event name correctness ─────────────────────────────────
// This group catches wrong event names. Tests check what the source actually
// listens on and flags divergences from Protocol.md.
// subscribe() is async, so we use waitFor to let it complete before asserting.

describe('usePoeProject — listen() event name correctness', () => {
  beforeEach(() => {
    mockInvoke.mockResolvedValue([]);
    mockListen.mockResolvedValue(() => {});
  });

  it('listens on poe-task-created', async () => {
    const { unmount } = renderHook(() => usePoeProject('project-1'));
    await waitFor(() => {
      expect(mockListen).toHaveBeenCalledWith('poe-task-created', expect.any(Function));
    });
    unmount();
  });

  it('listens on poe-node-updated', async () => {
    // Protocol.md §4 names this poe-node-updated.
    // Source correctly uses 'poe-node-updated'.
    // NOTE: The bead spec listed 'poe-task-update' — that name does NOT appear in
    // the source or Protocol.md. The correct name is 'poe-node-updated'.
    const { unmount } = renderHook(() => usePoeProject('project-1'));
    await waitFor(() => {
      expect(mockListen).toHaveBeenCalledWith('poe-node-updated', expect.any(Function));
    });
    unmount();
  });

  it('listens on poe-task-done', async () => {
    const { unmount } = renderHook(() => usePoeProject('project-1'));
    await waitFor(() => {
      expect(mockListen).toHaveBeenCalledWith('poe-task-done', expect.any(Function));
    });
    unmount();
  });

  it('listens on poe-decision-queued', async () => {
    // Protocol.md §4: poe-decision-queued
    // Source uses 'poe-decision-queued' — correct.
    // NOTE: The bead spec mentioned 'poe-decision' but Protocol.md and source use
    // 'poe-decision-queued'.
    const { unmount } = renderHook(() => usePoeProject('project-1'));
    await waitFor(() => {
      expect(mockListen).toHaveBeenCalledWith('poe-decision-queued', expect.any(Function));
    });
    unmount();
  });

  it('listens on poe-decision-resolved', async () => {
    const { unmount } = renderHook(() => usePoeProject('project-1'));
    await waitFor(() => {
      expect(mockListen).toHaveBeenCalledWith('poe-decision-resolved', expect.any(Function));
    });
    unmount();
  });

  it('listens on poe-event', async () => {
    // NOTE: The bead spec mentioned 'poe-chat-turn' and 'poe-advisor-turn' but
    // neither appears in the current source. The hook uses 'poe-event' for
    // generic event feed items. 'poe-chat-turn' and 'poe-advisor-turn' are not
    // yet implemented in usePoeProject.
    const { unmount } = renderHook(() => usePoeProject('project-1'));
    await waitFor(() => {
      expect(mockListen).toHaveBeenCalledWith('poe-event', expect.any(Function));
    });
    unmount();
  });

  it('listens on poe-agent-started', async () => {
    const { unmount } = renderHook(() => usePoeProject('project-1'));
    await waitFor(() => {
      expect(mockListen).toHaveBeenCalledWith('poe-agent-started', expect.any(Function));
    });
    unmount();
  });

  it('listens on poe-agent-exited', async () => {
    const { unmount } = renderHook(() => usePoeProject('project-1'));
    await waitFor(() => {
      expect(mockListen).toHaveBeenCalledWith('poe-agent-exited', expect.any(Function));
    });
    unmount();
  });

  it('listens on poe-artifact-created', async () => {
    // Protocol.md §4: poe-artifact-created — source matches.
    const { unmount } = renderHook(() => usePoeProject('project-1'));
    await waitFor(() => {
      expect(mockListen).toHaveBeenCalledWith('poe-artifact-created', expect.any(Function));
    });
    unmount();
  });

  it('listens on poe-knowledge-created', async () => {
    // NOTE: The bead spec listed 'poe-feed-item' but that name does not appear
    // in the source. Knowledge entries are surfaced via 'poe-knowledge-created'.
    const { unmount } = renderHook(() => usePoeProject('project-1'));
    await waitFor(() => {
      expect(mockListen).toHaveBeenCalledWith('poe-knowledge-created', expect.any(Function));
    });
    unmount();
  });

  it('listens on poe-agent-stream', async () => {
    const { unmount } = renderHook(() => usePoeProject('project-1'));
    await waitFor(() => {
      expect(mockListen).toHaveBeenCalledWith('poe-agent-stream', expect.any(Function));
    });
    unmount();
  });

  it('listens on poe-phase-update', async () => {
    const { unmount } = renderHook(() => usePoeProject('project-1'));
    await waitFor(() => {
      expect(mockListen).toHaveBeenCalledWith('poe-phase-update', expect.any(Function));
    });
    unmount();
  });

  it('does NOT listen on poe-task-update (wrong name — Protocol.md uses poe-node-updated)', async () => {
    // BUG (not in source): The bead spec referenced 'poe-task-update', which
    // is not a valid event name per Protocol.md §4. The correct name is
    // 'poe-node-updated'. This test asserts the wrong name is never registered.
    const { unmount } = renderHook(() => usePoeProject('project-1'));
    // Wait for all 12 listeners to be registered
    await waitFor(() => {
      expect(mockListen.mock.calls.length).toBeGreaterThanOrEqual(12);
    });
    const registeredEvents = mockListen.mock.calls.map((c: unknown[]) => c[0]);
    expect(registeredEvents).not.toContain('poe-task-update');
    unmount();
  });
});

// ── Group 3: cleanup ──────────────────────────────────────────────────────────

describe('usePoeProject — cleanup', () => {
  it('unlisten is called on unmount', async () => {
    const unlistenFn = vi.fn();
    // Each listen() call returns a Promise that resolves to the unlisten fn.
    mockInvoke.mockResolvedValue([]);
    mockListen.mockResolvedValue(unlistenFn);

    const { unmount } = renderHook(() => usePoeProject('project-1'));

    // Wait for all subscriptions to be established (subscribe() is async)
    await waitFor(() => {
      // The hook registers 12 listeners; wait until at least one has been called
      expect(mockListen.mock.calls.length).toBeGreaterThan(0);
    });

    const listenCallCount = mockListen.mock.calls.length;
    unmount();

    // Give the cleanup microtasks a tick to settle
    await act(async () => {});

    // Each registered listener should have been unlistened
    expect(unlistenFn).toHaveBeenCalledTimes(listenCallCount);
  });
});

// ── Group 4: state updates from events ───────────────────────────────────────

describe('usePoeProject — state updates from events', () => {
  beforeEach(() => {
    mockInvoke.mockResolvedValue([]);
    mockListen.mockResolvedValue(() => {});
  });

  it('updates phases when poe-phase-update fires', async () => {
    const updatedPhase = makePhase({ id: 'phase-42', projectId: 'project-1', title: 'Updated Phase' });

    // After poe-phase-update fires, the hook re-invokes list_phases.
    // Pre-configure invoke to return the updated phase list for list_phases.
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'list_phases') return Promise.resolve([updatedPhase]);
      return Promise.resolve([]);
    });

    const { result, unmount } = renderHook(() => usePoeProject('project-1'));

    // Wait for subscribe() to register the poe-phase-update listener
    await waitFor(() => {
      const calls = mockListen.mock.calls as [string, (e: { payload: unknown }) => void][];
      const found = calls.find(([name]) => name === 'poe-phase-update');
      expect(found).toBeDefined();
    });

    // Capture the poe-phase-update callback
    const calls = mockListen.mock.calls as [string, (e: { payload: unknown }) => void][];
    const [, phaseUpdateCb] = calls.find(([name]) => name === 'poe-phase-update')!;

    // Fire the event
    await act(async () => {
      phaseUpdateCb({ payload: { projectId: 'project-1', phaseId: 'phase-42', status: 'running' } });
    });

    await waitFor(() => {
      expect(result.current.phases).toEqual([updatedPhase]);
    });

    unmount();
  });

  it('updates nodes when poe-node-updated fires', async () => {
    const initialNode = makeNode({ id: 'node-1', projectId: 'project-1', status: 'pending' });
    const updatedNode = makeNode({ id: 'node-1', projectId: 'project-1', status: 'complete' });

    // Hydrate with an initial node
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'list_nodes') return Promise.resolve([initialNode]);
      return Promise.resolve([]);
    });

    const { result, unmount } = renderHook(() => usePoeProject('project-1'));

    // Wait for hydration to complete
    await waitFor(() => {
      expect(result.current.nodes).toEqual([initialNode]);
    });

    // Wait for subscribe() to register the poe-node-updated listener
    await waitFor(() => {
      const calls = mockListen.mock.calls as [string, (e: { payload: unknown }) => void][];
      const found = calls.find(([name]) => name === 'poe-node-updated');
      expect(found).toBeDefined();
    });

    const calls = mockListen.mock.calls as [string, (e: { payload: unknown }) => void][];
    const [, nodeUpdatedCb] = calls.find(([name]) => name === 'poe-node-updated')!;

    // Fire the event with the updated node
    await act(async () => {
      nodeUpdatedCb({ payload: updatedNode });
    });

    await waitFor(() => {
      expect(result.current.nodes[0].status).toBe('complete');
    });

    unmount();
  });

  it('updates queueItems when poe-decision-queued fires', async () => {
    const newItem = makeQueueItem({ id: 'queue-99', projectId: 'project-1', question: 'Proceed?' });

    const { result, unmount } = renderHook(() => usePoeProject('project-1'));

    // Wait for the poe-decision-queued listener to be registered
    await waitFor(() => {
      const calls = mockListen.mock.calls as [string, (e: { payload: unknown }) => void][];
      const found = calls.find(([name]) => name === 'poe-decision-queued');
      expect(found).toBeDefined();
    });

    const calls = mockListen.mock.calls as [string, (e: { payload: unknown }) => void][];
    const [, decisionCb] = calls.find(([name]) => name === 'poe-decision-queued')!;

    await act(async () => {
      decisionCb({ payload: newItem });
    });

    await waitFor(() => {
      expect(result.current.queueItems).toContainEqual(newItem);
    });

    unmount();
  });
});
