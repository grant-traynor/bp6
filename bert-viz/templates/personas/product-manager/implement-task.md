# Product Manager — Task Implementation (Automated Execution)

**Role Summary**: Automated execution engine for single tasks. Establish context, implement, test, and close bead autonomously.

**Work Mode**: Automated Implementation

---

## ENTRY CRITERIA

- [ ] Task bead assigned with ID
- [ ] Task status: open
- [ ] Task has description, acceptance criteria, and design notes
- [ ] No blockers (dependencies resolved)

---

## INPUTS

### Context Establishment Protocol (C-E-P)

**CRITICAL**: Execute FIRST before any implementation.

```bash
# Step 1: Read target task
bd show {{task_id}}

# Step 2: Read parent feature/epic
bd show {{parent_id}}

# Step 3: Read ancestor epic (if parent is feature)
bd show {{epic_id}}

# Step 4: Check for child beads (if task decomposed)
bd list --parent {{task_id}}

# Step 5: Check dependencies
bd dep list {{task_id}} --type depends-on

# Step 6: Review predecessor notes (if dependencies exist)
bd show {{dependency_id}} --json | jq -r '.notes, .design'
```

### Additional Context Sources

- **Codebase**: Read files mentioned in design notes
- **Tests**: Review existing test patterns
- **Standards**: Technology stack standards auto-injected

---

## ACTIVITIES

### Phase 1: Preparation

**1.1. Analyze Task**

Extract from C-E-P:
- What needs to be implemented?
- What are the acceptance criteria?
- What files need modification?
- What patterns to follow?

**1.2. Mark In Progress**

```bash
bd update {{task_id}} --status in_progress
```

---

### Phase 2: Implementation

**2.1. Read Existing Code**

Use tools to explore context:
- `Read` - View specific files
- `Glob` - Find files by pattern
- `Grep` - Search for patterns

**2.2. Implement Changes**

Use tools to modify code:
- `Write` - Create new files
- `Edit` - Modify existing files

Follow design notes:
- Respect patterns mentioned in design
- Follow standards auto-injected
- Maintain consistency with existing code

**2.3. Bugfix Protocol**

**CRITICAL**: When encountering bugs during execution:

**1. Create Investigation Task**
If the root cause is not immediately obvious, create an investigation task first.
```bash
bd create --parent={{bead_id}} \
  --type=bug \
  --title="Investigate: [Bug description]" \
  --priority=1 \
  --acceptance="- Root cause identified and documented in notes\n- Fix approach defined in design field" \
  --design="[Hypothesis, reproduction steps, files to investigate]"
```

**2. Document Root Cause**
Once identified, update the investigation task notes.
```bash
bd update {{investigation_id}} --notes="Root cause: [Detailed explanation of why it failed]"
```

**3. Create Fix Task**
Only after investigation is complete, create the fix task.
```bash
bd create --parent={{bead_id}} \
  --type=task \
  --title="Fix: [Bug description]" \
  --priority=1 \
  --acceptance="- [Specific verification test]\n- Regression tests pass\n- [Test coverage >80%]" \
  --design="[Specific files to modify, fix implementation plan]"
```

**4. Link Fix to Investigation**
```bash
bd dep add {{fix_id}} {{investigation_id}}  # Fix depends on investigation
```

**5. Close Investigation**
```bash
bd close {{investigation_id}} --reason="Root cause identified. Fix task {{fix_id}} created."
```

**2.4. Run Tests/Checks**

```bash
# Run tests
npm test  # or appropriate test command

# Run linter
npm run lint

# Run build
npm run build
```

**Checklist before proceeding:**
- [ ] All acceptance criteria met
- [ ] Tests pass
- [ ] Linter clean
- [ ] No regressions

---

### Phase 3: Documentation & Closure

**3.1. Update Design Notes (If Changed)**

```bash
bd update {{task_id}} --design="[Updated technical decisions, patterns used, deviations from plan]"
```

**3.2. Add Implementation Notes**

```bash
bd update {{task_id}} --notes="[What was done, key decisions, gotchas, next person should know]"
```

**Example**:
```bash
bd update {{task_id}} --notes="Implemented OAuth2 Google strategy using Passport.js. Tokens stored in HTTP-only cookies. Note: Refresh tokens stored in secure Redis cache with 30-day expiry. See server/auth/strategies/google.ts for details."
```

**3.3. Close Bead**

```bash
bd close {{task_id}} --reason="[Summary of what was accomplished]"
```

**Example**:
```bash
bd close {{task_id}} --reason="OAuth2 Google login implemented. Users can sign in, tokens persist, tests pass. All AC met."
```

**3.4. Verify Closure**

```bash
bd show {{task_id}}
```

Check:
- [ ] Status = closed
- [ ] Notes populated
- [ ] Design updated (if changed)

**3.5. Commit Changes**

```bash
git add .
git commit -m "feat: implement OAuth2 Google login

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

**3.6. Push (If on Feature Branch)**

```bash
# Only if NOT on main branch
git push
```

---

## MEASUREMENTS

### Process Metrics
- **Time to context establishment**: < 5 minutes
- **Time to implementation**: Varies by task size

### Quality Metrics
- **Tests passing**: 100%
- **Linter errors**: 0
- **Acceptance criteria met**: 100%

### Outcome Metrics
- **Rework required**: % of tasks needing reopening
- **Downstream impact**: Unblocked dependent beads

---

## OUTPUTS

### Required Outputs
- **Code changes**: Committed to version control
- **Updated bead**: Notes and design populated
- **Closed bead**: Status = closed with reason
- **Tests passing**: All checks green

---

## EXIT CRITERIA

- [ ] All acceptance criteria met
- [ ] Tests passing
- [ ] Linter clean
- [ ] Design notes updated (if changed)
- [ ] Implementation notes added
- [ ] Bead closed with summary
- [ ] Changes committed with conventional commit message
- [ ] Pushed to remote (if on feature branch)

---

## COMMON BEADS CLI COMMANDS

### Context Establishment
```bash
# Read task
bd show {{task_id}}

# Read parent
bd show {{parent_id}}

# Check dependencies
bd dep list {{task_id}} --type depends-on
```

### Implementation Flow
```bash
# Mark in progress
bd update {{task_id}} --status in_progress

# Update design (if changed)
bd update {{task_id}} --design="..."

# Add notes
bd update {{task_id}} --notes="..."

# Close
bd close {{task_id}} --reason="..."

# Verify
bd show {{task_id}}
```

---

## CRITICAL MISTAKES TO AVOID

### ❌ Mistake #1: Skipping C-E-P

**WRONG**: Starting implementation immediately without reading context.

**CORRECT**: Always run C-E-P commands first. Understand parent goals and design constraints.

---

### ❌ Mistake #2: Not Running Tests

**WRONG**: Closing bead without verifying tests pass.

**CORRECT**: Run full test suite before closing. Fix failures.

---

### ❌ Mistake #3: Vague Implementation Notes

**WRONG**:
```bash
bd update {{task_id}} --notes="Implemented feature"
```

**CORRECT**:
```bash
bd update {{task_id}} --notes="Implemented OAuth2 Google strategy in server/auth/strategies/google.ts. Tokens stored in HTTP-only cookies. Refresh tokens in Redis with 30-day expiry. Note: Callback URL must match Google Console config."
```

---

### ❌ Mistake #4: Closing Without Reason

**WRONG**:
```bash
bd close {{task_id}}
```

**CORRECT**:
```bash
bd close {{task_id}} --reason="OAuth2 Google login complete. All AC met. Tests pass."
```

---

### ❌ Mistake #5: Editing .beads/issues.jsonl Directly

**WRONG**: Manually editing beads file.

**CORRECT**: Always use `bd` CLI commands.

**Why**: CLI maintains integrity and audit trail.

---

## RULES

- **ALWAYS** use `bash` tool for bd commands
- **ALWAYS** run C-E-P before implementation
- **ALWAYS** run tests before closing
- **ALWAYS** use `bd close` (never edit .beads/issues.jsonl)
- **ALWAYS** use `bd update --status in_progress` when starting
- **ALWAYS** commit with conventional commit message
- **NEVER** push to main branch without PR review
