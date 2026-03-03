import { useState, useRef, useEffect, useCallback } from "react";
import { createPortal } from "react-dom";
import { cn } from "../../utils";
import { PERSONA_ICONS, type SessionInfo, createSessionWindow, resumeSpecificSession } from "../../api";

interface SessionIndicatorProps {
  sessions: SessionInfo[];
  className?: string;
  onSessionClick?: (sessionId: string) => void;
}

interface PopoverProps {
  session: SessionInfo;
  anchorRect: DOMRect;
  onClose: () => void;
  onOpen: (session: SessionInfo) => void;
}

function SessionPopover({ session, anchorRect, onClose, onOpen }: PopoverProps) {
  const popoverRef = useRef<HTMLDivElement>(null);
  const icon = PERSONA_ICONS[session.role || session.persona] || '🤖';
  const personaLabel = (session.role || session.persona || '').replace(/-/g, ' ');
  const isRunning = session.status === 'running';

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    const handleClickOutside = (e: MouseEvent) => {
      if (popoverRef.current && !popoverRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    document.addEventListener('mousedown', handleClickOutside);
    return () => {
      document.removeEventListener('keydown', handleKeyDown);
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [onClose]);

  // Position: above the anchor, centered
  const top = anchorRect.top - 8;
  const left = anchorRect.left + anchorRect.width / 2;

  return createPortal(
    <div
      ref={popoverRef}
      className="fixed z-[9999] min-w-[180px] max-w-[240px]"
      style={{ top, left, transform: 'translate(-50%, -100%)' }}
      onClick={e => e.stopPropagation()}
    >
      <div className="bg-[var(--background-secondary)] border border-[var(--border-primary)] rounded-lg shadow-xl p-3 text-left mb-2">
        {/* Header row: icon + persona */}
        <div className="flex items-center gap-2 mb-2">
          <span className="text-base">{icon}</span>
          <span className="text-xs font-semibold text-[var(--text-primary)] capitalize">
            {personaLabel}
          </span>
          <span className={cn(
            "ml-auto text-[9px] font-bold px-1.5 py-0.5 rounded-full uppercase tracking-wider",
            isRunning
              ? "bg-green-500/20 text-green-600 dark:text-green-400"
              : "bg-[var(--background-tertiary)] text-[var(--text-muted)]"
          )}>
            {session.status}
          </span>
        </div>

        {/* Task */}
        {session.task && (
          <p className="text-[10px] text-[var(--text-secondary)] mb-2 truncate">
            {session.task}
          </p>
        )}

        {/* Backend */}
        <p className="text-[10px] text-[var(--text-muted)] mb-3">
          {session.backendId}
        </p>

        {/* Open Session button */}
        <button
          className="w-full text-xs font-semibold bg-[var(--accent-primary)] text-white rounded-md px-3 py-1.5 hover:opacity-90 transition-opacity"
          onClick={(e) => { e.stopPropagation(); onOpen(session); }}
        >
          Open Session
        </button>
      </div>
    </div>,
    document.body
  );
}

export const SessionIndicator = ({ sessions, className, onSessionClick }: SessionIndicatorProps) => {
  const [openPopover, setOpenPopover] = useState<{ idx: number; rect: DOMRect } | null>(null);
  const [scrollOffset, setScrollOffset] = useState(0);
  const buttonRefs = useRef<(HTMLButtonElement | null)[]>([]);

  const VISIBLE = 3;

  if (!sessions || sessions.length === 0) return null;

  const canScrollLeft = scrollOffset > 0;
  const canScrollRight = scrollOffset + VISIBLE < sessions.length;
  const visibleSessions = sessions.slice(scrollOffset, scrollOffset + VISIBLE);

  const handleIconClick = useCallback((localIdx: number, e: React.MouseEvent) => {
    e.stopPropagation();
    const globalIdx = scrollOffset + localIdx;
    const btn = buttonRefs.current[localIdx];
    if (!btn) return;
    const rect = btn.getBoundingClientRect();
    setOpenPopover(prev =>
      prev?.idx === globalIdx ? null : { idx: globalIdx, rect }
    );
  }, [scrollOffset]);

  const handleOpenSession = useCallback(async (session: SessionInfo) => {
    setOpenPopover(null);
    try {
      if (session.status === 'stopped') {
        const sessionId = await resumeSpecificSession(session.sessionId);
        await createSessionWindow(sessionId);
        onSessionClick?.(sessionId);
      } else {
        await createSessionWindow(session.sessionId);
        onSessionClick?.(session.sessionId);
      }
    } catch (error) {
      console.error('Failed to open session:', error);
    }
  }, [onSessionClick]);

  const activeSession = openPopover !== null ? sessions[openPopover.idx] : null;

  return (
    <div className={cn("flex items-center gap-0.5 shrink-0", className)}>
      {/* Left scroll arrow */}
      {canScrollLeft && (
        <button
          className="w-3.5 h-3.5 flex items-center justify-center text-[9px] text-[var(--text-muted)] hover:text-[var(--text-primary)] rounded transition-colors shrink-0"
          onClick={(e) => { e.stopPropagation(); setScrollOffset(s => s - 1); setOpenPopover(null); }}
          title="Scroll left"
        >
          ‹
        </button>
      )}

      {/* Session icon buttons */}
      {visibleSessions.map((session, localIdx) => {
        const globalIdx = scrollOffset + localIdx;
        const icon = PERSONA_ICONS[session.role || session.persona] || '🤖';
        const isRunning = session.status === 'running';
        const isOpen = openPopover?.idx === globalIdx;

        return (
          <div key={session.sessionId} className="relative shrink-0">
            <button
              ref={el => { buttonRefs.current[localIdx] = el; }}
              title={`${(session.role || session.persona).replace(/-/g, ' ')} (${session.status})`}
              onClick={(e) => handleIconClick(localIdx, e)}
              className={cn(
                "w-[22px] h-[22px] rounded-full flex items-center justify-center text-[11px] border transition-all relative",
                isOpen
                  ? "bg-[var(--accent-primary)]/20 border-[var(--accent-primary)]/60 scale-110"
                  : isRunning
                    ? "bg-green-500/15 border-green-500/40 hover:bg-green-500/25 hover:scale-110"
                    : "bg-[var(--background-tertiary)] border-[var(--border-primary)] opacity-50 hover:opacity-80 hover:scale-110"
              )}
            >
              {icon}
              {isRunning && (
                <span className="absolute -top-0.5 -right-0.5 w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse-slow" />
              )}
            </button>
          </div>
        );
      })}

      {/* Right scroll arrow */}
      {canScrollRight && (
        <button
          className="w-3.5 h-3.5 flex items-center justify-center text-[9px] text-[var(--text-muted)] hover:text-[var(--text-primary)] rounded transition-colors shrink-0"
          onClick={(e) => { e.stopPropagation(); setScrollOffset(s => s + 1); setOpenPopover(null); }}
          title="Scroll right"
        >
          ›
        </button>
      )}

      {/* Popover portal */}
      {activeSession && openPopover && (
        <SessionPopover
          session={activeSession}
          anchorRect={openPopover.rect}
          onClose={() => setOpenPopover(null)}
          onOpen={handleOpenSession}
        />
      )}
    </div>
  );
};
