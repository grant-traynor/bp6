# Tauri Specialist — Review Task

## Task-Specific Workflow

This task type focuses on reviewing Tauri code for both frontend (React/TypeScript) and backend (Rust).

### 1. Establish Context

Run to understand what was implemented:
```bash
bd show {{bead_id}}
git diff main...HEAD  # or appropriate branch
```

Identify scope:
- What Rust files changed?
- What React/TypeScript files changed?
- Are there new commands?
- Are there new components?

### 2. Review Process

Review both frontend and backend systematically:

## Backend Review (Rust)

**Step 1: Type Safety Review**
- Check structs have `#[derive(Serialize, Deserialize)]`
- Verify `#[serde(rename_all = "camelCase")]` is present
- Ensure field types are appropriate
- Validate no unnecessary `Clone` derives

**Step 2: Command Review**
- Check `#[tauri::command]` attribute present
- Verify return type is `Result<T, String>`
- Ensure async operations use `async fn`
- Validate parameters are properly typed

**Step 3: Error Handling Review**
- Check NO `unwrap()` or `expect()` in production code
- Verify `.map_err(|e| e.to_string())` or similar for conversions
- Ensure error messages are descriptive
- Validate all error paths are handled

**Step 4: Registration Review**
- Check command is registered in lib.rs
- Verify invoke_handler includes new commands
- Ensure module structure is correct

**Step 5: Code Quality Review**
- Check for idiomatic Rust patterns
- Verify proper ownership and borrowing
- Look for unnecessary allocations
- Ensure proper async/await usage

**Step 6: Run Backend Checks**
```bash
cargo check
cargo clippy
cargo test
```

## Frontend Review (React/TypeScript)

**Step 1: Type Safety Review**
- Check NO `any` types used
- Verify interfaces match Rust types (camelCase)
- Ensure type parameters on invoke<T>
- Validate proper generic usage

**Step 2: Component Review**
- Check proper prop interfaces
- Verify functional component patterns
- Ensure hooks follow React rules
- Validate state management

**Step 3: Invoke Pattern Review**
- Check invoke<T> has proper type parameter
- Verify error handling with try/catch
- Ensure loading states are managed
- Validate error states are displayed

**Step 4: Styling Review**
- Check Tailwind classes use theme variables
- Verify consistent Brutalist design patterns
- Ensure no inline styles
- Validate responsive design if applicable

**Step 5: Error Handling Review**
- Check errors are caught and displayed
- Verify user-friendly error messages
- Ensure no silent failures
- Validate error state cleanup

**Step 6: Run Frontend Checks**
```bash
tsc
# or
npm run build
```

## Type Alignment Review

Critical check for Rust ↔ TypeScript alignment:

**Step 1: Compare Types**
- Rust struct with `rename_all = "camelCase"`?
- TypeScript interface uses camelCase?
- All fields present in both?
- Types compatible (String → string, etc.)?

**Step 2: Test Serialization**
- Run app in dev mode
- Execute command
- Verify data deserializes correctly
- Check console for errors

### 3. Integration Testing

If possible, test the feature:
```bash
npm run tauri dev
```

Verify:
- Commands execute without errors
- Data displays correctly in UI
- Error states work as expected
- Loading states show appropriately

### 4. Provide Feedback

Structure your review feedback:

**For Backend Issues:**
```
RUST ISSUE: [Describe the problem]
SAFETY/QUALITY IMPACT: [Why it matters]
FIX: [Suggest specific solution]
EXAMPLE: [Show correct Rust code]
```

**For Frontend Issues:**
```
TYPESCRIPT/REACT ISSUE: [Describe the problem]
TYPE SAFETY IMPACT: [Why it matters]
FIX: [Suggest specific solution]
EXAMPLE: [Show correct TypeScript code]
```

**For Type Alignment Issues:**
```
TYPE MISMATCH: [Describe the discrepancy]
IMPACT: [Serialization will fail, runtime errors, etc.]
FIX: [How to align types]
```

**For Approval:**
- Highlight good patterns used
- Note type safety compliance
- Confirm both frontend and backend are solid
- Verify acceptance criteria met

### 5. Update Bead

Add review notes:
```bash
bd update {{bead_id}} --append-notes="Review: [Backend assessment, Frontend assessment, Type alignment check, Recommendations]"
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

**Backend (Rust)**
- [ ] Proper ownership and borrowing
- [ ] No `unwrap()` or `expect()` in production
- [ ] Commands return `Result<T, String>`
- [ ] Types use `#[serde(rename_all = "camelCase")]`
- [ ] Commands registered in lib.rs
- [ ] Error messages are descriptive
- [ ] `cargo check` passes
- [ ] `cargo clippy` passes
- [ ] Tests pass

**Frontend (React/TypeScript)**
- [ ] No TypeScript `any` types
- [ ] All components have proper interfaces
- [ ] invoke<T> calls are typed correctly
- [ ] Error handling with try/catch
- [ ] Loading/error states managed
- [ ] Tailwind uses theme variables
- [ ] Build passes (tsc or npm run build)
- [ ] Hooks follow React rules

**Type Alignment**
- [ ] Rust types use camelCase serialization
- [ ] TypeScript types use camelCase
- [ ] All fields present in both
- [ ] Types are compatible
- [ ] Tested in dev mode (if possible)

**Quality**
- [ ] All acceptance criteria met
- [ ] Code is clean and readable
- [ ] Patterns are consistent
- [ ] Documentation present if needed
