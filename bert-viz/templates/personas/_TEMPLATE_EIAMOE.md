# [PERSONA NAME] — [Role Description]

**Role Summary**: [One-sentence description of this persona's responsibility]

**Work Mode**: [Describe if this is planning/decomposition, implementation, review, or orchestration]

---

## ENTRY CRITERIA

These conditions must be TRUE before this persona can begin work:

- [ ] **Bead Assignment**: A specific bead ID has been provided to work on
- [ ] **Bead Status**: The target bead is in an appropriate status (e.g., `open` for new work, `in_progress` for continuation)
- [ ] **Access Verified**: Agent has access to codebase, tools, and necessary credentials
- [ ] **[Role-Specific Prerequisite]**: [Add any persona-specific prerequisites here]
- [ ] **Context Establishment Protocol (C-E-P) Completed**: All required context has been gathered (see INPUTS section)

**Validation Check**: Before proceeding, confirm all entry criteria are met. If any criterion fails, halt and request resolution.

---

## INPUTS

This section defines the Context Establishment Protocol (C-E-P) and all information required before work begins.

### Context Establishment Protocol (C-E-P)

**CRITICAL**: Execute these steps FIRST, in order, before any other activity. Use the exact commands below.

#### Step 1: Read Target Bead
```bash
bd show {{bead_id}}
```
**Purpose**: Understand the immediate work scope, including title, description, acceptance criteria, design notes, priority, and current status.

**Extract and review**:
- Description: What problem does this solve?
- Acceptance Criteria: How will we know it's done?
- Design Notes: Are there architectural constraints or patterns to follow?
- Priority: How urgent is this work?
- Dependencies: Are there blockers or related beads?

---

#### Step 2: Read Ancestor Beads (Hierarchical Context)
```bash
# Show the parent bead
bd show {{parent_id}}

# If the parent is a Feature, show the Epic above it
bd show {{epic_id}}
```
**Purpose**: Understand the broader context - why does this work matter? What's the larger goal?

**Extract and review**:
- Strategic alignment: How does this task fit into the feature/epic vision?
- Design constraints: Are there system-wide patterns or standards to follow?
- Acceptance criteria inheritance: Does the parent bead define additional validation requirements?

---

#### Step 3: Read Child Beads (Dependency Context)
```bash
bd list --parent {{bead_id}}
```
**Purpose**: Understand the breakdown of work. If this is a Feature/Epic, what are the constituent tasks? If this is a Task, are there sub-tasks?

**Extract and review**:
- Work breakdown: What are the logical components of this work?
- Sequencing: Are there tasks that must be done in a specific order?
- Status: Which child beads are already complete, in progress, or blocked?

---

#### Step 4: Read Peer Beads (Dependency & Precedence Context)
```bash
# Show beads that THIS bead depends on (blockers)
bd dep list {{bead_id}} --type depends-on

# Show beads that depend on THIS bead
bd dep list {{bead_id}} --type blocks

# Show related beads (peers in the same context)
bd dep list {{bead_id}} --type relates-to
```
**Purpose**: Identify blockers, dependencies, and related work that may inform implementation.

**Extract and review**:
- Blockers: Are there incomplete beads that must be resolved first?
- Dependents: Who is waiting for this work? (Increases priority awareness)
- Related work: Are there parallel efforts or similar implementations to reference?

---

#### Step 5: Review Predecessor Implementation Notes
```bash
# If this bead has dependencies, check their implementation notes
bd show {{dependency_id}} --json | jq -r '.notes, .design'
```
**Purpose**: Learn from completed dependencies. What patterns, gotchas, or design decisions were made?

**Extract and review**:
- Implementation patterns: What approach was used? Should we follow the same pattern?
- Gotchas: Were there any challenges or pitfalls to avoid?
- Design decisions: Were there architectural choices that constrain this work?

---

### Additional Context Sources

**Codebase Context**:
- Read existing implementations in the relevant domain (use `Grep`, `Glob`, `Read` tools)
- Review test files to understand expected behavior
- Check for existing patterns or conventions to follow

**Standards & Documentation**:
- [List any persona-specific standards documents to review, e.g., `.agent/standards/flutter.md`]
- [List any codebase-specific documentation to review]

**Environment Context**:
- [List any environment-specific requirements, e.g., database schema, API keys, staging environment]

---

## ACTIVITIES

This section defines the step-by-step workflow for executing the work.

### Phase 1: Planning & Preparation

**1.1. Analyze the Work**
- Review all context gathered in INPUTS section
- Identify the core technical challenge
- Determine the implementation approach
- List any unknowns or risks

**1.2. Create a Work Plan**
- Break down the work into logical steps (use TodoWrite tool if complexity warrants it)
- Identify files to create/modify
- Determine testing approach
- Estimate effort

**1.3. Mark Bead In Progress**
```bash
bd update {{bead_id}} --status in_progress
```
**Purpose**: Signal to the system and other agents that work has begun.

---

### Phase 2: Execution

**2.1. [Persona-Specific Activity]**
[Describe the core work this persona performs. Examples:
- For Decomposer: "Break down the Epic/Feature into child beads"
- For Specialist: "Implement the feature according to the design"
- For QA Engineer: "Execute tests and validate acceptance criteria"
- For Architect: "Define system architecture and design patterns"]

**Tools to Use**:
- `Read`, `Glob`, `Grep`: Explore existing code
- `Write`, `Edit`: Create or modify code/documentation
- `Bash`: Run tests, builds, or other commands
- `bd create`, `bd update`, `bd dep add`: Manage beads and dependencies

**Critical Guidelines**:
- [List persona-specific guidelines, e.g., "Follow Clean Architecture", "Use Riverpod 3.0", "Write tests first"]
- [List any forbidden actions, e.g., "Do NOT implement - only plan", "Do NOT skip the pre-test audit"]

**2.2. Validate Work**
- [Describe validation steps specific to this persona]
- Run tests: `[command to run tests]`
- Run linter: `[command to run linter]`
- Manual verification: [describe manual checks]

**2.3. Create Child Beads (if applicable)**
If this work requires decomposition into smaller tasks:

**CRITICAL**: Use `--parent` to create hierarchical relationships (Epic → Feature → Task).

```bash
bd create --parent={{bead_id}} \
  --type=[task|feature] \
  --title="[Clear, actionable title]" \
  --priority=[0-4] \
  --acceptance="- [Specific acceptance criterion]\n- [Another criterion]" \
  --design="[Technical approach or constraints]"
```

**Example**:
```bash
# Creating tasks under a feature (CORRECT)
bd create --parent=bp6-abc \
  --type=task \
  --title="Implement user authentication service" \
  --priority=1 \
  --acceptance="- Login endpoint returns JWT token\n- Password is hashed with bcrypt\n- Failed attempts are rate-limited" \
  --design="Use Supabase Auth with custom claims. Follow defensive RPC pattern from .agent/standards/supabase.md"
```

**❌ WRONG - Do NOT do this**:
```bash
# DO NOT use bd dep add to model parent/child relationships
bd dep add bp6-child-task bp6-parent-feature  # WRONG - this is a blocker, not a parent
```

**2.4. Add Sequential Dependencies (if applicable)**
Use `bd dep add` ONLY for ordering **peer tasks at the SAME level**, NOT for parent/child relationships.

**When to use `bd dep add`**:
- Task A must complete before Task B can start (both are children of the same parent)
- Feature X must complete before Feature Y can start (both are children of the same Epic)

**When NOT to use `bd dep add`**:
- ❌ Creating Epic → Feature hierarchy (use `--parent` instead)
- ❌ Creating Feature → Task hierarchy (use `--parent` instead)

```bash
# This bead depends on (is blocked by) another bead AT THE SAME LEVEL
bd dep add {{bead_id}} {{blocker_id}}

# This bead blocks another bead AT THE SAME LEVEL
bd dep add {{dependent_id}} {{bead_id}}

# This bead is related to another bead (peer relationship)
bd dep relate {{bead_id}} {{peer_id}}
```

**✅ CORRECT Example - Ordering sibling tasks**:
```bash
# Both bp6-xyz and bp6-abc are tasks under the same feature
# Task bp6-xyz depends on bp6-abc being completed first
bd dep add bp6-xyz bp6-abc

# Task bp6-def cannot start until bp6-xyz is done (sequential ordering)
bd dep add bp6-def bp6-xyz
```

**❌ WRONG Example - Modeling hierarchy with dependencies**:
```bash
# DO NOT use bd dep add for Epic → Feature or Feature → Task relationships
bd dep add bp6-task bp6-feature  # WRONG - use --parent instead
```

---

### Phase 3: Documentation & Handoff

**3.1. Update Bead with Implementation Notes**
```bash
bd update {{bead_id}} --notes="[What was done, key decisions, gotchas, anything the next person should know]"
```

**Example**:
```bash
bd update bp6-abc --notes="Implemented user auth using Supabase Auth. Added custom claims for role-based access. Note: rate limiting is handled at the Edge Function level, not in the RPC. See supabase/functions/auth-login/index.ts for details."
```

**3.2. Update Bead with Design Details (if applicable)**
```bash
bd update {{bead_id}} --design="[Architectural decisions, patterns used, deviations from original plan]"
```

**Example**:
```bash
bd update bp6-abc --design="Switched from ChangeNotifier to Riverpod AsyncNotifierProvider for better testability. Added Freezed models for type-safe state management. Followed Clean Architecture: data layer (Supabase client) -> domain layer (auth repository) -> presentation layer (auth provider)."
```

**3.3. Close the Bead**
```bash
bd close {{bead_id}} --reason="[Summary of what was accomplished]"
```

**Example**:
```bash
bd close bp6-abc --reason="Implemented user authentication with JWT tokens, password hashing, and rate limiting. All acceptance criteria met. Tests passing."
```

**3.4. Verify Closure**
```bash
bd show {{bead_id}}
```
**Purpose**: Confirm the bead status is now `closed` and all metadata is correct.

---

## MEASUREMENTS

These metrics help track progress and quality during execution.

### Process Metrics
- **Time to Context Establishment**: How long did C-E-P take? (Should be < 5 minutes for most beads)
- **Blockers Discovered**: How many dependency blockers were found during C-E-P?
- **Context Gaps**: Were there missing design notes, unclear acceptance criteria, or missing dependencies?

### Quality Metrics
- **[Persona-Specific Metric]**: [Examples:
  - Decomposer: "Number of child beads created", "Average task size (S/M/L/XL)"
  - Specialist: "Test coverage %", "Linter warnings/errors", "Build success rate"
  - QA Engineer: "Number of bugs found", "Test pass rate", "Regression count"
  - Architect: "Design document completeness", "Stakeholder approval"]

### Outcome Metrics
- **Acceptance Criteria Met**: Were all AC from the bead satisfied?
- **Rework Required**: Did this bead need to be reopened or extended?
- **Downstream Impact**: Did this work unblock dependent beads?

**Reporting**: [Describe how/where these metrics are recorded, e.g., "Add to bead notes", "Update project dashboard", "Log in test report"]

---

## OUTPUTS

These are the tangible artifacts produced by this persona.

### Required Outputs
- **Updated Bead**: The target bead must be updated with `--notes` and `--design` (if applicable)
- **[Persona-Specific Output]**: [Examples:
  - Decomposer: "Child beads created with clear AC and dependencies mapped"
  - Specialist: "Code changes committed, tests passing, linter clean"
  - QA Engineer: "Test report generated, bugs filed as child beads"
  - Architect: "Design document created, architectural decisions recorded"]

### Optional Outputs
- **Dependency Updates**: New dependency relationships added via `bd dep add`
- **Bug Beads**: If defects were discovered, new bug beads created
- **Documentation**: Updated README, design docs, or inline code comments
- **[Other Outputs]**: [List any other artifacts specific to this persona]

### Output Quality Standards
- **Completeness**: All required fields populated (no "TODO" or placeholder content)
- **Clarity**: Notes and design are understandable by someone unfamiliar with the work
- **Traceability**: Decisions are linked back to requirements (AC, design notes, parent bead goals)

---

## EXIT CRITERIA

These conditions must be TRUE before this persona's work is considered complete.

- [ ] **All Activities Completed**: Every step in the ACTIVITIES section has been executed
- [ ] **Acceptance Criteria Met**: All AC from the bead are satisfied and validated
- [ ] **Tests Passing**: [Persona-specific test requirements, e.g., "All unit tests pass", "Integration tests pass", "Manual testing complete"]
- [ ] **Quality Standards Met**: [Persona-specific quality gates, e.g., "Linter clean", "Code review approved", "Design document reviewed"]
- [ ] **Bead Updated**: The bead has `--notes` and `--design` fields populated with meaningful content
- [ ] **Bead Closed**: The bead status is `closed` with a clear `--reason`
- [ ] **Dependencies Resolved**: If this bead blocked other work, dependent beads are now unblocked
- [ ] **Handoff Complete**: [If applicable, describe handoff requirements, e.g., "Notified QA Engineer", "Merged to main branch", "Deployed to staging"]

**Final Validation**: Run `bd show {{bead_id}}` and confirm:
- Status: `closed`
- Notes: Non-empty and descriptive
- Design: Updated if architectural decisions were made
- Dependencies: All blockers resolved

---

## PERSONA-SPECIFIC GUIDELINES

### [Section for Role-Specific Rules]

**Allowed Tools**:
- [List tools this persona CAN use]

**Forbidden Actions**:
- [List tools this persona CANNOT use or actions they must NOT take]

**Interaction Style**:
- [Describe how this persona should communicate with the user, e.g., "Ask clarifying questions", "Propose options and get approval", "Execute autonomously"]

**Escalation Path**:
- [Describe what this persona should do if blocked, e.g., "Create a bug bead and halt", "Escalate to Architect", "Ask user for guidance"]

---

## COMMON BEADS CLI COMMANDS REFERENCE

This section provides copy-paste ready commands for common operations.

### Reading & Context Gathering
```bash
# Show a single bead
bd show {{bead_id}}

# Show a bead as JSON (for parsing)
bd show {{bead_id}} --json

# List child beads
bd list --parent {{bead_id}}

# List all open beads
bd list --status open

# List ready work (open, no blockers)
bd ready

# Show dependency tree
bd dep tree {{bead_id}}

# List blockers
bd dep list {{bead_id}} --type depends-on

# List dependents
bd dep list {{bead_id}} --type blocks
```

### Creating & Updating Beads
```bash
# Create a task bead
bd create --parent={{parent_id}} \
  --type=task \
  --title="Task title" \
  --priority=2 \
  --acceptance="- AC 1\n- AC 2" \
  --design="Design approach"

# Create a feature bead
bd create --parent={{epic_id}} \
  --type=feature \
  --title="Feature title" \
  --priority=1 \
  --description="Detailed feature description" \
  --acceptance="- AC 1\n- AC 2" \
  --design="Technical design"

# Create a bug bead
bd create --parent={{parent_id}} \
  --type=bug \
  --title="Bug title" \
  --priority=0 \
  --description="Bug description and reproduction steps" \
  --acceptance="- Verification step" \
  --design="Root cause and fix approach"

# Update bead status
bd update {{bead_id}} --status in_progress
bd update {{bead_id}} --status open

# Update bead notes (append)
bd update {{bead_id}} --append-notes="Additional context or progress update"

# Update bead notes (replace)
bd update {{bead_id}} --notes="Complete implementation notes"

# Update bead design
bd update {{bead_id}} --design="Updated design approach"

# Update bead priority
bd update {{bead_id}} --priority=1

# Update bead acceptance criteria
bd update {{bead_id}} --acceptance="- New AC\n- Another AC"
```

### Closing & Reopening Beads
```bash
# Close a bead
bd close {{bead_id}} --reason="Summary of what was accomplished"

# Reopen a closed bead
bd reopen {{bead_id}}
```

### Managing Dependencies

**CRITICAL DISTINCTION**: Parent/child vs. blocking dependencies

**Parent/Child Hierarchy** (Epic → Feature → Task):
```bash
# ✅ CORRECT: Use --parent to create hierarchical relationships
bd create --parent=bp6-epic --type=feature --title="Feature under epic"
bd create --parent=bp6-feature --type=task --title="Task under feature"
```

**Sequential Ordering** (Task A → Task B at same level):
```bash
# ✅ CORRECT: Use bd dep add for peer tasks that must run in order
bd dep add {{bead_id}} {{blocker_id}}  # bead_id depends on blocker_id
bd dep add {{dependent_id}} {{bead_id}}  # dependent_id depends on bead_id

# ❌ WRONG: Do NOT use bd dep add to model parent/child hierarchy
bd dep add bp6-task bp6-feature  # WRONG - this is not a hierarchy
```

**Full Dependency Commands**:
```bash
# Add a blocking dependency (bead_id depends on blocker_id - PEER LEVEL ONLY)
bd dep add {{bead_id}} {{blocker_id}}

# Add a blocking relationship (dependent_id depends on bead_id - PEER LEVEL ONLY)
bd dep add {{dependent_id}} {{bead_id}}

# Create a peer relationship (related but not blocking)
bd dep relate {{bead_id}} {{peer_id}}

# Remove a dependency
bd dep remove {{bead_id}} {{blocker_id}}

# Remove a peer relationship
bd dep unrelate {{bead_id}} {{peer_id}}

# List dependencies
bd dep list {{bead_id}} --type depends-on  # What blocks this bead?
bd dep list {{bead_id}} --type blocks      # What does this bead block?
```

**Visual Example**:
```
Epic: User Authentication (bp6-auth)
  ├─ Feature: OAuth2 Login (bp6-auth-oauth) ← created with --parent=bp6-auth
  │   ├─ Task: Google Strategy (bp6-auth-oauth.1) ← created with --parent=bp6-auth-oauth
  │   └─ Task: GitHub Strategy (bp6-auth-oauth.2) ← created with --parent=bp6-auth-oauth
  │       └─ depends on bp6-auth-oauth.1 ← bd dep add bp6-auth-oauth.2 bp6-auth-oauth.1
  └─ Feature: JWT Tokens (bp6-auth-jwt) ← created with --parent=bp6-auth
      └─ depends on bp6-auth-oauth ← bd dep add bp6-auth-jwt bp6-auth-oauth
```

### Searching & Querying
```bash
# Search beads by text
bd search "search query"

# Query beads by status
bd query "status:open"

# Query beads by type
bd query "type:task"

# Query beads by priority
bd query "priority:0"

# Count beads
bd count --status open --type task
```

---

## TEMPLATE USAGE INSTRUCTIONS

**For Template Authors**:
1. Copy this file to create a new persona template
2. Replace all `[BRACKETED SECTIONS]` with persona-specific content
3. Customize the ACTIVITIES section with the persona's workflow
4. Add persona-specific MEASUREMENTS and OUTPUTS
5. Define clear EXIT CRITERIA based on the persona's role
6. Remove this section before finalizing the template

**For Agents Using This Template**:
1. Read this template BEFORE starting work on a bead
2. Follow the E-I-A-M-O-E sections IN ORDER
3. Do NOT skip the Context Establishment Protocol (C-E-P)
4. Use the exact `bd` commands provided - do NOT guess syntax
5. If blocked or uncertain, ask the user for guidance rather than proceeding with assumptions

---

## COMMON MISTAKES TO AVOID

### ❌ Mistake #1: Using `bd dep add` for Parent/Child Relationships

**WRONG**:
```bash
# Creating a feature under an epic
bd create --type=feature --title="OAuth Login"
bd dep add bp6-oauth bp6-auth-epic  # WRONG - this creates a blocking dependency, not a hierarchy
```

**CORRECT**:
```bash
# Use --parent to create hierarchical relationships
bd create --parent=bp6-auth-epic --type=feature --title="OAuth Login"
```

**Why it matters**: `bd dep add` is for **sequential ordering at the same level**, not for Epic → Feature → Task hierarchy.

---

### ❌ Mistake #2: Confusing "depends on" Direction

**Remember**: `bd dep add A B` means "A depends on B" (B blocks A).

**WRONG**:
```bash
# Task A must complete before Task B
bd dep add bp6-task-a bp6-task-b  # WRONG - this makes A wait for B
```

**CORRECT**:
```bash
# Task A must complete before Task B
bd dep add bp6-task-b bp6-task-a  # CORRECT - B depends on A (A blocks B)
```

**Mnemonic**: Think "B depends on A" → `bd dep add B A`

---

### ❌ Mistake #3: Skipping Context Establishment Protocol (C-E-P)

**WRONG**:
```bash
# Starting work immediately
bd update bp6-xyz --status in_progress
# ... start coding without reading bead context
```

**CORRECT**:
```bash
# ALWAYS run C-E-P first
bd show bp6-xyz
bd show bp6-parent
bd list --parent bp6-xyz
bd dep list bp6-xyz --type depends-on
# ... NOW start work
bd update bp6-xyz --status in_progress
```

---

## APPENDIX: E-I-A-M-O-E Pattern Explained

**E**ntry Criteria: Pre-conditions that must be true before work begins
**I**nputs: Context and information required (includes C-E-P)
**A**ctivities: Step-by-step workflow to execute the work
**M**easurements: Metrics to track progress and quality
**O**utputs: Artifacts produced by the work
**E**xit Criteria: Post-conditions that must be true before work is complete

This pattern ensures:
- **Clarity**: Agents know exactly what to do, when, and how
- **Consistency**: All personas follow the same structural framework
- **Completeness**: Nothing is missed - entry/exit criteria enforce quality gates
- **Context**: C-E-P ensures agents understand the "why" before executing the "what"
- **Traceability**: Measurements and outputs create an audit trail
