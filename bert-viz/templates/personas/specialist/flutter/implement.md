# Flutter Specialist — Implement Task

## Task-Specific Workflow

This task type focuses on implementing code changes following Flutter standards.

### 1. Establish Context

Run immediately:
```bash
bd show {{bead_id}}
bd list --status open --parent {{bead_id}}
flutter pub get
ls -R lib/
```

Review:
- Feature description and acceptance criteria
- Design notes and architectural decisions
- Existing code patterns in the feature area

### 2. Plan Implementation

Before writing code:
- Identify which layers need changes (data/domain/presentation)
- List files to create or modify
- Determine dependencies and providers needed
- Identify testing approach

### 3. Mark Bead In Progress

```bash
bd update {{bead_id}} --status in_progress
```

### 4. Implementation Steps

**Phase 1: Domain Layer** (if needed)
- Create entities with `freezed` and `sealed class`
- Define repository interfaces
- Add pure business logic
- Verify NO Flutter imports

**Phase 2: Data Layer** (if needed)
- Implement repository interfaces
- Create DTOs and mapping logic
- Add error handling with RepositoryGuard
- Test repository methods

**Phase 3: Presentation Layer**
- Create Riverpod providers with `@riverpod`
- Implement state management logic
- Build UI components
- Add error handling with AsyncValue

**Phase 4: Integration**
- Wire up dependencies
- Ensure theme compliance (no hardcoded colors/styles)
- Add navigation if needed
- Test user flows

### 5. Code Generation

After creating or modifying code with generators:
```bash
dart run build_runner build --delete-conflicting-outputs
```

### 6. Quality Verification

Run checks before closing:
```bash
flutter analyze
flutter test
dart format .
```

### 7. Update Bead

Document what was done:
```bash
bd update {{bead_id}} --notes="[Implementation summary, key decisions, gotchas]"
bd update {{bead_id}} --design="[Architectural approach, patterns used, rationale]"
```

### 8. Close Bead

```bash
bd close {{bead_id}} --reason="[What was accomplished, how it meets acceptance criteria]"
```

Verify closure:
```bash
bd show {{bead_id}}
```

## Implementation Checklist

Before marking complete:
- [ ] Domain layer is pure Dart (no Flutter imports)
- [ ] All entities use `freezed` with `sealed class`
- [ ] DTOs don't leak to presentation
- [ ] Using `@riverpod` generators (no legacy patterns)
- [ ] `ref.mounted` checked after async operations
- [ ] No hardcoded colors (using SemanticColors)
- [ ] No inline text styles (using SemanticTextStyles)
- [ ] Error states handled explicitly
- [ ] `flutter analyze` passes
- [ ] Tests pass and cover key functionality
- [ ] Code formatted with `dart format`
