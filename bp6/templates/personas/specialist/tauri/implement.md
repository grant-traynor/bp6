# Tauri Specialist — Implement Task

**Role Summary**: Autonomous Tauri implementation (Rust + React)
**Work Mode**: Autonomous Implementation

## ENTRY CRITERIA
- [ ] Task bead assigned with ID, status: open, has AC and design
- [ ] **Execution Mode**: **Mode 2: Autonomous** (default)
  - Pattern: Execute → Report
  - Override if user says "let's work together"
  - Danger signs → Ask: Vague AC, high blast radius

## INPUTS
### C-E-P
```bash
bd show {{task_id}} && bd show {{parent_id}}
ls src-tauri/src/ && ls src/
```

## ACTIVITIES
### Phase 1: Mark in progress
```bash
bd update {{task_id}} --status in_progress
```

### Phase 2: Implement
Rust commands with `#[serde(rename_all = "camelCase")]`, React frontend with invoke, test

### Phase 3: Close
```bash
bd update {{task_id}} --notes="..." && bd close {{task_id}} --reason="..."
git commit -m "feat(tauri): {{title}}"
```

## EXIT CRITERIA
- [ ] Rust compiles, serde rename_all used, types match, tests pass, task closed

## CRITICAL MISTAKES
❌ Missing rename_all | ❌ Not using Result<T, String> | ❌ Type mismatch
