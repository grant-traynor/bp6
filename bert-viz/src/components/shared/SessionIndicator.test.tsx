import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { SessionIndicator } from './SessionIndicator';
import type { SessionInfo } from '../../api';

describe('SessionIndicator', () => {
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

    it('should render nothing when no sessions have running status', () => {
      const pausedSessions: SessionInfo[] = [
        {
          session_id: 'session-1',
          bead_id: 'bp6-test',
          persona: 'product_manager',
          backend_id: 'gemini',
          status: 'paused',
          created_at: Date.now(),
        },
        {
          session_id: 'session-2',
          bead_id: 'bp6-test',
          persona: 'qa_engineer',
          backend_id: 'gemini',
          status: 'paused',
          created_at: Date.now(),
        },
      ];

      const { container } = render(<SessionIndicator sessions={pausedSessions} />);
      expect(container.firstChild).toBeNull();
    });

    it('should render pulsing dot for single active session', () => {
      const sessions: SessionInfo[] = [
        {
          session_id: 'session-1',
          bead_id: 'bp6-test',
          persona: 'product_manager',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now(),
        },
      ];

      const { container } = render(<SessionIndicator sessions={sessions} />);

      // Check for pulsing dot
      const pulsingDot = container.querySelector('.animate-pulse-slow');
      expect(pulsingDot).toBeTruthy();
      expect(pulsingDot?.classList.contains('bg-green-500')).toBe(true);
    });

    it('should render persona icon for single session', () => {
      const sessions: SessionInfo[] = [
        {
          session_id: 'session-1',
          bead_id: 'bp6-test',
          persona: 'product_manager',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now(),
        },
      ];

      const { container } = render(<SessionIndicator sessions={sessions} />);

      // Check for persona icon (should be 📋 for product_manager)
      expect(container.textContent).toContain('📋');
    });

    it('should render count badge for multiple sessions', () => {
      const sessions: SessionInfo[] = [
        {
          session_id: 'session-1',
          bead_id: 'bp6-test',
          persona: 'product_manager',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now(),
        },
        {
          session_id: 'session-2',
          bead_id: 'bp6-test',
          persona: 'qa_engineer',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now() + 1000,
        },
      ];

      const { container } = render(<SessionIndicator sessions={sessions} />);

      // Check for count badge showing "2"
      const countBadge = container.querySelector('.bg-indigo-600');
      expect(countBadge).toBeTruthy();
      expect(countBadge?.textContent).toBe('2');
    });

    it('should not render count badge for single session', () => {
      const sessions: SessionInfo[] = [
        {
          session_id: 'session-1',
          bead_id: 'bp6-test',
          persona: 'product_manager',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now(),
        },
      ];

      const { container } = render(<SessionIndicator sessions={sessions} />);

      // Count badge should not be present
      const countBadge = container.querySelector('.bg-indigo-600');
      expect(countBadge).toBeNull();
    });

    it('should apply custom className when provided', () => {
      const sessions: SessionInfo[] = [
        {
          session_id: 'session-1',
          bead_id: 'bp6-test',
          persona: 'product_manager',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now(),
        },
      ];

      const { container } = render(
        <SessionIndicator sessions={sessions} className="custom-class" />
      );

      expect((container.firstChild as HTMLElement)?.classList.contains('custom-class')).toBe(true);
    });
  });

  // ============================================================================
  // Persona Icon Tests
  // ============================================================================

  describe('Persona Icons', () => {
    it('should display correct icon for product_manager', () => {
      const sessions: SessionInfo[] = [
        {
          session_id: 'session-1',
          bead_id: 'bp6-test',
          persona: 'product_manager',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now(),
        },
      ];

      const { container } = render(<SessionIndicator sessions={sessions} />);
      expect(container.textContent).toContain('📋');
    });

    it('should display correct icon for qa_engineer', () => {
      const sessions: SessionInfo[] = [
        {
          session_id: 'session-1',
          bead_id: 'bp6-test',
          persona: 'qa_engineer',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now(),
        },
      ];

      const { container } = render(<SessionIndicator sessions={sessions} />);
      expect(container.textContent).toContain('🧪');
    });

    it('should display correct icon for specialist', () => {
      const sessions: SessionInfo[] = [
        {
          session_id: 'session-1',
          bead_id: 'bp6-test',
          persona: 'specialist',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now(),
        },
      ];

      const { container } = render(<SessionIndicator sessions={sessions} />);
      expect(container.textContent).toContain('⚡');
    });

    it('should display fallback icon for unknown persona', () => {
      const sessions: SessionInfo[] = [
        {
          session_id: 'session-1',
          bead_id: 'bp6-test',
          persona: 'unknown_persona',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now(),
        },
      ];

      const { container } = render(<SessionIndicator sessions={sessions} />);
      expect(container.textContent).toContain('🤖');
    });

    it('should use first active session persona when multiple sessions', () => {
      const sessions: SessionInfo[] = [
        {
          session_id: 'session-1',
          bead_id: 'bp6-test',
          persona: 'product_manager',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now(),
        },
        {
          session_id: 'session-2',
          bead_id: 'bp6-test',
          persona: 'qa_engineer',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now() + 1000,
        },
      ];

      const { container } = render(<SessionIndicator sessions={sessions} />);

      // Should show product_manager icon (📋) as it's the first session
      const personaIcon = container.querySelector('[class*="bg-[var(--background-secondary)]"]');
      expect(personaIcon?.textContent).toBe('📋');
    });
  });

  // ============================================================================
  // Tooltip Tests
  // ============================================================================

  describe('Tooltips', () => {
    it('should show persona name in tooltip for single session', () => {
      const sessions: SessionInfo[] = [
        {
          session_id: 'session-1',
          bead_id: 'bp6-test',
          persona: 'product_manager',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now(),
        },
      ];

      render(<SessionIndicator sessions={sessions} />);

      const indicator = screen.getByTitle(/1 active session/);
      expect(indicator).toBeTruthy();
      expect(indicator.title).toContain('product manager');
      expect(indicator.title).toContain('📋');
    });

    it('should show count and multiple personas for multiple sessions', () => {
      const sessions: SessionInfo[] = [
        {
          session_id: 'session-1',
          bead_id: 'bp6-test',
          persona: 'product_manager',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now(),
        },
        {
          session_id: 'session-2',
          bead_id: 'bp6-test',
          persona: 'qa_engineer',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now() + 1000,
        },
        {
          session_id: 'session-3',
          bead_id: 'bp6-test',
          persona: 'specialist',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now() + 2000,
        },
      ];

      render(<SessionIndicator sessions={sessions} />);

      const indicator = screen.getByTitle(/3 active sessions/);
      expect(indicator).toBeTruthy();
      expect(indicator.title).toContain('📋 product manager');
      expect(indicator.title).toContain('🧪 qa engineer');
      expect(indicator.title).toContain('⚡ specialist');
    });

    it('should use plural sessions in tooltip when count > 1', () => {
      const sessions: SessionInfo[] = [
        {
          session_id: 'session-1',
          bead_id: 'bp6-test',
          persona: 'product_manager',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now(),
        },
        {
          session_id: 'session-2',
          bead_id: 'bp6-test',
          persona: 'qa_engineer',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now() + 1000,
        },
      ];

      render(<SessionIndicator sessions={sessions} />);

      const indicator = screen.getByTitle(/2 active sessions/);
      expect(indicator.title).toMatch(/2 active sessions:/);
    });

    it('should use singular session in tooltip when count = 1', () => {
      const sessions: SessionInfo[] = [
        {
          session_id: 'session-1',
          bead_id: 'bp6-test',
          persona: 'product_manager',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now(),
        },
      ];

      render(<SessionIndicator sessions={sessions} />);

      const indicator = screen.getByTitle(/1 active session/);
      expect(indicator.title).toMatch(/1 active session:/);
    });

    it('should replace underscores with spaces in persona names', () => {
      const sessions: SessionInfo[] = [
        {
          session_id: 'session-1',
          bead_id: 'bp6-test',
          persona: 'qa_engineer',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now(),
        },
      ];

      render(<SessionIndicator sessions={sessions} />);

      const indicator = screen.getByTitle(/1 active session/);
      expect(indicator.title).toContain('qa engineer');
      expect(indicator.title).not.toContain('qa_engineer');
    });
  });

  // ============================================================================
  // Status Filtering Tests
  // ============================================================================

  describe('Status Filtering', () => {
    it('should only show running sessions, not paused sessions', () => {
      const sessions: SessionInfo[] = [
        {
          session_id: 'session-1',
          bead_id: 'bp6-test',
          persona: 'product_manager',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now(),
        },
        {
          session_id: 'session-2',
          bead_id: 'bp6-test',
          persona: 'qa_engineer',
          backend_id: 'gemini',
          status: 'paused',
          created_at: Date.now() + 1000,
        },
        {
          session_id: 'session-3',
          bead_id: 'bp6-test',
          persona: 'specialist',
          backend_id: 'gemini',
          status: 'paused',
          created_at: Date.now() + 2000,
        },
      ];

      render(<SessionIndicator sessions={sessions} />);

      // Should show only 1 active session (the running one)
      const indicator = screen.getByTitle(/1 active session/);
      expect(indicator).toBeTruthy();
      expect(indicator.title).toContain('📋 product manager');
      expect(indicator.title).not.toContain('qa engineer');
      expect(indicator.title).not.toContain('specialist');
    });

    it('should filter out paused sessions before counting', () => {
      const sessions: SessionInfo[] = [
        {
          session_id: 'session-1',
          bead_id: 'bp6-test',
          persona: 'product_manager',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now(),
        },
        {
          session_id: 'session-2',
          bead_id: 'bp6-test',
          persona: 'qa_engineer',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now() + 1000,
        },
        {
          session_id: 'session-3',
          bead_id: 'bp6-test',
          persona: 'specialist',
          backend_id: 'gemini',
          status: 'paused',
          created_at: Date.now() + 2000,
        },
      ];

      const { container } = render(<SessionIndicator sessions={sessions} />);

      // Count badge should show 2, not 3
      const countBadge = container.querySelector('.bg-indigo-600');
      expect(countBadge?.textContent).toBe('2');
    });

    it('should handle mixed status sessions correctly', () => {
      const sessions: SessionInfo[] = [
        {
          session_id: 'session-1',
          bead_id: 'bp6-test',
          persona: 'product_manager',
          backend_id: 'gemini',
          status: 'paused',
          created_at: Date.now(),
        },
        {
          session_id: 'session-2',
          bead_id: 'bp6-test',
          persona: 'qa_engineer',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now() + 1000,
        },
        {
          session_id: 'session-3',
          bead_id: 'bp6-test',
          persona: 'specialist',
          backend_id: 'gemini',
          status: 'paused',
          created_at: Date.now() + 2000,
        },
        {
          session_id: 'session-4',
          bead_id: 'bp6-test',
          persona: 'product_manager',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now() + 3000,
        },
      ];

      const { container } = render(<SessionIndicator sessions={sessions} />);

      // Should show 2 running sessions
      const indicator = screen.getByTitle(/2 active sessions/);
      expect(indicator).toBeTruthy();

      // Count badge should show 2
      const countBadge = container.querySelector('.bg-indigo-600');
      expect(countBadge?.textContent).toBe('2');

      // Tooltip should only include running sessions
      expect(indicator.title).toContain('🧪 qa engineer');
      expect(indicator.title).toContain('📋 product manager');
    });
  });

  // ============================================================================
  // Edge Cases
  // ============================================================================

  describe('Edge Cases', () => {
    it('should handle sessions with same persona', () => {
      const sessions: SessionInfo[] = [
        {
          session_id: 'session-1',
          bead_id: 'bp6-test',
          persona: 'product_manager',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now(),
        },
        {
          session_id: 'session-2',
          bead_id: 'bp6-test',
          persona: 'product_manager',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now() + 1000,
        },
      ];

      const { container } = render(<SessionIndicator sessions={sessions} />);

      // Should show count of 2
      const countBadge = container.querySelector('.bg-indigo-600');
      expect(countBadge?.textContent).toBe('2');

      // Should show both in tooltip
      const indicator = screen.getByTitle(/2 active sessions/);
      expect(indicator.title).toContain('📋 product manager, 📋 product manager');
    });

    it('should handle large number of sessions', () => {
      const sessions: SessionInfo[] = Array.from({ length: 10 }, (_, i) => ({
        session_id: `session-${i}`,
        bead_id: 'bp6-test',
        persona: i % 3 === 0 ? 'product_manager' : i % 3 === 1 ? 'qa_engineer' : 'specialist',
        backend_id: 'gemini',
        status: 'running' as const,
        created_at: Date.now() + i * 1000,
      }));

      const { container } = render(<SessionIndicator sessions={sessions} />);

      // Should show count of 10
      const countBadge = container.querySelector('.bg-indigo-600');
      expect(countBadge?.textContent).toBe('10');

      // Should show correct count in tooltip
      const indicator = screen.getByTitle(/10 active sessions/);
      expect(indicator).toBeTruthy();
    });

    it('should handle sessions with different backend_ids', () => {
      const sessions: SessionInfo[] = [
        {
          session_id: 'session-1',
          bead_id: 'bp6-test',
          persona: 'product_manager',
          backend_id: 'gemini',
          status: 'running',
          created_at: Date.now(),
        },
        {
          session_id: 'session-2',
          bead_id: 'bp6-test',
          persona: 'qa_engineer',
          backend_id: 'claude',
          status: 'running',
          created_at: Date.now() + 1000,
        },
      ];

      const { container } = render(<SessionIndicator sessions={sessions} />);

      // Should still show count of 2 regardless of backend
      const countBadge = container.querySelector('.bg-indigo-600');
      expect(countBadge?.textContent).toBe('2');
    });
  });
});
