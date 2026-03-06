/**
 * Reactive event bridge
 *
 * Subscribes to Tauri DAG events and applies them as deltas to the Jotai store.
 * No polling — all updates arrive via events.
 */
import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useSetAtom } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import {
  nodesAtom, edgesAtom, projectAtom, queueItemsAtom,
  workflowsAtom, agentStdoutAtom, openProjectsAtom,
} from "../store/dag";
import type {
  DagNode, DagEdge, DagSnapshot, ProjectInfo, QueueItem, WorkflowInfo, AgentStdoutLine,
} from "../types";

interface ProjectWithSnapshot {
  info: ProjectInfo;
  snapshot: DagSnapshot;
}

function loadSnapshot(
  snapshot: DagSnapshot,
  setNodes: (v: Map<string, DagNode>) => void,
  setEdges: (v: Map<string, DagEdge>) => void,
  setQueueItems: (v: Map<string, QueueItem>) => void,
) {
  const nodes = new Map<string, DagNode>();
  const edges = new Map<string, DagEdge>();
  for (const node of snapshot.nodes) nodes.set(node.id, node);
  for (const edge of snapshot.edges) edges.set(edge.id, edge);
  setNodes(nodes);
  setEdges(edges);

  invoke<QueueItem[]>("list_queue_items")
    .then((items) => {
      const map = new Map<string, QueueItem>();
      for (const item of items) map.set(item.id, item);
      setQueueItems(map);
    })
    .catch(console.error);
}

export function useDagEvents() {
  const setNodes = useSetAtom(nodesAtom);
  const setEdges = useSetAtom(edgesAtom);
  const setProject = useSetAtom(projectAtom);
  const setQueueItems = useSetAtom(queueItemsAtom);
  const setWorkflows = useSetAtom(workflowsAtom);
  const setAgentStdout = useSetAtom(agentStdoutAtom);
  const setOpenProjects = useSetAtom(openProjectsAtom);

  useEffect(() => {
    const unlisten: Array<() => void> = [];

    // ── project:opened — new project added to the open set ────────────────────
    listen<ProjectWithSnapshot>("project:opened", (event) => {
      const { info, snapshot } = event.payload;
      setProject(info);
      setOpenProjects((prev) => {
        const next = new Map(prev);
        next.set(info.projectDir, info);
        return next;
      });
      loadSnapshot(snapshot, setNodes, setEdges, setQueueItems);
    }).then((u) => unlisten.push(u));

    // ── project:switched — active project changed (stays in open set) ─────────
    listen<ProjectWithSnapshot>("project:switched", (event) => {
      const { info, snapshot } = event.payload;
      setProject(info);
      setOpenProjects((prev) => {
        // Ensure it's in the map (it should be, but guard anyway)
        const next = new Map(prev);
        next.set(info.projectDir, info);
        return next;
      });
      loadSnapshot(snapshot, setNodes, setEdges, setQueueItems);
    }).then((u) => unlisten.push(u));

    // ── project:closed — remove from open set; clear active if it was active ──
    listen<{ projectDir: string }>("project:closed", (event) => {
      const { projectDir } = event.payload;
      setOpenProjects((prev) => {
        const next = new Map(prev);
        next.delete(projectDir);
        return next;
      });
      // If backend emitted project:switched after this, it will set projectAtom.
      // If not (no remaining projects), clear everything.
      setProject((prev) => {
        if (prev?.projectDir === projectDir) return null;
        return prev;
      });
      // DAG will be replaced by project:switched if another project becomes active.
    }).then((u) => unlisten.push(u));

    // ── queue:item:added ───────────────────────────────────────────────────────
    listen<{ item: QueueItem }>("queue:item:added", (event) => {
      setQueueItems((prev) => {
        const next = new Map(prev);
        next.set(event.payload.item.id, event.payload.item);
        return next;
      });
    }).then((u) => unlisten.push(u));

    // ── queue:item:resolved ────────────────────────────────────────────────────
    listen<{ itemId: string }>("queue:item:resolved", (event) => {
      setQueueItems((prev) => {
        const next = new Map(prev);
        next.delete(event.payload.itemId);
        return next;
      });
    }).then((u) => unlisten.push(u));

    // ── dag:node:upserted ──────────────────────────────────────────────────────
    listen<{ node: DagNode }>("dag:node:upserted", (event) => {
      setNodes((prev) => {
        const next = new Map(prev);
        next.set(event.payload.node.id, event.payload.node);
        return next;
      });
    }).then((u) => unlisten.push(u));

    // ── dag:node:deleted ───────────────────────────────────────────────────────
    listen<{ id: string }>("dag:node:deleted", (event) => {
      setNodes((prev) => {
        const next = new Map(prev);
        next.delete(event.payload.id);
        return next;
      });
    }).then((u) => unlisten.push(u));

    // ── dag:edge:upserted ──────────────────────────────────────────────────────
    listen<{ edge: DagEdge }>("dag:edge:upserted", (event) => {
      setEdges((prev) => {
        const next = new Map(prev);
        next.set(event.payload.edge.id, event.payload.edge);
        return next;
      });
    }).then((u) => unlisten.push(u));

    // ── dag:edge:deleted ───────────────────────────────────────────────────────
    listen<{ id: string }>("dag:edge:deleted", (event) => {
      setEdges((prev) => {
        const next = new Map(prev);
        next.delete(event.payload.id);
        return next;
      });
    }).then((u) => unlisten.push(u));

    // ── workflow:status ────────────────────────────────────────────────────────
    listen<WorkflowInfo>("workflow:status", (event) => {
      setWorkflows((prev) => {
        const next = new Map(prev);
        next.set(event.payload.workflowId, event.payload);
        return next;
      });
    }).then((u) => unlisten.push(u));

    // ── agent:stdout ───────────────────────────────────────────────────────────
    listen<AgentStdoutLine>("agent:stdout", (event) => {
      setAgentStdout((prev) => {
        const next = new Map(prev);
        const lines = [...(prev.get(event.payload.workflowId) ?? []), event.payload];
        next.set(event.payload.workflowId, lines.slice(-500));
        return next;
      });
    }).then((u) => unlisten.push(u));

    return () => {
      unlisten.forEach((fn) => fn());
    };
  }, [setNodes, setEdges, setProject, setQueueItems, setWorkflows, setAgentStdout, setOpenProjects]);
}
