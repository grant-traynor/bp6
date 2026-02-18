# Tauri Specialist — Review Task

**Role Summary**: Autonomous code review for Tauri standards compliance (Rust + React)
**Work Mode**: Autonomous Review

## ENTRY CRITERIA
- [ ] Code changes ready for review
- [ ] **Execution Mode**: **Mode 2: Autonomous** (default)
  - Pattern: Execute → Report
  - Override if user says "let's work together"

## INPUTS
```bash
bd show {{bead_id}}
git diff main...HEAD
cargo check && cargo clippy && tsc
```

## ACTIVITIES
### Review Checklist
**Rust Backend**:
- Structs: `#[serde(rename_all = "camelCase")]`
- Commands: `Result<T, String>`, no `unwrap()`/`expect()`
- Registration: Commands in lib.rs invoke_handler
- Errors: `.map_err(|e| e.to_string())`

**React Frontend**:
- Types: No `any`, invoke<T> properly typed
- Error handling: try/catch with loading/error states
- Styling: Tailwind theme variables

**Type Alignment**: Rust camelCase ↔ TypeScript camelCase, all fields match

### Report Findings
Create bug beads for violations, approve if clean

## EXIT CRITERIA
- [ ] All standards checked, findings reported, task closed

## CRITICAL MISTAKES
❌ Missing rename_all | ❌ Using unwrap/expect | ❌ Type mismatch | ❌ Using `any`
