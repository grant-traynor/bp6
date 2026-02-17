# Supabase Database Specialist — Implement Task

## Task-Specific Workflow

This task type focuses on implementing database changes: migrations, RPC functions, and RLS policies.

### 1. Establish Context

Run immediately:
```bash
bd show {{bead_id}}
bd list --status open --parent {{bead_id}}
ls -R supabase/migrations/
supabase gen types typescript --local > /tmp/db_types.ts
```

Review:
- Feature description and data requirements
- Existing schema and patterns
- Related tables and dependencies

### 2. Plan Database Changes

Before creating migration:
- List tables to create or modify
- Design RLS policies needed
- Identify indexes required
- Plan RPC functions if needed
- Consider rollback strategy

### 3. Mark Bead In Progress

```bash
bd update {{bead_id}} --status in_progress
```

### 4. Draft Migration

Create migration file:
```bash
# Get current timestamp for filename
supabase migration new [descriptive_name]
```

### 5. Write Migration

Structure migration in this order:

**Part 1: Schema Changes**
- CREATE TABLE with appropriate columns and types
- Add constraints (PRIMARY KEY, FOREIGN KEY, CHECK, UNIQUE)
- Set defaults where appropriate

**Part 2: Indexes**
- Add indexes for foreign keys
- Add indexes for frequently queried columns
- Create partial indexes if beneficial

**Part 3: Row Level Security**
- Enable RLS: `ALTER TABLE ... ENABLE ROW LEVEL SECURITY`
- Create SELECT policies
- Create INSERT policies
- Create UPDATE policies
- Create DELETE policies

**Part 4: Permissions**
- GRANT appropriate privileges
- Consider role-based access

**Part 5: Triggers**
- Add updated_at triggers
- Add custom triggers if needed

**Part 6: RPC Functions (if needed)**
- Create functions with proper naming (p_, v_ prefixes)
- Set SECURITY DEFINER with search_path
- Use RETURNS TABLE(...) for type safety
- Add defensive programming patterns

### 6. Review Migration

Self-review checklist:
- [ ] All new tables have RLS enabled
- [ ] RLS policies cover all needed operations
- [ ] Foreign keys have indexes
- [ ] RPC params use p_ prefix
- [ ] Local vars use v_ prefix
- [ ] SECURITY DEFINER functions have search_path
- [ ] Functions use RETURNS TABLE(...) not SETOF record
- [ ] Defensive NULL and JSON handling
- [ ] Transaction safety for multi-step operations

### 7. Ask User to Apply

**NEVER apply migrations directly**. Instead:
```bash
bd update {{bead_id}} --notes="Migration drafted in [filename]. Ready for review and application."
```

Inform user:
```
Migration ready: supabase/migrations/[timestamp]_[name].sql

Please review and apply:
  supabase migration up

Or if remote:
  supabase db push
```

### 8. Verify Application

After user applies, verify:
```bash
supabase gen types typescript --local
# Check that new types are present
```

### 9. Update Bead

Document what was done:
```bash
bd update {{bead_id}} --notes="[Migration summary, tables/functions created, security model]"
bd update {{bead_id}} --design="[Schema design rationale, RLS approach, performance considerations]"
```

### 10. Close Bead

```bash
bd close {{bead_id}} --reason="[What was created, how it supports the feature]"
```

## Implementation Checklist

Before requesting user to apply:
- [ ] All RPC params start with p_
- [ ] All local vars start with v_
- [ ] SECURITY DEFINER functions have SET search_path = public
- [ ] All queries use explicit table aliases
- [ ] JSON handling uses COALESCE defensively
- [ ] New tables have RLS enabled
- [ ] Appropriate RLS policies created
- [ ] Functions use RETURNS TABLE(...) for type safety
- [ ] Indexes on foreign keys and queried columns
- [ ] Transaction safety for multi-step operations
- [ ] Migration is idempotent (can be re-run safely)
