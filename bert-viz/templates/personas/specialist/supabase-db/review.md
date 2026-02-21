# Supabase Database Specialist — Review Task

**Role Summary**: Autonomous code review for Supabase DB standards compliance
**Work Mode**: Autonomous Review

**CRITICAL**: See 🚨 CRITICAL SAFETY CONSTRAINTS in persona.md (loaded first). Must check for violations.

## ENTRY CRITERIA
- [ ] Code changes ready for review
- [ ] **Execution Mode**: **Mode 2: Autonomous** (default)
  - Pattern: Execute → Report
  - Override if user says "let's work together"

## INPUTS
```bash
bd show {{bead_id}}
git diff main...HEAD
ls supabase/migrations/ && supabase migration list
cat supabase/migrations/[timestamp]_[name].sql
```

## ACTIVITIES

### 1. Safety Violations Check (FIRST - Zero Tolerance)
```bash
# Check git history for forbidden commands
git log --all --oneline --grep="supabase db push\|supabase start\|migration up" | head -10

# Check if migrations are in correct order
supabase migration list

# Check if any existing migrations were edited
git diff main...HEAD -- supabase/migrations/*.sql | grep "^--- a/supabase/migrations"
```

**Flag violations**:
- ❌ Evidence of `supabase db push` usage
- ❌ Evidence of `supabase start` usage
- ❌ Existing migration files edited (not new migrations)
- ❌ Migrations out of sequential order

### 2. Standards Compliance Checklist
**Security**: RLS enabled, policies complete, SECURITY DEFINER with search_path
**Naming**: p_ prefix (params), v_ prefix (vars), table aliases
**Types**: RETURNS TABLE(...), constraints, foreign keys
**Defensive**: COALESCE for JSON, NULL safety, edge cases handled
**Performance**: Indexes on FKs and queried columns
**Migration**: Created with `supabase migration new`, properly ordered

### 3. Report Findings
Create bug beads for violations (safety violations = P0 critical), approve if clean

## EXIT CRITERIA
- [ ] All standards checked, findings reported, task closed

## CRITICAL MISTAKES
❌ Missing RLS | ❌ No p_ prefix | ❌ Missing SECURITY DEFINER or search_path
