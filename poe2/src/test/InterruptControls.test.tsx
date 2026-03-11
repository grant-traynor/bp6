import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import './mocks/tauri';
import InterruptControls from '../components/InterruptControls';

describe('InterruptControls', () => {
  it('renders nothing when runningAgentCount is 0', () => {
    const { container } = render(
      <InterruptControls
        projectId="project-1"
        activePhaseId="phase-1"
        runningAgentCount={0}
      />
    );
    expect(container.firstChild).toBeNull();
  });

  it('renders without crashing when agents are running', () => {
    render(
      <InterruptControls
        projectId="project-1"
        activePhaseId="phase-1"
        runningAgentCount={3}
      />
    );

    // Both control buttons should be present
    expect(screen.getByRole('button', { name: /pause stage/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /abort/i })).toBeInTheDocument();
  });

  it('renders without crashing when activePhaseId is null', () => {
    render(
      <InterruptControls
        projectId="project-1"
        activePhaseId={null}
        runningAgentCount={2}
      />
    );

    // Only Abort button renders (no Pause stage when no active phase)
    expect(screen.getByRole('button', { name: /abort/i })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /pause stage/i })).toBeNull();
  });
});
