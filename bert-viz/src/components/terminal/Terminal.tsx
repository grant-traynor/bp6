import { useEffect, useRef, useState } from 'react';
import { Terminal as XTerm } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { ClipboardAddon } from '@xterm/addon-clipboard';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { spawnProjectShell } from '../../api';
import '@xterm/xterm/css/xterm.css';
import './Terminal.css';

interface TerminalProps {
  sessionId: string;
  projectPath?: string;
  onReady?: () => void;
}

interface PtyDataEvent {
  sessionId: string;
  data: string;
}

function buildTheme(isDark: boolean) {
  return isDark
    ? {
        background: '#0e1624',
        foreground: '#e9f1ff',
        cursor: '#5cf1ff',
        cursorAccent: '#0e1624',
        selectionBackground: 'rgba(59, 163, 255, 0.3)',
        black: '#05070d',
        red: '#f06d76',
        green: '#2fc6a0',
        yellow: '#f2a93b',
        blue: '#3ba3ff',
        magenta: '#9c5cff',
        cyan: '#5cf1ff',
        white: '#b6c4d9',
        brightBlack: '#1f2a3a',
        brightRed: '#ff8087',
        brightGreen: '#3dd7b0',
        brightYellow: '#ffba4a',
        brightBlue: '#5cb6ff',
        brightMagenta: '#b87cff',
        brightCyan: '#7cf5ff',
        brightWhite: '#e9f1ff',
      }
    : {
        background: '#f6f9ff',
        foreground: '#0c1a2a',
        cursor: '#0f8bff',
        cursorAccent: '#f6f9ff',
        selectionBackground: 'rgba(15, 139, 255, 0.25)',
        black: '#0c1a2a',
        red: '#f06d76',
        green: '#2fc6a0',
        yellow: '#d97706',
        blue: '#0f8bff',
        magenta: '#9c5cff',
        cyan: '#3bc8ff',
        white: '#55657d',
        brightBlack: '#24344d',
        brightRed: '#ff8087',
        brightGreen: '#3dd7b0',
        brightYellow: '#f2a93b',
        brightBlue: '#3ba3ff',
        brightMagenta: '#b87cff',
        brightCyan: '#5cf1ff',
        brightWhite: '#0c1a2a',
      };
}

/**
 * Terminal - Xterm.js terminal component for PTY sessions
 *
 * Features:
 * - Renders Xterm.js terminal emulator
 * - Connects to backend PTY via Tauri events
 * - Handles keyboard input and sends to backend
 * - Auto-resizes with container
 * - Supports ANSI colors and formatting
 * - Copy/paste with keyboard shortcuts (Cmd+C/V on macOS, Ctrl+C/V on Windows/Linux)
 * - Right-click context menu for copy/paste
 */
export const Terminal: React.FC<TerminalProps> = ({ sessionId, projectPath, onReady }) => {
  const terminalRef = useRef<HTMLDivElement>(null);
  const xtermRef = useRef<XTerm | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const initialLoadRef = useRef<boolean>(true);
  const scrollTimeoutRef = useRef<number | null>(null);
  const resizeTimeoutRef = useRef<number | null>(null);
  const [isDark, setIsDark] = useState<boolean>(() => {
    return document.documentElement.classList.contains('dark');
  });

  // Detect theme changes
  useEffect(() => {
    const observer = new MutationObserver(() => {
      const newIsDark = document.documentElement.classList.contains('dark');
      if (newIsDark !== isDark) {
        setIsDark(newIsDark);
      }
    });

    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ['class'],
    });

    return () => observer.disconnect();
  }, [isDark]);

  // Update terminal theme when dark mode changes
  useEffect(() => {
    if (!xtermRef.current) return;
    xtermRef.current.options.theme = buildTheme(isDark);
  }, [isDark]);

  useEffect(() => {
    if (!terminalRef.current) return;

    // Track whether this effect instance is still active.
    // Used to cancel setup if the component unmounts while fonts are loading.
    let isActive = true;
    // These are set inside the async callback but need to be reachable by
    // the synchronous cleanup function.
    let unlistenFn: (() => void) | undefined;
    let resizeObserver: ResizeObserver | undefined;

    // Open the terminal AFTER fonts are ready.
    //
    // xterm.js measures character cell dimensions during terminal.open().
    // If the call happens before the custom Nerd Font loads, xterm.js uses the
    // fallback font's metrics. This causes fitAddon.proposeDimensions() to return
    // the wrong col/row count, which we then send to the PTY. tmux draws its
    // panes based on those wrong dimensions, so split-pane dividers end up at the
    // wrong visual position and pane content bleeds across column boundaries.
    document.fonts.ready.then(() => {
      if (!isActive || !terminalRef.current) return;

      const terminal = new XTerm({
        cursorBlink: true,
        fontSize: 13,
        fontFamily: '"JetBrainsMono Nerd Font", "JetBrains Mono", "MesloLGS NF", "Hack Nerd Font", "FiraCode Nerd Font Mono", "Cascadia Code NF", monospace',
        fontWeight: '400',
        fontWeightBold: '700',
        letterSpacing: 0,
        lineHeight: 1.2,
        theme: buildTheme(isDark),
        scrollback: 1000,
      });

      const fitAddon = new FitAddon();
      terminal.loadAddon(fitAddon);

      const clipboardAddon = new ClipboardAddon();
      terminal.loadAddon(clipboardAddon);

      terminal.open(terminalRef.current!);
      fitAddon.fit();

      xtermRef.current = terminal;
      fitAddonRef.current = fitAddon;
      initialLoadRef.current = true;

      // Handle terminal input (send to backend)
      terminal.onData((data) => {
        invoke('write_to_pty', { sessionId, data }).catch((error) => {
          console.error('Failed to write to PTY:', error);
        });
      });

      // ORDERING IS CRITICAL:
      //   1. Register pty-data listener FIRST
      //   2. Spawn the shell AFTER listener is confirmed registered (.then callback)
      //
      // Why this order matters:
      //   - tmux draws its initial screen immediately after spawn. If we spawn
      //     before the listener is registered we miss that draw.
      //   - For view-switch remounts (PTY already exists, idempotent spawn):
      //     spawn_project_shell returns false, and we send a rows+1 → rows
      //     ping-pong to force tmux to repaint the blank xterm.
      //   - For new spawns, spawn_project_shell returns true: the listener
      //     already captured the initial draw — no ping-pong needed. Sending
      //     rapid SIGWINCHs races with tmux's first draw and causes content
      //     (including split pane borders) to be overwritten.
      const unlistenPromise = listen<PtyDataEvent>('pty-data', (event) => {
        if (event.payload.sessionId === sessionId) {
          terminal.write(event.payload.data);

          // On initial load, debounce scrolling to bottom
          if (initialLoadRef.current) {
            if (scrollTimeoutRef.current) clearTimeout(scrollTimeoutRef.current);
            scrollTimeoutRef.current = setTimeout(() => {
              terminal.scrollToBottom();
              initialLoadRef.current = false;
            }, 200);
          }
        }
      }).then((unlisten) => {
        // If unmounted while listen() was in flight, immediately unlisten
        if (!isActive) {
          unlisten();
          return unlisten;
        }
        unlistenFn = unlisten;

        // Re-fit now that the listener is registered (layout is fully settled)
        if (fitAddonRef.current) {
          fitAddonRef.current.fit();
          const dims = fitAddonRef.current.proposeDimensions();
          if (dims) {
            const { cols, rows } = dims;

            // rows+1 → rows ping-pong: forces SIGWINCH with an actual size change
            // so tmux unconditionally redraws. Only used for remounts (PTY already
            // exists, spawn is a no-op) where tmux won't emit a fresh draw on its own.
            // Note: cols is intentionally left unchanged — a cols ping-pong causes
            // tmux to draw junction characters at the wrong position, leaving a
            // stale character artifact at pane border intersections.
            const doRepaint = () => {
              invoke('resize_pty', { sessionId, cols, rows: rows + 1 })
                .then(() => invoke('resize_pty', { sessionId, cols, rows }))
                .catch(() => {});
            };

            if (projectPath) {
              spawnProjectShell(sessionId, projectPath, cols, rows)
                .then((isNewSession) => {
                  if (!isNewSession) {
                    // Remount: PTY existed, xterm is blank — ping-pong forces tmux repaint.
                    doRepaint();
                  }
                  // New session: listener captured the initial draw — no ping-pong.
                })
                .catch(() => {
                  // Spawn failed: attempt repaint in case PTY partially exists.
                  doRepaint();
                });
            } else {
              doRepaint();
            }
          }
        }
        return unlisten;
      });

      // Handle window resize (debounced)
      const handleResize = () => {
        if (!xtermRef.current) return;

        if (resizeTimeoutRef.current) clearTimeout(resizeTimeoutRef.current);

        resizeTimeoutRef.current = setTimeout(() => {
          if (!xtermRef.current) return;

          const wasAtBottom =
            xtermRef.current.buffer.active.viewportY ===
            xtermRef.current.buffer.active.baseY;

          fitAddon.fit();

          if (wasAtBottom) xtermRef.current.scrollToBottom();

          const dimensions = fitAddon.proposeDimensions();
          if (dimensions) {
            invoke('resize_pty', {
              sessionId,
              cols: dimensions.cols,
              rows: dimensions.rows,
            }).catch((error) => {
              console.error('Failed to resize PTY:', error);
            });
          }
        }, 150);
      };

      const ro = new ResizeObserver(handleResize);
      ro.observe(terminalRef.current!);
      resizeObserver = ro;

      if (onReady) onReady();

      // Guard against the (rare) case where the component unmounted during the
      // synchronous setup above (possible if React strict mode double-fires).
      if (!isActive) {
        unlistenPromise.then((u) => u());
        ro.disconnect();
        terminal.dispose();
        xtermRef.current = null;
        fitAddonRef.current = null;
      }
    });

    return () => {
      isActive = false;
      if (scrollTimeoutRef.current) clearTimeout(scrollTimeoutRef.current);
      if (resizeTimeoutRef.current) clearTimeout(resizeTimeoutRef.current);
      if (unlistenFn) unlistenFn();
      if (resizeObserver) resizeObserver.disconnect();
      if (xtermRef.current) {
        xtermRef.current.dispose();
        xtermRef.current = null;
      }
      fitAddonRef.current = null;
    };
  }, [sessionId, projectPath, onReady, isDark]);

  return (
    <div className="terminal-container">
      <div ref={terminalRef} className="terminal" />
    </div>
  );
};
