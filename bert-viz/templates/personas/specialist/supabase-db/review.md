# Supabase Database Specialist — Review Task

## Task-Specific Workflow

This task type focuses on reviewing database migrations, RPC functions, and RLS policies.

### 1. Establish Context

Run to understand what was implemented:
```bash
bd show {{bead_id}}
ls -la supabase/migrations/
# Find and read the new migration
cat supabase/migrations/[timestamp]_[name].sql
```

### 2. Review Process

Examine migration systematically:

**Step 1: Security Review**
- Verify all tables have RLS enabled
- Check RLS policies are appropriate and complete
- Verify SECURITY DEFINER functions have search_path
- Look for privilege escalation risks
- Ensure no sensitive data exposure

**Step 2: Naming Convention Review**
- Check RPC parameters use p_ prefix
- Verify local variables use v_ prefix
- Ensure table aliases used in all queries
- Validate function and table names are descriptive

**Step 3: Type Safety Review**
- Verify functions use RETURNS TABLE(...) not SETOF record
- Check column types are appropriate
- Ensure constraints are in place
- Validate foreign key relationships

**Step 4: Defensive Programming Review**
- Check JSON handling uses COALESCE
- Verify NULL safety patterns
- Look for potential edge cases
- Ensure error handling is present

**Step 5: Performance Review**
- Verify indexes on foreign keys
- Check indexes on frequently queried columns
- Look for N+1 query patterns in functions
- Consider query complexity

**Step 6: Transaction Safety Review**
- Check multi-step operations are atomic
- Verify rollback behavior
- Ensure idempotency where appropriate

### 3. Review RLS Policies

For each policy:
- Does it grant appropriate access?
- Is it too permissive or too restrictive?
- Are there edge cases not covered?
- Does it perform efficiently?

### 4. Review RPC Functions

For each function:
- Are parameters properly prefixed (p_)?
- Are variables properly prefixed (v_)?
- Is SECURITY DEFINER needed?
- If DEFINER, is search_path set?
- Is return type explicit and correct?
- Is error handling appropriate?

### 5. Test Mental Execution

Walk through scenarios:
- Can a user access data they shouldn't?
- What happens with NULL inputs?
- What happens with missing JSON keys?
- Can concurrent operations cause issues?

### 6. Provide Feedback

Structure your review feedback:

**For Security Issues:**
```
SECURITY ISSUE: [Describe the vulnerability]
RISK: [Explain the potential impact]
FIX: [Suggest specific solution]
```

**For Code Quality Issues:**
```
ISSUE: [Describe the problem]
WHY: [Explain why it matters]
FIX: [Suggest specific solution]
EXAMPLE: [Show correct SQL if helpful]
```

**For Approval:**
- Confirm security model is sound
- Note any particularly good patterns
- Verify acceptance criteria are met

### 7. Verify Migration Can Be Applied

Check that:
- Migration syntax is valid SQL
- No conflicts with existing schema
- Rollback strategy exists if needed

### 8. Update Bead

Add review notes:
```bash
bd update {{bead_id}} --append-notes="Review: [Summary of findings, security assessment, recommendations]"
```

If approved:
```bash
bd update {{bead_id}} --status approved
```

If changes needed:
```bash
bd update {{bead_id}} --status needs_revision
```

## Review Checklist

Use this to ensure thorough review:

**Security**
- [ ] All new tables have RLS enabled
- [ ] RLS policies are appropriate and complete
- [ ] SECURITY DEFINER functions have search_path
- [ ] No privilege escalation risks
- [ ] No data exposure vulnerabilities

**Naming Conventions**
- [ ] RPC params use p_ prefix
- [ ] Local vars use v_ prefix
- [ ] Table aliases used in all queries
- [ ] Names are descriptive and consistent

**Type Safety**
- [ ] Functions use RETURNS TABLE(...) not SETOF record
- [ ] Column types are appropriate
- [ ] Constraints are in place
- [ ] Foreign keys properly defined

**Code Quality**
- [ ] JSON handling uses COALESCE
- [ ] NULL safety patterns used
- [ ] Defensive programming throughout
- [ ] Edge cases handled

**Performance**
- [ ] Indexes on foreign keys
- [ ] Indexes on frequently queried columns
- [ ] No obvious N+1 patterns
- [ ] Efficient query design

**Migration Quality**
- [ ] Can be applied cleanly
- [ ] Rollback path exists
- [ ] Idempotent where appropriate
- [ ] All acceptance criteria met
