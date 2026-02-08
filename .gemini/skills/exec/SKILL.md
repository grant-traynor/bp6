---
name: exec
description: Autonomous execution mode. Finds ready beads, works through them (spawning a Claude Code agent team for parallel work), closes beads with bd close, and commits. Use when you need to execute tasks defined as beads in the project.
---

# Exec — Autonomous Bead Execution

Work through ready beads autonomously: find them, do the work, close them, commit.

## 1. Find Ready Work

bd ready

- If a filter argument was provided (e.g. /exec frontend), only consider beads matching that filter.
- If no ready beads: run bd list --status open and report what exists and what's blocking progress.

## 2. Triage

Group ready beads by workstream based on the project structure (e.g. backend, frontend, docs, infra). Read the project's CLAUDE.md and directory layout to understand the workstreams.

Decide: can tasks run in parallel (different workstreams, no shared files) or must they be sequential?

- *Single task* — work on it directly, no team needed. Go to step 3b.
- *Multiple parallel tasks* — spawn an agent team. Go to step 3a.

## 3a. Execution — Agent Team for Parallel Tasks

Use Claude Code's built-in agent team system:

### Step 1: Create team and tasks

TeamCreate(team_name="exec-run", description="Executing ready beads")

For each bead, create a tracked task:

TaskCreate(subject="<bead-id>: <title>", description="...", activeForm="Working on <bead-id>")

### Step 2: Spawn workers with the Task tool

For each parallel bead, spawn a worker using the Task tool:

Task(
  subagent_type="general-purpose",
  name="worker-<bead-id>",
  team_name="exec-run",
  mode="bypassPermissions",
  prompt="..."
)

Each worker's prompt *MUST* include:

1. The specific bead ID they own (e.g. CS-042)
2. Tech stack context from the project's CLAUDE.md
3. Issue tracking: bd CLI — never edit .beads/issues.jsonl directly
4. Completion instructions — the worker MUST do ALL of these:
   - Run appropriate checks/tests for the code they changed
   - Close the bead: bd close <bead-id> --reason "description of what was done"
   - Mark its TaskList task as completed via TaskUpdate

### Step 3: Monitor and coordinate

- Use TaskList to track worker progress
- Use SendMessage to communicate with workers if needed
- When all children of an epic are closed, close the parent epic with bd close
- When all work is done, send shutdown_request to each worker via SendMessage

## 3b. Execution — Single Task

Work on it directly. Run appropriate checks/tests when done.

Close the bead:

bd close <bead-id> --reason "description of what was done"

## 4. Completion

1. Verify closure: bd list --status open — confirm completed work is closed.
2. Commit with a conventional commit message.
3. Push if on a feature branch (not the main branch).
4. Clean up: TeamDelete if a team was created.

## Rules

- *ALWAYS* use bd close to close beads — never edit .beads/issues.jsonl directly.
- *ALWAYS* close beads after completing work — internal TaskList completion is not enough.
- *ALWAYS* run tests/checks before closing a bead.
- Workers close their own beads via bd close — the lead doesn't close on their behalf.
- Use Claude Code's built-in TeamCreate`/Task`/`SendMessage`/`TaskList` for all orchestration — no custom scripts.