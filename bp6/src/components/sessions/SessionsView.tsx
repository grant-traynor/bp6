import { useState, useMemo } from "react";
import { ChevronDown, ChevronRight, FolderOpen, MessageSquare, Activity, Square, Play } from "lucide-react";
import { SessionInfo, PERSONA_ICONS, resumeSpecificSession, createSessionWindow } from "../../api";
import { cn } from "../../utils";

interface SessionsViewProps {
  sessions: SessionInfo[];
  currentProjectPath: string;
  onSessionClick: (sessionId: string) => void;
  onTerminate: (sessionId: string) => void;
}

interface ProjectGroup {
  projectPath: string;
  displayName: string;
  isCurrent: boolean;
  sessions: SessionInfo[];
}

function getProjectDisplayName(projectPath: string): string {
  if (!projectPath) return "Unknown Project";
  // Use last non-empty segment of the path
  const parts = projectPath.replace(/\\/g, "/").split("/").filter(Boolean);
  return parts[parts.length - 1] ?? "Unknown Project";
}

function abbreviateSessionId(sessionId: string): string {
  if (sessionId.length <= 12) return sessionId;
  return `${sessionId.slice(0, 8)}…`;
}

function getPersonaIcon(persona: string, role?: string | null): string {
  if (role && PERSONA_ICONS[role]) return PERSONA_ICONS[role];
  return PERSONA_ICONS[persona] ?? "🤖";
}

function StatusBadge({ status }: { status: string }) {
  const isRunning = status === "running";
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 px-1.5 py-0.5 rounded-md text-[10px] font-bold uppercase tracking-wide",
        isRunning
          ? "bg-emerald-500/15 text-emerald-600 dark:text-emerald-400"
          : "bg-[var(--background-tertiary)] text-[var(--text-muted)]"
      )}
    >
      <span
        className={cn(
          "w-1.5 h-1.5 rounded-full",
          isRunning ? "bg-emerald-500 animate-pulse" : "bg-[var(--text-muted)]"
        )}
      />
      {isRunning ? "Running" : "Idle"}
    </span>
  );
}

function SessionRow({
  session,
  onClick,
  onTerminate,
}: {
  session: SessionInfo;
  onClick: () => void;
  onTerminate: () => void;
}) {
  const icon = getPersonaIcon(session.persona, session.role);
  const abbrevId = abbreviateSessionId(session.sessionId);
  const isRunning = session.status === "running";

  async function handleResume(e: React.MouseEvent) {
    e.stopPropagation();
    try {
      await resumeSpecificSession(session.sessionId);
      await createSessionWindow(session.sessionId);
    } catch (err) {
      console.error("Failed to resume session:", err);
    }
  }

  return (
    <div className="flex items-center border-b border-[var(--border-primary)]/40 last:border-b-0 group">
      <button
        onClick={onClick}
        className="flex-1 flex items-center gap-3 px-4 py-2.5 text-left hover:bg-[var(--background-tertiary)] transition-colors"
      >
        {/* Persona icon */}
        <span className="text-lg leading-none flex-shrink-0 w-6 text-center" aria-label={session.persona}>
          {icon}
        </span>

        {/* Session info */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 flex-wrap">
            <span className="font-mono text-xs font-bold text-[var(--text-primary)] group-hover:text-[var(--accent-primary)] transition-colors">
              {abbrevId}
            </span>
            {session.beadId && (
              <span className="font-mono text-[10px] text-[var(--text-muted)] bg-[var(--background-tertiary)] px-1.5 py-0.5 rounded-md border border-[var(--border-primary)]/60">
                {session.beadId}
              </span>
            )}
          </div>
          <div className="flex items-center gap-2 mt-0.5 text-xs text-[var(--text-muted)]">
            <span className="capitalize">{session.role ?? session.persona}</span>
            {session.task && (
              <span className="text-[var(--text-muted)]/60">· {session.task}</span>
            )}
          </div>
        </div>

        {/* Status + message count */}
        <div className="flex items-center gap-2 flex-shrink-0">
          <StatusBadge status={session.status} />
          <span className="flex items-center gap-1 text-[10px] text-[var(--text-muted)]">
            <MessageSquare size={10} strokeWidth={2.5} />
            {session.messageCount}
          </span>
        </div>
      </button>

      {/* Action button — visible on hover */}
      {isRunning ? (
        <button
          onClick={(e) => { e.stopPropagation(); onTerminate(); }}
          className="flex-shrink-0 p-2 mr-2 text-[var(--text-muted)] hover:text-rose-500 rounded-lg transition-colors opacity-0 group-hover:opacity-100"
          title="Terminate session"
        >
          <Square size={13} strokeWidth={2.5} fill="currentColor" />
        </button>
      ) : (
        <button
          onClick={handleResume}
          className="flex-shrink-0 p-2 mr-2 text-[var(--text-muted)] hover:text-emerald-500 rounded-lg transition-colors opacity-0 group-hover:opacity-100"
          title="Resume session"
        >
          <Play size={13} strokeWidth={2.5} fill="currentColor" />
        </button>
      )}
    </div>
  );
}

function ProjectGroupSection({
  group,
  onSessionClick,
  onTerminate,
  defaultOpen,
}: {
  group: ProjectGroup;
  onSessionClick: (sessionId: string) => void;
  onTerminate: (sessionId: string) => void;
  defaultOpen: boolean;
}) {
  const [isOpen, setIsOpen] = useState(defaultOpen);

  return (
    <div className="mb-3 rounded-xl border border-[var(--border-primary)] overflow-hidden shadow-sm">
      {/* Group header */}
      <button
        onClick={() => setIsOpen((v) => !v)}
        className="w-full flex items-center gap-2 px-4 py-3 bg-[var(--background-secondary)] hover:bg-[var(--background-tertiary)] transition-colors text-left"
      >
        <span className="text-[var(--text-muted)] flex-shrink-0">
          {isOpen ? (
            <ChevronDown size={14} strokeWidth={2.5} />
          ) : (
            <ChevronRight size={14} strokeWidth={2.5} />
          )}
        </span>
        <FolderOpen
          size={14}
          strokeWidth={2.5}
          className={cn(
            "flex-shrink-0",
            group.isCurrent
              ? "text-[var(--accent-primary)]"
              : "text-[var(--text-muted)]"
          )}
        />
        <span
          className={cn(
            "text-xs font-black uppercase tracking-[0.2em] truncate flex-1",
            group.isCurrent ? "text-[var(--accent-primary)]" : "text-[var(--text-primary)]"
          )}
          title={group.projectPath || "Unknown Project"}
        >
          {group.displayName}
        </span>
        {group.isCurrent && (
          <span className="flex-shrink-0 text-[10px] font-bold text-[var(--accent-primary)] bg-[var(--accent-primary)]/10 px-1.5 py-0.5 rounded-md border border-[var(--accent-primary)]/30 uppercase tracking-wider">
            Current
          </span>
        )}
        <span className="flex-shrink-0 text-[10px] font-bold text-[var(--text-muted)] bg-[var(--background-primary)] px-1.5 py-0.5 rounded-md border border-[var(--border-primary)]/60 ml-1">
          {group.sessions.length}
        </span>
      </button>

      {/* Session rows */}
      {isOpen && (
        <div className="bg-[var(--background-primary)]">
          {group.sessions.map((session) => (
            <SessionRow
              key={session.sessionId}
              session={session}
              onClick={() => onSessionClick(session.sessionId)}
              onTerminate={() => onTerminate(session.sessionId)}
            />
          ))}
        </div>
      )}
    </div>
  );
}

export function SessionsView({
  sessions,
  currentProjectPath,
  onSessionClick,
  onTerminate,
}: SessionsViewProps) {
  const groups = useMemo<ProjectGroup[]>(() => {
    const map = new Map<string, SessionInfo[]>();

    for (const session of sessions) {
      const key = (session as SessionInfo & { projectPath?: string }).projectPath || "";
      if (!map.has(key)) map.set(key, []);
      map.get(key)!.push(session);
    }

    const result: ProjectGroup[] = [];

    // Current project first
    if (currentProjectPath && map.has(currentProjectPath)) {
      result.push({
        projectPath: currentProjectPath,
        displayName: getProjectDisplayName(currentProjectPath),
        isCurrent: true,
        sessions: map.get(currentProjectPath)!,
      });
      map.delete(currentProjectPath);
    }

    // Remaining projects sorted by display name
    const remaining = Array.from(map.entries())
      .map(([path, sess]) => ({
        projectPath: path,
        displayName: getProjectDisplayName(path),
        isCurrent: false,
        sessions: sess,
      }))
      .sort((a, b) => {
        // "Unknown Project" always last
        if (!a.projectPath && b.projectPath) return 1;
        if (a.projectPath && !b.projectPath) return -1;
        return a.displayName.localeCompare(b.displayName);
      });

    result.push(...remaining);
    return result;
  }, [sessions, currentProjectPath]);

  const runningCount = useMemo(
    () => sessions.filter((s) => s.status === "running").length,
    [sessions]
  );

  return (
    <div className="flex-1 flex flex-col overflow-hidden bg-[var(--background-primary)]">
      {/* View header */}
      <div className="flex-shrink-0 px-6 py-4 border-b-2 border-[var(--border-primary)] bg-[var(--background-secondary)] flex items-center gap-3">
        <Activity size={18} strokeWidth={2.5} className="text-[var(--accent-primary)]" />
        <h2 className="text-sm font-black text-[var(--text-primary)] uppercase tracking-[0.25em]">
          All Sessions
        </h2>
        <div className="flex items-center gap-2 ml-auto">
          <span className="text-xs text-[var(--text-muted)] bg-[var(--background-tertiary)] px-2 py-0.5 rounded-md border border-[var(--border-primary)]/60 font-bold">
            {sessions.length} total
          </span>
          {runningCount > 0 && (
            <span className="text-xs text-emerald-600 dark:text-emerald-400 bg-emerald-500/10 px-2 py-0.5 rounded-md border border-emerald-500/30 font-bold flex items-center gap-1">
              <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse" />
              {runningCount} running
            </span>
          )}
        </div>
      </div>

      {/* Scrollable content */}
      <div className="flex-1 overflow-y-auto px-4 py-4">
        {sessions.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-center py-20">
            <MessageSquare
              size={40}
              strokeWidth={1.5}
              className="text-[var(--text-muted)] mb-4 opacity-40"
            />
            <p className="text-sm font-bold text-[var(--text-muted)]">No sessions</p>
            <p className="text-xs text-[var(--text-muted)] mt-1 opacity-70">
              Start or resume a session to see it here
            </p>
          </div>
        ) : (
          groups.map((group, idx) => (
            <ProjectGroupSection
              key={group.projectPath || "__unknown__"}
              group={group}
              onSessionClick={onSessionClick}
              onTerminate={onTerminate}
              defaultOpen={idx === 0 || group.isCurrent}
            />
          ))
        )}
      </div>
    </div>
  );
}

export default SessionsView;
