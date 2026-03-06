import { useState } from "react";
import { useAtomValue } from "jotai";
import { projectAtom } from "./store/dag";
import { useDagEvents } from "./hooks/useDagEvents";
import { EmptyState } from "./components/EmptyState";
import { ProjectHeader } from "./components/ProjectHeader";
import { QueueView } from "./components/QueueView";
import { RestateView } from "./components/RestateView";

type Tab = "queue" | "restate";

const TABS: { id: Tab; label: string }[] = [
  { id: "queue", label: "Queue" },
  { id: "restate", label: "Restate" },
];

export default function App() {
  const project = useAtomValue(projectAtom);
  const [activeTab, setActiveTab] = useState<Tab>("queue");

  // Wire up all reactive event subscriptions
  useDagEvents();

  if (!project) {
    return <EmptyState />;
  }

  return (
    <div className="flex flex-col h-full">
      <ProjectHeader />

      {/* Tab bar */}
      <div className="border-b-4 border-black flex">
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

      {/* Tab content */}
      {activeTab === "queue" && <QueueView />}
      {activeTab === "restate" && <RestateView />}
    </div>
  );
}
