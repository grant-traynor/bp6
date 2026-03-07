import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { Node, QueueItem, EventRecord, FeedItem } from '../types';

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

export function usePoeProject(projectId: string | null): {
  nodes: Node[];
  queueItems: QueueItem[];
  feedItems: FeedItem[];
  setQueueItems: React.Dispatch<React.SetStateAction<QueueItem[]>>;
} {
  const [nodes, setNodes] = useState<Node[]>([]);
  const [queueItems, setQueueItems] = useState<QueueItem[]>([]);
  const [feedItems, setFeedItems] = useState<FeedItem[]>([]);

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
      return;
    }

    let cancelled = false;
    const unlisteners: UnlistenFn[] = [];

    async function hydrate() {
      const [fetchedNodes, fetchedQueue, fetchedEvents] = await Promise.all([
        invoke<Node[]>('list_nodes', { projectId, phaseId: null }),
        invoke<QueueItem[]>('list_queue_items', { projectId, unresolvedOnly: true }),
        invoke<EventRecord[]>('list_events', { projectId, since: null }),
      ]);
      if (cancelled) return;
      setNodes(fetchedNodes);
      setQueueItems(fetchedQueue.filter(q => q.resolvedAt === null));
      setFeedItems(fetchedEvents.map(eventRecordToFeedItem));
    }

    async function subscribe() {
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
      }>('poe-agent-started', ({ payload }) => {
        if (payload.projectId !== projectId) return;
        setFeedItems(prev => [
          ...prev,
          {
            id: `as-${payload.agentId}-${Date.now()}`,
            type: 'agent-start',
            eventType: 'poe-agent-started',
            taskId: payload.taskId,
            skillId: payload.skillId,
            message: `Agent started (skill: ${payload.skillId})`,
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
    }

    void hydrate();
    void subscribe();

    return () => {
      cancelled = true;
      for (const u of unlisteners) u();
    };
  }, [projectId, updateNode]);

  return { nodes, queueItems, feedItems, setQueueItems };
}
