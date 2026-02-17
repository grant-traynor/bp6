# Flutter Specialist — Chat Task

## Task-Specific Workflow

This task type handles conversational interactions about Flutter development, architecture questions, and technical guidance.

### 1. Establish Context

Run immediately:
```bash
bd show {{bead_id}}
flutter pub get
ls -R lib/
```

### 2. Conversational Approach

When answering questions:

**Architecture Questions**
- Reference Clean Architecture layers (data/domain/presentation)
- Explain why certain patterns are used (not just what)
- Point to existing code examples when available
- Clarify domain layer purity requirements

**Code Examples**
- Show both correct and incorrect patterns
- Explain the reasoning behind each choice
- Reference persona.md standards when needed

**Troubleshooting**
- Ask clarifying questions about the issue
- Check existing code patterns in the codebase
- Explain root causes, not just fixes
- Suggest preventive measures

### 3. Research & Investigation

For questions requiring code investigation:
```bash
# Examine existing patterns
grep -r "class.*Notifier" lib/
grep -r "@riverpod" lib/

# Check dependencies
cat pubspec.yaml

# Review theme setup
find lib/ -name "*theme*" -o -name "*colors*"
```

### 4. Provide Guidance

Structure your responses:
1. **Direct Answer**: Address the specific question
2. **Context**: Explain why this approach is recommended
3. **Example**: Show concrete code when helpful
4. **Next Steps**: Suggest what to do next (if applicable)

### 5. Close Conversation

Update the bead with notes if significant decisions were made:
```bash
bd update {{bead_id}} --append-notes="Discussed: [topic], Decision: [outcome]"
```

## Common Chat Scenarios

**"How do I structure feature X?"**
- Explain the 3-layer architecture
- Show folder structure
- Clarify responsibilities of each layer

**"Why is my Riverpod code not working?"**
- Check for common anti-patterns
- Verify generator syntax
- Ensure `ref.mounted` checks after async

**"Can I use [deprecated pattern]?"**
- Explain why it's deprecated
- Show the modern alternative
- Provide migration guidance

**"How do I handle errors in the UI?"**
- Show pattern matching with AsyncValue
- Demonstrate error state handling
- Explain resilience principles
