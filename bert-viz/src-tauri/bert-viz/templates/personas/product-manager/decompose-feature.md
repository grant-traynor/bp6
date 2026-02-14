# Decompose Feature: {{feature_id}}

You are tasked with breaking down this feature into implementable tasks.

## Objective

Decompose the feature into 3-10 granular tasks that:
- Are small enough to complete in 2-6 hours each
- Have clear, verifiable completion criteria
- Can be worked on with minimal context switching
- Form a logical implementation sequence

## Process

1. **Understand the Feature**: Review description, acceptance criteria, and context
2. **Identify Work Streams**: Frontend, backend, database, testing, documentation
3. **Define Task Sequence**: What order should tasks be completed?
4. **Specify Task Details**: What exactly needs to be done in each task?
5. **Set Dependencies**: Which tasks block others?

## Output Format

For each task, use `bd create` to create the task:

```bash
bd create --title="Task title" --type=task --priority=1 --parent={{feature_id}}
```

**CRITICAL**: Always use `--parent={{feature_id}}` to make each task a child of the feature being decomposed.

For each task, provide:
- **Title**: Specific, implementation-focused (e.g., "Add user_id column to profiles table")
- **Description**: Technical details of what to implement
- **Acceptance Criteria**: Code-level verification (e.g., "Column exists in schema, migration runs successfully")
- **Files/Components**: Which parts of the codebase will change
- **Dependencies**: Which tasks must complete first (use `bd dep add <task> <depends-on>` after creation)

## Task Types to Consider

- **Database**: Schema changes, migrations, indexes
- **Backend**: API endpoints, business logic, data access
- **Frontend**: UI components, state management, API integration
- **Testing**: Unit tests, integration tests, E2E tests
- **Documentation**: API docs, code comments, user guides

## Guidelines

- **ALWAYS set `--parent={{feature_id}}`** when creating tasks - this is mandatory for proper work breakdown structure
- Each task should modify 1-3 files ideally
- Define task boundaries clearly to avoid scope creep
- Consider rollback/migration strategies for DB changes
- Think about error handling and edge cases
- Include testing as separate tasks, not afterthoughts
- Use `bd dep add` to set task-to-task dependencies AFTER creating all tasks

Review the feature context below and propose the task breakdown.
