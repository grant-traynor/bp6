import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// Mock AdvisorChatbot to avoid tauri deps in its internals
vi.mock('../components/AdvisorChatbot', () => ({
  default: () => <div data-testid="advisor-chatbot" />,
}));

import QueuePanel from '../components/QueuePanel';
import { makeQueueItem, makeNode } from './factories';

describe('QueuePanel', () => {
  const baseProps = {
    items: [],
    nodes: [],
    projectId: 'project-1',
    advisorTaskId: null,
    onResolve: vi.fn().mockResolvedValue(undefined),
  };

  it('renders empty queue message when items = []', () => {
    render(<QueuePanel {...baseProps} />);
    expect(screen.getByText('No pending decisions')).toBeInTheDocument();
  });

  it('renders pending decision with question text', () => {
    const items = [
      makeQueueItem({ id: 'q1', question: 'Should we proceed?', resolvedAt: null }),
    ];
    render(<QueuePanel {...baseProps} items={items} />);
    expect(screen.getByText('Should we proceed?')).toBeInTheDocument();
  });

  it('Submit button calls onResolve with correct itemId and resolution', async () => {
    const onResolve = vi.fn().mockResolvedValue(undefined);
    const items = [
      makeQueueItem({ id: 'q-abc', question: 'What approach?', resolvedAt: null }),
    ];
    render(<QueuePanel {...baseProps} items={items} onResolve={onResolve} />);

    const input = screen.getByPlaceholderText('Type a response…');
    await userEvent.type(input, 'Use option A');

    const submitBtn = screen.getByRole('button', { name: 'Submit' });
    await userEvent.click(submitBtn);

    expect(onResolve).toHaveBeenCalledWith('q-abc', 'Use option A');
  });

  it('resolved items show their resolution text in thread mode', () => {
    // A resolved item + a pending item with the same taskId renders a ThreadCard
    const resolved = makeQueueItem({
      id: 'q1',
      taskId: 'task-1',
      question: 'First question?',
      resolution: 'Yes, proceed',
      resolvedAt: '2026-01-01T01:00:00Z',
    });
    const pending = makeQueueItem({
      id: 'q2',
      taskId: 'task-1',
      question: 'Second question?',
      resolution: null,
      resolvedAt: null,
    });
    render(<QueuePanel {...baseProps} items={[resolved, pending]} />);

    // The resolved answer should be visible in the thread
    expect(screen.getByText('Yes, proceed')).toBeInTheDocument();
    // The pending question should also be visible
    expect(screen.getByText('Second question?')).toBeInTheDocument();
  });

  it('shows total pending count badge', () => {
    const items = [
      makeQueueItem({ id: 'q1', question: 'Q1?', resolvedAt: null }),
      makeQueueItem({ id: 'q2', question: 'Q2?', resolvedAt: null }),
    ];
    render(<QueuePanel {...baseProps} items={items} />);
    expect(screen.getByText('2')).toBeInTheDocument();
  });

  it('does not show pending count badge when no items pending', () => {
    render(<QueuePanel {...baseProps} items={[]} />);
    // No numeric badge
    expect(screen.queryByText('2')).not.toBeInTheDocument();
  });
});
