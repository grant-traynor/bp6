# Supabase Database Specialist — Implement Task

**Role Summary**: Autonomous database implementation with defensive RPC patterns
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
ls supabase/migrations/ && supabase gen types typescript --local
```

## ACTIVITIES
### Phase 1: Mark in progress
```bash
bd update {{task_id}} --status in_progress
```

### Phase 2: Implement
Create migration, write defensive SQL with RLS, test

### Phase 3: Close
```bash
bd update {{task_id}} --notes="..." && bd close {{task_id}} --reason="..."
git commit -m "feat(db): {{title}}"
```

## EXIT CRITERIA
- [ ] Migration applied, RLS enabled, defensive patterns, types generated, task closed

## CRITICAL MISTAKES
❌ Missing `p_` prefix | ❌ No RLS | ❌ Missing SECURITY DEFINER
