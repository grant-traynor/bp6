import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import './mocks/tauri';
import { mockInvoke } from './mocks/tauri';
import { makeNode } from './factories';
import AgentHandover from '../components/AgentHandover';

// AgentHandover uses @xterm/xterm and @xterm/addon-fit — mock both to avoid
// jsdom canvas/DOM errors.
vi.mock('@xterm/xterm', () => ({
  Terminal: class {
    open() {}
    dispose() {}
    onData() { return { dispose() {} }; }
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

// Also mock the CSS import that AgentHandover pulls in via xterm
vi.mock('@xterm/xterm/css/xterm.css', () => ({}));

describe('AgentHandover', () => {
  const node = makeNode({
    id: 'node-1',
    title: 'Analyse Requirements',
    skillId: 'requirements-analyst',
    status: 'waiting',
  });

  beforeEach(() => {
    // agent_handover_open returns a port number; agent_handover_close is fire-and-forget
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'agent_handover_open') return Promise.resolve(9999);
      return Promise.resolve(undefined);
    });

    // WebSocket is not available in jsdom — stub it to prevent the open() call
    // from throwing and leaving unresolved promises.
    vi.stubGlobal('WebSocket', class {
      binaryType = '';
      readyState = 3; // CLOSED
      onopen: (() => void) | null = null;
      onmessage: ((e: MessageEvent) => void) | null = null;
      onclose: (() => void) | null = null;
      onerror: ((e: Event) => void) | null = null;
      close() {}
      send() {}
    });
  });

  it('renders without crashing', () => {
    render(<AgentHandover nodeId="node-1" node={node} onClose={vi.fn()} />);
    // Header shows node title and skill
    expect(screen.getByText(/Analyse Requirements/)).toBeInTheDocument();
  });

  it('renders node skill in header', () => {
    render(<AgentHandover nodeId="node-1" node={node} onClose={vi.fn()} />);
    expect(screen.getByText(/requirements-analyst/)).toBeInTheDocument();
  });

  it('shows "connecting…" status on initial render', () => {
    render(<AgentHandover nodeId="node-1" node={node} onClose={vi.fn()} />);
    expect(screen.getByText('connecting…')).toBeInTheDocument();
  });

  it('Close button calls onClose', () => {
    const onClose = vi.fn();
    render(<AgentHandover nodeId="node-1" node={node} onClose={onClose} />);
    const closeBtn = screen.getByText('✕ close');
    fireEvent.click(closeBtn);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('calls invoke agent_handover_open with nodeId on mount', async () => {
    render(<AgentHandover nodeId="node-1" node={node} onClose={vi.fn()} />);
    // Give the async open() a tick to run
    await vi.waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('agent_handover_open', { nodeId: 'node-1' });
    });
  });
});
