/**
 * bp6-h82: xterm.js terminal pane for agent stdout
 *
 * Subscribes to agent:stdout Tauri events for a given workflowId and renders
 * the raw ANSI output (colours, cursor movement, progress bars) correctly.
 */
import { useEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { listen } from "@tauri-apps/api/event";
import type { AgentStdoutLine } from "../types";

interface XtermPaneProps {
  workflowId: string;
  height?: number;
}

export function XtermPane({ workflowId, height = 300 }: XtermPaneProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    const term = new Terminal({
      theme: {
        background: "#0a0a0a",
        foreground: "#e5e5e5",
        cursor: "#e5e5e5",
      },
      fontFamily: "monospace",
      fontSize: 13,
      scrollback: 5000,
      convertEol: true,
    });

    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(containerRef.current);
    fit.fit();

    termRef.current = term;
    fitRef.current = fit;

    let unlisten: (() => void) | undefined;
    listen<AgentStdoutLine>("agent:stdout", (event) => {
      if (event.payload.workflowId === workflowId) {
        term.writeln(event.payload.line);
      }
    }).then((fn) => {
      unlisten = fn;
    });

    const observer = new ResizeObserver(() => fit.fit());
    observer.observe(containerRef.current);

    return () => {
      unlisten?.();
      observer.disconnect();
      term.dispose();
    };
  }, [workflowId]);

  return (
    <div
      ref={containerRef}
      style={{ height, width: "100%" }}
      className="border-2 border-black overflow-hidden"
    />
  );
}
