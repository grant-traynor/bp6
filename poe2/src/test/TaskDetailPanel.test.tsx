import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import './mocks/tauri';
import { mockInvoke, mockListen } from './mocks/tauri';
import { makeNode, makeArtifact, makeEdge } from './factories';
import TaskDetailPanel from '../components/TaskDetailPanel';

describe('TaskDetailPanel', () => {
  const baseNode = makeNode({
    id: 'node-1',
    title: 'Write ConOps',
    status: 'running',
    skillId: 'operational-analyst',
    // skillModes: null → defaults to ['autonomous']
    skillModes: null,
    phaseId: 'phase-1',
  });

  const baseProps = {
    node: baseNode,
    projectId: 'p1',
    nodes: [],
    artifacts: [],
    edges: [],
    onClose: vi.fn(),
    onArtifactOpen: vi.fn(),
  };

  beforeEach(() => {
    mockInvoke.mockResolvedValue([]);
    mockListen.mockResolvedValue(() => {});
  });

  it('renders task title', () => {
    render(<TaskDetailPanel {...baseProps} />);
    expect(screen.getByText('Write ConOps')).toBeInTheDocument();
  });

  it('renders task status', () => {
    render(<TaskDetailPanel {...baseProps} />);
    expect(screen.getByText('running')).toBeInTheDocument();
  });

  it('renders skill name when skillId is set', () => {
    render(<TaskDetailPanel {...baseProps} />);
    expect(screen.getByText('operational-analyst')).toBeInTheDocument();
  });

  it('artifact links call onArtifactOpen when clicked', async () => {
    const artifact = makeArtifact({
      id: 'art-1',
      phaseId: 'phase-1',
      filename: 'conops.md',
    });
    const onArtifactOpen = vi.fn();

    render(
      <TaskDetailPanel
        {...baseProps}
        artifacts={[artifact]}
        onArtifactOpen={onArtifactOpen}
      />
    );

    // Artifact button renders with filename
    const artifactLink = await screen.findByText(/conops\.md/);
    fireEvent.click(artifactLink);

    expect(onArtifactOpen).toHaveBeenCalledWith(artifact);
  });

  it('Close button calls onClose', () => {
    const onClose = vi.fn();
    render(<TaskDetailPanel {...baseProps} onClose={onClose} />);
    const closeBtn = screen.getByText('✕ close');
    fireEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('calls get_node_ancestry and list_events on mount', async () => {
    render(<TaskDetailPanel {...baseProps} />);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_node_ancestry', { nodeId: 'node-1' });
      expect(mockInvoke).toHaveBeenCalledWith('list_events', {
        projectId: 'p1',
        since: null,
      });
    });
  });
});
