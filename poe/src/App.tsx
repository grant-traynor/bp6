import { useState } from "react";
import { useAtomValue, useAtom } from "jotai";
import { projectAtom, selectedNodeIdAtom } from "./store/dag";
import { useDagEvents } from "./hooks/useDagEvents";
import { EmptyState } from "./components/EmptyState";
import { ProjectHeader } from "./components/ProjectHeader";
import { QueueView } from "./components/QueueView";
import { GanttView } from "./components/GanttView";
import { AgentActivityView } from "./components/AgentActivityView";
import { RestateView } from "./components/RestateView";
import { ProbePanel } from "./components/ProbePanel";
import { ProvenanceView } from "./components/ProvenanceView";

type Tab = "queue" | "dag" | "agents" | "restate";

const TABS: { id: Tab; label: string }[] = [
  { id: "queue", label: "Queue" },
  { id: "dag", label: "DAG" },
  { id: "agents", label: "Agents" },
  { id: "restate", label: "Restate" },
];

export default function App() {
  const project = useAtomValue(projectAtom);
  const [activeTab, setActiveTab] = useState<Tab>("queue");
  const [selectedNodeId] = useAtom(selectedNodeIdAtom);
  const [showProvenance, setShowProvenance] = useState(false);

  // Wire up all reactive event subscriptions
  useDagEvents();

  if (!project) {
    return <EmptyState />;
  }

  return (
    <div className="flex flex-col h-full">
      <ProjectHeader />

      {/* Tab bar */}
      <div className="border-b-4 border-black flex shrink-0">
        {TABS.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            className={`font-mono text-xs px-4 py-2 border-r-4 border-black transition-colors ${
              activeTab === tab.id
                ? "bg-black text-white font-bold"
                : "bg-white text-black hover:bg-stone-100"
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Content + Probe panel */}
      <div className="flex-1 flex overflow-hidden">
        {/* Main content */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {activeTab === "queue" && <QueueView />}
          {activeTab === "dag" && <GanttView />}
          {activeTab === "agents" && <AgentActivityView />}
          {activeTab === "restate" && <RestateView />}
        </div>

        {/* Probe panel — slides in when a node is selected */}
        {selectedNodeId && (
          <div className="w-96 border-l-4 border-black flex flex-col overflow-hidden shrink-0">
            {/* Provenance sub-view toggle */}
            {showProvenance ? (
              <div className="flex flex-col flex-1 overflow-hidden">
                <div className="border-b-2 border-stone-200 px-3 py-1.5 flex items-center shrink-0">
                  <button
                    onClick={() => setShowProvenance(false)}
                    className="font-mono text-xs border-2 border-black px-2 py-0.5 hover:bg-stone-100"
                  >
                    ← Back
                  </button>
                  <span className="font-mono text-xs font-bold ml-3">Provenance</span>
                </div>
                <div className="flex-1 overflow-hidden">
                  <ProvenanceView nodeId={selectedNodeId} />
                </div>
              </div>
            ) : (
              <div className="flex flex-col flex-1 overflow-hidden">
                {/* Provenance button in probe header area */}
                <div className="flex-1 overflow-hidden">
                  <ProbePanel nodeId={selectedNodeId} />
                </div>
                <div className="border-t-2 border-stone-200 px-4 py-2 shrink-0">
                  <button
                    onClick={() => setShowProvenance(true)}
                    className="w-full border-2 border-black font-mono text-xs py-1 hover:bg-stone-100 transition-colors"
                  >
                    Provenance →
                  </button>
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
