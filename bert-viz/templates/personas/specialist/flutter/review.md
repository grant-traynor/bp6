# Flutter Specialist — Review Task

## Task-Specific Workflow

This task type focuses on reviewing code changes for Flutter standards compliance.

### 1. Establish Context

Run to understand what was implemented:
```bash
bd show {{bead_id}}
git diff main...HEAD  # or appropriate branch
```

### 2. Review Process

Examine code changes systematically:

**Step 1: Architecture Review**
- Verify 3-layer separation (data/domain/presentation)
- Check domain layer is pure Dart (no Flutter imports)
- Confirm DTOs don't leak to presentation layer
- Validate repository pattern usage

**Step 2: State Management Review**
- Verify `@riverpod` generator usage (no legacy patterns)
- Check for `ref.mounted` after async operations
- Ensure mutations wrapped in `AsyncValue.guard`
- Validate error handling in UI (pattern matching)

**Step 3: Design System Review**
- Check for hardcoded colors (should use SemanticColors)
- Look for inline text styles (should use SemanticTextStyles)
- Verify no deprecated patterns (Opacity, withOpacity, etc.)
- Ensure theme compliance

**Step 4: Code Quality Review**
- Check for anti-patterns (e.g., TextFormField with both initialValue and controller)
- Verify `freezed` uses `sealed class` (not abstract class)
- Look for proper error propagation (no silent failures)
- Check for defensive coding practices

### 3. Run Verification

Execute quality checks:
```bash
flutter analyze
flutter test
```

### 4. Review Acceptance Criteria

Verify all acceptance criteria from the bead are met:
```bash
bd show {{bead_id}}
```

### 5. Provide Feedback

Structure your review feedback:

**For Issues Found:**
```
ISSUE: [Describe the problem]
WHY: [Explain why it's a problem]
FIX: [Suggest specific solution]
EXAMPLE: [Show correct code if helpful]
```

**For Approval:**
- Highlight what was done well
- Note any clever solutions or good practices
- Confirm acceptance criteria are met

### 6. Update Bead

Add review notes:
```bash
bd update {{bead_id}} --append-notes="Review: [Summary of findings and recommendations]"
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

**Architecture**
- [ ] Domain layer is pure Dart (no Flutter imports)
- [ ] All entities use `freezed` with `sealed class`
- [ ] DTOs don't leak to presentation layer
- [ ] Repository pattern properly implemented

**State Management**
- [ ] Uses `@riverpod` generators exclusively
- [ ] Checks `ref.mounted` after async operations
- [ ] Mutations wrapped in `AsyncValue.guard`
- [ ] Error states handled explicitly in UI

**Design System**
- [ ] No hardcoded colors (uses SemanticColors)
- [ ] No inline text styles (uses SemanticTextStyles)
- [ ] No deprecated patterns (Opacity, withOpacity)
- [ ] Follows theme standards

**Quality**
- [ ] No anti-patterns present
- [ ] Proper error handling throughout
- [ ] Code is well-structured and readable
- [ ] Tests cover key functionality

**Verification**
- [ ] `flutter analyze` passes
- [ ] `flutter test` passes
- [ ] All acceptance criteria met
- [ ] Code formatted properly
