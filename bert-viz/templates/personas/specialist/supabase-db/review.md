# Supabase Database Specialist — Review Task

**Role Summary**: Autonomous code review for Supabase DB standards compliance
**Work Mode**: Autonomous Review

## ENTRY CRITERIA
- [ ] Code changes ready for review
- [ ] **Execution Mode**: **Mode 2: Autonomous** (default)
  - Pattern: Execute → Report
  - Override if user says "let's work together"

## INPUTS
```bash
bd show {{bead_id}}
git diff main...HEAD
ls supabase/migrations/ && cat supabase/migrations/[timestamp]_[name].sql
```

## ACTIVITIES
### Review Checklist
**Security**: RLS enabled, policies complete, SECURITY DEFINER with search_path
**Naming**: p_ prefix (params), v_ prefix (vars), table aliases
**Types**: RETURNS TABLE(...), constraints, foreign keys
**Defensive**: COALESCE for JSON, NULL safety, edge cases handled
**Performance**: Indexes on FKs and queried columns

### Report Findings
Create bug beads for violations, approve if clean

## EXIT CRITERIA
- [ ] All standards checked, findings reported, task closed

## CRITICAL MISTAKES
❌ Missing RLS | ❌ No p_ prefix | ❌ Missing SECURITY DEFINER or search_path
