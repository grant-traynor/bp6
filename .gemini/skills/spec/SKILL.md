---
name: spec
description: Interactive planning mode. Discuss requirements, create/refine beads, break down epics, and prioritize work. No code modifications allowed. Use for project planning, requirement refinement, and issue management.
---

# Spec Mode — Planning & Issue Management

You are in *planning-only mode*.

## On Invocation

Immediately run these commands to establish context:

bd list --status open    # Show current open issues
bd stats                 # Show progress overview

If a topic argument was provided (e.g., /spec auth flow), focus the discussion on that topic after showing the overview.

## Tool Restrictions

*ALLOWED — read and plan:*
- Read, Glob, Grep — read files for context
- Bash — ONLY for bd commands
- TaskCreate, TaskUpdate, TaskList, TaskGet — manage session tasks

*FORBIDDEN — no code changes:*
- Write — do NOT create or modify files
- Edit — do NOT edit source code
- NotebookEdit — do NOT edit notebooks

This is a planning session. All output is beads and discussion, not code.

## What You Help With

1. *Creating beads*: Use bd create to define new work items with clear titles, descriptions, and acceptance criteria
2. *Setting dependencies*: Use bd dep add <from> <to> to establish ordering between beads
3. *Defining Epics*: Create beads for all top-level work items
4. *Breaking Down Epics Into Features*: Create beads for all features within an epic. All beads MUST have acceptance criteria based upon the verification by inspection of code of the subordinate tasks.
5. *Breaking down epics and features into tasks*: Decompose large beads into smaller, actionable tasks. Each task must have acceptance criteria defined based upon the verification by inspection of code of the tasks definition.
6. *Architecture discussion*: Read existing code for context, discuss design tradeoffs
7. *Requirements refinement*: Sharpen acceptance criteria, identify edge cases, clarify scope
8. *Prioritization*: Help decide what to work on next using bd ready and dependency analysis

## Issue Tracking

Always use the bd CLI. Never edit .beads/issues.jsonl directly.

bd create                    # Create new bead (interactive)
bd list --status open        # List open beads
bd ready                     # Show unblocked beads ready for work
bd show <id>                 # Show bead details
bd dep add <from> <to>       # Add dependency (from depends on to)
bd dep rm <from> <to>        # Remove dependency
bd stats                     # Progress overview
bd close <id> --reason "..." # Close a bead

## Output Goal

Produce refined, well-structured beads that are ready for /exec to pick up. Each bead should have:

- A clear, concise title
- Description with enough context to implement without ambiguity
- Acceptance criteria where applicable
- Dependencies set correctly so bd ready surfaces the right next steps
- You CAN NOT under any circumstances close a task until you have verified that it's scope and acceptance criteria have been validated.