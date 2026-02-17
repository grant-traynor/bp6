# Tauri Specialist — Implement Task

## Task-Specific Workflow

This task type focuses on implementing Tauri features that may involve frontend, backend, or both.

### 1. Establish Context

Run immediately:
```bash
bd show {{bead_id}}
bd list --status open --parent {{bead_id}}
cargo check
ls -R src-tauri/src/
ls -R src/
```

Review:
- Feature description and requirements
- UI mockups or design notes
- Data flow requirements
- Existing patterns in codebase

### 2. Determine Scope

Identify what needs to be built:

**Backend Only** (Rust)
- File system operations
- OS API interactions
- Data processing
- Security-sensitive logic

**Frontend Only** (React/TypeScript)
- UI components
- User interactions
- Presentation logic
- Styling

**Full-Stack** (Both)
- New commands with UI
- Data persistence with display
- Complex workflows

### 3. Plan Implementation

Before writing code:
- List Rust commands needed (if any)
- Define data structures and types
- Plan React components needed (if any)
- Design type mapping (camelCase alignment)
- Identify testing approach

### 4. Mark Bead In Progress

```bash
bd update {{bead_id}} --status in_progress
```

### 5. Backend Implementation (if needed)

**Phase 1: Define Types**
```rust
// In src-tauri/src/[module]/types.rs
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]  // CRITICAL for frontend compatibility
pub struct MyData {
    pub id: String,
    pub name: String,
    // ...
}
```

**Phase 2: Implement Commands**
```rust
// In src-tauri/src/[module]/commands.rs
#[tauri::command]
pub async fn my_command(param: String) -> Result<MyData, String> {
    // Implementation
    // Use .map_err(|e| e.to_string()) for error conversion
}
```

**Phase 3: Register Commands**
```rust
// In src-tauri/src/lib.rs
.invoke_handler(tauri::generate_handler![
    my_module::commands::my_command,
    // ...
])
```

**Phase 4: Test Backend**
```bash
cargo check
cargo test
cargo clippy
```

### 6. Frontend Implementation (if needed)

**Phase 1: Define Types**
```typescript
// Match Rust types (use camelCase)
interface MyData {
  id: string;
  name: string;
  // ...
}
```

**Phase 2: Create Components**
```tsx
interface MyComponentProps {
  onUpdate: (data: MyData) => void;
}

export function MyComponent({ onUpdate }: MyComponentProps) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Component logic
}
```

**Phase 3: Invoke Commands**
```typescript
import { invoke } from "@tauri-apps/api/core";

const data = await invoke<MyData>("my_command", { param: "value" });
```

**Phase 4: Test Frontend**
```bash
npm run build
# or
tsc
```

### 7. Integration Testing

Test full flow:
```bash
# Run Tauri app in dev mode
npm run tauri dev
```

Verify:
- Commands execute correctly
- Data serializes properly
- Errors are handled gracefully
- UI updates as expected

### 8. Code Quality Checks

**Backend Checks:**
```bash
cargo check
cargo test
cargo clippy
cargo fmt -- --check
```

**Frontend Checks:**
```bash
npm run build  # or tsc
npm run lint   # if configured
```

### 9. Update Bead

Document what was done:
```bash
bd update {{bead_id}} --notes="[Implementation summary: commands created, components built, data flow]"
bd update {{bead_id}} --design="[Architecture decisions: why Rust vs React for each piece, type design]"
```

### 10. Close Bead

```bash
bd close {{bead_id}} --reason="[What was implemented, how it meets requirements]"
```

## Implementation Checklist

**Backend (if applicable):**
- [ ] Commands use `#[tauri::command]` attribute
- [ ] Return type is `Result<T, String>`
- [ ] Types use `#[serde(rename_all = "camelCase")]`
- [ ] Async operations use `async fn`
- [ ] No `unwrap()` or `expect()` in production code
- [ ] Proper error messages (descriptive)
- [ ] Commands registered in lib.rs
- [ ] `cargo check` passes
- [ ] `cargo clippy` passes
- [ ] Tests pass

**Frontend (if applicable):**
- [ ] No TypeScript `any` types
- [ ] All components have proper prop interfaces
- [ ] invoke<T> calls are properly typed
- [ ] Error handling with try/catch
- [ ] Loading states for async operations
- [ ] Tailwind uses theme variables
- [ ] Build passes (tsc or npm run build)
- [ ] Types match Rust types (camelCase)

**Integration:**
- [ ] Data flows correctly backend → frontend
- [ ] Types align (camelCase serialization works)
- [ ] Error propagation works end-to-end
- [ ] Tested in dev mode
