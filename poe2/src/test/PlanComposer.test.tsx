import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import './mocks/tauri';
import { mockInvoke } from './mocks/tauri';

// Mutable state so individual tests can override useNodesState's return value
let mockNodes: any[] = [];
const mockSetNodes = vi.fn();
const mockOnNodesChange = vi.fn();

// Mock @xyflow/react — it does not work in jsdom
vi.mock('@xyflow/react', () => ({
  ReactFlow: ({ children }: any) => <div data-testid="react-flow">{children}</div>,
  useNodesState: () => [mockNodes, mockSetNodes, mockOnNodesChange],
  useEdgesState: () => [[], vi.fn(), vi.fn()],
  addEdge: vi.fn(),
  Controls: () => null,
  Background: () => null,
  BackgroundVariant: { Dots: 'dots' },
}));

// Mock the CSS import
vi.mock('@xyflow/react/dist/style.css', () => ({}));

import PlanComposer, { STAGE_TYPES } from '../components/PlanComposer';

describe('PlanComposer', () => {
  beforeEach(() => {
    mockNodes = [];
    mockInvoke.mockResolvedValue(undefined);
  });

  it('renders stage type buttons for all 4 stage types', () => {
    render(<PlanComposer projectId="project-1" onComplete={vi.fn()} />);

    for (const st of STAGE_TYPES) {
      // Buttons render as "+ conops" etc. (underscores replaced with spaces)
      const label = st.replace(/_/g, ' ');
      expect(screen.getByText(`+ ${label}`)).toBeInTheDocument();
    }
  });

  it('Run button is disabled when no stages added', () => {
    mockNodes = [];
    render(<PlanComposer projectId="project-1" onComplete={vi.fn()} />);

    const runButton = screen.getByRole('button', { name: /run phase/i });
    expect(runButton).toBeDisabled();
  });

  it('Run button calls invoke(create_phase) for each stage node and invoke(activate_phase) with both projectId and phaseId', async () => {
    // Arrange: pre-populate nodes so Run button is enabled
    const fakePhaseId = 'phase-created-1';
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'create_phase') return Promise.resolve({ id: fakePhaseId });
      return Promise.resolve(undefined);
    });

    mockNodes = [
      {
        id: 'stage-1',
        type: 'default',
        position: { x: 100, y: 80 },
        data: { stageType: 'conops', label: 'conops', number: 1 },
      },
    ];

    const onComplete = vi.fn();
    render(<PlanComposer projectId="project-1" onComplete={onComplete} />);

    const runButton = screen.getByRole('button', { name: /run phase/i });
    // Button should now be enabled
    expect(runButton).not.toBeDisabled();
    fireEvent.click(runButton);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('create_phase', expect.objectContaining({
        projectId: 'project-1',
        stageType: 'conops',
      }));
    });

    await waitFor(() => {
      // Critical regression test: activate_phase must receive BOTH projectId AND phaseId
      expect(mockInvoke).toHaveBeenCalledWith('activate_phase', {
        projectId: 'project-1',
        phaseId: fakePhaseId,
      });
    });

    await waitFor(() => {
      expect(onComplete).toHaveBeenCalled();
    });
  });
});
