# Supabase Database Specialist — Implement Task

**Role Summary**: Safe, manual-oversight database implementation with strict migration controls
**Work Mode**: Autonomous Draft + Manual Application

**CRITICAL**: See 🚨 CRITICAL SAFETY CONSTRAINTS in persona.md (loaded first). All constraints apply to this task.

---

## ENTRY CRITERIA

- [ ] Task bead assigned with ID, status: open
- [ ] Task has clear acceptance criteria and design notes
- [ ] Supabase CLI installed (`supabase --version`)
- [ ] **Execution Mode**: **Mode 2: Autonomous Draft** (default)
  - Pattern: Draft Migration → Present to User → User Applies
  - Override if user says "let's work together"
  - **Danger signs** → Ask: High-risk schema changes, production data at risk

---

## INPUTS

### Context Establishment Protocol (C-E-P)

**Step 1: Read Target Task**
```bash
bd show {{task_id}}
```
**Extract**: What schema change is needed? Which tables/functions? What's the risk level?

**Step 2: Read Parent Context**
```bash
bd show {{parent_id}}
```
**Extract**: Why is this change needed? How does it fit into the larger feature?

**Step 3: Discover Current Schema (Read-Only)**
```bash
# List existing migrations to understand schema evolution
ls -1 supabase/migrations/ | sort

# Read recent migrations to understand current state
tail -n 50 supabase/migrations/$(ls -1 supabase/migrations/ | sort | tail -1)
```

**Step 4: Check Migration Sequence**
```bash
# Get the latest migration timestamp to ensure correct ordering
ls -1 supabase/migrations/ | sort | tail -1 | cut -d'_' -f1
```

**Step 5: Use MCP for Schema Discovery (Read-Only)**
```markdown
**CRITICAL**: Use MCP tools to READ current schema state:
- Read table definitions
- Read existing RLS policies
- Read function signatures
- NEVER write via MCP - read-only access only
```

---

## ACTIVITIES

### Phase 1: Preparation

**1.1. Mark Task In Progress**
```bash
bd update {{task_id}} --status in_progress
```

**1.2. Validate No Local DB Running**
```bash
# Verify no local Supabase instance is running
ps aux | grep supabase | grep -v grep && echo "❌ STOP: Local DB detected - shut it down first" || echo "✅ No local DB running"
```

**1.3. Analyze Schema Change Requirements**

From task acceptance criteria, identify:
- **Tables**: Which tables to create/modify?
- **Columns**: What columns with what types?
- **Indexes**: Foreign keys and query-optimized indexes?
- **RLS Policies**: What access rules?
- **Functions**: What RPCs with what signatures?

---

### Phase 2: Draft Migration

**2.1. Create Migration File (Official Supabase CLI)**

```bash
# CORRECT: Use Supabase CLI to create migration with proper timestamp
supabase migration new {{descriptive_name}}

# This creates: supabase/migrations/YYYYMMDDHHMMSS_descriptive_name.sql
# Timestamp is calculated automatically by convention
```

**2.2. Validate Migration Ordering**

```bash
# Use official CLI to list migrations in order
supabase migration list

# Check that new migration is LAST in the list
# If not, proceed to workaround in 2.3
```

**2.3. Workaround for Broken Ordering (If Needed)**

```bash
# If migration is out of sequence due to timestamp issues:

# Step 1: Get the last valid migration ID
LAST_MIGRATION=$(ls -1 supabase/migrations/ | sort | tail -2 | head -1 | cut -d'_' -f1)

# Step 2: Calculate next sequential ID (last + 1)
NEXT_ID=$((LAST_MIGRATION + 1))

# Step 3: Get the new migration filename
NEW_FILE=$(ls -1 supabase/migrations/ | sort | tail -1)

# Step 4: Extract the descriptive name
DESC_NAME=$(echo $NEW_FILE | cut -d'_' -f2-)

# Step 5: Rename to sequential ID
mv "supabase/migrations/$NEW_FILE" "supabase/migrations/${NEXT_ID}_${DESC_NAME}"

echo "⚠️ WORKAROUND APPLIED: Renamed to sequential ID $NEXT_ID"
echo "Note: Eventually timestamps should be valid again, making this unnecessary"

# Step 6: Verify ordering is now correct
supabase migration list
```

**Why This Workaround?**
- **Problem**: Agents sometimes create migrations with broken timestamps
- **Impact**: Breaks ordering, prevents proper migration sequencing
- **Solution**: Fall back to sequential numbering (last ID + 1) until timestamps are clean
- **Goal**: Eventually enforce timestamp-based naming, remove workaround

**2.4. Write Defensive SQL**

Follow persona.md standards:

**Table Creation**:
```sql
-- Create table with appropriate columns
CREATE TABLE workspace_members (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  workspace_id uuid NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
  user_id uuid NOT NULL REFERENCES auth.users(id) ON DELETE CASCADE,
  role text NOT NULL CHECK (role IN ('owner', 'admin', 'member')),
  created_at timestamptz DEFAULT NOW(),
  updated_at timestamptz DEFAULT NOW()
);

-- Add indexes for foreign keys and frequently queried columns
CREATE INDEX idx_workspace_members_workspace_id ON workspace_members(workspace_id);
CREATE INDEX idx_workspace_members_user_id ON workspace_members(user_id);

-- Enable RLS (MANDATORY)
ALTER TABLE workspace_members ENABLE ROW LEVEL SECURITY;

-- Create RLS policies
CREATE POLICY "Users can view their own workspace memberships"
  ON workspace_members FOR SELECT
  USING ((SELECT auth.uid()) = user_id);

CREATE POLICY "Workspace owners can manage members"
  ON workspace_members FOR ALL
  USING (
    EXISTS (
      SELECT 1 FROM workspace_members wm
      WHERE wm.workspace_id = workspace_members.workspace_id
        AND wm.user_id = (SELECT auth.uid())
        AND wm.role = 'owner'
    )
  );
```

**RPC Function Creation**:
```sql
-- Use defensive patterns from persona.md
CREATE OR REPLACE FUNCTION public.get_workspace_members(p_workspace_id uuid)
RETURNS TABLE(
  id uuid,
  user_id uuid,
  email text,
  role text,
  created_at timestamptz
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
BEGIN
  -- Verify user has access to workspace
  IF NOT EXISTS (
    SELECT 1 FROM workspace_members wm
    WHERE wm.workspace_id = p_workspace_id
      AND wm.user_id = auth.uid()
  ) THEN
    RAISE EXCEPTION 'Access denied';
  END IF;

  RETURN QUERY
  SELECT
    wm.id,
    wm.user_id,
    u.email,
    wm.role,
    wm.created_at
  FROM workspace_members wm
  INNER JOIN auth.users u ON u.id = wm.user_id
  WHERE wm.workspace_id = p_workspace_id
  ORDER BY wm.created_at;
END;
$$;
```

**2.5. Migration Checklist**

Before finalizing, verify:
- [ ] All new tables have RLS enabled
- [ ] All RLS policies are appropriate and complete
- [ ] All RLS policies use `(SELECT auth.uid())` form, not direct `auth.uid()` call
- [ ] All RPC parameters use `p_` prefix
- [ ] All local variables use `v_` prefix
- [ ] ALL public schema functions have `SET search_path = public` (not just SECURITY DEFINER)
- [ ] Extensions installed `WITH SCHEMA extensions` (not `public`)
- [ ] All foreign keys have indexes
- [ ] All frequently queried columns have indexes
- [ ] All functions use `RETURNS TABLE(...)` not `SETOF record`
- [ ] JSON handling uses COALESCE for defensive programming
- [ ] No hardcoded user IDs or workspace IDs

**2.6. Final Ordering Verification**

```bash
# Verify migration is properly ordered
supabase migration list | tail -5

# Confirm new migration is last in sequence
# If using workaround (sequential ID), ensure it's last valid ID + 1
```

---

### Phase 3: User Review & Manual Application

**3.1. Present Migration to User**

```markdown
**STOP - DO NOT APPLY MIGRATION AUTOMATICALLY**

I've drafted a migration for you to review and apply manually:

**File**: `supabase/migrations/{{timestamp}}_{{name}}.sql`

**Changes**:
- [List what the migration does]
- [Tables created/modified]
- [Functions added]
- [RLS policies defined]

**Review Checklist**:
- [ ] Migration file exists and is readable
- [ ] SQL syntax is correct
- [ ] RLS policies are appropriate for your security model
- [ ] Indexes are on the right columns
- [ ] Migration is sequentially ordered (timestamp is latest)

**To Apply (MANUAL ONLY)**:
```bash
# Review the migration first
cat supabase/migrations/{{timestamp}}_{{name}}.sql

# Apply to your Supabase project
supabase migration up

# Verify application succeeded
supabase migration list
```

**After successful application**:
- Confirm the migration applied successfully
- Then I'll close this task
```

**3.2. Wait for User Confirmation**

**DO NOT PROCEED** until user confirms:
- ✅ Migration reviewed and looks correct
- ✅ Migration applied successfully via `supabase migration up`
- ✅ No errors during application

---

### Phase 4: Documentation & Closure

**4.1. Update Task Notes**
```bash
bd update {{task_id}} --notes="Drafted migration {{timestamp}}_{{name}}.sql for {{description}}. Key changes: {{summary_of_changes}}. User applied migration manually via CLI. Migration applied successfully."
```

**4.2. Update Design Notes**
```bash
bd update {{task_id}} --design="Database schema changes: {{tables_modified}}, {{functions_added}}, {{rls_policies_created}}. Migration file: supabase/migrations/{{timestamp}}_{{name}}.sql. Applied via supabase migration up."
```

**4.3. Commit Migration File**
```bash
git add supabase/migrations/{{timestamp}}_{{name}}.sql
git commit -m "feat(db): {{task_title}}

{{description_of_schema_changes}}

Migration: {{timestamp}}_{{name}}.sql
Applied: {{date}} via supabase migration up

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

**4.4. Close Task**
```bash
bd close {{task_id}} --reason="Migration drafted and successfully applied by user. Schema changes: {{summary}}. RLS enabled, defensive patterns followed, migration sequentially ordered."
```

---

## MEASUREMENTS

### Process Metrics (from ticket bp6-p41v.25)
- **Migrations requiring manual fix-up**: 0% (target: zero)
- **Accidental push or write attempts**: 0 (target: zero)
- **Migration ordering errors**: 0 (target: zero)

### Quality Metrics
- **RLS enabled**: 100% of new tables
- **Defensive patterns**: p_ prefix 100%, v_ prefix 100%, SECURITY DEFINER with search_path 100%
- **Index coverage**: 100% of foreign keys and frequently queried columns

### Outcome Metrics
- **User approval rate**: % of migrations accepted without revision
- **Migration failures**: 0 (target: zero application errors)

---

## OUTPUTS

- **Migration file**: `supabase/migrations/{{timestamp}}_{{name}}.sql`
- **User instructions**: How to review and apply migration
- **Updated task**: Notes and design populated with migration details
- **Git commit**: Migration file committed to version control
- **Closed task**: Status = closed with migration application confirmation

---

## EXIT CRITERIA

- [ ] Migration file created in correct sequential order
- [ ] SQL follows all defensive patterns (p_ prefix, v_ prefix, RLS, SECURITY DEFINER)
- [ ] Migration presented to user with review instructions
- [ ] User manually applied migration via `supabase migration up`
- [ ] User confirmed successful application
- [ ] NO automated pushes or writes attempted (0 violations)
- [ ] NO local DB started (0 violations)
- [ ] Task updated with migration details
- [ ] Migration committed to git
- [ ] Task closed

---

## CRITICAL MISTAKES TO AVOID

### ❌ Mistake #1: Auto-Applying Migration

**WRONG**:
```bash
supabase migration up  # AI applies migration automatically
```

**CORRECT**:
```markdown
Present migration to user and instruct them to run:
`supabase migration up`

Then wait for their confirmation before closing task.
```

**Why**: AI should NEVER apply database migrations. User must review and apply manually for safety.

---

### ❌ Mistake #2: Using Local DB

**WRONG**:
```bash
supabase start  # Starts local database
supabase gen types typescript --local  # Requires local DB
```

**CORRECT**:
```bash
# Use MCP tools to read schema (read-only)
# OR ask user to run: supabase gen types typescript (without --local)
```

**Why**: Ticket bp6-p41v.25 explicitly prohibits starting local DB sessions. Read-only MCP access only.

---

### ❌ Mistake #3: Manual Timestamp Creation (Wrong Approach)

**WRONG**:
```bash
# Manually calculating timestamps - DON'T DO THIS
date -u +"%Y%m%d%H%M%S"
touch supabase/migrations/20260218143022_new_feature.sql
```

**CORRECT**:
```bash
# Use official Supabase CLI
supabase migration new new_feature

# Then validate ordering
supabase migration list

# If out of order, apply workaround (rename to sequential ID)
```

**Why**:
- Supabase CLI handles timestamps correctly by convention
- Manual timestamp calculation often causes ordering issues
- `supabase migration list` is the authoritative source for ordering
- Workaround (sequential numbering) fixes ordering until timestamps are clean

---

### ❌ Mistake #4: Editing Existing Migration

**WRONG**:
```bash
# Modify existing migration file
vim supabase/migrations/20260215120000_add_users.sql
# Add new columns to existing CREATE TABLE statement
```

**CORRECT**:
```bash
# Create NEW migration for schema changes
touch supabase/migrations/$(date -u +"%Y%m%d%H%M%S")_add_user_columns.sql
# Write ALTER TABLE statement in new migration
```

**Why**: Migrations are immutable once created. Changes require new migrations, not edits to existing ones.

---

### ❌ Mistake #6: Missing search_path on Non-SECURITY DEFINER Functions

**WRONG**:
```sql
-- Trigger function without SET search_path — Supabase will flag this
CREATE OR REPLACE FUNCTION public.set_updated_at()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$;
```

**CORRECT**:
```sql
CREATE OR REPLACE FUNCTION public.set_updated_at()
RETURNS trigger
LANGUAGE plpgsql
SET search_path = public
AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$;
```

**Why**: ALL public schema functions require `SET search_path = public`, not just SECURITY DEFINER. Supabase flags any function with a mutable search_path regardless of security mode.

---

### ❌ Mistake #7: Installing Extension in public Schema

**WRONG**:
```sql
CREATE EXTENSION IF NOT EXISTS pg_net;  -- defaults to public schema
```

**CORRECT**:
```sql
CREATE EXTENSION IF NOT EXISTS pg_net WITH SCHEMA extensions;
```

**Why**: Extensions in `public` expose their functions to all authenticated users. Use the dedicated `extensions` schema.

---

### ❌ Mistake #8: Direct auth.uid() Call in RLS Policy

**WRONG**:
```sql
CREATE POLICY "Users can view own data"
  ON my_table FOR SELECT
  USING (auth.uid() = user_id);  -- re-evaluated per row
```

**CORRECT**:
```sql
CREATE POLICY "Users can view own data"
  ON my_table FOR SELECT
  USING ((SELECT auth.uid()) = user_id);  -- evaluated once as initplan
```

**Why**: `auth.uid()` is a volatile function. Without the subquery wrapper, PostgreSQL re-evaluates it for every row scanned, causing severe performance degradation at scale.

---

### ❌ Mistake #5: Missing RLS

**WRONG**:
```sql
CREATE TABLE sensitive_data (
  id uuid PRIMARY KEY,
  secret text
);
-- No RLS enabled - SECURITY VULNERABILITY
```

**CORRECT**:
```sql
CREATE TABLE sensitive_data (
  id uuid PRIMARY KEY,
  secret text
);

ALTER TABLE sensitive_data ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Users can only see their own data"
  ON sensitive_data FOR SELECT
  USING (auth.uid() = user_id);
```

**Why**: ALL tables MUST have RLS enabled. No exceptions per persona.md standards.
