import { useCallback, useState } from 'react';
import {
  ReactFlow,
  addEdge,
  useNodesState,
  useEdgesState,
  Controls,
  Background,
  type Connection,
  type Edge as FlowEdge,
  BackgroundVariant,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { invoke } from '@tauri-apps/api/core';
import type { Phase } from '../types';

// Stage type catalogue — static, not a SQLite table
export const STAGE_TYPES = [
  'conops',
  'guardrails',
  'increment_planning',
  'execution',
  'pm_review',
  'rework',
  'validity_analysis',
  'retrospective',
  'onboarding',
] as const;

type StageType = (typeof STAGE_TYPES)[number];

// Declared inputs and outputs per stage type for connector validation
const STAGE_IO: Record<StageType, { inputs: string[]; outputs: string[] }> = {
  conops:              { inputs: [],                           outputs: ['conops'] },
  guardrails:          { inputs: ['conops'],                   outputs: ['guardrails'] },
  increment_planning:  { inputs: ['conops', 'guardrails'],     outputs: ['phase-plan'] },
  execution:           { inputs: ['phase-plan'],               outputs: ['implementation'] },
  pm_review:           { inputs: ['phase-plan'],               outputs: ['review'] },
  rework:              { inputs: ['review'],                   outputs: ['phase-plan'] },
  validity_analysis:   { inputs: ['implementation'],           outputs: ['validity-report'] },
  retrospective:       { inputs: ['implementation', 'review'], outputs: ['retrospective'] },
  onboarding:          { inputs: [],                           outputs: ['onboarding'] },
};

const STAGE_COLORS: Record<StageType, string> = {
  conops:             '#1a3a5c',
  guardrails:         '#3a2a1a',
  increment_planning: '#1a2a3a',
  execution:          '#1a3a2a',
  pm_review:          '#2a1a3a',
  rework:             '#3a1a1a',
  validity_analysis:  '#1a3a3a',
  retrospective:      '#2a3a1a',
  onboarding:         '#1a1a3a',
};

interface StageNodeData {
  stageType: StageType;
  label: string;
  number: number;
  [key: string]: unknown;
}

// ReactFlow node shape: { id, type, position, data, style }
interface FlowNode {
  id: string;
  type?: string;
  position: { x: number; y: number };
  data: StageNodeData;
  style?: React.CSSProperties;
}

function stageNodeStyle(stageType: StageType): React.CSSProperties {
  return {
    background: STAGE_COLORS[stageType] ?? '#1a1a1a',
    border: '1px solid #374151',
    borderRadius: '6px',
    padding: '8px 12px',
    color: '#d1d5db',
    fontSize: '11px',
    fontFamily: 'monospace',
    minWidth: '120px',
    textAlign: 'center',
  };
}

function validateConnection(
  connection: Connection,
  nodes: FlowNode[]
): boolean {
  const sourceNode = nodes.find(n => n.id === connection.source);
  const targetNode = nodes.find(n => n.id === connection.target);
  if (!sourceNode || !targetNode) return false;

  const sourceOutputs = STAGE_IO[sourceNode.data.stageType]?.outputs ?? [];
  const targetInputs = STAGE_IO[targetNode.data.stageType]?.inputs ?? [];

  // Allow if any output of source satisfies any input of target
  return sourceOutputs.some(out => targetInputs.includes(out));
}

// Topological sort to determine phase creation order
function topoSort(nodes: FlowNode[], edges: FlowEdge[]): FlowNode[] {
  const inDegree = new Map<string, number>();
  const adj = new Map<string, string[]>();

  for (const n of nodes) {
    inDegree.set(n.id, 0);
    adj.set(n.id, []);
  }
  for (const e of edges) {
    adj.get(e.source)!.push(e.target);
    inDegree.set(e.target, (inDegree.get(e.target) ?? 0) + 1);
  }

  const queue = nodes.filter(n => inDegree.get(n.id) === 0);
  const result: FlowNode[] = [];

  while (queue.length > 0) {
    const node = queue.shift()!;
    result.push(node);
    for (const neighborId of adj.get(node.id) ?? []) {
      const deg = (inDegree.get(neighborId) ?? 1) - 1;
      inDegree.set(neighborId, deg);
      if (deg === 0) {
        const neighbor = nodes.find(n => n.id === neighborId);
        if (neighbor) queue.push(neighbor);
      }
    }
  }

  return result;
}

interface Props {
  projectId: string;
  onComplete: () => void;
}

let nodeIdCounter = 1;

export default function PlanComposer({ projectId, onComplete }: Props) {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const [flowNodes, setFlowNodes, onNodesChange] = useNodesState<any>([]);
  const [flowEdges, setFlowEdges, onEdgesChange] = useEdgesState([]);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [connectionError, setConnectionError] = useState<string | null>(null);

  const onConnect = useCallback(
    (connection: Connection) => {
      const valid = validateConnection(connection, flowNodes as FlowNode[]);
      if (!valid) {
        setConnectionError(
          'Incompatible connection: source outputs do not satisfy target inputs.'
        );
        setTimeout(() => setConnectionError(null), 3000);
        return;
      }
      setConnectionError(null);
      setFlowEdges(eds => addEdge(connection, eds));
    },
    [flowNodes, setFlowEdges]
  );

  function addStageNode(stageType: StageType) {
    const id = `stage-${nodeIdCounter++}`;
    const newNode: FlowNode = {
      id,
      type: 'default',
      position: { x: 100 + (flowNodes.length % 4) * 160, y: 80 + Math.floor(flowNodes.length / 4) * 120 },
      data: {
        stageType,
        label: stageType.replace(/_/g, ' '),
        number: 1,
      },
      style: stageNodeStyle(stageType),
    };
    setFlowNodes((nds: FlowNode[]) => [...nds, newNode]);
  }

  async function handleStart() {
    if (flowNodes.length === 0) return;
    setRunning(true);
    setError(null);

    const typedNodes = flowNodes as FlowNode[];
    const ordered = topoSort(typedNodes, flowEdges);

    try {
      let firstPhaseId: string | null = null;
      for (let i = 0; i < ordered.length; i++) {
        const n = ordered[i];
        const phase = await invoke<Phase>('create_phase', {
          projectId,
          name: n.data.label,
          stageType: n.data.stageType,
          number: i + 1,
        });
        if (i === 0) firstPhaseId = phase.id;
      }

      if (firstPhaseId) {
        await invoke('activate_phase', { phaseId: firstPhaseId });
      }

      onComplete();
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(false);
    }
  }

  return (
    <div className="flex h-full overflow-hidden">
      {/* Stage library panel */}
      <div className="w-[180px] shrink-0 border-r border-neutral-800 flex flex-col bg-neutral-900/40">
        <div className="px-3 py-2 border-b border-neutral-800 shrink-0">
          <p className="text-[10px] uppercase tracking-widest text-neutral-600">Stage Types</p>
        </div>
        <div className="flex-1 overflow-y-auto p-2 space-y-1">
          {STAGE_TYPES.map(st => (
            <button
              key={st}
              className="w-full text-left px-2 py-1.5 rounded text-[11px] font-mono text-neutral-400 border border-neutral-800 hover:bg-neutral-800 hover:text-neutral-200 hover:border-neutral-700 transition-colors"
              onClick={() => addStageNode(st)}
              disabled={running}
            >
              + {st.replace(/_/g, ' ')}
            </button>
          ))}
        </div>
        {/* IO legend */}
        <div className="px-3 py-2 border-t border-neutral-800 shrink-0">
          <p className="text-[9px] uppercase tracking-widest text-neutral-700 mb-1">Connections</p>
          <p className="text-[10px] text-neutral-700 leading-4">
            Connect stages where the upstream output matches the downstream input requirement.
          </p>
        </div>
      </div>

      {/* Canvas */}
      <div className="flex-1 flex flex-col overflow-hidden">
        <div className="flex items-center justify-between px-3 py-1.5 border-b border-neutral-800 shrink-0">
          <span className="text-[10px] text-neutral-600">
            {flowNodes.length} stage{flowNodes.length !== 1 ? 's' : ''}
          </span>
          <div className="flex items-center gap-2">
            {connectionError && (
              <span className="text-[10px] text-red-400 font-mono">{connectionError}</span>
            )}
            {error && (
              <span className="text-[10px] text-red-400 font-mono">{error}</span>
            )}
            <button
              className="px-3 py-1 rounded text-[11px] bg-emerald-800 hover:bg-emerald-700 text-white disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
              disabled={flowNodes.length === 0 || running}
              onClick={() => void handleStart()}
            >
              {running ? 'Starting…' : 'Run Phase →'}
            </button>
          </div>
        </div>

        <div className="flex-1 bg-neutral-950">
          <ReactFlow
            nodes={flowNodes}
            edges={flowEdges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
            fitView
            colorMode="dark"
            style={{ background: '#0a0a0a' }}
          >
            <Controls />
            <Background variant={BackgroundVariant.Dots} color="#1f2937" gap={20} />
          </ReactFlow>
        </div>
      </div>
    </div>
  );
}
