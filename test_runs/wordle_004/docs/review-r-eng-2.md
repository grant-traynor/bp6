# Plan Review — Plan Increment (Wordle Clone Stage 1)

**Review ID**: r-eng-2
**Verdict**: APPROVED_WITH_CONDITIONS

## Summary

The plan is structurally sound and the prior review (r-eng) reached the correct verdict. This review confirms APPROVED_WITH_CONDITIONS. The two WARNs from r-eng stand: (1) t11's acceptance criteria reference 14 MN items — must-nots.md actually contains 16 enumerated identifiers; t11 must audit against the full list. (2) t6's task description conflates `textContent` (letter rendering) with CSS class manipulation (color state) — both operations are required and must not be conflated by the implementer. One additional WARN is raised: the t9→t6 dependency is absent from the DAG, creating ambiguity about where color CSS classes (`.green`, `.yellow`, `.gray`) are defined; this must be resolved before execution begins.

---

## Findings

### [PASS] Conops coverage is complete

The plan's 12 tasks map cleanly to every CONOPS §5 workflow step: word selection (t2), validation (t3), color scoring (t4), integration (t5), tile rendering (t6), keyboard input (t7), game-over (t8), responsiveness (t9). All CONOPS §7 non-functional requirements are covered (performance implicit in static delivery; responsiveness via t9; cross-browser via t11). All CONOPS §9 out-of-scope items (accounts, daily mode, multiplayer, on-screen keyboard, animations) are correctly absent from the plan.

### [PASS] Task right-sizing

All 12 tasks are in the 1–2h range. No task is an undeclared epic. The hardest task — `scoreGuess()` with duplicate-letter handling (t4, ~2h) — is well-bounded: the algorithm has known inputs, known edge cases enumerated in MN-QG-02, and a unit test task (t10) providing acceptance criteria. The compliance audit (t11, ~2h) is tight but achievable as a systematic code review against a checklist of 16 identifiers.

### [PASS] Skill assignments

`frontend` for all implementation tasks (t1–t9): correct. `test` for unit tests and compliance audit (t10–t11): correct. `senior-engineer` for code review (t12): correct. Epic e3 (quality assurance) assigned to `senior-engineer` is appropriate. No mis-assignments.

### [PASS] DAG — no cycles, critical paths complete

All 19 edges are directionally sound. No cycle exists. Critical prerequisite chains are intact:

- Corpus (t2) → isValidGuess (t3) → submitGuess (t5) ✓
- Corpus (t2) → scoreGuess (t4) → submitGuess (t5) ✓
- HTML skeleton (t1) gates all DOM-touching tasks (t5, t6, t7, t9) ✓
- All implementation tasks gate audit (t11) ✓
- Audit (t11) gates code review (t12) ✓

The interface-first interpretation of t5→t6, t5→t7, t5→t8 (submitGuess establishes the integration contract, callees implement against it) is coherent for top-down development. Implementing agent must treat t5 as a skeleton with clearly marked stub callsites for `renderRow()` and `checkGameOver()` before t6 and t8 begin.

### [WARN] MN count discrepancy — t11 must audit 16 identifiers, not 14

The plan's t11 acceptance criteria state "all 14 MN items." The must-nots.md overview header also states "14 hard constraints" — but the document enumerates **16** distinct must-not identifiers:

| Category | Identifiers | Count |
|----------|-------------|-------|
| Security | MN-SEC-01, MN-SEC-02, MN-SEC-03 | 3 |
| Privacy | MN-PRIV-01, MN-PRIV-02, MN-PRIV-03 | 3 |
| Trust | MN-TRUST-01, MN-TRUST-02, MN-TRUST-03 | 3 |
| Architecture | MN-ARCH-01, MN-ARCH-02, MN-ARCH-03 | 3 |
| Scope | MN-SCOPE-01, MN-SCOPE-02 | 2 |
| Quality Gate | MN-QG-01, MN-QG-02 | 2 |
| **Total** | | **16** |

must-nots.md is self-contradictory: the stated count of 14 is wrong. If the t11 auditor uses "14" as a stop condition rather than enumerating the full list, MN-PRIV-03 (no logging of player guesses to external endpoints) and at least one other constraint will be silently skipped.

**Required fix before t11 is marked done**: Update t11 acceptance criteria to list all 16 identifiers (MN-SEC-01 through MN-QG-02) as a checklist. The auditor must work from enumerated identifiers, not the stated count.

This WARN was also raised in r-eng. It remains unresolved in the plan as submitted.

### [WARN] t6 description conflates two distinct operations

t6 is described as "renderRow() tile color update using textContent." This description is misleading:

- **Letter content** — the letter character in each tile — must be set via `element.textContent = letter`. This is the MN-SEC-03 compliance mechanism (never `innerHTML` with user-derived input).
- **Color state** — green/yellow/gray — must be applied via CSS class manipulation: `tile.classList.add('green' | 'yellow' | 'gray')`. Color state cannot be expressed via `textContent`.

The title implies `textContent` handles color, which is incorrect. An implementer reading the task literally may attempt to express color via text content rather than CSS classes.

**Required fix before t6 is marked done**: The t6 implementation must perform both operations: (a) `tile.textContent = letter` for letter rendering (MN-SEC-03), and (b) `tile.classList.add('green' | 'yellow' | 'gray')` for color state. The CSS class definitions must be available when t6 runs — see the t9→t6 dependency WARN below.

This WARN was also raised in r-eng. It remains unresolved in the plan as submitted.

### [WARN] Missing dependency edge: t9 (CSS) must precede or co-deliver with t6 (color classes)

t6 applies CSS classes `.green`, `.yellow`, `.gray` to tiles. These classes must be defined in CSS before t6 can be visually verified. The plan's dependency edges include t1→t6 and t5→t6, but no t9→t6.

Two resolutions are acceptable — pick one and document it:

**Option A**: Define the color-state CSS classes (`.green`, `.yellow`, `.gray`) as part of t1's HTML skeleton in a `<style>` block. Color classes are tightly coupled to tile rendering, not to responsive layout. This keeps t9 focused on layout/responsiveness and makes t1→t6 sufficient.

**Option B**: Add a t9→t6 dependency edge. This means t6 cannot be executed until t9 (responsive CSS including color classes) is complete.

If Option A is chosen, t1's acceptance criteria must explicitly state that color classes are included. If Option B is chosen, add the missing edge to the plan before execution begins.

**Required fix before t6 implementation starts**: Resolve which approach is used and document it. An implementer who encounters `tile.classList.add('green')` while the CSS class is undefined will produce visually broken output that passes code review but fails visual QA.

### [PASS] Must-not architectural controls properly reflected in implementation tasks

| MN | Control | Task |
|----|---------|------|
| MN-SEC-01/02 | No network calls, no CDN resources | t2 (corpus inline), t11 (audit) |
| MN-SEC-03 | No innerHTML with user-controlled input | t6 (textContent explicit), t11 |
| MN-PRIV-01 | No localStorage/storage | t11 (audit) |
| MN-PRIV-02 | No analytics/tracking | t11 (audit) |
| MN-PRIV-03 | No logging of player guesses | t11 (audit) |
| MN-TRUST-01 | No attempt consumed on invalid guess | t3 (isValidGuess gating), t5 |
| MN-TRUST-02 | Answer not revealed before game over | t8 (checkGameOver loss branch only) |
| MN-TRUST-03 | Target word immutable mid-game | t2 (selected once, stored in closure) |
| MN-ARCH-01/02 | No frameworks, no build step | t11 (audit) |
| MN-ARCH-03 | No backend server | correctly absent from plan |
| MN-SCOPE-01/02 | No auth, no daily mode | correctly absent from plan |
| MN-QG-01 | No JS errors in all four browsers | t11 (audit) |
| MN-QG-02 | Correct duplicate-letter color logic | t4 (greens-first algorithm), t10 (unit tests) |

All 13 of the 16 MN identifiers with implementation-time impact have explicit task coverage. MN-ARCH-03, MN-SCOPE-01, and MN-SCOPE-02 are satisfied by absence (no tasks implement what is prohibited) — this is the correct control mechanism.

### [PASS] Unit test coverage is appropriate for project scope

t10 targets `scoreGuess()` duplicate-letter edge cases — the only algorithm with non-trivial correctness requirements (MN-QG-02). `isValidGuess()` (simple dictionary lookup) and `submitGuess()` (integration behavior verified via MN-TRUST-01 compliance check in t11) do not require targeted unit testing at this scale. For a zero-build-tool project, this test scope is right-sized. t12 code review provides additional coverage of the full implementation.

### [PASS] Stage completeness

The plan delivers a complete, playable Wordle clone in one stage. No CONOPS workflow step is unimplemented. No in-scope requirement is unaddressed. The plan is a valid Stage 1 complete delivery.

---

## Verdict Rationale

APPROVED_WITH_CONDITIONS. The plan is structurally sound with valid DAG, right-sized tasks, correct skill assignments, and complete must-not control coverage. No finding rises to BLOCK level.

Three WARNs require inline resolution before the corresponding tasks are marked done:

1. **t11 — MN count**: Auditor must enumerate all 16 MN identifiers as a checklist, not stop at 14. Update t11 acceptance criteria before starting the audit.

2. **t6 — renderRow description**: Implementation must use both `element.textContent = letter` (letters, MN-SEC-03) and `tile.classList.add(...)` (colors). The task description is incomplete; implementer must apply both operations.

3. **t6/t9 — Color CSS class availability**: Resolve whether `.green`, `.yellow`, `.gray` classes are defined in t1's HTML skeleton or in t9's CSS file, and add t9→t6 edge if the latter. This must be resolved before t6 implementation begins, not discovered during visual QA.

WARNs 1 and 2 were also raised in r-eng and remain unresolved. WARN 3 is new — the prior review noted the t6 description issue but did not formally flag the missing CSS dependency edge.
