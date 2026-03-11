import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import ActivityFeed from '../components/ActivityFeed';
import { makeFeedItem, makeNode } from './factories';

describe('ActivityFeed', () => {
  it('renders empty state message when items = []', () => {
    render(
      <ActivityFeed items={[]} nodes={[]} onHandoverOpen={vi.fn()} />
    );
    expect(screen.getByText('Waiting for agent output…')).toBeInTheDocument();
  });

  it('renders feed items with correct content', () => {
    const items = [
      makeFeedItem({ id: 'f1', message: 'Agent started task', eventType: 'poe-step' }),
      makeFeedItem({ id: 'f2', message: 'Artifact produced', eventType: 'poe-artifact' }),
    ];
    render(<ActivityFeed items={items} nodes={[]} onHandoverOpen={vi.fn()} />);
    expect(screen.getByText('Agent started task')).toBeInTheDocument();
    expect(screen.getByText('Artifact produced')).toBeInTheDocument();
  });

  it('renders task status indicators via event type badges', () => {
    const items = [
      makeFeedItem({ id: 'f1', type: 'agent-start', message: 'Agent launched' }),
      makeFeedItem({ id: 'f2', type: 'agent-exit', message: 'Agent finished' }),
    ];
    render(<ActivityFeed items={items} nodes={[]} onHandoverOpen={vi.fn()} />);
    // agent-start and agent-exit badges should appear
    expect(screen.getByText('agent-start')).toBeInTheDocument();
    expect(screen.getByText('agent-exit')).toBeInTheDocument();
  });

  it('renders node-created badge for node-created type', () => {
    const items = [
      makeFeedItem({ id: 'f1', type: 'node-created', message: 'Task was created' }),
    ];
    render(<ActivityFeed items={items} nodes={[]} onHandoverOpen={vi.fn()} />);
    expect(screen.getByText('task-created')).toBeInTheDocument();
    expect(screen.getByText('Task was created')).toBeInTheDocument();
  });

  it('renders model badge when item has model', () => {
    const items = [
      makeFeedItem({ id: 'f1', message: 'Step done', model: 'claude-3-5' }),
    ];
    render(<ActivityFeed items={items} nodes={[]} onHandoverOpen={vi.fn()} />);
    expect(screen.getByText('claude-3-5')).toBeInTheDocument();
  });

  it('calls onHandoverOpen with taskId when feed item is clicked', async () => {
    const { default: userEvent } = await import('@testing-library/user-event');
    const onHandoverOpen = vi.fn();
    const items = [
      makeFeedItem({ id: 'f1', taskId: 'task-abc', message: 'Click me' }),
    ];
    render(<ActivityFeed items={items} nodes={[]} onHandoverOpen={onHandoverOpen} />);
    await userEvent.click(screen.getByText('Click me'));
    expect(onHandoverOpen).toHaveBeenCalledWith('task-abc');
  });
});
