import { atom } from "jotai";
import type { DagNode, DagEdge, ProjectInfo, ProbeData, QueueItem, WorkflowInfo, AgentStdoutLine } from "../types";

// ── Atoms ──────────────────────────────────────────────────────────────────────

export const projectAtom = atom<ProjectInfo | null>(null);

export const nodesAtom = atom<Map<string, DagNode>>(new Map());

export const edgesAtom = atom<Map<string, DagEdge>>(new Map());

export const queueItemsAtom = atom<Map<string, QueueItem>>(new Map());

// Mission control atoms (bp6-80q)
export const selectedNodeIdAtom = atom<string | null>(null);

export const probeDataAtom = atom<ProbeData | null>(null);

export const workflowsAtom = atom<Map<string, WorkflowInfo>>(new Map());

export const agentStdoutAtom = atom<Map<string, AgentStdoutLine[]>>(new Map());

// ── Derived ────────────────────────────────────────────────────────────────────

export const nodesListAtom = atom((get) => Array.from(get(nodesAtom).values()));

export const edgesListAtom = atom((get) => Array.from(get(edgesAtom).values()));

export const queueItemsListAtom = atom((get) =>
  Array.from(get(queueItemsAtom).values()).sort(
    (a, b) => a.priority - b.priority || a.createdAt.localeCompare(b.createdAt)
  )
);

export const workflowsListAtom = atom((get) =>
  Array.from(get(workflowsAtom).values())
);
