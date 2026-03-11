import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import './mocks/tauri';
import { makeNode } from './factories';
import DebugPanel from '../components/DebugPanel';

describe('DebugPanel', () => {
  const baseNode = makeNode({
    id: 'node-1',
    title: 'Analyse Market Data',
    status: 'running',
    skillId: 'data-analyst',
  });

  beforeEach(() => {
    // DebugPanel does not invoke or listen — no mock setup needed
  });

  it('renders without crashing', () => {
    render(
      <DebugPanel
        node={baseNode}
        streamEvents={[]}
        onHandoverOpen={vi.fn()}
        onClose={vi.fn()}
      />
    );
    expect(screen.getByText('Analyse Market Data')).toBeInTheDocument();
  });

  it('shows empty state message when no stream events', () => {
    render(
      <DebugPanel
        node={baseNode}
        streamEvents={[]}
        onHandoverOpen={vi.fn()}
        onClose={vi.fn()}
      />
    );
    expect(screen.getByText('No stream-json events received yet.')).toBeInTheDocument();
  });

  it('shows running status indicator', () => {
    render(
      <DebugPanel
        node={baseNode}
        streamEvents={[]}
        onHandoverOpen={vi.fn()}
        onClose={vi.fn()}
      />
    );
    expect(screen.getByText('● running')).toBeInTheDocument();
  });

  it('shows waiting status indicator when node is waiting', () => {
    const waitingNode = makeNode({ ...baseNode, status: 'waiting' });
    render(
      <DebugPanel
        node={waitingNode}
        streamEvents={[]}
        onHandoverOpen={vi.fn()}
        onClose={vi.fn()}
      />
    );
    expect(screen.getByText('? waiting')).toBeInTheDocument();
  });

  it('Close button calls onClose', () => {
    const onClose = vi.fn();
    render(
      <DebugPanel
        node={baseNode}
        streamEvents={[]}
        onHandoverOpen={onClose}
        onClose={onClose}
      />
    );
    // The close button contains ✕
    const closeBtn = screen.getByText('✕');
    fireEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalled();
  });

  it('renders stream events when provided', () => {
    const events = [
      {
        agentId: 'agent-1',
        taskId: 'node-1',
        projectId: 'p1',
        event: { type: 'system', subtype: 'init' },
      },
      {
        agentId: 'agent-1',
        taskId: 'node-1',
        projectId: 'p1',
        event: { type: 'result', subtype: 'success' },
      },
    ];
    render(
      <DebugPanel
        node={baseNode}
        streamEvents={events}
        onHandoverOpen={vi.fn()}
        onClose={vi.fn()}
      />
    );
    expect(screen.getByText('init')).toBeInTheDocument();
    expect(screen.getByText('result:success')).toBeInTheDocument();
  });

  it('shows Handover button when node is waiting', () => {
    const waitingNode = makeNode({ ...baseNode, status: 'waiting' });
    const onHandoverOpen = vi.fn();
    render(
      <DebugPanel
        node={waitingNode}
        streamEvents={[]}
        onHandoverOpen={onHandoverOpen}
        onClose={vi.fn()}
      />
    );
    const handoverBtn = screen.getByText('Handover →');
    expect(handoverBtn).toBeInTheDocument();
    fireEvent.click(handoverBtn);
    expect(onHandoverOpen).toHaveBeenCalledWith('node-1');
  });
});
