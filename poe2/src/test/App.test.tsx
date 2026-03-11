import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import './mocks/tauri'; // ensure tauri is mocked

// ── Component mocks (avoid xyflow / xterm / tauri internals) ─────────────────

vi.mock('../components/PlanComposer', () => ({
  default: () => <div data-testid="plan-composer" />,
}));

vi.mock('../components/PhaseMatrix', () => ({
  default: () => <div data-testid="phase-matrix" />,
}));

vi.mock('../components/NodeTree', () => ({
  default: () => <div data-testid="node-tree" />,
}));

vi.mock('../components/AgentHandover', () => ({
  default: () => <div data-testid="agent-handover" />,
}));

vi.mock('../components/ArtifactViewer', () => ({
  default: () => <div data-testid="artifact-viewer" />,
}));

vi.mock('../components/KnowledgePanel', () => ({
  default: () => <div data-testid="knowledge-panel" />,
}));

vi.mock('../components/StageGate', () => ({
  default: () => <div data-testid="stage-gate" />,
}));

vi.mock('../components/InterruptControls', () => ({
  default: () => <div data-testid="interrupt-controls" />,
}));

vi.mock('../components/DebugPanel', () => ({
  default: () => <div data-testid="debug-panel" />,
}));

vi.mock('../components/TaskDetailPanel', () => ({
  default: () => <div data-testid="task-detail-panel" />,
}));

vi.mock('../components/AdvisorChatbot', () => ({
  default: () => <div data-testid="advisor-chatbot" />,
}));

// ── Hook mock ─────────────────────────────────────────────────────────────────

const mockUsePoeProject = vi.fn(() => ({
  nodes: [],
  phases: [],
  artifacts: [],
  queueItems: [],
  feedItems: [],
  knowledgeEntries: [],
  handoverNodeId: null,
  setHandoverNodeId: vi.fn(),
  streamEventMap: new Map(),
}));

vi.mock('../hooks/usePoeProject', () => ({
  usePoeProject: (...args: unknown[]) => mockUsePoeProject(...args),
  getAncestry: () => [],
}));

// ── Import App after mocks ────────────────────────────────────────────────────

import App from '../App';
import { mockInvoke, setInvokeResponse } from './mocks/tauri';
import { makeProject, makePhase, makeNode } from './factories';

describe('App', () => {
  beforeEach(() => {
    // Default: list_projects returns empty, list_edges returns empty
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'list_projects') return Promise.resolve([]);
      if (cmd === 'list_edges') return Promise.resolve([]);
      return Promise.resolve(undefined);
    });
  });

  it('renders without crashing with no projects', async () => {
    render(<App />);
    // POE header should always be visible
    expect(screen.getByText('POE')).toBeInTheDocument();
  });

  it('shows "No projects open" empty state', async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByText('No projects open')).toBeInTheDocument();
    });
  });

  it('shows PlanComposer when selected project has nodes=[] and phases=[]', async () => {
    const project = makeProject({ id: 'p1', name: 'Alpha' });

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'list_projects') return Promise.resolve([project]);
      if (cmd === 'list_edges') return Promise.resolve([]);
      return Promise.resolve(undefined);
    });

    // usePoeProject returns empty nodes + phases (default mock)
    mockUsePoeProject.mockReturnValue({
      nodes: [],
      phases: [],
      artifacts: [],
      queueItems: [],
      feedItems: [],
      knowledgeEntries: [],
      handoverNodeId: null,
      setHandoverNodeId: vi.fn(),
      streamEventMap: new Map(),
    });

    render(<App />);

    // Wait for projects to load and be rendered
    await waitFor(() => {
      expect(screen.getByText('Alpha')).toBeInTheDocument();
    });

    // Click the project card to select it
    const { userEvent } = await import('@testing-library/user-event');
    const card = screen.getByText('Alpha');
    await userEvent.setup().click(card);

    await waitFor(() => {
      expect(screen.getByTestId('plan-composer')).toBeInTheDocument();
    });
  });

  it('shows PhaseMatrix section when project has phases', async () => {
    const project = makeProject({ id: 'p1', name: 'Beta' });

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'list_projects') return Promise.resolve([project]);
      if (cmd === 'list_edges') return Promise.resolve([]);
      return Promise.resolve(undefined);
    });

    const phase = makePhase({ id: 'ph1', title: 'Planning' });
    const node = makeNode({ id: 'n1', phaseId: 'ph1' });

    mockUsePoeProject.mockReturnValue({
      nodes: [node],
      phases: [phase],
      artifacts: [],
      queueItems: [],
      feedItems: [],
      knowledgeEntries: [],
      handoverNodeId: null,
      setHandoverNodeId: vi.fn(),
      streamEventMap: new Map(),
    });

    render(<App />);

    await waitFor(() => {
      expect(screen.getByText('Beta')).toBeInTheDocument();
    });

    const { userEvent } = await import('@testing-library/user-event');
    await userEvent.setup().click(screen.getByText('Beta'));

    await waitFor(() => {
      expect(screen.getByTestId('phase-matrix')).toBeInTheDocument();
    });
  });
});
