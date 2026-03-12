# Plan Review — Plan Increment (Wordle Clone Stage 1)

**Review ID**: r-eng
**Verdict**: APPROVED_WITH_CONDITIONS

## Summary

The plan is technically sound and complete for Stage 1 delivery. All 12 tasks are right-sized, the DAG is acyclic, function decomposition maps correctly to the CONOPS §5 game loop, and must-not controls are properly assigned across implementation and audit tasks. Two conditions must be resolved inline before the affected tasks are marked done: (1) t11's acceptance criteria reference "14 MN items" but must-nots.md enumerates 16 distinct identifiers — the auditor must work from the full enumerated list; (2) t6's description conflates `textContent` (letter content) with color state, which requires `classList` manipulation, not text assignment. Neither condition requires replanning.

---

## Findings

### [PASS] Function decomposition maps correctly to CONOPS workflow

The proposed chain — corpus + random selector (t2) → `isValidGuess()` (t3) → `scoreGuess()` (t4) → `submitGuess()` (t5) → `renderRow()` (t6) → `checkGameOver()` (t8) — correctly models CONOPS §5: load → validate → score → reveal feedback → game-over check. Each function has a single, bounded responsibility. No function is doing double duty. The keyboard listener (t7) correctly sits outside the game logic chain as an input adapter.

### [PASS] Task right-sizing

All 12 tasks are 1–2 hours of focused work with clear boundaries. No undeclared epics. The most complex task, `scoreGuess()` with duplicate-letter handling (t4, ~2h), is appropriately sized for a bounded algorithm with known edge cases. t11 (compliance audit of all MN identifiers, ~2h) is tight but achievable as a systematic code review against a defined checklist. t12 (senior engineer code review, ~2h) is a standard review scope for a project of this scale.

### [PASS] Skill assignments

`frontend` for all HTML/JS/CSS implementation tasks (t1–t9): correct. `test` for unit tests (t10) and compliance audit (t11): correct. `senior-engineer` for code review (t12): correct. Epic e3 (quality assurance) under `senior-engineer` is sound.

### [PASS] DAG validity — no cycles, prerequisite chains complete

All 19 declared edges are directionally correct. Critical paths verified:

- Corpus reaches submitGuess: t2→t3→t5 and t2→t4→t5 ✓
- HTML skeleton gates all DOM tasks: t1→t5, t1→t6, t1→t7, t1→t9 ✓
- All implementation tasks reach audit: t5→t11, t6→t11, t7→t11, t8→t11, t9→t11, t10→t11 ✓
- Audit gates review: t11→t12 ✓
- No cycles exist in any path.

**Note for implementing agent**: The interface-first ordering (t5 before t6 and t8) requires that t5 be implemented as a skeleton with clearly stubbed callsites for `renderRow()` and `checkGameOver()`. If t5 is written as a completed integration before t6 and t8 exist, it cannot be marked done. Implement t5 with stub markers, then fill in t6 and t8 in sequence.

### [PASS] Must-not architectural controls present in implementation tasks

All must-nots with implementation-time impact are addressed in specific tasks:

| MN | Control | Task |
|----|---------|------|
| MN-SEC-01/02 | No network calls, no CDN resources | t2 (corpus inline), t11 audit |
| MN-SEC-03 | No innerHTML with user input | t6 (`textContent` for letters), t11 |
| MN-PRIV-01 | No localStorage/storage | t11 audit |
| MN-PRIV-03 | No logging of player guesses | t11 audit |
| MN-TRUST-01 | No attempt consumed on invalid guess | t3 (isValidGuess gating), t5 (handler logic) |
| MN-TRUST-02 | Answer not revealed before game over | t8 (loss branch only) |
| MN-TRUST-03 | Target word immutable mid-game | t2 (selected once, stored in closure) |
| MN-ARCH-01/02 | No frameworks, no build step | t11 audit, conops §8 |
| MN-QG-02 | Greens-first duplicate-letter handling | t4 algorithm spec, t10 unit tests |
| MN-QG-01 | Zero JS errors across four browsers | t11 audit |
| MN-SCOPE-01/02 | No auth, no daily mode | absent from plan (correct omission) |

### [WARN] MN item count discrepancy — plan says 14, actual enumeration is 16

t11's acceptance criteria reference "all 14 MN items." must-nots.md's own overview section also states "14 hard constraints." The actual enumerated list contains **16 distinct identifiers**:

- MN-SEC-01, 02, 03 (3)
- MN-PRIV-01, 02, 03 (3)
- MN-TRUST-01, 02, 03 (3)
- MN-ARCH-01, 02, 03 (3)
- MN-SCOPE-01, 02 (2)
- MN-QG-01, 02 (2)

**Total: 16.** The discrepancy is a self-contradiction in must-nots.md — its stated count does not match its own enumerated items. If the t11 auditor uses the stated count (14) as a stop condition rather than working through the full identifier list, MN-PRIV-03 (no logging of player guesses) and at least one other constraint will be silently skipped.

**Required fix before t11 is marked done**: t11 auditor must audit against the complete enumerated list — MN-SEC-01 through MN-QG-02 — not the stated count of 14. Update t11 acceptance criteria to list all 16 identifiers explicitly.

### [WARN] t6 renderRow description conflates two distinct operations

t6 is titled "renderRow() tile color update using textContent." This is ambiguous in a way that could produce an incorrect implementation:

- **Letter content** (the character in the tile): must use `element.textContent = letter` — correct per MN-SEC-03.
- **Color state** (green/yellow/gray): must use `tile.classList.add('green' | 'yellow' | 'gray')` — CSS class toggling. This cannot be done via `textContent`.

If an implementer reads the task title literally and attempts to represent color state through `textContent` (e.g., setting text to "green"), the implementation will be wrong. Additionally, the CSS classes `green`, `yellow`, and `gray` must be defined before t6 runs — they should be included in the HTML skeleton (t1) or the responsive CSS task (t9). The plan does not make this explicit.

**Required fix before t6 is marked done**: t6 implementation must perform both operations: (a) `tile.textContent = letter` for letter content (MN-SEC-03), and (b) `tile.classList.add('green' | 'yellow' | 'gray')` for color state. The implementing agent must verify CSS color classes are defined in t1 or t9 before marking t6 done.

### [PASS] Unit test scope appropriate for project scale

t10 covers `scoreGuess()` duplicate-letter edge cases — the one algorithm where correctness complexity warrants targeted unit testing (MN-QG-02). `isValidGuess()` is a simple dictionary lookup; `submitGuess()` integration behavior is verified via MN-TRUST-01 in t11. For a zero-build-tool vanilla JS project, this test scope is appropriate. t12 code review provides additional coverage for the ununit-tested paths.

### [PASS] Stage completeness

The plan delivers a complete, playable Wordle clone in one stage. All CONOPS §5 workflow steps are covered: word selection (t2), validation (t3), scoring (t4), render (t6), game-over (t8). All §7 non-functional requirements have coverage: performance is implicit in static file delivery; responsiveness via t9; cross-browser compatibility via MN-QG-01 in t11. All CONOPS §9 out-of-scope items — accounts, daily mode, multiplayer, on-screen keyboard, animations, backend — are correctly absent from the plan.

---

## Verdict Rationale

APPROVED_WITH_CONDITIONS. No finding rises to BLOCK level — the plan is structurally sound and complete.

Two WARNs require inline resolution by the executing agent before their respective tasks are marked done:

1. **t11 — MN count**: Audit must enumerate all 16 MN identifiers as a checklist. Update acceptance criteria before starting the audit task.
2. **t6 — renderRow description**: Implementation requires both `textContent` (letters) and `classList` manipulation (colors). The implementing agent must treat the task description as incomplete and apply both operations. Verify CSS color classes exist in t1 or t9 before marking t6 done.

The interface-first interpretation of t5→t6/t8 is coherent but requires t5 to be implemented as a skeleton with stubbed callsites — normal top-down development practice, not a planning defect.
