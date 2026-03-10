import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Node, Artifact, EventRecord, Edge } from '../types';
import AdvisorChatbot from './AdvisorChatbot';

interface Props {
  node: Node;
  projectId: string;
  nodes: Node[];
  artifacts: Artifact[];
  edges: Edge[];
  onClose: () => void;
  onArtifactOpen: (artifact: Artifact) => void;
}

function parseSkillModes(skillModes: string | null): string[] {
  if (!skillModes) return ['autonomous'];
  try {
    const parsed = JSON.parse(skillModes);
    return Array.isArray(parsed) ? (parsed as string[]) : ['autonomous'];
  } catch {
    return ['autonomous'];
  }
}

function statusCls(status: Node['status']): string {
  switch (status) {
    case 'running':   return 'text-emerald-400';
    case 'waiting':   return 'text-amber-400';
    case 'complete':  return 'text-neutral-500';
    case 'cancelled': return 'text-neutral-700';
    case 'blocked':   return 'text-red-500';
    default:          return 'text-neutral-500';
  }
}

export default function TaskDetailPanel({
  node,
  projectId,
  nodes,
  artifacts,
  edges,
  onClose,
  onArtifactOpen,
}: Props) {
  const [ancestry, setAncestry] = useState<Node[]>([]);
  const [events, setEvents] = useState<EventRecord[]>([]);
  const [advisorTaskId, setAdvisorTaskId] = useState<string | null>(null);
  const [showAdvisor, setShowAdvisor] = useState(false);

  const skillModes = parseSkillModes(node.skillModes);
  const isInteractive = skillModes.includes('interactive');

  // Fetch node ancestry
  useEffect(() => {
    invoke<Node[]>('get_node_ancestry', { nodeId: node.id })
      .then(setAncestry)
      .catch(console.error);
  }, [node.id]);

  // Fetch events for this task
  useEffect(() => {
    invoke<EventRecord[]>('list_events', { projectId, since: null })
      .then(evts => setEvents(evts.filter(e => e.taskId === node.id)))
      .catch(console.error);
  }, [node.id, projectId]);

  // Compute upstream and downstream tasks from edges
  const upstreamIds = edges.filter(e => e.toId === node.id).map(e => e.fromId);
  const downstreamIds = edges.filter(e => e.fromId === node.id).map(e => e.toId);
  const upstreamNodes = upstreamIds.map(id => nodes.find(n => n.id === id)).filter(Boolean) as Node[];
  const downstreamNodes = downstreamIds.map(id => nodes.find(n => n.id === id)).filter(Boolean) as Node[];

  // Artifacts for this task
  const taskArtifacts = artifacts.filter(a => a.phaseId === node.phaseId);

  async function handleStartAdvisor() {
    try {
      await invoke('start_advisor_session', {
        projectId,
        decisionId: null,
      });
      // The advisor task ID will be set via the event listener in AdvisorChatbot.
      // We set a placeholder to trigger showing the panel.
      setAdvisorTaskId('pending');
      setShowAdvisor(true);
    } catch (e) {
      console.error('Failed to start advisor session:', e);
    }
  }

  async function handleCancel() {
    try {
      await invoke('cancel_node', { nodeId: node.id, projectId });
    } catch (e) {
      console.error('Failed to cancel node:', e);
    }
  }

  async function handleRetry() {
    try {
      await invoke('retry_node', { nodeId: node.id, projectId });
    } catch (e) {
      console.error('Failed to retry node:', e);
    }
  }

  return (
    <div className="fixed inset-y-0 right-0 z-40 w-[480px] flex flex-col bg-neutral-950 border-l border-neutral-800 shadow-2xl overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-neutral-800 shrink-0">
        <div className="min-w-0">
          <p className="text-xs font-semibold text-neutral-200 truncate">{node.title}</p>
          <p className="text-[10px] text-neutral-600 font-mono">{node.nodeType} · {node.id.slice(0, 8)}…</p>
        </div>
        <button
          className="text-neutral-500 hover:text-neutral-200 text-xs px-2 py-1 shrink-0"
          onClick={onClose}
        >
          ✕ close
        </button>
      </div>

      <div className="flex-1 overflow-y-auto">
        {/* Breadcrumb ancestry */}
        {ancestry.length > 1 && (
          <div className="px-4 py-2 border-b border-neutral-800/60">
            <p className="text-[9px] uppercase tracking-widest text-neutral-600 mb-1">Ancestry</p>
            <div className="flex flex-wrap items-center gap-1 text-[11px] text-neutral-500 font-mono">
              {ancestry.slice().reverse().map((n, idx) => (
                <span key={n.id} className="flex items-center gap-1">
                  {idx > 0 && <span className="text-neutral-700">›</span>}
                  <span className={n.id === node.id ? 'text-neutral-300 font-semibold' : 'text-neutral-600'}>
                    {n.title}
                  </span>
                </span>
              ))}
            </div>
          </div>
        )}

        {/* Task metadata */}
        <div className="px-4 py-3 border-b border-neutral-800/60 space-y-2">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <span className={`text-xs font-mono font-semibold ${statusCls(node.status)}`}>
                {node.status}
              </span>
              {node.skillId && (
                <span className="text-[10px] text-neutral-600 font-mono border border-neutral-800 px-1 py-0 rounded">
                  {node.skillId}
                </span>
              )}
            </div>
            <div className="flex items-center gap-1">
              {skillModes.map(mode => (
                <span
                  key={mode}
                  className="text-[9px] px-1 py-0 border border-neutral-800 rounded text-neutral-600"
                >
                  {mode}
                </span>
              ))}
            </div>
          </div>
          {node.description && (
            <p className="text-[12px] text-neutral-400 leading-5 whitespace-pre-wrap">
              {node.description}
            </p>
          )}
        </div>

        {/* Action buttons */}
        <div className="px-4 py-2 border-b border-neutral-800/60 flex items-center gap-2">
          {node.status === 'running' && (
            <button
              className="px-2 py-1 rounded text-[11px] border border-neutral-700 text-neutral-400 hover:text-neutral-200 hover:border-neutral-500 transition-colors"
              onClick={() => void handleCancel()}
            >
              Cancel
            </button>
          )}
          {(node.status === 'blocked' || node.status === 'cancelled') && (
            <button
              className="px-2 py-1 rounded text-[11px] border border-neutral-700 text-neutral-400 hover:text-neutral-200 hover:border-neutral-500 transition-colors"
              onClick={() => void handleRetry()}
            >
              Retry
            </button>
          )}
          {isInteractive ? (
            <button
              className="px-2 py-1 rounded text-[11px] border border-teal-900 text-teal-600 hover:border-teal-700 hover:text-teal-400 transition-colors"
              onClick={() => void handleStartAdvisor()}
            >
              Open Advisor Chat
            </button>
          ) : (
            <span
              className="text-[10px] text-neutral-700 font-mono"
              title="This skill runs autonomously. View its activity feed to see what it did."
            >
              Autonomous-only skill — no conversation available
            </span>
          )}
        </div>

        {/* Artifacts */}
        {taskArtifacts.length > 0 && (
          <div className="px-4 py-2 border-b border-neutral-800/60">
            <p className="text-[9px] uppercase tracking-widest text-neutral-600 mb-1.5">Artifacts</p>
            <div className="space-y-1">
              {taskArtifacts.map(a => (
                <button
                  key={a.id}
                  className="block text-left text-[11px] text-emerald-500 hover:text-emerald-300 font-mono"
                  onClick={() => onArtifactOpen(a)}
                >
                  📄 {a.filename}
                  <span className="text-neutral-700 ml-1">({a.artifactType})</span>
                </button>
              ))}
            </div>
          </div>
        )}

        {/* Dependency chain */}
        {(upstreamNodes.length > 0 || downstreamNodes.length > 0) && (
          <div className="px-4 py-2 border-b border-neutral-800/60">
            <p className="text-[9px] uppercase tracking-widest text-neutral-600 mb-1.5">Dependencies</p>
            {upstreamNodes.length > 0 && (
              <div className="mb-1">
                <p className="text-[10px] text-neutral-700 mb-0.5">Must finish first:</p>
                {upstreamNodes.map(n => (
                  <div key={n.id} className="flex items-center gap-1 text-[11px] font-mono text-neutral-500">
                    <span className="text-neutral-700">↑</span>
                    <span>{n.title}</span>
                    <span className={`text-[10px] ${statusCls(n.status)}`}>[{n.status}]</span>
                  </div>
                ))}
              </div>
            )}
            {downstreamNodes.length > 0 && (
              <div>
                <p className="text-[10px] text-neutral-700 mb-0.5">Blocked by this:</p>
                {downstreamNodes.map(n => (
                  <div key={n.id} className="flex items-center gap-1 text-[11px] font-mono text-neutral-500">
                    <span className="text-neutral-700">↓</span>
                    <span>{n.title}</span>
                    <span className={`text-[10px] ${statusCls(n.status)}`}>[{n.status}]</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {/* Event log */}
        <div className="px-4 py-2 border-b border-neutral-800/60">
          <p className="text-[9px] uppercase tracking-widest text-neutral-600 mb-1.5">Event Log</p>
          {events.length === 0 ? (
            <p className="text-[11px] text-neutral-700">No events yet</p>
          ) : (
            <div className="space-y-1">
              {events.map(ev => {
                let preview = '';
                try {
                  const p = JSON.parse(ev.payload) as Record<string, unknown>;
                  if (typeof p['content'] === 'string') preview = p['content'].slice(0, 80);
                  else if (typeof p['name'] === 'string') preview = p['name'];
                  else if (typeof p['summary'] === 'string') preview = p['summary'].slice(0, 80);
                  else preview = ev.payload.slice(0, 80);
                } catch {
                  preview = ev.payload.slice(0, 80);
                }
                return (
                  <div key={ev.id} className="text-[11px] font-mono">
                    <span className="text-neutral-600 mr-1">
                      {new Date(ev.createdAt).toLocaleTimeString()}
                    </span>
                    <span className="text-teal-700 mr-1">{ev.eventType}</span>
                    <span className="text-neutral-500 truncate">{preview}</span>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* Advisor chat panel (shown when started) */}
        {showAdvisor && (
          <div className="h-[320px] border-t border-neutral-800">
            <AdvisorChatbot
              projectId={projectId}
              taskNodeId={advisorTaskId}
            />
          </div>
        )}
      </div>
    </div>
  );
}
