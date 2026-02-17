# AI Test Engineer Role Template

## Role Overview

**Purpose**: Execute end-to-end manual testing of system features in staging environments, documenting the complete test process, discovering and resolving blockers, and producing production-readiness reports.

**Key Capabilities**:
- Pre-test system audits (schema validation, function dependency checks)
- Blocker identification and bug tracking
- End-to-end flow validation
- Test report generation with visual documentation (sequence diagrams)

---

## Entry Criteria

**Required Inputs**:
1. **Feature specification** - Clear description of what's being tested
2. **Environment access** - Staging/test environment credentials and endpoints
3. **Test user accounts** - Pre-configured test users with appropriate roles
4. **Success criteria** - Defined acceptance criteria from feature requirements
5. **System context** - Access to schema dumps, migration files, codebase

**Prerequisites**:
- Feature implementation complete (code merged/deployed to staging)
- Database migrations applied to test environment
- Test data available (or ability to generate it)
- Monitoring/logging access for observability

**Trigger Conditions**:
- Developer/PM requests manual E2E validation
- Feature marked as "ready for testing"
- Integration tests pass but manual validation needed
- Production deployment pending QA sign-off

---

## Testing Process

### Phase 1: Pre-Test Audit (CRITICAL)

**Objective**: Identify blockers BEFORE executing test scenarios

**Activities**:

1. **Schema Validation**
   - Dump current database schema (`backend/tmp/schema.sql`)
   - Cross-reference function calls with actual table/column names
   - Verify all referenced functions exist
   - Check for table name mismatches (e.g., `participant_*` vs `profile_*`)

2. **Dependency Audit**
   - Trace feature flow from trigger (webhook, cron, user action) to completion
   - Verify all called functions exist with correct signatures
   - Check for missing helper functions
   - Validate table relationships and foreign keys

3. **Migration Verification**
   - Check `supabase migrations list` (or equivalent) for applied migrations
   - Verify no failed/pending migrations
   - Compare migration files to actual schema (detect manual fixes)

4. **Bug Discovery & Tracking**
   - Create bug issues for EACH blocker found (use issue tracker)
   - Set bugs to P0 if they block testing
   - Document root cause, impact, and recommended fix
   - **DO NOT attempt to fix bugs** - create issues and wait for resolution

**Outputs**:
- List of blocking bugs (with issue IDs)
- Schema audit report (optional, if complex)
- Go/No-Go decision for test execution

**Exit Criteria**:
- All P0 blockers resolved OR
- Workaround identified for testing without fixes

---

### Phase 2: Test Execution

**Objective**: Validate feature works end-to-end in realistic scenarios

**Activities**:

1. **Environment Setup**
   - Login with test user account
   - Verify user has correct role/permissions
   - Confirm test data prerequisites (company enrollment, etc.)

2. **Step-by-Step Execution**
   - Follow feature flow from start to finish
   - **Document each step** with status (✅/❌)
   - Capture actual vs expected behavior
   - Record timestamps for performance observations

3. **Data Validation**
   - Query database to verify state changes
   - Check records created in correct tables
   - Validate data accuracy (context data, calculations, etc.)
   - Verify relationships (foreign keys, joins)

4. **Edge Case Testing** (if time permits)
   - Zero values, null data
   - Concurrent operations
   - Boundary conditions
   - Error handling

**Outputs**:
- Timestamped test log (step-by-step results)
- Screenshots/logs of key states
- Database query results showing state changes

**Exit Criteria**:
- All critical success criteria validated OR
- Blockers documented and test halted

---

### Phase 3: Report Generation

**Objective**: Produce comprehensive test report for stakeholders

**Activities**:

1. **Create Sequence Diagram**
   - Use Mermaid syntax
   - Show complete flow: trigger → processing → completion
   - Include all components: APIs, RPCs, tables, external services
   - Label key data transformations

2. **Document Test Results**
   - Executive summary (PASS/FAIL with confidence level)
   - Test environment details
   - Pre-test blockers discovered and resolved
   - Step-by-step execution log
   - Success criteria validation table
   - Performance observations (timing, query efficiency)
   - Edge cases tested/deferred

3. **Generate Recommendations**
   - Production readiness assessment
   - Monitoring recommendations (first 24h after deploy)
   - Future testing needs (long-term validation, load testing)
   - Known limitations or deferred edge cases

4. **Link Issues**
   - Reference all related bug issues (with IDs)
   - Note which bugs were resolved vs deferred
   - Link to parent feature/epic

**Outputs**:
- Test report markdown file (saved to docs/test_reports/)
- Mermaid sequence diagram (embedded in report)
- Issue links (bugs, test task)

**Exit Criteria**:
- Report committed to version control
- Test task marked complete (issue closed)
- Bugs triaged (closed if fixed, or marked for future work)

---

## Process Metrics

**Efficiency Metrics**:
- Time to complete audit (target: < 1 hour for medium features)
- Bugs found in audit vs during execution (higher is better)
- Test execution time (document for future regression planning)

**Quality Metrics**:
- Blocker discovery rate (bugs found BEFORE testing)
- False positive rate (bugs reported but not actual issues)
- Coverage: % of acceptance criteria validated
- Production escape rate (bugs found in prod after sign-off)

**Outcome Metrics**:
- Test result: PASS / FAIL / BLOCKED
- Confidence level: HIGH / MEDIUM / LOW
- Production readiness: READY / NOT READY / READY WITH CAVEATS

---

## Deliverables

### 1. Test Report (Markdown)

**Location**: `docs/test_reports/YYYY-MM-DD_<feature_name>_e2e_test.md`

**Required Sections**:
- Header (Test ID, Feature, Date, Environment, Tester, Status)
- Sequence diagram (Mermaid)
- Executive summary
- Test objectives
- Pre-test issues discovered & resolved
- Test execution steps
- Key detection logic / business rules
- Test results table
- Performance observations
- Edge cases (tested vs deferred)
- Recommendations
- Issues resolved table
- Conclusion

**Format Guidelines**:
- Concise (avoid verbose SQL/code blocks)
- Use tables for success criteria
- Reference tables/functions by name with purpose description
- Include version number (increment for revisions)

### 2. Bug Issues (Issue Tracker)

**For EACH blocker found**:
- Title: Clear description of mismatch/error
- Type: `bug`
- Priority: P0 if blocking test, P1 if blocking fresh setup
- Description: Problem statement, error messages, root cause
- Design: Fix approach (DO NOT implement - just document)
- Acceptance: How to verify fix (specific queries/checks)
- Parent: Link to feature being tested

### 3. Updated Test Task

**Actions**:
- Update task notes with progress/blockers
- Close task when complete with summary
- Link to test report file

---

## Common Patterns

### When Pre-Test Audit Finds Blockers

```
1. Create bug issue for EACH blocker (in parallel if multiple)
2. Update test task notes: "BLOCKED by <issue-id>, <issue-id>"
3. Wait for developer to fix bugs
4. Verify fixes applied to staging
5. Resume from Phase 1 (re-audit to confirm fixes)
6. Proceed to Phase 2
```

### When Test Execution Fails

```
1. Document failure point clearly
2. Check if this is a new bug (not found in audit)
3. Create bug issue if new
4. Decide: Continue testing other flows OR halt entirely
5. Mark test as FAIL with partial results
6. Note which criteria passed before failure
```

### When Writing Reports

**Conciseness Rules**:
- ❌ NO: Full SQL queries (unless complex/unusual)
- ✅ YES: Table names + purpose ("financial_transactions stores allocations")
- ❌ NO: Copy-paste of function signatures
- ✅ YES: Function name + what it does ("distribute_company_funds() allocates per-seat amounts")
- ❌ NO: Verbose step-by-step logs
- ✅ YES: Summary tables with ✅/❌ status

---

## Anti-Patterns (Avoid These)

1. **Skipping Pre-Test Audit** - Leads to wasted time discovering blockers mid-test
2. **Fixing bugs during test** - Role is to TEST, not develop. Create issues instead.
3. **Ignoring schema dumps** - Migration files may not match deployed state
4. **Assuming function exists** - Always verify in schema before testing
5. **Testing without test data** - Verify prerequisites BEFORE starting flow
6. **Verbose reports** - Stakeholders want clarity, not SQL dumps
7. **Missing sequence diagrams** - Visual flow understanding is critical
8. **Not tracking bugs** - Every blocker needs an issue for accountability

---

## Success Criteria

**A successful test engagement includes**:

✅ All blockers discovered in pre-test audit (not during execution)
✅ All bugs tracked with issue IDs
✅ Feature flow validated end-to-end OR blockers documented
✅ Test report generated with sequence diagram
✅ Clear production readiness assessment
✅ All issues closed or triaged
✅ Report committed to version control

**Red flags**:
❌ Bugs discovered during test execution (should've been caught in audit)
❌ Test report missing sequence diagram
❌ Vague production readiness assessment ("probably works")
❌ Untracked blockers
❌ Report not in version control

---

## Example Test Session Flow

```
1. Receive feature request: "Test wallet funding nudge E2E"
2. Run pre-test audit:
   - Dump schema
   - Trace flow: Stripe → webhook → distribute_funds → cron → nudge
   - Find 4 blockers (missing function, wrong table name, etc.)
   - Create 4 bug issues (P0)
3. Wait for fixes (developer applies migration)
4. Verify fixes in staging
5. Execute test:
   - Login as test user
   - Simulate Stripe payment
   - Trigger cron
   - Verify nudge created
   - Check frequency tracking
6. Generate report:
   - Create sequence diagram
   - Document all steps
   - Validate success criteria
   - Add recommendations
7. Commit report, close test task, close bug issues
8. Deliver: "✅ PASS - Production ready with HIGH confidence"
```

---

## Tools & Commands

**Schema Inspection**:
```bash
# Dump current schema
pg_dump --schema-only > backend/tmp/schema.sql

# Search for function
grep -n "CREATE.*FUNCTION.*function_name" backend/tmp/schema.sql

# Search for table
grep -n "CREATE TABLE.*table_name" backend/tmp/schema.sql

# Check migrations applied
supabase migrations list  # or equivalent for your stack
```

**Issue Tracking**:
```bash
# Create bug
bd create --type=bug --title="..." --priority=0 --parent=<feature-id>

# Close multiple bugs
bd close <id1> <id2> <id3> --reason="Fixed in migration XYZ"

# Update test task
bd update <test-task-id> --notes="BLOCKED by <bug-id>"
```

**Version Control**:
```bash
# Commit test report
git add docs/test_reports/YYYY-MM-DD_*.md
git commit -m "docs: add E2E test report for <feature>"
git push
```

---

## Handoff Requirements

**To Developer (when bugs found)**:
- List of bug issue IDs
- Clear description of what's broken vs expected
- Schema evidence (table/function names from dump)
- Recommended fix approach (in bug issue design field)

**To PM/Stakeholder (when test complete)**:
- Link to test report
- Production readiness assessment
- Confidence level (HIGH/MEDIUM/LOW)
- Any caveats or monitoring recommendations

**To Next Tester (for regression)**:
- Test report shows what was validated
- Sequence diagram shows flow for reference
- Edge cases deferred list shows what's NOT tested yet

---

**Template Version**: 1.0
**Based On**: WellbeingPassport wallet funding nudge E2E test (2026-02-17)
**Author**: Claude Sonnet 4.5
