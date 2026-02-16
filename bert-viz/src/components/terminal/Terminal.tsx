import { useEffect, useRef } from 'react';
import { Terminal as XTerm } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
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
 */
export const Terminal: React.FC<TerminalProps> = ({ sessionId, onReady }) => {
  const terminalRef = useRef<HTMLDivElement>(null);
  const xtermRef = useRef<XTerm | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const initialLoadRef = useRef<boolean>(true);
  const scrollTimeoutRef = useRef<number | null>(null);

  useEffect(() => {
    console.log('Terminal component mounting for session:', sessionId);
    if (!terminalRef.current) return;

    // Initialize Xterm.js terminal
    console.log('Initializing Xterm.js terminal');
    const terminal = new XTerm({
      cursorBlink: true,
      fontSize: 14,
      fontFamily: 'Menlo, Monaco, "Courier New", monospace',
      theme: {
        background: '#1e1e1e',
        foreground: '#d4d4d4',
        cursor: '#ffffff',
        selectionBackground: 'rgba(255, 255, 255, 0.3)',
        black: '#000000',
        red: '#cd3131',
        green: '#0dbc79',
        yellow: '#e5e510',
        blue: '#2472c8',
        magenta: '#bc3fbc',
        cyan: '#11a8cd',
        white: '#e5e5e5',
        brightBlack: '#666666',
        brightRed: '#f14c4c',
        brightGreen: '#23d18b',
        brightYellow: '#f5f543',
        brightBlue: '#3b8eea',
        brightMagenta: '#d670d6',
        brightCyan: '#29b8db',
        brightWhite: '#ffffff',
      },
      scrollback: 1000,
      convertEol: true,
    });

    // Initialize fit addon for auto-resize
    const fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);

    // Open terminal in DOM
    terminal.open(terminalRef.current);
    fitAddon.fit();

    // Store refs
    xtermRef.current = terminal;
    fitAddonRef.current = fitAddon;

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

    // Handle window resize
    const handleResize = () => {
      fitAddon.fit();

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
      unlistenPromise.then((unlisten) => unlisten());
      resizeObserver.disconnect();
      terminal.dispose();
      xtermRef.current = null;
      fitAddonRef.current = null;
    };
  }, [sessionId, onReady]);

  return (
    <div className="terminal-container">
      <div ref={terminalRef} className="terminal" />
    </div>
  );
};
