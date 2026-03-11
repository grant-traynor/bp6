import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import './mocks/tauri';
import { mockInvoke, mockListen } from './mocks/tauri';
import { makeArtifact } from './factories';
import ArtifactViewer from '../components/ArtifactViewer';

// ArtifactViewer imports @xterm/xterm indirectly through AgentHandover — mock it
// to avoid jsdom canvas errors.
vi.mock('@xterm/xterm', () => ({
  Terminal: class {
    open() {}
    dispose() {}
    onData() {}
    loadAddon() {}
    write() {}
  },
}));
vi.mock('@xterm/addon-fit', () => ({
  FitAddon: class {
    fit() {}
    proposeDimensions() { return null; }
  },
}));

// react-diff-viewer-continued uses canvas — give it a dummy
vi.mock('react-diff-viewer-continued', () => ({
  default: () => <div data-testid="diff-viewer" />,
}));

describe('ArtifactViewer', () => {
  beforeEach(() => {
    mockInvoke.mockResolvedValue(undefined);
    mockListen.mockResolvedValue(() => {});
  });

  it('renders artifact filename when artifact prop provided', () => {
    const artifact = makeArtifact({ filename: 'analysis-report.md' });
    render(
      <ArtifactViewer
        artifact={artifact}
        projectId="p1"
        onClose={vi.fn()}
      />
    );
    expect(screen.getByText('analysis-report.md')).toBeInTheDocument();
  });

  it('calls invoke read_artifact_content with artifactId and projectId to load content', async () => {
    const artifact = makeArtifact({ id: 'art-1', projectId: 'p1' });
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'read_artifact_content') return Promise.resolve('# Report content');
      if (cmd === 'get_chat_turns') return Promise.resolve([]);
      return Promise.resolve(undefined);
    });

    render(
      <ArtifactViewer
        artifact={artifact}
        projectId="p1"
        onClose={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('read_artifact_content', {
        artifactId: 'art-1',
        projectId: 'p1',
      });
    });
  });

  it('re-fetches content when artifact.updatedAt changes (upsert regression guard)', async () => {
    const artifact = makeArtifact({
      id: 'art-1',
      projectId: 'p1',
      updatedAt: '2026-01-01T00:00:00Z',
    });
    mockInvoke.mockResolvedValue('initial content');

    const { rerender } = render(
      <ArtifactViewer
        artifact={artifact}
        projectId="p1"
        onClose={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('read_artifact_content', expect.any(Object));
    });

    const callCountBefore = mockInvoke.mock.calls.filter(
      (c: unknown[]) => c[0] === 'read_artifact_content'
    ).length;

    // Simulate an upsert — same id, new updatedAt
    const updatedArtifact = { ...artifact, updatedAt: '2026-01-02T00:00:00Z' };
    rerender(
      <ArtifactViewer
        artifact={updatedArtifact}
        projectId="p1"
        onClose={vi.fn()}
      />
    );

    await waitFor(() => {
      const callCountAfter = mockInvoke.mock.calls.filter(
        (c: unknown[]) => c[0] === 'read_artifact_content'
      ).length;
      expect(callCountAfter).toBeGreaterThan(callCountBefore);
    });
  });

  it('chat panel listens on poe-chat-turn when taskId provided', () => {
    const artifact = makeArtifact();
    render(
      <ArtifactViewer
        artifact={artifact}
        projectId="p1"
        taskId="task-1"
        onClose={vi.fn()}
      />
    );
    const listenCalls = mockListen.mock.calls.map((c: unknown[]) => c[0]);
    expect(listenCalls).toContain('poe-chat-turn');
  });

  it('respond_to_chat button calls invoke respond_to_chat with projectId, turnId, response', async () => {
    // Seed a pending chat turn via get_chat_turns
    const pendingTurn = {
      id: 'chat-turn-1',
      taskId: 'task-1',
      content: 'What do you think about this section?',
      response: null,
      createdAt: '2026-01-01T00:00:00Z',
      respondedAt: null,
    };

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'get_chat_turns') return Promise.resolve([pendingTurn]);
      if (cmd === 'read_artifact_content') return Promise.resolve('# Doc');
      return Promise.resolve(undefined);
    });

    const artifact = makeArtifact({ id: 'art-1' });
    render(
      <ArtifactViewer
        artifact={artifact}
        projectId="p1"
        taskId="task-1"
        onClose={vi.fn()}
      />
    );

    // Wait for chat panel to become active (turns loaded → setChatActive(true))
    await waitFor(() => {
      expect(screen.getByText('What do you think about this section?')).toBeInTheDocument();
    });

    const textarea = screen.getByPlaceholderText('type response…');
    fireEvent.change(textarea, { target: { value: 'Great section!' } });

    const sendBtn = screen.getByText('Send ↵');
    expect(sendBtn).not.toBeDisabled();
    fireEvent.click(sendBtn);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('respond_to_chat', {
        projectId: 'p1',
        turnId: 'chat-turn-1',
        response: 'Great section!',
      });
    });
  });

  it('Close button calls onClose', async () => {
    const onClose = vi.fn();
    const artifact = makeArtifact();
    render(
      <ArtifactViewer
        artifact={artifact}
        projectId="p1"
        onClose={onClose}
      />
    );
    const closeBtn = screen.getByText('✕ close');
    fireEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
