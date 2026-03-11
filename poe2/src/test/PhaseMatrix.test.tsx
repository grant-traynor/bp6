import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import './mocks/tauri';
import PhaseMatrix from '../components/PhaseMatrix';
import { makePhase, makeNode } from './factories';

describe('PhaseMatrix', () => {
  it('renders without crashing with empty phases and nodes', () => {
    render(
      <PhaseMatrix
        phases={[]}
        nodes={[]}
        edges={[]}
        onTaskClick={vi.fn()}
      />
    );
    expect(screen.getByText('No phases defined')).toBeInTheDocument();
  });

  it('renders phase name headers', () => {
    const phase1 = makePhase({ id: 'phase-1', title: 'ConOps Phase' });
    const phase2 = makePhase({ id: 'phase-2', title: 'Execution Phase', lifecycleStage: 'execution' });

    render(
      <PhaseMatrix
        phases={[phase1, phase2]}
        nodes={[]}
        edges={[]}
        onTaskClick={vi.fn()}
      />
    );

    expect(screen.getByText('ConOps Phase')).toBeInTheDocument();
    expect(screen.getByText('Execution Phase')).toBeInTheDocument();
  });

  it('renders task rows with status indicators', () => {
    const phase = makePhase({ id: 'phase-1', title: 'Alpha Phase' });
    const node = makeNode({
      id: 'node-1',
      phaseId: 'phase-1',
      title: 'My Task',
      status: 'running',
    });

    render(
      <PhaseMatrix
        phases={[phase]}
        nodes={[node]}
        edges={[]}
        onTaskClick={vi.fn()}
      />
    );

    expect(screen.getByText('My Task')).toBeInTheDocument();
    // running status badge
    expect(screen.getByText('● running')).toBeInTheDocument();
  });

  it('clicking a task cell calls onTaskClick with the node', () => {
    const phase = makePhase({ id: 'phase-1', title: 'Alpha Phase' });
    const node = makeNode({
      id: 'node-1',
      phaseId: 'phase-1',
      title: 'Clickable Task',
      status: 'pending',
    });
    const onTaskClick = vi.fn();

    render(
      <PhaseMatrix
        phases={[phase]}
        nodes={[node]}
        edges={[]}
        onTaskClick={onTaskClick}
      />
    );

    // The status badge button in the phase cell
    const badge = screen.getByText('░ pending');
    fireEvent.click(badge.closest('button')!);

    expect(onTaskClick).toHaveBeenCalledWith(node);
  });
});
