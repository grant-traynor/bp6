import { useState, useRef, useEffect } from 'react';
import { Sun, Moon, Monitor, ChevronDown, FolderOpen, X, Star } from 'lucide-react';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { useTheme } from './hooks/useTheme';
import { useRecentProjects } from './hooks/useRecentProjects';
import { usePoeProject } from './hooks/usePoeProject';
import { ActivityFeed } from './components/ActivityFeed';
import { QueuePanel } from './components/QueuePanel';
import type { ProjectInfo } from './types';

// ── ProjectSwitcher ────────────────────────────────────────────────────────────

interface ProjectSwitcherProps {
  onProjectSelected: (projectId: string) => void;
}

function ProjectSwitcher({ onProjectSelected }: ProjectSwitcherProps) {
  const { favourites, recentNonFav, recordOpen, toggleFavourite } = useRecentProjects();
  const [open, setOpen] = useState(false);
  const [currentProject, setCurrentProject] = useState<ProjectInfo | null>(null);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    function handler(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    }
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [open]);

  async function openProject(dir: string) {
    setOpen(false);
    try {
      const info = await invoke<ProjectInfo>('open_project', { dir });
      void recordOpen(info.projectDir, info.name);
      setCurrentProject(info);
      onProjectSelected(info.projectId);
    } catch (err) {
      console.error('Failed to open project:', err);
    }
  }

  async function closeProject(dir: string, e: React.MouseEvent) {
    e.stopPropagation();
    try {
      await invoke('close_project', { dir });
      if (currentProject?.projectDir === dir) {
        setCurrentProject(null);
      }
    } catch (err) {
      console.error('Failed to close project:', err);
    }
  }

  async function handleBrowse() {
    setOpen(false);
    const selected = await openDialog({ directory: true, multiple: false, title: 'Open Project Directory' });
    if (!selected || typeof selected !== 'string') return;
    await openProject(selected);
  }

  const closedFavs = favourites;
  const closedRecents = recentNonFav;
  const hasAnyRecent = closedFavs.length > 0 || closedRecents.length > 0;

  return (
    <div className="relative" ref={ref} style={{ width: '100%' }}>
      <button
        onClick={() => setOpen((v) => !v)}
        style={{
          width: '100%', display: 'flex', alignItems: 'center', justifyContent: 'space-between',
          padding: '0 12px', height: 44, background: 'transparent', border: 'none',
          cursor: 'pointer', gap: 6,
        }}
        onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--sidebar-hover-bg)')}
        onMouseLeave={(e) => (e.currentTarget.style.background = 'transparent')}
      >
        <span style={{ fontSize: 13, fontWeight: 600, color: 'var(--text-primary)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', flex: 1, textAlign: 'left' }}>
          {currentProject?.name ?? 'No Project'}
        </span>
        <ChevronDown size={12} style={{ color: 'var(--text-tertiary)', flexShrink: 0, transform: open ? 'rotate(180deg)' : 'none', transition: 'transform 0.15s' }} />
      </button>

      {open && (
        <div className="mac-card absolute z-50 overflow-hidden" style={{ top: '100%', left: 8, right: 8, borderRadius: 10 }}>

          {/* Pinned (closed) */}
          {closedFavs.length > 0 && (
            <>
              <p className="mac-section-header" style={{ paddingTop: 8 }}>Pinned</p>
              {closedFavs.map((p) => (
                <div
                  key={p.path}
                  onClick={() => void openProject(p.path)}
                  style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 12px', cursor: 'pointer' }}
                  onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--sidebar-hover-bg)')}
                  onMouseLeave={(e) => (e.currentTarget.style.background = 'transparent')}
                >
                  <Star size={10} fill="currentColor" style={{ color: 'var(--status-paused)', flexShrink: 0 }} />
                  <div style={{ minWidth: 0, flex: 1 }}>
                    <p style={{ fontSize: 12, color: 'var(--text-primary)', margin: 0, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                      {p.name}
                    </p>
                    <p className="mono" style={{ fontSize: 10, color: 'var(--text-tertiary)', margin: 0, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                      {p.path}
                    </p>
                  </div>
                  <button
                    onClick={(e) => { e.stopPropagation(); toggleFavourite(p.path); }}
                    className="mac-btn mac-btn-ghost"
                    style={{ padding: '1px 4px', flexShrink: 0 }}
                    title="Unpin"
                  >
                    <Star size={10} fill="currentColor" style={{ color: 'var(--status-paused)' }} />
                  </button>
                  <button
                    onClick={(e) => void closeProject(p.path, e)}
                    className="mac-btn mac-btn-ghost"
                    style={{ padding: '1px 4px', flexShrink: 0 }}
                    title="Close project"
                  >
                    <X size={10} />
                  </button>
                </div>
              ))}
              {hasAnyRecent && closedRecents.length > 0 && <div className="mac-divider" />}
            </>
          )}

          {/* Recent (non-favourite) */}
          {closedRecents.length > 0 && (
            <>
              <p className="mac-section-header" style={{ paddingTop: closedFavs.length ? 4 : 8 }}>Recent</p>
              {closedRecents.map((p) => (
                <div
                  key={p.path}
                  onClick={() => void openProject(p.path)}
                  style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 12px', cursor: 'pointer' }}
                  onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--sidebar-hover-bg)')}
                  onMouseLeave={(e) => (e.currentTarget.style.background = 'transparent')}
                >
                  <span style={{ width: 10, flexShrink: 0 }} />
                  <div style={{ minWidth: 0, flex: 1 }}>
                    <p style={{ fontSize: 12, color: 'var(--text-primary)', margin: 0, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                      {p.name}
                    </p>
                    <p className="mono" style={{ fontSize: 10, color: 'var(--text-tertiary)', margin: 0, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                      {p.path}
                    </p>
                  </div>
                  <button
                    onClick={(e) => { e.stopPropagation(); toggleFavourite(p.path); }}
                    className="mac-btn mac-btn-ghost"
                    style={{ padding: '1px 4px', flexShrink: 0 }}
                    title="Pin"
                  >
                    <Star size={10} style={{ color: 'var(--text-placeholder)' }} />
                  </button>
                </div>
              ))}
            </>
          )}

          <div className="mac-divider" />
          <button
            onClick={() => void handleBrowse()}
            style={{
              width: '100%', display: 'flex', alignItems: 'center', gap: 8,
              padding: '8px 12px', background: 'transparent', border: 'none',
              cursor: 'pointer', color: 'var(--accent)', fontSize: 12, fontFamily: 'inherit',
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

// ── ThemeToggle ────────────────────────────────────────────────────────────────

function ThemeToggle() {
  const { theme, setTheme } = useTheme();
  const next: Record<string, () => void> = {
    system: () => setTheme('dark'),
    dark:   () => setTheme('light'),
    light:  () => setTheme('system'),
  };
  const icon = theme === 'dark' ? <Moon size={13} /> : theme === 'light' ? <Sun size={13} /> : <Monitor size={13} />;
  return (
    <button
      onClick={next[theme]}
      title={`Theme: ${theme}`}
      className="mac-btn mac-btn-ghost"
      style={{ padding: '4px 6px' }}
    >
      {icon}
    </button>
  );
}

// ── Badge ──────────────────────────────────────────────────────────────────────

interface DecisionBadgeProps {
  count: number;
}

function DecisionBadge({ count }: DecisionBadgeProps) {
  if (count === 0) return null;
  return (
    <span
      style={{
        display: 'inline-flex',
        alignItems: 'center',
        justifyContent: 'center',
        minWidth: 16,
        height: 16,
        padding: '0 4px',
        borderRadius: 8,
        background: 'var(--status-paused)',
        color: '#fff',
        fontSize: 10,
        fontWeight: 700,
        lineHeight: 1,
      }}
    >
      {count}
    </span>
  );
}

// ── App ────────────────────────────────────────────────────────────────────────

export default function App() {
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const { snapshot, activityFeed, openDecisions } = usePoeProject(selectedProjectId);
  useTheme();

  async function handleResolveDecision(decisionId: number, resolution: string) {
    await invoke('resolve_decision', { decisionId, resolution });
  }

  return (
    <div style={{ display: 'flex', height: '100vh', background: 'var(--content-bg)' }}>
      {/* Left sidebar */}
      <div className="mac-sidebar" style={{ width: 200, display: 'flex', flexDirection: 'column', flexShrink: 0 }}>
        <div className="mac-toolbar" style={{ minHeight: 44 }}>
          <ProjectSwitcher onProjectSelected={setSelectedProjectId} />
        </div>
        <div style={{ flex: 1, overflowY: 'auto', padding: '8px 0' }}>
          {snapshot ? (
            <div>
              <p className="mac-section-header">Tasks</p>
              <div style={{ padding: '4px 12px' }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 4 }}>
                  <span style={{ fontSize: 11, color: 'var(--text-secondary)' }}>Running</span>
                  <span style={{ fontSize: 11, fontWeight: 600, color: 'var(--status-running)' }}>
                    {snapshot.tasks.filter((t) => t.status === 'running').length}
                  </span>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 4 }}>
                  <span style={{ fontSize: 11, color: 'var(--text-secondary)' }}>Pending</span>
                  <span style={{ fontSize: 11, fontWeight: 600, color: 'var(--text-tertiary)' }}>
                    {snapshot.tasks.filter((t) => t.status === 'pending').length}
                  </span>
                </div>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 4 }}>
                  <span style={{ fontSize: 11, color: 'var(--text-secondary)' }}>Done</span>
                  <span style={{ fontSize: 11, fontWeight: 600, color: 'var(--status-completed)' }}>
                    {snapshot.tasks.filter((t) => t.status === 'done').length}
                  </span>
                </div>
                {openDecisions.length > 0 && (
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 4 }}>
                    <span style={{ fontSize: 11, color: 'var(--status-paused)' }}>Waiting</span>
                    <DecisionBadge count={openDecisions.length} />
                  </div>
                )}
              </div>
              {snapshot.phases.length > 0 && (
                <>
                  <p className="mac-section-header" style={{ marginTop: 8 }}>Phases</p>
                  {snapshot.phases.map((phase) => (
                    <div key={phase.id} className="mac-sidebar-item">
                      <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                        {phase.name}
                      </span>
                    </div>
                  ))}
                </>
              )}
            </div>
          ) : (
            <div style={{ padding: '12px', textAlign: 'center' }}>
              <p style={{ fontSize: 11, color: 'var(--text-placeholder)', margin: 0 }}>
                No project open
              </p>
            </div>
          )}
        </div>
        <div style={{ padding: '8px 12px', borderTop: '1px solid var(--divider)', display: 'flex', justifyContent: 'flex-end' }}>
          <ThemeToggle />
        </div>
      </div>

      {/* Right content */}
      <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
        {selectedProjectId && snapshot ? (
          <>
            {/* Project header */}
            <div
              className="mac-toolbar"
              style={{
                padding: '0 16px', minHeight: 44, display: 'flex', alignItems: 'center',
                borderBottom: '1px solid var(--divider)',
              }}
            >
              <span style={{ fontSize: 13, fontWeight: 600, color: 'var(--text-primary)' }}>
                {snapshot.project?.name ?? selectedProjectId}
              </span>
              <span style={{ marginLeft: 12, fontSize: 11, color: 'var(--text-tertiary)' }}>
                {snapshot.tasks.filter((t) => t.status === 'running').length} running
                {' · '}
                {snapshot.tasks.filter((t) => t.status === 'pending').length} pending
              </span>
            </div>

            {/* Main + Queue split */}
            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
              {/* Activity feed — top, flex-1 */}
              <div style={{ flex: 1, overflow: 'hidden' }}>
                <ActivityFeed items={activityFeed} tasks={snapshot.tasks} />
              </div>

              {/* Queue panel — always visible, fixed bottom */}
              <div style={{ height: 300, borderTop: '1px solid var(--divider)', flexShrink: 0 }}>
                <QueuePanel
                  decisions={openDecisions}
                  tasks={snapshot.tasks}
                  onResolve={handleResolveDecision}
                />
              </div>
            </div>
          </>
        ) : (
          <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
            <p style={{ fontSize: 13, color: 'var(--text-tertiary)' }}>Select a project to get started</p>
          </div>
        )}
      </div>
    </div>
  );
}
