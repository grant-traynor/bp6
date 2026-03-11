import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import './mocks/tauri';
import NodeTree from '../components/NodeTree';
import { makeNode } from './factories';

describe('NodeTree', () => {
  it('renders nothing (null) when nodes = []', () => {
    const { container } = render(
      <NodeTree nodes={[]} onHandoverOpen={vi.fn()} onDebugOpen={vi.fn()} />
    );
    expect(container.firstChild).toBeNull();
  });

  it('renders node titles', () => {
    const nodes = [
      makeNode({ id: 'node-1', title: 'Alpha Task', status: 'pending' }),
      makeNode({ id: 'node-2', title: 'Beta Task', status: 'complete' }),
    ];

    render(
      <NodeTree nodes={nodes} onHandoverOpen={vi.fn()} onDebugOpen={vi.fn()} />
    );

    expect(screen.getByText('Alpha Task')).toBeInTheDocument();
    expect(screen.getByText('Beta Task')).toBeInTheDocument();
  });

  it('clicking a waiting node calls onHandoverOpen with the node id', () => {
    const onHandoverOpen = vi.fn();
    const node = makeNode({ id: 'node-waiting', title: 'Waiting Task', status: 'waiting' });

    render(
      <NodeTree nodes={[node]} onHandoverOpen={onHandoverOpen} onDebugOpen={vi.fn()} />
    );

    fireEvent.click(screen.getByText('Waiting Task'));
    expect(onHandoverOpen).toHaveBeenCalledWith('node-waiting');
  });
});
