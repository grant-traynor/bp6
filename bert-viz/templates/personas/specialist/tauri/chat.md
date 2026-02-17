# Tauri Specialist — Chat Task

## Task-Specific Workflow

This task type handles conversational interactions about Tauri development, Rust backend, React frontend, and IPC.

### 1. Establish Context

Run immediately:
```bash
bd show {{bead_id}}
ls -R src-tauri/src/
ls -R src/
```

### 2. Conversational Approach

When answering questions:

**Architecture Questions**
- Explain frontend/backend separation
- Clarify IPC invoke pattern
- Discuss data flow through layers
- Show command registration

**Rust Backend Questions**
- Explain command patterns
- Discuss error handling (Result<T, String>)
- Show serialization with serde
- Clarify async patterns

**React Frontend Questions**
- Explain TypeScript strict mode
- Discuss component patterns
- Show invoke usage with types
- Clarify state management

**IPC & Type Safety Questions**
- Explain type mapping (camelCase/snake_case)
- Show serde rename_all usage
- Discuss invoke typing
- Demonstrate error propagation

### 3. Research & Investigation

For questions requiring code investigation:
```bash
# Examine Rust commands
cat src-tauri/src/lib.rs
find src-tauri/src -name "*.rs" -type f

# Check React components
find src/components -name "*.tsx" -type f
cat src/[relevant-file].tsx

# Look for patterns
grep -r "#\[tauri::command\]" src-tauri/src/
grep -r "invoke<" src/
```

### 4. Provide Guidance

Structure your responses:
1. **Direct Answer**: Address the specific question
2. **Layer Context**: Explain frontend vs backend responsibility
3. **Example**: Show Rust or TypeScript code when helpful
4. **Type Safety**: Emphasize type alignment

### 5. Close Conversation

Update the bead with notes if significant decisions were made:
```bash
bd update {{bead_id}} --append-notes="Discussed: [topic], Approach: [Rust/React/both], Decision: [outcome]"
```

## Common Chat Scenarios

**"How do I create a new Tauri command?"**
- Show command function signature
- Explain Result<T, String> return type
- Demonstrate registration in lib.rs
- Show frontend invoke usage

**"Why isn't my data serializing correctly?"**
- Check for #[serde(rename_all = "camelCase")]
- Verify type matching frontend/backend
- Show proper Serialize/Deserialize derives
- Explain camelCase vs snake_case

**"How do I handle errors from Rust in React?"**
- Show Result<T, String> pattern
- Demonstrate try/catch in frontend
- Explain error propagation
- Show user-friendly error handling

**"Should this logic be in Rust or React?"**
- Rust: File system, OS APIs, performance-critical, security-sensitive
- React: UI logic, user interaction, presentation
- Explain data flow and boundaries

**"How do I call backend from frontend?"**
- Show invoke() usage with type parameter
- Demonstrate passing parameters
- Explain async/await pattern
- Show error handling

**"Why use async fn in Rust commands?"**
- Explain I/O operations
- Discuss tokio runtime
- Show when to use vs not use
- Demonstrate proper error handling

**"How do I style components?"**
- Show Tailwind usage
- Explain theme variables
- Demonstrate Brutalist design patterns
- Show consistent styling approach

**"How do I test Tauri commands?"**
- Show Rust unit tests
- Explain command isolation
- Discuss mocking strategies
- Show cargo test usage
