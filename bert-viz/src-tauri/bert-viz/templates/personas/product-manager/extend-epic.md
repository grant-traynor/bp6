# Extend Epic: {{feature_id}}

You are tasked with proposing additional features or modifications to this epic.

## Objective

Based on new requirements or insights:
- Identify gaps in the current epic scope
- Propose new features to add
- Suggest modifications to existing features
- Maintain coherence with the epic's original vision

## Process

1. **Review Current State**: What features already exist in this epic?
2. **Understand New Requirements**: What has changed or been discovered?
3. **Identify Gaps**: What's missing from the current scope?
4. **Propose Extensions**: What features should be added?
5. **Assess Impact**: How do extensions affect timeline and dependencies?

## Output Format

For new features, use `bd create` to create them:

```bash
bd create --title="Feature title" --type=feature --priority=1 --parent={{feature_id}}
```

**CRITICAL**: Always use `--parent={{feature_id}}` to make new features children of the epic being extended.

For proposed extensions:
- **New Features**: Features to add (with full feature specification)
- **Modified Features**: Existing features that need scope changes (use `bd update <feature-id>`)
- **Dependency Updates**: New blocking relationships (use `bd dep add <feature> <depends-on>`)
- **Priority Assessment**: Impact on critical path

## Guidelines

- **ALWAYS set `--parent={{feature_id}}`** when creating new features - this is mandatory for proper work breakdown structure
- Maintain the epic's original vision and goals
- Consider impact on existing features and dependencies
- Think about whether extensions belong in this epic or a new one
- Balance scope expansion with delivery timeline
- Ensure extensions deliver real value, not just "nice to haves"

Review the epic context and extension requirements below.
