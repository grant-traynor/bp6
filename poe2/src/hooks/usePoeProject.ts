import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { Node, QueueItem, EventRecord, FeedItem, Phase, Artifact, KnowledgeEntry, PoeAgentActivityEvent } from '../types';

export interface AgentStreamEvent {
  agentId: string;
  taskId: string;
  projectId: string;
  event: Record<string, unknown>;
}

function eventRecordToFeedItem(ev: EventRecord): FeedItem {
  return {
    id: ev.id,
    type: 'event',
    eventType: ev.eventType,
    taskId: ev.taskId,
    message: (() => {
      try {
        const p = JSON.parse(ev.payload) as Record<string, unknown>;
        if (typeof p['content'] === 'string') return p['content'];
        if (typeof p['name'] === 'string') return p['name'];
        if (typeof p['summary'] === 'string') return p['summary'];
        return ev.payload.slice(0, 120);
      } catch {
        return ev.payload.slice(0, 120);
      }
    })(),
    ts: ev.createdAt,
  };
}

export function getAncestry(nodeId: string | null | undefined, nodes: Node[]): Node[] {
  const chain: Node[] = [];
  let id = nodeId;
  const visited = new Set<string>();
  while (id && !visited.has(id)) {
    visited.add(id);
    const node = nodes.find(n => n.id === id);
    if (!node) break;
    chain.push(node);
    id = node.parentId ?? null;
  }
  return chain; // closest first, root last
}

export function usePoeProject(projectId: string | null): {
  nodes: Node[];
  queueItems: QueueItem[];
  feedItems: FeedItem[];
  phases: Phase[];
  artifacts: Artifact[];
  knowledgeEntries: KnowledgeEntry[];
  handoverNodeId: string | null;
  setHandoverNodeId: React.Dispatch<React.SetStateAction<string | null>>;
  setQueueItems: React.Dispatch<React.SetStateAction<QueueItem[]>>;
  streamEventMap: Map<string, AgentStreamEvent[]>;
  addNode: (node: Node) => void;
} {
  const [nodes, setNodes] = useState<Node[]>([]);
  const [queueItems, setQueueItems] = useState<QueueItem[]>([]);
  const [feedItems, setFeedItems] = useState<FeedItem[]>([]);
  const [phases, setPhases] = useState<Phase[]>([]);
  const [artifacts, setArtifacts] = useState<Artifact[]>([]);
  const [knowledgeEntries, setKnowledgeEntries] = useState<KnowledgeEntry[]>([]);
  const [handoverNodeId, setHandoverNodeId] = useState<string | null>(null);
  // taskId → ring buffer of stream-json events received via poe-agent-stream
  const [streamEventMap, setStreamEventMap] = useState<Map<string, AgentStreamEvent[]>>(new Map());

  const updateNode = useCallback((updated: Node) => {
    setNodes(prev => {
      const idx = prev.findIndex(n => n.id === updated.id);
      if (idx === -1) return [...prev, updated];
      const next = [...prev];
      next[idx] = updated;
      return next;
    });
  }, []);

  useEffect(() => {
    if (!projectId) {
      setNodes([]);
      setQueueItems([]);
      setFeedItems([]);
      setPhases([]);
      setArtifacts([]);
      setKnowledgeEntries([]);
      setStreamEventMap(new Map());

      return;
    }

    let cancelled = false;
    const unlisteners: UnlistenFn[] = [];

    async function hydrate() {
      const [fetchedNodes, fetchedQueue, fetchedEvents, fetchedPhases, fetchedArtifacts] = await Promise.all([
        invoke<Node[]>('list_nodes', { projectId, phaseId: null }),
        invoke<QueueItem[]>('list_queue_items', { projectId }),
        invoke<EventRecord[]>('list_events', { projectId, since: null }),
        invoke<Phase[]>('list_phases', { projectId }),
        invoke<Artifact[]>('list_artifacts', { projectId }),
      ]);
      if (cancelled) return;
      setNodes(fetchedNodes);
      setQueueItems(fetchedQueue);
      setFeedItems(fetchedEvents.map(eventRecordToFeedItem));
      setPhases(fetchedPhases);
      setArtifacts(fetchedArtifacts);
    }

    async function subscribe() {
      console.log('[usePoeProject] subscribe() called, projectId=', projectId);
      const u1 = await listen<Node>('poe-task-created', ({ payload }) => {
        if (payload.projectId !== projectId) return;
        updateNode(payload);
        setFeedItems(prev => [
          ...prev,
          {
            id: `nc-${payload.id}`,
            type: 'node-created',
            taskId: payload.id,
            message: `Task created: ${payload.title}`,
            ts: payload.createdAt,
          },
        ]);
      });
      unlisteners.push(u1);

      const u2 = await listen<Node>('poe-node-updated', ({ payload }) => {
        if (payload.projectId !== projectId) return;
        updateNode(payload);
      });
      unlisteners.push(u2);

      const u3 = await listen<Node>('poe-task-done', ({ payload }) => {
        if (payload.projectId !== projectId) return;
        updateNode(payload);
        setFeedItems(prev => [
          ...prev,
          {
            id: `done-${payload.id}-${Date.now()}`,
            type: 'event',
            eventType: 'poe-task-done',
            taskId: payload.id,
            message: `Task done: ${payload.title}`,
            ts: payload.updatedAt,
          },
        ]);
      });
      unlisteners.push(u3);

      const u4 = await listen<QueueItem>('poe-decision-queued', ({ payload }) => {
        if (payload.projectId !== projectId) return;
        setQueueItems(prev => {
          if (prev.some(q => q.id === payload.id)) return prev;
          return [...prev, payload];
        });
      });
      unlisteners.push(u4);

      const u5 = await listen<{
        eventType: string;
        projectId: string;
        agentId: string | null;
        taskId: string | null;
        payload: string;
      }>('poe-event', ({ payload }) => {
        if (payload.projectId !== projectId) return;
        const message = (() => {
          try {
            const p = JSON.parse(payload.payload) as Record<string, unknown>;
            if (typeof p['content'] === 'string') return p['content'];
            if (typeof p['name'] === 'string') return p['name'];
            if (typeof p['summary'] === 'string') return p['summary'];
            return payload.payload.slice(0, 120);
          } catch {
            return payload.payload.slice(0, 120);
          }
        })();
        setFeedItems(prev => [
          ...prev,
          {
            id: `ev-${Date.now()}-${Math.random()}`,
            type: 'event',
            eventType: payload.eventType,
            taskId: payload.taskId,
            message,
            ts: new Date().toISOString(),
          },
        ]);
      });
      unlisteners.push(u5);

      const u6 = await listen<{
        agentId: string;
        taskId: string;
        projectId: string;
        skillId: string;
        model?: string | null;
      }>('poe-agent-started', ({ payload }) => {
        console.log('[poe-agent-started] agentId=', payload.agentId, 'taskId=', payload.taskId, 'projectId=', payload.projectId);
        if (payload.projectId !== projectId) return;
        setFeedItems(prev => [
          ...prev,
          {
            id: `as-${payload.agentId}-${Date.now()}`,
            type: 'agent-start',
            eventType: 'poe-agent-started',
            taskId: payload.taskId,
            skillId: payload.skillId,
            model: payload.model ?? null,
            // Model displayed as a separate badge in ActivityFeed; omit from message.
            message: `Agent started · ${payload.skillId}`,
            ts: new Date().toISOString(),
          },
        ]);
      });
      unlisteners.push(u6);

      const u7 = await listen<{
        agentId: string;
        taskId: string;
        projectId: string;
        success: boolean;
      }>('poe-agent-exited', ({ payload }) => {
        if (payload.projectId !== projectId) return;
        setFeedItems(prev => [
          ...prev,
          {
            id: `ae-${payload.agentId}-${Date.now()}`,
            type: 'agent-exit',
            eventType: 'poe-agent-exited',
            taskId: payload.taskId,
            message: payload.success ? 'Agent exited (success)' : 'Agent exited (failed)',
            ts: new Date().toISOString(),
          },
        ]);
      });
      unlisteners.push(u7);

      const u8 = await listen<Artifact>('poe-artifact-created', ({ payload }) => {
        if (payload.projectId !== projectId) return;
        setArtifacts(prev => {
          const idx = prev.findIndex(a => a.id === payload.id);
          if (idx === -1) return [...prev, payload];
          const next = [...prev]; next[idx] = payload; return next;
        });
        setFeedItems(prev => [...prev, {
          id: `art-${payload.id}`,
          type: 'event' as const,
          eventType: 'poe:artifact',
          taskId: null,
          message: `Artifact: ${payload.filename}`,
          ts: payload.createdAt,
        }]);
      });
      unlisteners.push(u8);

      const u9 = await listen<KnowledgeEntry>('poe-knowledge-created', ({ payload }) => {
        if (payload.projectId !== projectId) return;
        setKnowledgeEntries(prev => {
          const idx = prev.findIndex(k => k.id === payload.id);
          if (idx === -1) return [...prev, payload];
          const next = [...prev]; next[idx] = payload; return next;
        });
        setFeedItems(prev => [...prev, {
          id: `know-${payload.id}`,
          type: 'event' as const,
          eventType: 'poe:knowledge',
          taskId: null,
          message: `Knowledge: ${payload.key}`,
          ts: payload.createdAt,
        }]);
      });
      unlisteners.push(u9);

      const u10 = await listen<{ itemId: string; projectId: string; resolution: string }>('poe-decision-resolved', ({ payload }) => {
        if (payload.projectId !== projectId) return;
        // Mark resolved in-place so thread mode retains history; pending items have resolvedAt === null.
        setQueueItems(prev => prev.map(q =>
          q.id === payload.itemId
            ? { ...q, resolution: payload.resolution, resolvedAt: new Date().toISOString() }
            : q
        ));
      });
      unlisteners.push(u10);

      const MAX_STREAM_EVENTS = 500;
      console.log('[usePoeProject] registering poe-agent-stream listener, projectId=', projectId);
      const u11 = await listen<AgentStreamEvent>('poe-agent-stream', ({ payload }) => {
        console.log('[poe-agent-stream] raw event arrived, payload.projectId=', payload.projectId, 'watching=', projectId, 'event.type=', payload.event?.['type']);
        if (payload.projectId !== projectId) {
          console.warn('[poe-agent-stream] DROPPED — projectId mismatch');
          return;
        }
        setStreamEventMap(prev => {
          const taskId = payload.taskId;
          const existing = prev.get(taskId) ?? [];
          const next = existing.length >= MAX_STREAM_EVENTS
            ? [...existing.slice(1), payload]
            : [...existing, payload];
          const m = new Map(prev);
          m.set(taskId, next);
          return m;
        });
      });
      unlisteners.push(u11);

      // Reload phases whenever the orchestrator updates phase state
      // (gate reached, advance, revise, rerun, or activate).
      const u12 = await listen<{ projectId: string }>('poe-phase-update', ({ payload }) => {
        if (payload.projectId !== projectId) return;
        invoke<Phase[]>('list_phases', { projectId }).then(setPhases).catch(console.error);
      });
      unlisteners.push(u12);

      // poe-agent-activity: emitted for poe:step and poe:brief (u7s.6).
      // Provides real-time agent progress between agent-started and agent-exited.
      // Feed is capped at MAX_FEED_ITEMS (50) — oldest entries are dropped when full.
      const MAX_FEED_ITEMS = 50;
      const u13 = await listen<PoeAgentActivityEvent>('poe-agent-activity', ({ payload }) => {
        setFeedItems(prev => {
          const entry: FeedItem = {
            id: `activity-${payload.agentId}-${payload.type}-${Date.now()}-${Math.random()}`,
            type: 'event',
            eventType: `poe:${payload.type}`,
            taskId: payload.taskId,
            message: payload.content,
            ts: new Date().toISOString(),
          };
          const next = [...prev, entry];
          return next.length > MAX_FEED_ITEMS ? next.slice(next.length - MAX_FEED_ITEMS) : next;
        });
      });
      unlisteners.push(u13);

    }

    void hydrate();
    void subscribe();

    return () => {
      cancelled = true;
      for (const u of unlisteners) u();
    };
  }, [projectId, updateNode]);

  return {
    nodes,
    queueItems,
    feedItems,
    phases,
    artifacts,
    knowledgeEntries,
    handoverNodeId,
    setHandoverNodeId,
    setQueueItems,
    streamEventMap,
    addNode: updateNode,
  };
}
