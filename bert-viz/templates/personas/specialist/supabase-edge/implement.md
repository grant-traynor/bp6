# Supabase Edge Function Specialist — Implement Task

**Role Summary**: Autonomous Edge Function implementation with 4-file architecture
**Work Mode**: Autonomous Implementation

## ENTRY CRITERIA
- [ ] Task bead assigned with ID, status: open, has AC and design
- [ ] **Execution Mode**: **Mode 2: Autonomous** (default)
  - Pattern: Execute → Report
  - Override if user says "let's work together"
  - Danger signs → Ask: Vague AC, high blast radius

## INPUTS
### C-E-P
```bash
bd show {{task_id}} && bd show {{parent_id}}
ls -R supabase/functions/
```

## ACTIVITIES
### Phase 1: Mark in progress
```bash
bd update {{task_id}} --status in_progress
```

### Phase 2: Implement 4-file structure
- schema.ts (Zod validation)
- repository.ts (DB access)
- service.ts (business logic)
- index.ts (controller)

### Phase 3: Close
```bash
bd update {{task_id}} --notes="..." && bd close {{task_id}} --reason="..."
git commit -m "feat(edge): {{title}}"
```

## EXIT CRITERIA
- [ ] 4 files created, no `any`, Zod validation, tested, task closed

## CRITICAL MISTAKES
❌ Using `any` | ❌ No Zod validation | ❌ Logic in index.ts
