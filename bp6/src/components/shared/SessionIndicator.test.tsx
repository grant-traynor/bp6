import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render } from '@testing-library/react';
import { SessionIndicator } from './SessionIndicator';
import type { SessionInfo } from '../../api';

// Mock the Tauri API calls used by SessionIndicator
vi.mock('../../api', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../api')>();
  return {
    ...actual,
    createSessionWindow: vi.fn().mockResolvedValue('agent-session-test'),
    resumeSpecificSession: vi.fn().mockResolvedValue('session-1'),
  };
});

const makeSession = (overrides: Partial<SessionInfo> = {}): SessionInfo => ({
  sessionId: 'session-1',
  beadId: 'bp6-test',
  persona: 'product-manager',
  backendId: 'gemini',
  status: 'running',
  createdAt: Date.now(),
  lastActivity: Date.now() / 1000,
  hasUnread: false,
  messageCount: 0,
  projectPath: '',
  ...overrides,
});

describe('SessionIndicator', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ============================================================================
  // Rendering Tests
  // ============================================================================

  describe('Rendering', () => {
    it('should render nothing when sessions array is empty', () => {
      const { container } = render(<SessionIndicator sessions={[]} />);
      expect(container.firstChild).toBeNull();
    });

    it('should render nothing when sessions array is undefined', () => {
      const { container } = render(<SessionIndicator sessions={undefined as any} />);
      expect(container.firstChild).toBeNull();
    });

    it('should render an icon button for a single session', () => {
      const { container } = render(<SessionIndicator sessions={[makeSession()]} />);
      const buttons = container.querySelectorAll('button');
      // At least one button (the session icon)
      expect(buttons.length).toBeGreaterThanOrEqual(1);
    });

    it('should render one icon per session', () => {
      const sessions = [
        makeSession({ sessionId: 'session-1', persona: 'product-manager' }),
        makeSession({ sessionId: 'session-2', persona: 'qa-engineer' }),
        makeSession({ sessionId: 'session-3', persona: 'specialist' }),
      ];
      const { container } = render(<SessionIndicator sessions={sessions} />);
      // 3 session icon buttons (no scroll arrows since exactly 3)
      expect(container.querySelectorAll('button').length).toBe(3);
    });

    it('should show scroll arrows when more than 3 sessions', () => {
      const sessions = Array.from({ length: 4 }, (_, i) =>
        makeSession({ sessionId: `session-${i}`, persona: 'product-manager' })
      );
      const { container } = render(<SessionIndicator sessions={sessions} />);
      // 3 icon buttons + 1 right arrow = 4 buttons
      expect(container.querySelectorAll('button').length).toBe(4);
    });

    it('should apply custom className to container', () => {
      const { container } = render(
        <SessionIndicator sessions={[makeSession()]} className="custom-class" />
      );
      expect((container.firstChild as HTMLElement)?.classList.contains('custom-class')).toBe(true);
    });

    it('should render stopped sessions (no status filter)', () => {
      const stoppedSession = makeSession({ status: 'stopped' });
      const { container } = render(<SessionIndicator sessions={[stoppedSession]} />);
      expect(container.firstChild).not.toBeNull();
    });
  });

  // ============================================================================
  // Persona Icon Tests
  // ============================================================================

  describe('Persona Icons', () => {
    it('should display correct icon for product-manager', () => {
      const { container } = render(<SessionIndicator sessions={[makeSession({ persona: 'product-manager' })]} />);
      expect(container.textContent).toContain('🤖');
    });

    it('should display correct icon for qa-engineer', () => {
      const { container } = render(<SessionIndicator sessions={[makeSession({ persona: 'qa-engineer' })]} />);
      expect(container.textContent).toContain('🛡️');
    });

    it('should display correct icon for specialist', () => {
      const { container } = render(<SessionIndicator sessions={[makeSession({ persona: 'specialist' })]} />);
      expect(container.textContent).toContain('⚡');
    });

    it('should display fallback icon for unknown persona', () => {
      const { container } = render(<SessionIndicator sessions={[makeSession({ persona: 'unknown-persona' })]} />);
      expect(container.textContent).toContain('🤖');
    });

    it('should use role icon when provided', () => {
      const { container } = render(
        <SessionIndicator sessions={[makeSession({ persona: 'specialist', role: 'tauri' })]} />
      );
      expect(container.textContent).toContain('🦀');
    });
  });

  // ============================================================================
  // Session Status Tests
  // ============================================================================

  describe('Session Status', () => {
    it('should show all sessions regardless of status', () => {
      const sessions = [
        makeSession({ sessionId: 's1', status: 'running' }),
        makeSession({ sessionId: 's2', status: 'paused' }),
        makeSession({ sessionId: 's3', status: 'stopped' }),
      ];
      const { container } = render(<SessionIndicator sessions={sessions} />);
      // All 3 visible (3 icon buttons, no scroll arrows)
      expect(container.querySelectorAll('button').length).toBe(3);
    });

    it('running sessions have pulse indicator', () => {
      const { container } = render(<SessionIndicator sessions={[makeSession({ status: 'running' })]} />);
      const pulse = container.querySelector('.animate-pulse-slow');
      expect(pulse).toBeTruthy();
    });

    it('stopped sessions do not have pulse indicator', () => {
      const { container } = render(<SessionIndicator sessions={[makeSession({ status: 'stopped' })]} />);
      const pulse = container.querySelector('.animate-pulse-slow');
      expect(pulse).toBeNull();
    });
  });

  // ============================================================================
  // Title Attribute Tests
  // ============================================================================

  describe('Title Attributes', () => {
    it('each icon button has a title describing the session', () => {
      const session = makeSession({ persona: 'product-manager', status: 'running' });
      const { container } = render(<SessionIndicator sessions={[session]} />);
      const button = container.querySelector('button[title]');
      expect(button?.getAttribute('title')).toContain('product manager');
      expect(button?.getAttribute('title')).toContain('running');
    });

    it('stopped session title shows stopped status', () => {
      const session = makeSession({ persona: 'qa-engineer', status: 'stopped' });
      const { container } = render(<SessionIndicator sessions={[session]} />);
      const button = container.querySelector('button[title]');
      expect(button?.getAttribute('title')).toContain('stopped');
    });
  });
});
