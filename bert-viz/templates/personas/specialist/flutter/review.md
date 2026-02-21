# Flutter Specialist — Review Task

**Role Summary**: Autonomous code review for Flutter standards compliance
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
flutter analyze
```

## ACTIVITIES
### Review Checklist
**Architecture**: 3-layer separation, pure domain, no leaks
**State**: `@riverpod`, `ref.mounted` checks, AsyncValue
**Design**: SemanticColors/TextStyles, no hardcoded values
**Quality**: No anti-patterns, freezed sealed classes

### Report Findings
Create bug beads for violations, approve if clean

## EXIT CRITERIA
- [ ] All standards checked, findings reported, task closed
