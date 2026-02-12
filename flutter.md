---
name: dart_flutter_dev_standard
title: Dart and Flutter Engineering Standards
context: [flutter, dart, frontend, app, riverpod, freezed, user_interface]
description: Mandatory patterns for front end development and code review
capabilities: [review, implement]
priority: mandatory  
triggers:
  extensions: [.dart]
  paths: ["**/app/lib/*.dart"]
  exclude: ["**/generated/**", "**/*.g.dart", "**/*.freezed.dart", "**/*.gr.dart"]
---

# Pairti Flutter & Dart Engineering Standards

**STATUS: MANDATORY**
**APPLIES TO:** All `.dart` files, Flutter widgets, Riverpod providers, and logic.

---

## 1. 🧱 Architecture (The 3-Layer Shield)

We strictly enforce **Clean Architecture**.

### Folder Structure
```
lib/feature/[feature_name]/
├── data/         # Infrastructure: DTOs, Repository Impl (External World)
├── domain/       # Business Logic: Entities, Repository Interfaces (The Truth)
└── presentation/ # UI/State: Riverpod Notifiers, Views (The Mirror)
```

### Layer Rules
1.  **Domain (`domain/`)**:
    *   **MUST** be Pure Dart.
    *   **NO** `flutter/*`, `dart:ui`, `riverpod` (annotations ok), or `data/` imports.
    *   **MUST** use `freezed` with `sealed class` for all Entities.
2.  **Data (`data/`)**:
    *   **MUST** implement Domain interfaces.
    *   **MUST** map DTOs to Entities. Never leak DTOs (generated `fromJson` classes) to the UI.
3.  **Presentation (`presentation/`)**:
    *   **MUST** use Riverpod 3.0.
    *   **MUST** depend on Domain entities and Repository Interfaces.

---

## 2. 🌊 State Management (Riverpod 3.0)

**Legacy Ban**: usage of `ChangeNotifier`, `StateNotifier`, or `FutureProvider` (without notifier) is **PROHIBITED**.

### Generators & Syntax
*   **Mandate**: Use `@riverpod` generators for everything.
*   **Class**: `class MyNotifier extends _$MyNotifier` (using `Notifier<T>` or `AsyncNotifier<T>`).
*   **Usage**: `ref.watch(myProvider)`, `ref.listen(myProvider, ...)`.

### Resilience Pattern (No Silent Failures)
*   **Mutations**: ALL mutations must be wrapped in `AsyncValue.guard`.
    ```dart
    // CORRECT
    Future<void> save() async {
      state = const AsyncValue.loading();
      state = await AsyncValue.guard(() => repository.saveData(data));
    }
    ```
*   **Async Gaps**: ALWAYS check mounted status after `await`.
    ```dart
    await doAsyncWork();
    if (!ref.mounted) return;
    // ... proceed
    ```

---

## 3. 🛡️ Resilience & Error Handling

*   **The "Black Hole" Ban**: `catch (e) { print(e); }` is strictly prohibited.
*   **Repository Guard**: Wrap external calls in a guard that converts technical errors to domain errors.
    ```dart
    // Data Layer
    return RepositoryGuard.run(() async {
      final dto = await supabase.from(...).select();
      return dto.toEntity();
    });
    ```
*   **UI Error Handling**: Use `AsyncValue` pattern matching.
    ```dart
    return switch (state) {
      AsyncData(:final value) => DataWidget(value),
      AsyncError(:final error) => ErrorWidget(error),
      _ => const LoadingWidget(),
    };
    ```

---

## 4. 🎨 UI & UX Design System

**Hardcoding Ban**: Never hardcode hex colors or text styles in widgets.

### The "Green" List (Required)
*   **Colors**: `Theme.of(context).extension<SemanticColors>()!.primaryAction`
*   **Text**: `Theme.of(context).extension<SemanticTextStyles>()!.bodyLarge`
*   **Routing**: `AutoRouter.declarative`. Navigation is a function of State.

### The "Red" List (Forbidden)
*   `Opacity` widget (Use `color.withValues(alpha: ...)`).
*   Inline `InputDecoration(border: ...)` (Configure in Theme).
*   `Container` for basic styling (Use `Card`, `Material` or `DecoratedBox`).

---

## 5. 🚫 Common AI Anti-Patterns (Legacy Traps)

**WARNING**: Your training data contains obsolete practices. **IGNORE THEM.**

### ❄️ Freezed & Dart 3
*   **❌ OLD**: `abstract class MyState with _$MyState` ... `state.when(...)`
*   **✅ NEW**: `sealed class MyState with _$MyState` ... `switch (state)`
*   *Why?* Native Dart 3 pattern matching is more performant and readable.

### 🌊 Riverpod 3.0
*   **❌ OLD**: `ProviderRef`, `WidgetRef` (in logic), manual `StateNotifier`.
*   **✅ NEW**: Unified `Ref` everywhere. `@riverpod` annotations only.
*   *Why?* The new `Ref` type unifies interaction across all provider types.

### 🎨 Flutter UI
*   **❌ OLD**: `color.withOpacity(0.5)`
*   **✅ NEW**: `color.withValues(alpha: 0.5)`
*   *Why?* `withOpacity` is deprecated and expensive.

*   **❌ OLD**: `TextFormField(initialValue: ...)` AND `controller: ...` (Crash)
*   **✅ NEW**: Use `controller` ONLY if you need to listen to changes. Use `initialValue` for simple forms.

---

## 6. ✅ Code Review Checklist (Self-Correction)

**Target**: All Flutter Pull Requests.
**Enforcement**: Manual Review + AI Audit.

### 🏗️ Architecture & Logic
- [ ] **Clean Architecture**: `domain` layer imports ONLY Dart core. No `flutter`, `data`, or `riverpod` specific ref types.
- [ ] **Feature Isolation**: Logic resides in `lib/feature/[name]`, not global buckets.
- [ ] **Immutability**: All Entities and States use `freezed` with `sealed class`.

### 🌊 Riverpod 3.0
- [ ] **Annotations**: Uses `@riverpod` generator. No manual providers.
- [ ] **Mounted Check**: Checks `ref.mounted` after every `await`.
- [ ] **Selective Watch**: Uses `ref.watch(provider.select(...))` for performance.

### 🛡️ Resilience & Observability
- [ ] **No Swallowed Exceptions**: Are there any empty `catch` blocks or `catch { print(e); }`? (FAIL immediately).
- [ ] **Guarded Repositories**: Do Repository methods wrap external calls (Supabase/Http) in `RepositoryGuard.run` or explicit `try/catch` mapping to `AppException`?
- [ ] **Async Guard**: Are Notifier mutations wrapped in `state = await AsyncValue.guard(...)`?
- [ ] **Error UI**: Does the UI handle error states explicitly via `when` or `switch`?

### 🎨 UI & Design System
- [ ] **No Hardcoded Colors**: Uses `SemanticColors` (e.g., `colors.surfaceScrim`).
- [ ] **No Inline Styling**: Text styles come from `SemanticTextStyles`.
- [ ] **No Opacity Widget**: Uses alpha-blended colors instead.
- [ ] **No Inline Borders**: `InputDecoration` relies on Theme, not inline definitions.

### 🛣️ Routing
- [ ] **Declarative**: Navigation is state-driven (`AutoRouter.declarative`). No `push` logic in Notifiers.
- [ ] **Type Safe**: Uses generated `Route.page` and strongly typed args.

### 🛡️ Quality & Lints
- [ ] **Deprecations**: No `withOpacity` (use `withValues`). No `value` in Forms (use `initialValue`).
- [ ] **Pattern Matching**: Uses `switch` over `.when` for Freezed unions.
