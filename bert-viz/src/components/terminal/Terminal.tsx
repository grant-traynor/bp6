import { useEffect, useRef, useState } from 'react';
import { Terminal as XTerm } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { ClipboardAddon } from '@xterm/addon-clipboard';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import '@xterm/xterm/css/xterm.css';
import './Terminal.css';

interface TerminalProps {
  sessionId: string;
  onReady?: () => void;
}

interface PtyDataEvent {
  sessionId: string;
  data: string;
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
export const Terminal: React.FC<TerminalProps> = ({ sessionId, onReady }) => {
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

    const theme = isDark
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

    xtermRef.current.options.theme = theme;
  }, [isDark]);

  useEffect(() => {
    console.log('Terminal component mounting for session:', sessionId);
    if (!terminalRef.current) return;

    // Get initial theme based on dark mode
    const theme = isDark
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

    // Initialize Xterm.js terminal
    console.log('Initializing Xterm.js terminal');
    const terminal = new XTerm({
      cursorBlink: true,
      fontSize: 13,
      fontFamily: 'SF Mono, Monaco, Menlo, "Courier New", monospace',
      fontWeight: '400',
      fontWeightBold: '700',
      letterSpacing: 0,
      lineHeight: 1.2,
      theme,
      scrollback: 1000,
      convertEol: true,
    });

    // Initialize fit addon for auto-resize
    const fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);

    // Initialize clipboard addon for copy/paste support
    const clipboardAddon = new ClipboardAddon();
    terminal.loadAddon(clipboardAddon);

    // Open terminal in DOM
    terminal.open(terminalRef.current);
    fitAddon.fit();

    // Store refs
    xtermRef.current = terminal;
    fitAddonRef.current = fitAddon;

    // Notify backend of initial terminal size
    const initialDimensions = fitAddon.proposeDimensions();
    if (initialDimensions) {
      invoke('resize_pty', {
        sessionId,
        cols: initialDimensions.cols,
        rows: initialDimensions.rows,
      }).catch((error) => {
        console.error('Failed to set initial PTY size:', error);
      });
    }

    // Handle terminal input (send to backend)
    terminal.onData((data) => {
      invoke('write_to_pty', { sessionId, data }).catch((error) => {
        console.error('Failed to write to PTY:', error);
      });
    });

    // Listen for PTY output from backend
    console.log('Setting up pty-data event listener for session:', sessionId);
    const unlistenPromise = listen<PtyDataEvent>('pty-data', (event) => {
      console.log('Received pty-data event:', event.payload.sessionId, event.payload.data.substring(0, 50));
      if (event.payload.sessionId === sessionId) {
        terminal.write(event.payload.data);

        // On initial load, debounce scrolling to bottom to avoid watching the buffer fill
        if (initialLoadRef.current) {
          // Clear any existing timeout
          if (scrollTimeoutRef.current) {
            clearTimeout(scrollTimeoutRef.current);
          }

          // Set timeout to scroll to bottom after data stops flowing
          scrollTimeoutRef.current = setTimeout(() => {
            terminal.scrollToBottom();
            initialLoadRef.current = false;
            console.log('Initial load complete, scrolled to bottom');
          }, 200); // Wait 200ms after last data chunk
        }
      }
    });

    // Handle window resize (debounced)
    const handleResize = () => {
      if (!xtermRef.current) return;

      // Clear any existing resize timeout
      if (resizeTimeoutRef.current) {
        clearTimeout(resizeTimeoutRef.current);
      }

      // Debounce resize to avoid rapid reflows
      resizeTimeoutRef.current = setTimeout(() => {
        if (!xtermRef.current) return;

        // Save current scroll position
        const wasAtBottom = xtermRef.current.buffer.active.viewportY === xtermRef.current.buffer.active.baseY;

        // Resize the terminal display
        fitAddon.fit();

        // Restore scroll position: stay at bottom if we were at bottom
        if (wasAtBottom) {
          xtermRef.current.scrollToBottom();
        }

        // Notify backend of new terminal size
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
      }, 150); // Wait 150ms after last resize event
    };

    // Set up resize observer
    const resizeObserver = new ResizeObserver(handleResize);
    resizeObserver.observe(terminalRef.current);

    // Call onReady callback
    if (onReady) {
      onReady();
    }

    // Cleanup
    return () => {
      if (scrollTimeoutRef.current) {
        clearTimeout(scrollTimeoutRef.current);
      }
      if (resizeTimeoutRef.current) {
        clearTimeout(resizeTimeoutRef.current);
      }
      unlistenPromise.then((unlisten) => unlisten());
      resizeObserver.disconnect();
      terminal.dispose();
      xtermRef.current = null;
      fitAddonRef.current = null;
    };
  }, [sessionId, onReady, isDark]);

  return (
    <div className="terminal-container">
      <div ref={terminalRef} className="terminal" />
    </div>
  );
};
