/**
 * bp6-80q.2: Agent activity view
 *
 * Per-agent real-time view. Subscribes to workflow:status and agent:stdout
 * events (ready for Phase 3). Shows live stdout stream, current step, elapsed time.
 */
import { useState, useEffect, useRef } from "react";
import { useAtomValue, useSetAtom } from "jotai";
import { workflowsListAtom, agentStdoutAtom, selectedNodeIdAtom } from "../store/dag";
import type { WorkflowInfo } from "../types";

const STATUS_DOT: Record<string, string> = {
  running: "bg-blue-500 animate-pulse",
  paused: "bg-amber-400",
  blocked: "bg-red-500",
  completed: "bg-green-500",
  cancelled: "bg-stone-400",
  failed: "bg-red-700",
};

function elapsed(startedAt: string): string {
  const ms = Date.now() - new Date(startedAt).getTime();
  const s = Math.floor(ms / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${s % 60}s`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}

function AgentPanel({ workflow }: { workflow: WorkflowInfo }) {
  const agentStdout = useAtomValue(agentStdoutAtom);
  const setSelectedNodeId = useSetAtom(selectedNodeIdAtom);
  const lines = agentStdout.get(workflow.workflowId) ?? [];
  const logRef = useRef<HTMLDivElement>(null);
  const [, setTick] = useState(0);

  // Re-render elapsed every 5s
  useEffect(() => {
    const id = setInterval(() => setTick((t) => t + 1), 5000);
    return () => clearInterval(id);
  }, []);

  // Auto-scroll stdout
  useEffect(() => {
    if (logRef.current) {
      logRef.current.scrollTop = logRef.current.scrollHeight;
    }
  }, [lines.length]);

  const dot = STATUS_DOT[workflow.status] ?? "bg-stone-300";

  return (
    <div className="border-4 border-black flex flex-col">
      {/* Agent header */}
      <div className="border-b-4 border-black px-4 py-2 flex items-center justify-between bg-white">
        <div className="flex items-center gap-3">
          <span className={`w-3 h-3 rounded-full ${dot} shrink-0`} />
          <div>
            <p className="font-bold text-sm font-mono">{workflow.agentId}</p>
            <p className="font-mono text-xs text-stone-400">
              wf: {workflow.workflowId.slice(0, 12)}…
            </p>
          </div>
        </div>
        <div className="flex items-center gap-3">
          {workflow.currentStep && (
            <span className="font-mono text-xs border-2 border-black px-2 py-0.5">
              {workflow.currentStep}
            </span>
          )}
          <span className="font-mono text-xs text-stone-400">{elapsed(workflow.startedAt)}</span>
          <button
            onClick={() => setSelectedNodeId(workflow.nodeId)}
            className="font-mono text-xs border-2 border-black px-2 py-0.5 hover:bg-black hover:text-white transition-colors"
          >
            Probe →
          </button>
        </div>
      </div>

      {/* Stdout stream */}
      <div
        ref={logRef}
        className="flex-1 overflow-auto bg-stone-950 text-green-400 font-mono text-xs p-3 min-h-32 max-h-48"
      >
        {lines.length === 0 ? (
          <span className="text-stone-600">waiting for output…</span>
        ) : (
          lines.map((l, i) => (
            <div key={i} className="whitespace-pre-wrap break-all leading-5">
              {l.line}
            </div>
          ))
        )}
      </div>
    </div>
  );
}

export function AgentActivityView() {
  const workflows = useAtomValue(workflowsListAtom);
  const active = workflows.filter((w) => w.status === "running" || w.status === "paused" || w.status === "blocked");

  if (active.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="border-4 border-dashed border-stone-300 p-16 text-center">
          <p className="font-mono text-stone-400 text-sm">No agents running.</p>
          <p className="font-mono text-stone-300 text-xs mt-1">
            Agents will appear here when workflows start (Phase 3).
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-auto p-4 space-y-4">
      {active.map((wf) => (
        <AgentPanel key={wf.workflowId} workflow={wf} />
      ))}
    </div>
  );
}
