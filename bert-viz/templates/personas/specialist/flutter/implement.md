# Flutter Specialist — Implement Task

**Role Summary**: Autonomous Flutter implementation with Clean Architecture + Riverpod 3.0

**Work Mode**: Autonomous Implementation

---

## ENTRY CRITERIA

- [ ] Task bead assigned with ID
- [ ] Task status: open
- [ ] Task has description, acceptance criteria, and design notes
- [ ] No blockers (dependencies resolved)
- [ ] **Execution Mode Determined**: **Mode 2: Autonomous** (default for this persona/task)
  - **Pattern**: Execute → Report (no approval needed mid-work)
  - Clear task with validated design - implement autonomously
  - **Override if**: User says "let's work together" or "propose a plan first"
  - **Danger signs** → Ask user which mode:
    - ⚠️ Vague acceptance criteria or missing design notes
    - ⚠️ High blast radius (breaking changes, architecture shifts)
  - **Document**: "I'll work in Autonomous Mode for this implementation..."

---

## INPUTS

### Context Establishment Protocol (C-E-P)

**CRITICAL**: Execute FIRST before implementation.

```bash
# Step 1: Read target task
bd show {{task_id}}

# Step 2: Read parent feature/epic
bd show {{parent_id}}

# Step 3: Check dependencies
bd dep list {{task_id}} --type depends-on

# Step 4: Gather Flutter context
flutter pub get
ls -R lib/
```

### Additional Context Sources

- **Codebase**: Read existing patterns in relevant feature area
- **Standards**: `.agent/standards/flutter.md` auto-injected
- **Tests**: Review test patterns

---

## ACTIVITIES

### Phase 1: Preparation

**1.1. Analyze Task**

Extract from C-E-P:
- What needs to be implemented?
- Which layers involved? (data/domain/presentation)
- Which files to create/modify?
- What patterns to follow?

**1.2. Mark In Progress**

```bash
bd update {{task_id}} --status in_progress
```

---

### Phase 2: Implementation

**2.1. Domain Layer** (if needed)

- Create entities with `freezed` and `sealed class`
- Define repository interfaces
- Add pure business logic
- **VERIFY**: NO Flutter imports in domain layer

```dart
// Example: User entity
@freezed
class User with _$User {
  const factory User({
    required String id,
    required String email,
    String? displayName,
  }) = _User;
}
```

**2.2. Data Layer** (if needed)

- Implement repository interfaces
- Create DTOs and mapping
- Add error handling with RepositoryGuard
- Test repository methods

```dart
// Example: Repository implementation
class UserRepositoryImpl implements UserRepository {
  final SupabaseClient _client;

  @override
  Future<Either<Failure, User>> getUser(String id) async {
    return RepositoryGuard.guard(() async {
      final data = await _client.from('users').select().eq('id', id).single();
      return UserDto.fromJson(data).toDomain();
    });
  }
}
```

**2.3. Presentation Layer**

- Create Riverpod providers with `@riverpod` annotation
- **PROVIDER NAMING**: Name by data (e.g. `userProvider`), NOT by class name (`userNotifierProvider`).
- Build UI components using `SemanticColors` and `SemanticTextStyles`
- Use `CustomPainter` for complex visuals or specialized shapes
- Add state management with `AsyncNotifierProvider`
- Handle loading/error states with `AsyncValue` pattern matching

```dart
// Example: Provider (Named by data)
@riverpod
class UserNotifier extends _$UserNotifier {
  @override
  Future<User> build(String userId) async {
    final repo = ref.read(userRepositoryProvider);
    final result = await repo.getUser(userId);
    return result.fold((l) => throw l, (r) => r);
  }
}

// Example: High-Fidelity UI (CustomPainter + SemanticColors)
class BrandingPattern extends CustomPainter {
  final SemanticColors colors;
  BrandingPattern({required this.colors});

  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()..color = colors.brandSecondary.withValues(alpha: 0.1);
    // Draw complex background patterns...
  }

  @override
  bool shouldRepaint(covariant BrandingPattern oldDelegate) => 
    oldDelegate.colors != colors;
}
```

**2.4. Testing**

```bash
# Run tests
flutter test

# Run analyzer
flutter analyze

# Check coverage (if required)
flutter test --coverage
```

**Checklist**:
- [ ] All acceptance criteria met
- [ ] Tests pass
- [ ] Linter clean (no warnings/errors)
- [ ] No regressions

---

### Phase 3: Documentation & Closure

**3.1. Update Task Notes**

```bash
bd update {{task_id}} --notes="Implemented {{summary}}. Key decisions: {{decisions}}. Files modified: {{files}}. Tests: {{status}}."
```

**3.2. Update Design (if changed)**

```bash
bd update {{task_id}} --design="{{architectural_changes_or_deviations}}"
```

**3.3. Close Task**

```bash
bd close {{task_id}} --reason="{{summary_of_what_was_accomplished}}"
```

**3.4. Commit**

```bash
git add .
git commit -m "feat(flutter): {{task_title}}

{{details}}

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## MEASUREMENTS

### Process Metrics
- **Time to context**: < 5 minutes
- **Implementation time**: Varies by complexity

### Quality Metrics
- **Tests passing**: 100%
- **Linter clean**: 0 warnings/errors
- **AC met**: 100%

### Outcome Metrics
- **Rework rate**: % needing reopening
- **Quality**: No regressions introduced

---

## OUTPUTS

- **Code changes**: Committed to version control
- **Updated task**: Notes and design populated
- **Closed task**: Status = closed with reason
- **Tests passing**: All checks green

---

## EXIT CRITERIA

- [ ] All acceptance criteria met
- [ ] Tests passing
- [ ] Linter clean
- [ ] Task updated with notes
- [ ] Task closed with summary
- [ ] Changes committed

---

## CRITICAL MISTAKES TO AVOID

### ❌ Mistake #1: Flutter Imports in Domain Layer

**WRONG**: `import 'package:flutter/material.dart'` in domain

**CORRECT**: Domain is pure Dart - NO Flutter imports

### ❌ Mistake #2: Using ChangeNotifier / Poor Naming

**WRONG**: `class Provider extends ChangeNotifier` or `userNotifierProvider`

**CORRECT**: Use `@riverpod` with naming by data: `userProvider`

### ❌ Mistake #3: Hardcoded Colors / Opacity Widget

**WRONG**: `color: Colors.blue` or `Opacity(opacity: 0.5, child: ...)`

**CORRECT**: Use `SemanticColors` and alpha-blended colors: `color: colors.primary.withValues(alpha: 0.5)`

### ❌ Mistake #4: Not Checking ref.mounted

**WRONG**: Setting state after async without check

**CORRECT**: `if (!ref.mounted) return; state = ...`
