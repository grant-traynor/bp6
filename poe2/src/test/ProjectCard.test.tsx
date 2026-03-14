import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import ProjectCard from '../components/ProjectCard';
import { makeProject, makeNode, makePhase } from './factories';

describe('ProjectCard', () => {
  const baseProps = {
    project: makeProject({ name: 'My Project' }),
    nodes: [],
    phases: [],
    queueCount: 0,
    selected: false,
    onSelect: vi.fn(),
    onClose: vi.fn(),
  };

  it('renders project name', () => {
    render(<ProjectCard {...baseProps} />);
    expect(screen.getByText('My Project')).toBeInTheDocument();
  });

  it('renders phase count — shows active phase title when present', () => {
    const phase = makePhase({ title: 'Alpha Phase', status: 'planning' });
    render(<ProjectCard {...baseProps} phases={[phase]} />);
    expect(screen.getByText(/Alpha Phase/)).toBeInTheDocument();
  });

  it('renders queue badge when queueCount > 0', () => {
    render(<ProjectCard {...baseProps} queueCount={3} />);
    expect(screen.getByText('3')).toBeInTheDocument();
  });

  it('does not render queue badge when queueCount = 0', () => {
    render(<ProjectCard {...baseProps} queueCount={0} />);
    // The badge text would be "0" — it should not appear
    expect(screen.queryByText('0')).not.toBeInTheDocument();
  });

  it('calls onSelect when clicked', async () => {
    const onSelect = vi.fn();
    render(<ProjectCard {...baseProps} onSelect={onSelect} />);
    await userEvent.click(screen.getByText('My Project'));
    expect(onSelect).toHaveBeenCalledTimes(1);
  });

  it('calls onClose when close button clicked', async () => {
    const onClose = vi.fn();
    render(<ProjectCard {...baseProps} onClose={onClose} />);
    const closeBtn = screen.getByTitle('Close project');
    await userEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('shows running node count', () => {
    const nodes = [
      makeNode({ id: 'n1', status: 'running' }),
      makeNode({ id: 'n2', status: 'pending' }),
    ];
    render(<ProjectCard {...baseProps} nodes={nodes} />);
    expect(screen.getByText(/1 running/)).toBeInTheDocument();
  });
});
