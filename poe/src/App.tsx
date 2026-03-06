import { useState, useRef, useEffect } from "react";
import { useAtom, useAtomValue } from "jotai";
import {
  Inbox, GitBranch, Bot, Settings2, Sun, Moon, Monitor,
  ChevronDown, FolderOpen,
} from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { projectAtom, selectedNodeIdAtom } from "./store/dag";
import { useDagEvents } from "./hooks/useDagEvents";
import { useTheme } from "./hooks/useTheme";
import { useRecentProjects } from "./hooks/useRecentProjects";
import { EmptyState } from "./components/EmptyState";
import { ProjectHeader } from "./components/ProjectHeader";
import { QueueView } from "./components/QueueView";
import { GanttView } from "./components/GanttView";
import { AgentActivityView } from "./components/AgentActivityView";
import { RestateView } from "./components/RestateView";
import { ProbePanel } from "./components/ProbePanel";
import { ProvenanceView } from "./components/ProvenanceView";
import type { ProjectInfo } from "./types";

type Tab = "queue" | "dag" | "agents" | "restate";

const NAV_ITEMS: { id: Tab; label: string; icon: React.ReactNode }[] = [
  { id: "queue",   label: "Queue",   icon: <Inbox size={15} /> },
  { id: "dag",     label: "DAG",     icon: <GitBranch size={15} /> },
  { id: "agents",  label: "Agents",  icon: <Bot size={15} /> },
  { id: "restate", label: "Restate", icon: <Settings2 size={15} /> },
];

function ProjectSwitcher() {
  const [project, setProject] = useAtom(projectAtom);
  const { favourites, recentNonFav, recordOpen } = useRecentProjects();
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    function handler(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    }
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  async function switchTo(dir: string) {
    setOpen(false);
    try {
      if (project) await invoke("close_project");
      const info = await invoke<ProjectInfo>("open_project", { dir });
      void recordOpen(info.projectDir, info.name);
      setProject(info);
    } catch (err) {
      console.error("Failed to switch project:", err);
    }
  }

  async function handleBrowse() {
    setOpen(false);
    const selected = await openDialog({ directory: true, multiple: false, title: "Open Project Directory" });
    if (!selected || typeof selected !== "string") return;
    await switchTo(selected);
  }

  const allRecents = [...favourites, ...recentNonFav];

  return (
    <div className="relative" ref={ref} style={{ width: "100%" }}>
      <button
        onClick={() => setOpen((v) => !v)}
        style={{
          width: "100%", display: "flex", alignItems: "center", justifyContent: "space-between",
          padding: "0 12px", height: 44, background: "transparent", border: "none",
          cursor: "pointer", gap: 6,
        }}
        onMouseEnter={(e) => (e.currentTarget.style.background = "var(--sidebar-hover-bg)")}
        onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
      >
        <span style={{ fontSize: 13, fontWeight: 600, color: "var(--text-primary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", flex: 1, textAlign: "left" }}>
          {project?.name ?? "No Project"}
        </span>
        <ChevronDown size={12} style={{ color: "var(--text-tertiary)", flexShrink: 0, transform: open ? "rotate(180deg)" : "none", transition: "transform 0.15s" }} />
      </button>

      {open && (
        <div
          className="mac-card absolute z-50 overflow-hidden"
          style={{ top: "100%", left: 8, right: 8, borderRadius: 10 }}
        >
          {allRecents.length > 0 && (
            <>
              {allRecents.map((p) => {
                const isActive = p.path === project?.projectDir;
                return (
                  <button
                    key={p.path}
                    onClick={() => switchTo(p.path)}
                    style={{
                      width: "100%", display: "flex", alignItems: "center", gap: 8,
                      padding: "7px 12px", background: "transparent", border: "none",
                      cursor: "pointer", textAlign: "left",
                    }}
                    onMouseEnter={(e) => (e.currentTarget.style.background = "var(--sidebar-hover-bg)")}
                    onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
                  >
                    <span style={{
                      width: 6, height: 6, borderRadius: "50%", flexShrink: 0,
                      background: isActive ? "var(--accent)" : "transparent",
                      border: isActive ? "none" : "1px solid var(--border-strong)",
                    }} />
                    <div style={{ minWidth: 0, flex: 1 }}>
                      <p style={{ fontSize: 12, fontWeight: isActive ? 600 : 400, color: isActive ? "var(--accent)" : "var(--text-primary)", margin: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                        {p.name}
                      </p>
                      <p className="mono" style={{ fontSize: 10, color: "var(--text-tertiary)", margin: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                        {p.path}
                      </p>
                    </div>
                  </button>
                );
              })}
              <div className="mac-divider" />
            </>
          )}
          <button
            onClick={handleBrowse}
            style={{
              width: "100%", display: "flex", alignItems: "center", gap: 8,
              padding: "8px 12px", background: "transparent", border: "none",
              cursor: "pointer", color: "var(--accent)", fontSize: 12, fontFamily: "inherit",
            }}
          >
            <FolderOpen size={12} />
            Open Project…
          </button>
        </div>
      )}
    </div>
  );
}

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
        {/* Project switcher */}
        <div className="mac-toolbar" style={{ minHeight: 44 }}>
          <ProjectSwitcher />
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
