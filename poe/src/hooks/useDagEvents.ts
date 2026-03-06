/**
 * bp6-ayk: Reactive event bridge
 *
 * Subscribes to Tauri DAG events and applies them as deltas to the Jotai store.
 * No polling — all updates arrive via events.
 */
import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useSetAtom } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import { nodesAtom, edgesAtom, projectAtom, queueItemsAtom } from "../store/dag";
import type { DagNode, DagEdge, DagSnapshot, ProjectInfo, QueueItem } from "../types";

export function useDagEvents() {
  const setNodes = useSetAtom(nodesAtom);
  const setEdges = useSetAtom(edgesAtom);
  const setProject = useSetAtom(projectAtom);
  const setQueueItems = useSetAtom(queueItemsAtom);

  useEffect(() => {
    const unlisten: Array<() => void> = [];

    // ── project:opened — load full initial snapshot ────────────────────────────
    listen<DagSnapshot>("project:opened", (event) => {
      const nodes = new Map<string, DagNode>();
      const edges = new Map<string, DagEdge>();
      for (const node of event.payload.nodes) nodes.set(node.id, node);
      for (const edge of event.payload.edges) edges.set(edge.id, edge);
      setNodes(nodes);
      setEdges(edges);

      // Load pending queue items
      invoke<QueueItem[]>("list_queue_items").then((items) => {
        const map = new Map<string, QueueItem>();
        for (const item of items) map.set(item.id, item);
        setQueueItems(map);
      }).catch(console.error);
    }).then((u) => unlisten.push(u));

    // ── project:closed — clear state ──────────────────────────────────────────
    listen("project:closed", () => {
      setProject(null);
      setNodes(new Map());
      setEdges(new Map());
      setQueueItems(new Map());
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

    return () => {
      unlisten.forEach((fn) => fn());
    };
  }, [setNodes, setEdges, setProject, setQueueItems]);
}

/** Convenience: subscribe only to project:opened to capture ProjectInfo */
export function useProjectEvents() {
  const setProject = useSetAtom(projectAtom);

  useEffect(() => {
    let cancel: (() => void) | undefined;

    listen<ProjectInfo>("project:opened", (event) => {
      setProject(event.payload);
    }).then((u) => {
      cancel = u;
    });

    return () => cancel?.();
  }, [setProject]);
}
