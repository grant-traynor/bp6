import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
  ProjectStateSnapshot,
  ActivityItem,
  DecisionRow,
  EventLogRow,
  PoeEventPayload,
  PoeTaskUpdatePayload,
  PoeDecisionPayload,
  TaskRow,
} from '../poe-types';

function eventToActivity(e: EventLogRow): ActivityItem {
  return {
    id: String(e.id),
    taskId: e.taskId,
    eventType: e.eventType,
    payload: (() => {
      try {
        return JSON.parse(e.payload);
      } catch {
        return e.payload;
      }
    })(),
    createdAt: e.createdAt,
  };
}

export function usePoeProject(projectId: string | null) {
  const [snapshot, setSnapshot] = useState<ProjectStateSnapshot | null>(null);
  const [activityFeed, setActivityFeed] = useState<ActivityItem[]>([]);
  const [openDecisions, setOpenDecisions] = useState<DecisionRow[]>([]);

  useEffect(() => {
    if (!projectId) {
      setSnapshot(null);
      setActivityFeed([]);
      setOpenDecisions([]);
      return;
    }

    // 1. Hydrate from SQLite
    invoke<ProjectStateSnapshot>('get_project_state', { projectId }).then((s) => {
      setSnapshot(s);
      setActivityFeed(s.recentEvents.map(eventToActivity));
      setOpenDecisions(s.openDecisions);
    }).catch((err) => {
      console.error('get_project_state failed:', err);
    });

    // 2. Listen for live events
    const unlisten1 = listen<PoeEventPayload>('poe://event', (e) => {
      setActivityFeed((prev) => [
        ...prev,
        {
          id: `${e.payload.taskId}-${e.payload.createdAt}`,
          taskId: e.payload.taskId,
          eventType: e.payload.eventType,
          payload: e.payload.payload,
          createdAt: e.payload.createdAt,
        },
      ]);
    });

    const unlisten2 = listen<PoeTaskUpdatePayload>('poe://task-update', (e) => {
      setSnapshot((prev) => {
        if (!prev) return prev;
        return {
          ...prev,
          tasks: prev.tasks.map((t): TaskRow =>
            t.id === e.payload.taskId
              ? {
                  ...t,
                  status: e.payload.status as TaskRow['status'],
                  title: e.payload.title ?? t.title,
                  updatedAt: e.payload.updatedAt,
                }
              : t
          ),
        };
      });
    });

    const unlisten3 = listen<PoeDecisionPayload>('poe://decision', (e) => {
      setOpenDecisions((prev) => [
        ...prev,
        {
          id: e.payload.decisionId,
          taskId: e.payload.taskId,
          question: e.payload.question,
          options: e.payload.options ? JSON.stringify(e.payload.options) : null,
          resolution: null,
          createdAt: Date.now(),
          resolvedAt: null,
        },
      ]);
    });

    const unlisten4 = listen<{ decisionId: number }>('poe://decision-resolved', (e) => {
      setOpenDecisions((prev) => prev.filter((d) => d.id !== e.payload.decisionId));
    });

    return () => {
      Promise.all([unlisten1, unlisten2, unlisten3, unlisten4])
        .then((fns) => fns.forEach((fn) => fn()))
        .catch(() => {});
    };
  }, [projectId]);

  return { snapshot, activityFeed, openDecisions, setOpenDecisions };
}
