import { useState } from "react";
import { useAtomValue, useAtom } from "jotai";
import {
  Inbox, GitBranch, Bot, Settings2, Sun, Moon, Monitor,
} from "lucide-react";
import { projectAtom, selectedNodeIdAtom } from "./store/dag";
import { useDagEvents } from "./hooks/useDagEvents";
import { useTheme } from "./hooks/useTheme";
import { EmptyState } from "./components/EmptyState";
import { ProjectHeader } from "./components/ProjectHeader";
import { QueueView } from "./components/QueueView";
import { GanttView } from "./components/GanttView";
import { AgentActivityView } from "./components/AgentActivityView";
import { RestateView } from "./components/RestateView";
import { ProbePanel } from "./components/ProbePanel";
import { ProvenanceView } from "./components/ProvenanceView";

type Tab = "queue" | "dag" | "agents" | "restate";

const NAV_ITEMS: { id: Tab; label: string; icon: React.ReactNode }[] = [
  { id: "queue",   label: "Queue",   icon: <Inbox size={15} /> },
  { id: "dag",     label: "DAG",     icon: <GitBranch size={15} /> },
  { id: "agents",  label: "Agents",  icon: <Bot size={15} /> },
  { id: "restate", label: "Restate", icon: <Settings2 size={15} /> },
];

function ThemeToggle() {
  const { theme, setTheme } = useTheme();
  const next: Record<string, () => void> = {
    system: () => setTheme("dark"),
    dark:   () => setTheme("light"),
    light:  () => setTheme("system"),
  };
  const icon = theme === "dark" ? <Moon size={13} /> : theme === "light" ? <Sun size={13} /> : <Monitor size={13} />;
  return (
    <button
      onClick={next[theme]}
      title={`Theme: ${theme}`}
      className="mac-btn mac-btn-ghost"
      style={{ padding: "4px 6px" }}
    >
      {icon}
    </button>
  );
}

export default function App() {
  const project = useAtomValue(projectAtom);
  const [activeTab, setActiveTab] = useState<Tab>("queue");
  const [selectedNodeId] = useAtom(selectedNodeIdAtom);
  const [showProvenance, setShowProvenance] = useState(false);

  useDagEvents();
  useTheme(); // Apply theme on mount

  if (!project) return <EmptyState />;

  return (
    <div className="flex h-full" style={{ background: "var(--content-bg)" }}>
      {/* ── Left sidebar ────────────────────────────────────────────────────── */}
      <div className="mac-sidebar flex flex-col shrink-0" style={{ width: 200 }}>
        {/* Project name */}
        <div className="mac-toolbar flex items-center px-3" style={{ height: 44, minHeight: 44 }}>
          <span className="font-semibold truncate" style={{ fontSize: 13, color: "var(--text-primary)" }}>
            {project.name}
          </span>
        </div>

        {/* Nav items */}
        <nav className="flex-1 pt-2 overflow-y-auto">
          <p className="mac-section-header">Navigator</p>
          {NAV_ITEMS.map((item) => (
            <div
              key={item.id}
              className={`mac-sidebar-item ${activeTab === item.id ? "active" : ""}`}
              onClick={() => setActiveTab(item.id)}
            >
              <span style={{ opacity: activeTab === item.id ? 1 : 0.6 }}>{item.icon}</span>
              {item.label}
            </div>
          ))}
        </nav>

        {/* Bottom controls */}
        <div className="flex items-center justify-end px-3 py-2 mac-divider" style={{ borderTop: "1px solid var(--divider)" }}>
          <ThemeToggle />
        </div>
      </div>

      {/* ── Content + Inspector ──────────────────────────────────────────────── */}
      <div className="flex-1 flex flex-col overflow-hidden">
        <ProjectHeader />

        <div className="flex-1 flex overflow-hidden">
          {/* Main content */}
          <div className="flex-1 flex flex-col overflow-hidden mac-content">
            {activeTab === "queue"   && <QueueView />}
            {activeTab === "dag"     && <GanttView />}
            {activeTab === "agents"  && <AgentActivityView />}
            {activeTab === "restate" && <RestateView />}
          </div>

          {/* Inspector / Probe panel */}
          {selectedNodeId && (
            <div className="mac-inspector flex flex-col shrink-0 overflow-hidden" style={{ width: 300 }}>
              {showProvenance ? (
                <>
                  <div className="mac-toolbar flex items-center gap-2 px-3" style={{ height: 36, minHeight: 36 }}>
                    <button className="mac-btn mac-btn-ghost" style={{ padding: "2px 6px", fontSize: 12 }} onClick={() => setShowProvenance(false)}>
                      ← Back
                    </button>
                    <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>Provenance</span>
                  </div>
                  <div className="flex-1 overflow-hidden">
                    <ProvenanceView nodeId={selectedNodeId} />
                  </div>
                </>
              ) : (
                <>
                  <div className="flex-1 overflow-hidden">
                    <ProbePanel nodeId={selectedNodeId} />
                  </div>
                  <div style={{ borderTop: "1px solid var(--divider)", padding: "8px 12px" }}>
                    <button className="mac-btn" style={{ width: "100%", justifyContent: "center", fontSize: 12 }} onClick={() => setShowProvenance(true)}>
                      Provenance →
                    </button>
                  </div>
                </>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
