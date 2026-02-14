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
          sessionId: 'session-1',
          beadId: 'bp6-test',
          persona: 'product-manager',
          backendId: 'gemini',
          status: 'paused',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
        },
        {
          sessionId: 'session-2',
          beadId: 'bp6-test',
          persona: 'qa-engineer',
          backendId: 'gemini',
          status: 'paused',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
        },
      ];

      const { container } = render(<SessionIndicator sessions={pausedSessions} />);
      expect(container.firstChild).toBeNull();
    });

    it('should render pulsing dot for single active session', () => {
      const sessions: SessionInfo[] = [
        {
          sessionId: 'session-1',
          beadId: 'bp6-test',
          persona: 'product-manager',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
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
          sessionId: 'session-1',
          beadId: 'bp6-test',
          persona: 'product-manager',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
        },
      ];

      const { container } = render(<SessionIndicator sessions={sessions} />);

      // Check for persona icon (should be 📋 for product_manager)
      expect(container.textContent).toContain('📋');
    });

    it('should render count badge for multiple sessions', () => {
      const sessions: SessionInfo[] = [
        {
          sessionId: 'session-1',
          beadId: 'bp6-test',
          persona: 'product-manager',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
        },
        {
          sessionId: 'session-2',
          beadId: 'bp6-test',
          persona: 'qa-engineer',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now() + 1000,
          lastActivity: (Date.now() + 1000) / 1000,
          hasUnread: false,
          messageCount: 0,
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
          sessionId: 'session-1',
          beadId: 'bp6-test',
          persona: 'product-manager',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
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
          sessionId: 'session-1',
          beadId: 'bp6-test',
          persona: 'product-manager',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
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
          sessionId: 'session-1',
          beadId: 'bp6-test',
          persona: 'product-manager',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
        },
      ];

      const { container } = render(<SessionIndicator sessions={sessions} />);
      expect(container.textContent).toContain('📋');
    });

    it('should display correct icon for qa_engineer', () => {
      const sessions: SessionInfo[] = [
        {
          sessionId: 'session-1',
          beadId: 'bp6-test',
          persona: 'qa-engineer',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
        },
      ];

      const { container } = render(<SessionIndicator sessions={sessions} />);
      expect(container.textContent).toContain('🧪');
    });

    it('should display correct icon for specialist', () => {
      const sessions: SessionInfo[] = [
        {
          sessionId: 'session-1',
          beadId: 'bp6-test',
          persona: 'specialist',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
        },
      ];

      const { container } = render(<SessionIndicator sessions={sessions} />);
      expect(container.textContent).toContain('⚡');
    });

    it('should display fallback icon for unknown persona', () => {
      const sessions: SessionInfo[] = [
        {
          sessionId: 'session-1',
          beadId: 'bp6-test',
          persona: 'unknown_persona',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
        },
      ];

      const { container } = render(<SessionIndicator sessions={sessions} />);
      expect(container.textContent).toContain('🤖');
    });

    it('should use first active session persona when multiple sessions', () => {
      const sessions: SessionInfo[] = [
        {
          sessionId: 'session-1',
          beadId: 'bp6-test',
          persona: 'product-manager',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
        },
        {
          sessionId: 'session-2',
          beadId: 'bp6-test',
          persona: 'qa-engineer',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now() + 1000,
          lastActivity: (Date.now() + 1000) / 1000,
          hasUnread: false,
          messageCount: 0,
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
          sessionId: 'session-1',
          beadId: 'bp6-test',
          persona: 'product-manager',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
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
          sessionId: 'session-1',
          beadId: 'bp6-test',
          persona: 'product-manager',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
        },
        {
          sessionId: 'session-2',
          beadId: 'bp6-test',
          persona: 'qa-engineer',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now() + 1000,
          lastActivity: (Date.now() + 1000) / 1000,
          hasUnread: false,
          messageCount: 0,
        },
        {
          sessionId: 'session-3',
          beadId: 'bp6-test',
          persona: 'specialist',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now() + 2000,
          lastActivity: (Date.now() + 2000) / 1000,
          hasUnread: false,
          messageCount: 0,
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
          sessionId: 'session-1',
          beadId: 'bp6-test',
          persona: 'product-manager',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
        },
        {
          sessionId: 'session-2',
          beadId: 'bp6-test',
          persona: 'qa-engineer',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now() + 1000,
          lastActivity: (Date.now() + 1000) / 1000,
          hasUnread: false,
          messageCount: 0,
        },
      ];

      render(<SessionIndicator sessions={sessions} />);

      const indicator = screen.getByTitle(/2 active sessions/);
      expect(indicator.title).toMatch(/2 active sessions:/);
    });

    it('should use singular session in tooltip when count = 1', () => {
      const sessions: SessionInfo[] = [
        {
          sessionId: 'session-1',
          beadId: 'bp6-test',
          persona: 'product-manager',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
        },
      ];

      render(<SessionIndicator sessions={sessions} />);

      const indicator = screen.getByTitle(/1 active session/);
      expect(indicator.title).toMatch(/1 active session:/);
    });

    it('should replace underscores with spaces in persona names', () => {
      const sessions: SessionInfo[] = [
        {
          sessionId: 'session-1',
          beadId: 'bp6-test',
          persona: 'qa-engineer',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
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
          sessionId: 'session-1',
          beadId: 'bp6-test',
          persona: 'product-manager',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
        },
        {
          sessionId: 'session-2',
          beadId: 'bp6-test',
          persona: 'qa-engineer',
          backendId: 'gemini',
          status: 'paused',
          createdAt: Date.now() + 1000,
          lastActivity: (Date.now() + 1000) / 1000,
          hasUnread: false,
          messageCount: 0,
        },
        {
          sessionId: 'session-3',
          beadId: 'bp6-test',
          persona: 'specialist',
          backendId: 'gemini',
          status: 'paused',
          createdAt: Date.now() + 2000,
          lastActivity: (Date.now() + 2000) / 1000,
          hasUnread: false,
          messageCount: 0,
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
          sessionId: 'session-1',
          beadId: 'bp6-test',
          persona: 'product-manager',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
        },
        {
          sessionId: 'session-2',
          beadId: 'bp6-test',
          persona: 'qa-engineer',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now() + 1000,
          lastActivity: (Date.now() + 1000) / 1000,
          hasUnread: false,
          messageCount: 0,
        },
        {
          sessionId: 'session-3',
          beadId: 'bp6-test',
          persona: 'specialist',
          backendId: 'gemini',
          status: 'paused',
          createdAt: Date.now() + 2000,
          lastActivity: (Date.now() + 2000) / 1000,
          hasUnread: false,
          messageCount: 0,
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
          sessionId: 'session-1',
          beadId: 'bp6-test',
          persona: 'product-manager',
          backendId: 'gemini',
          status: 'paused',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
        },
        {
          sessionId: 'session-2',
          beadId: 'bp6-test',
          persona: 'qa-engineer',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now() + 1000,
          lastActivity: (Date.now() + 1000) / 1000,
          hasUnread: false,
          messageCount: 0,
        },
        {
          sessionId: 'session-3',
          beadId: 'bp6-test',
          persona: 'specialist',
          backendId: 'gemini',
          status: 'paused',
          createdAt: Date.now() + 2000,
          lastActivity: (Date.now() + 2000) / 1000,
          hasUnread: false,
          messageCount: 0,
        },
        {
          sessionId: 'session-4',
          beadId: 'bp6-test',
          persona: 'product-manager',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now() + 3000,
          lastActivity: (Date.now() + 3000) / 1000,
          hasUnread: false,
          messageCount: 0,
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
          sessionId: 'session-1',
          beadId: 'bp6-test',
          persona: 'product-manager',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
        },
        {
          sessionId: 'session-2',
          beadId: 'bp6-test',
          persona: 'product-manager',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now() + 1000,
          lastActivity: (Date.now() + 1000) / 1000,
          hasUnread: false,
          messageCount: 0,
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
        sessionId: `session-${i}`,
        beadId: 'bp6-test',
        persona: i % 3 === 0 ? 'product_manager' : i % 3 === 1 ? 'qa_engineer' : 'specialist',
        backendId: 'gemini',
        status: 'running' as const,
        createdAt: Date.now() + i * 1000,
        lastActivity: (Date.now() + i * 1000) / 1000,
        hasUnread: false,
        messageCount: 0,
      }));

      const { container } = render(<SessionIndicator sessions={sessions} />);

      // Should show count of 10
      const countBadge = container.querySelector('.bg-indigo-600');
      expect(countBadge?.textContent).toBe('10');

      // Should show correct count in tooltip
      const indicator = screen.getByTitle(/10 active sessions/);
      expect(indicator).toBeTruthy();
    });

    it('should handle sessions with different backendIds', () => {
      const sessions: SessionInfo[] = [
        {
          sessionId: 'session-1',
          beadId: 'bp6-test',
          persona: 'product-manager',
          backendId: 'gemini',
          status: 'running',
          createdAt: Date.now(),
          lastActivity: Date.now() / 1000,
          hasUnread: false,
          messageCount: 0,
        },
        {
          sessionId: 'session-2',
          beadId: 'bp6-test',
          persona: 'qa-engineer',
          backendId: 'claude',
          status: 'running',
          createdAt: Date.now() + 1000,
          lastActivity: (Date.now() + 1000) / 1000,
          hasUnread: false,
          messageCount: 0,
        },
      ];

      const { container } = render(<SessionIndicator sessions={sessions} />);

      // Should still show count of 2 regardless of backend
      const countBadge = container.querySelector('.bg-indigo-600');
      expect(countBadge?.textContent).toBe('2');
    });
  });
});
