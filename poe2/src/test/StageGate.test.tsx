import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import './mocks/tauri';
import { mockInvoke } from './mocks/tauri';
import StageGate from '../components/StageGate';
import { makePhase, makeArtifact } from './factories';

describe('StageGate', () => {
  beforeEach(() => {
    mockInvoke.mockResolvedValue(undefined);
  });

  it('renders artifact list', () => {
    const phase = makePhase({ id: 'phase-1', title: 'Alpha Phase' });
    const artifact = makeArtifact({
      id: 'artifact-1',
      phaseId: 'phase-1',
      filename: 'conops.md',
    });

    render(
      <StageGate
        phase={phase}
        projectId="project-1"
        artifacts={[artifact]}
        onArtifactOpen={vi.fn()}
        onAction={vi.fn()}
      />
    );

    expect(screen.getByText(/conops\.md/)).toBeInTheDocument();
  });

  it('Advance button calls invoke(advance_stage_gate) with both phaseId and projectId', async () => {
    const phase = makePhase({ id: 'phase-42', title: 'Beta Phase' });
    const onAction = vi.fn();

    render(
      <StageGate
        phase={phase}
        projectId="project-1"
        artifacts={[]}
        onArtifactOpen={vi.fn()}
        onAction={onAction}
      />
    );

    // First click opens confirmation dialog
    fireEvent.click(screen.getByRole('button', { name: /advance/i }));

    // Confirm button appears
    fireEvent.click(screen.getByRole('button', { name: /confirm/i }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('advance_stage_gate', {
        phaseId: 'phase-42',
        projectId: 'project-1',
      });
    });

    await waitFor(() => {
      expect(onAction).toHaveBeenCalled();
    });
  });
});
