import { useState, useRef, useEffect } from "react";
import { useAtom, useAtomValue } from "jotai";
import {
  Inbox, GitBranch, Bot, Settings2, Sun, Moon, Monitor,
  ChevronDown, FolderOpen, X, Star, MessageSquare, BookOpen, BarChart2,
} from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { projectAtom, selectedNodeIdAtom, openProjectsAtom } from "./store/dag";
import { useDagEvents } from "./hooks/useDagEvents";
import { useTheme } from "./hooks/useTheme";
import { useRecentProjects } from "./hooks/useRecentProjects";
import { EmptyState } from "./components/EmptyState";
import { ProjectHeader } from "./components/ProjectHeader";
import { QueueView } from "./components/QueueView";
import { GanttView } from "./components/GanttView";
import { AgentActivityView } from "./components/AgentActivityView";
import { RestateView } from "./components/RestateView";
import { AdvisorView } from "./components/AdvisorView";
import { ProbePanel } from "./components/ProbePanel";
import { ProvenanceView } from "./components/ProvenanceView";
import { ConversationalView } from "./components/ConversationalView";
import { ArtifactsView } from "./components/ArtifactsView";
import { MetricsView } from "./components/MetricsView";
import type { ProjectInfo, LifecycleStatus } from "./types";

type Tab = "conversation" | "queue" | "dag" | "agents" | "restate" | "advisor" | "artefacts" | "metrics";

const NAV_ITEMS: { id: Tab; label: string; icon: React.ReactNode }[] = [
  { id: "conversation", label: "Chat",      icon: <MessageSquare size={15} /> },
  { id: "queue",        label: "Queue",     icon: <Inbox size={15} /> },
  { id: "dag",          label: "DAG",       icon: <GitBranch size={15} /> },
  { id: "agents",       label: "Agents",    icon: <Bot size={15} /> },
  { id: "restate",      label: "Restate",   icon: <Settings2 size={15} /> },
  { id: "advisor",      label: "Advisor",   icon: <MessageSquare size={15} /> },
  { id: "artefacts",    label: "Artefacts", icon: <BookOpen size={15} /> },
  { id: "metrics",      label: "Metrics",   icon: <BarChart2 size={15} /> },
];

function ProjectSwitcher() {
  const project = useAtomValue(projectAtom);
  const openProjects = useAtomValue(openProjectsAtom);
  const { favourites, recentNonFav, recordOpen, toggleFavourite } = useRecentProjects();
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

  async function openProject(dir: string) {
    setOpen(false);
    try {
      const info = await invoke<ProjectInfo>("open_project", { dir });
      void recordOpen(info.projectDir, info.name);
    } catch (err) {
      console.error("Failed to open project:", err);
    }
  }

  async function closeProject(dir: string, e: React.MouseEvent) {
    e.stopPropagation();
    try {
      await invoke("close_project", { dir });
    } catch (err) {
      console.error("Failed to close project:", err);
    }
  }

  async function handleBrowse() {
    setOpen(false);
    const selected = await openDialog({ directory: true, multiple: false, title: "Open Project Directory" });
    if (!selected || typeof selected !== "string") return;
    await openProject(selected);
  }

  const openDirs = new Set(openProjects.keys());
  const openList = Array.from(openProjects.values());
  // Recents not already open
  const closedFavs = favourites.filter((p) => !openDirs.has(p.path));
  const closedRecents = recentNonFav.filter((p) => !openDirs.has(p.path));
  const hasAnyRecent = closedFavs.length > 0 || closedRecents.length > 0;

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
        <div className="mac-card absolute z-50 overflow-hidden" style={{ top: "100%", left: 8, right: 8, borderRadius: 10 }}>

          {/* Open projects */}
          {openList.length > 0 && (
            <>
              <p className="mac-section-header" style={{ paddingTop: 8 }}>Open</p>
              {openList.map((p) => {
                const isActive = p.projectDir === project?.projectDir;
                return (
                  <div
                    key={p.projectDir}
                    onClick={() => openProject(p.projectDir)}
                    style={{
                      display: "flex", alignItems: "center", gap: 8,
                      padding: "6px 12px", cursor: "pointer",
                    }}
                    onMouseEnter={(e) => (e.currentTarget.style.background = "var(--sidebar-hover-bg)")}
                    onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
                  >
                    <span style={{
                      width: 6, height: 6, borderRadius: "50%", flexShrink: 0,
                      background: isActive ? "var(--accent)" : "var(--border-strong)",
                    }} />
                    <div style={{ minWidth: 0, flex: 1 }}>
                      <p style={{ fontSize: 12, fontWeight: isActive ? 600 : 400, color: isActive ? "var(--accent)" : "var(--text-primary)", margin: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                        {p.name}
                      </p>
                    </div>
                    <button
                      onClick={(e) => closeProject(p.projectDir, e)}
                      className="mac-btn mac-btn-ghost"
                      style={{ padding: "1px 4px", flexShrink: 0 }}
                      title="Close project"
                    >
                      <X size={10} />
                    </button>
                  </div>
                );
              })}
              {hasAnyRecent && <div className="mac-divider" />}
            </>
          )}

          {/* Pinned (closed) */}
          {closedFavs.length > 0 && (
            <>
              <p className="mac-section-header" style={{ paddingTop: openList.length ? 4 : 8 }}>Pinned</p>
              {closedFavs.map((p) => (
                <div
                  key={p.path}
                  onClick={() => openProject(p.path)}
                  style={{
                    display: "flex", alignItems: "center", gap: 8,
                    padding: "6px 12px", cursor: "pointer",
                  }}
                  onMouseEnter={(e) => (e.currentTarget.style.background = "var(--sidebar-hover-bg)")}
                  onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
                >
                  <Star size={10} fill="currentColor" style={{ color: "var(--status-paused)", flexShrink: 0 }} />
                  <div style={{ minWidth: 0, flex: 1 }}>
                    <p style={{ fontSize: 12, color: "var(--text-primary)", margin: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {p.name}
                    </p>
                    <p className="mono" style={{ fontSize: 10, color: "var(--text-tertiary)", margin: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {p.path}
                    </p>
                  </div>
                  <button
                    onClick={(e) => { e.stopPropagation(); toggleFavourite(p.path); }}
                    className="mac-btn mac-btn-ghost"
                    style={{ padding: "1px 4px", flexShrink: 0 }}
                    title="Unpin"
                  >
                    <Star size={10} fill="currentColor" style={{ color: "var(--status-paused)" }} />
                  </button>
                </div>
              ))}
            </>
          )}

          {/* Recent (closed, non-favourite) */}
          {closedRecents.length > 0 && (
            <>
              <p className="mac-section-header" style={{ paddingTop: closedFavs.length ? 4 : (openList.length ? 4 : 8) }}>Recent</p>
              {closedRecents.map((p) => (
                <div
                  key={p.path}
                  onClick={() => openProject(p.path)}
                  style={{
                    display: "flex", alignItems: "center", gap: 8,
                    padding: "6px 12px", cursor: "pointer",
                  }}
                  onMouseEnter={(e) => (e.currentTarget.style.background = "var(--sidebar-hover-bg)")}
                  onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
                >
                  <span style={{ width: 10, flexShrink: 0 }} />
                  <div style={{ minWidth: 0, flex: 1 }}>
                    <p style={{ fontSize: 12, color: "var(--text-primary)", margin: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {p.name}
                    </p>
                    <p className="mono" style={{ fontSize: 10, color: "var(--text-tertiary)", margin: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {p.path}
                    </p>
                  </div>
                  <button
                    onClick={(e) => { e.stopPropagation(); toggleFavourite(p.path); }}
                    className="mac-btn mac-btn-ghost"
                    style={{ padding: "1px 4px", flexShrink: 0 }}
                    title="Pin"
                  >
                    <Star size={10} style={{ color: "var(--text-placeholder)" }} />
                  </button>
                </div>
              ))}
            </>
          )}

          <div className="mac-divider" />
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

const CONVERSATIONAL_STEPS = new Set([1, 2, 3, 6]);

// Partial shape emitted by emit_status_event on the Rust side.
interface LifecycleStatusEvent {
  projectId: string;
  step: number;
  substep: string | null;
  status: LifecycleStatus["status"];
}

function useLifecycleStatus(projectId: string | null) {
  const [lifecycleStatus, setLifecycleStatus] = useState<LifecycleStatus | null>(null);

  useEffect(() => {
    if (!projectId) {
      setLifecycleStatus(null);
      return;
    }

    let cancelled = false;

    // One initial fetch to populate the full status object.
    invoke<LifecycleStatus>("get_lifecycle_status", { projectId })
      .then((s) => { if (!cancelled) setLifecycleStatus(s); })
      .catch(() => {});

    // From here, state transitions are pushed via lifecycle:status events emitted
    // by the Rust run handler at every step/status change — no polling needed.
    const unlistenPromise = listen<LifecycleStatusEvent>("lifecycle:status", (event) => {
      if (event.payload.projectId !== projectId) return;
      setLifecycleStatus((prev) => {
        if (!prev) {
          // No previous state yet — trigger a full fetch to fill in all fields.
          invoke<LifecycleStatus>("get_lifecycle_status", { projectId })
            .then((s) => { if (!cancelled) setLifecycleStatus(s); })
            .catch(() => {});
          return prev;
        }
        return {
          ...prev,
          step: event.payload.step,
          substep: event.payload.substep,
          status: event.payload.status,
        };
      });
    });

    return () => {
      cancelled = true;
      unlistenPromise.then((fn) => fn()).catch(() => {});
    };
  }, [projectId]);

  return lifecycleStatus;
}

export default function App() {
  const project = useAtomValue(projectAtom);
  const [activeTab, setActiveTab] = useState<Tab>("conversation");
  const [selectedNodeId] = useAtom(selectedNodeIdAtom);
  const [showProvenance, setShowProvenance] = useState(false);
  const lifecycleStatus = useLifecycleStatus(project?.projectId ?? null);

  useDagEvents();
  useTheme(); // Apply theme on mount

  const isConversationalStep =
    lifecycleStatus !== null && CONVERSATIONAL_STEPS.has(lifecycleStatus.step);

  async function handleStartLifecycle() {
    if (!project) return;
    try {
      await invoke("start_lifecycle", { projectId: project.projectId });
    } catch (err) {
      console.error("start_lifecycle error:", err);
    }
  }

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
            {activeTab === "conversation" && (
              lifecycleStatus ? (
                <ConversationalView
                  projectId={project.projectId}
                  lifecycleStatus={lifecycleStatus}
                  onStartLifecycle={handleStartLifecycle}
                />
              ) : !isConversationalStep ? (
                <div className="flex-1 flex items-center justify-center">
                  <div style={{ border: "1px dashed var(--border-strong)", borderRadius: 10, padding: "48px 32px", textAlign: "center" }}>
                    <p style={{ fontSize: 13, color: "var(--text-tertiary)", margin: 0 }}>Lifecycle not started.</p>
                    <p style={{ fontSize: 11, color: "var(--text-placeholder)", marginTop: 4 }}>Start a lifecycle to begin the conversational workflow.</p>
                    <button
                      onClick={handleStartLifecycle}
                      className="mac-btn mac-btn-primary"
                      style={{ marginTop: 16, padding: "6px 20px", fontSize: 12 }}
                    >
                      Start Lifecycle
                    </button>
                  </div>
                </div>
              ) : (
                <div className="flex-1 flex items-center justify-center">
                  <p className="mono" style={{ fontSize: 12, color: "var(--text-tertiary)" }}>Loading lifecycle status…</p>
                </div>
              )
            )}
            {activeTab === "queue"      && <QueueView />}
            {activeTab === "dag"        && <GanttView />}
            {activeTab === "agents"     && <AgentActivityView />}
            {activeTab === "restate"    && <RestateView />}
            {activeTab === "advisor"    && <AdvisorView />}
            {activeTab === "artefacts"  && <ArtifactsView />}
            {activeTab === "metrics"    && <MetricsView />}
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
