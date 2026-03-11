import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import './mocks/tauri';
import { mockInvoke, mockListen } from './mocks/tauri';
import { makeAdvisorTurn } from './factories';
import AdvisorChatbot from '../components/AdvisorChatbot';

describe('AdvisorChatbot', () => {
  beforeEach(() => {
    mockInvoke.mockResolvedValue([]);
    mockListen.mockResolvedValue(() => {});
  });

  it('renders "Ask Advisor" button when taskNodeId is null', () => {
    render(<AdvisorChatbot projectId="p1" taskNodeId={null} />);
    expect(screen.getByText('Ask Advisor')).toBeInTheDocument();
  });

  it('listens on poe-advisor-turn (NOT advisor-turn-added) when taskNodeId is set', () => {
    // PROTOCOL.md §4 specifies the event name is poe-advisor-turn.
    // The old name advisor-turn-added was a bug — this test guards the regression.
    render(<AdvisorChatbot projectId="p1" taskNodeId="t1" />);
    const listenCalls = mockListen.mock.calls.map((c: unknown[]) => c[0]);
    expect(listenCalls).toContain('poe-advisor-turn');
    expect(listenCalls).not.toContain('advisor-turn-added');
  });

  it('listens on poe-task-done when taskNodeId is set', () => {
    render(<AdvisorChatbot projectId="p1" taskNodeId="t1" />);
    const listenCalls = mockListen.mock.calls.map((c: unknown[]) => c[0]);
    expect(listenCalls).toContain('poe-task-done');
  });

  it('does not register event listeners when taskNodeId is null', () => {
    render(<AdvisorChatbot projectId="p1" taskNodeId={null} />);
    // No listen calls expected — session not yet started
    expect(mockListen).not.toHaveBeenCalled();
  });

  it('Send button is disabled when there is no pending turn', async () => {
    // list_advisor_turns returns empty array — no turns, no pending turn
    mockInvoke.mockResolvedValue([]);
    render(<AdvisorChatbot projectId="p1" taskNodeId="t1" />);
    const sendBtn = await screen.findByText('Send ↵');
    expect(sendBtn).toBeDisabled();
  });

  it('renders advisor turns from list_advisor_turns response', async () => {
    const turn = makeAdvisorTurn({
      id: 'turn-1',
      taskId: 't1',
      content: 'What is the project scope?',
    });
    mockInvoke.mockResolvedValue([turn]);
    render(<AdvisorChatbot projectId="p1" taskNodeId="t1" />);
    await waitFor(() => {
      expect(screen.getByText('What is the project scope?')).toBeInTheDocument();
    });
  });

  it('shows "Advisor session complete." when sessionDone', async () => {
    // We need to simulate poe-task-done firing. We do this by capturing the
    // listen callback and calling it with matching payload.
    let taskDoneCallback: ((ev: { payload: { id: string; projectId: string; status: string } }) => void) | undefined;

    mockListen.mockImplementation((event: string, cb: unknown) => {
      if (event === 'poe-task-done') {
        taskDoneCallback = cb as typeof taskDoneCallback;
      }
      return Promise.resolve(() => {});
    });
    mockInvoke.mockResolvedValue([]);

    render(<AdvisorChatbot projectId="p1" taskNodeId="t1" />);

    // Wait for listeners to register
    await waitFor(() => {
      expect(taskDoneCallback).toBeDefined();
    });

    // Fire the event with matching ids — wrap in act so React processes the state update
    await act(async () => {
      taskDoneCallback!({ payload: { id: 't1', projectId: 'p1', status: 'complete' } });
    });

    await waitFor(() => {
      // The component renders this string in two locations when sessionDone is true
      // (turn-list empty state + footer banner) — use getAllByText accordingly.
      const matches = screen.getAllByText('Advisor session complete.');
      expect(matches.length).toBeGreaterThanOrEqual(1);
    });
  });

  it('Send button calls invoke respond_to_advisor with projectId, turnId, and response', async () => {
    const turn = makeAdvisorTurn({
      id: 'turn-99',
      taskId: 't1',
      content: 'Please answer this question.',
      respondedAt: null,
    });

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'list_advisor_turns') return Promise.resolve([turn]);
      return Promise.resolve(undefined);
    });

    render(<AdvisorChatbot projectId="p1" taskNodeId="t1" />);

    // Wait for turn to render
    await waitFor(() => {
      expect(screen.getByText('Please answer this question.')).toBeInTheDocument();
    });

    const textarea = screen.getByRole('textbox');
    fireEvent.change(textarea, { target: { value: 'My answer here' } });

    const sendBtn = screen.getByText('Send ↵');
    expect(sendBtn).not.toBeDisabled();
    fireEvent.click(sendBtn);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('respond_to_advisor', {
        projectId: 'p1',
        turnId: 'turn-99',
        response: 'My answer here',
      });
    });
  });
});
