# Extend Feature: {{feature_id}}

You are tasked with adding new tasks or modifying the scope of this feature.

## Objective

Based on implementation discoveries or new requirements:
- Propose additional tasks needed
- Modify existing task scope as needed
- Maintain feature coherence
- Update dependencies accordingly

## Process

1. **Review Existing Tasks**: What's already planned for this feature?
2. **Understand Changes**: What new requirements or issues have emerged?
3. **Identify New Work**: What additional tasks are needed?
4. **Update Existing Tasks**: Which tasks need scope adjustments?
5. **Dependency Impact**: How do changes affect task sequence?

## Output Format

For new tasks, use `bd create` to create them:

```bash
bd create --title="Task title" --type=task --priority=1 --parent={{feature_id}}
```

**CRITICAL**: Always use `--parent={{feature_id}}` to make new tasks children of the feature being extended.

- **New Tasks**: Additional tasks with full specification
- **Modified Tasks**: Changes to existing tasks (use `bd update <task-id>` to modify)
- **Dependency Updates**: New or changed blocking relationships (use `bd dep add <task> <depends-on>`)
- **Acceptance Criteria Updates**: Changes to feature-level success criteria

## Guidelines

- **ALWAYS set `--parent={{feature_id}}`** when creating new tasks - this is mandatory for proper work breakdown structure
- Keep tasks granular (2-6 hours each)
- Don't expand scope unnecessarily
- Consider if changes belong in this feature or a new one
- Update acceptance criteria to reflect new scope
- Communicate impact on feature timeline

Review the feature and extension context below.
