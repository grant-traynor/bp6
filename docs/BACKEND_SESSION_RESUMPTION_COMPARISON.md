# Backend Session Resumption: Claude Code vs Gemini CLI

**Investigation Date:** 2026-02-15
**Task:** bp6-643.7.1
**Status:** ✅ Both Supported

## Executive Summary

**Both Claude Code and Gemini CLI fully support session resumption** with different but complementary approaches:

| Feature | Claude Code | Gemini CLI |
|---------|-------------|------------|
| **Session Creation** | Explicit UUID required | Auto-generated, indexed |
| **Resume Method** | `--resume <uuid>` | `--resume <index>` or `latest` |
| **Headless Mode** | `--print` flag | `--prompt` flag |
| **State Preservation** | ✅ Full | ✅ Full |
| **Session ID Control** | ✅ User-specified | ❌ Auto-generated only |
| **Session Picker** | ✅ Built-in | ✅ Built-in |
| **Best For** | Multi-session orchestration | Single-user workflows |

---

## Claude Code Session Management

### Session Creation with Explicit ID

```bash
# Generate UUID
SESSION_ID=$(uuidgen | tr '[:upper:]' '[:lower:]')

# Create session with explicit ID
claude --print --session-id $SESSION_ID "Read README.md"
```

**Key Characteristics:**
- ✅ **User controls session ID** (UUID v4 required)
- ✅ **Predictable resumption** (use same UUID)
- ✅ **Multi-session orchestration** (manage multiple UUIDs)
- ⚠️ **UUID format strict** (must be valid v4)

### Session Resume

```bash
# Resume with explicit UUID
claude --print --resume 85633ae2-f7d1-4163-b156-6b9c0c4e0d4f "Next command"

# Resume with picker (no UUID)
claude --resume  # Interactive fuzzy search
```

### Headless→Interactive Transition

```bash
# Headless phase
claude --print --session-id $SESSION "Command 1"
claude --print --resume $SESSION "Command 2"

# Interactive takeover
claude --resume $SESSION  # Enters interactive mode
```

**Flags:**
- `--session-id <uuid>` - Create/use specific session
- `--resume <uuid>` - Resume by UUID
- `--continue` - Resume most recent in directory
- `--fork-session` - Branch session
- `--print` - Headless mode (exit after response)

---

## Gemini CLI Session Management

### Auto-Generated Sessions

```bash
# Create session (auto-generates ID)
gemini --prompt "Read README.md"

# List sessions
gemini --list-sessions
# Output:
#   1. Read README.md and tell me... (Just now) [7f30af9b-...]
#   2. Another session (5 minutes ago) [f8b811b0-...]
```

**Key Characteristics:**
- ✅ **Zero config** (auto-generates UUIDs)
- ✅ **Human-readable list** (indexed by recency)
- ✅ **Session descriptions** (uses first prompt as title)
- ⚠️ **No explicit ID control** (can't pre-specify UUID)

### Session Resume

```bash
# Resume by index
gemini --resume 1 --prompt "What file did you read?"

# Resume latest
gemini --resume latest --prompt "Continue working"

# Resume by UUID (if known)
gemini --resume 7f30af9b-64ea-40df-b674-a48b2c619e8f --prompt "Next command"
```

**Resume Options:**
- `latest` - Most recent session
- `<number>` - Session by index (1, 2, 3...)
- `<uuid>` - Session by full UUID

### Headless→Interactive Transition

```bash
# Headless phase
gemini --prompt "Command 1"  # Creates session, auto-assigned index
gemini --resume latest --prompt "Command 2"

# Interactive takeover
gemini --resume latest  # Enters interactive mode (no --prompt)
```

**Flags:**
- `--prompt <text>` - Headless mode (non-interactive)
- `--resume <index|latest|uuid>` - Resume session
- `--prompt-interactive` - Execute prompt then enter interactive
- `--list-sessions` - Show all sessions for project
- `--delete-session <index>` - Clean up old sessions

---

## State Preservation Comparison

### Both CLIs Preserve:
✅ **Conversation History** - Full message thread
✅ **Tool Outputs** - File reads, searches, command results
✅ **Working Directory Context** - Tool access paths
✅ **System Prompts** - Agent configuration

### Testing Results

**Claude Code:**
```bash
SESSION=85633ae2-f7d1-4163-b156-6b9c0c4e0d4f
claude --print --session-id $SESSION "Read README.md"
# Output: "The README.md contains a test document..."

claude --print --resume $SESSION "What file did you read?"
# Output: "I just read the README.md file located at /private/tmp/..."
```
**Result:** ✅ PASS - Full context preserved

**Gemini CLI:**
```bash
gemini --prompt "Read README.md and tell me what it says"
# Session auto-created with index 1

gemini --resume latest --prompt "What file did you just read?"
# Output: "I just read the `README.md` file."
```
**Result:** ✅ PASS - Full context preserved

---

## Architecture Differences

### Claude Code: UUID-First Design

```
User Workflow:
1. Generate UUID: uuidgen
2. Start session: --session-id <uuid>
3. Resume session: --resume <uuid>
4. Manage multiple UUIDs in parallel
```

**Pros:**
- ✅ Explicit session IDs for programmatic control
- ✅ Multi-session orchestration (multiple UUIDs tracked)
- ✅ Predictable resume (same UUID always)
- ✅ Session forking/branching

**Cons:**
- ⚠️ UUID generation required
- ⚠️ UUIDs harder for humans to track
- ⚠️ Strict format enforcement

**Best For:**
- Automated workflows
- Multi-agent systems
- Programmatic session management
- **bp6-643.7 use case: Perfect fit**

---

### Gemini CLI: Index-First Design

```
User Workflow:
1. Start session: gemini --prompt "..."
2. List sessions: gemini --list-sessions
3. Resume by index: --resume 1 (or "latest")
4. Human-readable session titles
```

**Pros:**
- ✅ Zero configuration (auto-generates IDs)
- ✅ Human-readable session list
- ✅ Simple resume by index
- ✅ Session descriptions (first prompt)

**Cons:**
- ⚠️ No explicit ID control (can't pre-specify)
- ⚠️ Index changes as sessions are created/deleted
- ⚠️ Harder for programmatic multi-session tracking

**Best For:**
- Interactive user workflows
- Single-user development
- Quick prototyping
- Session browsing and discovery

---

## Integration Strategy for bp6-643.7

### Recommended Approach: Hybrid Design

**Use Claude Code for multi-session orchestration:**
- Explicit UUID control for backend tracking
- Predictable session resumption
- Multi-session management in `SessionState`

**Optionally support Gemini CLI:**
- Use Gemini's auto-generated UUIDs (extract from output)
- Track session index → UUID mapping
- Provide fallback for users without Claude Code

### Implementation Pattern

**Claude Code (Primary):**
```rust
// Generate session ID
let cli_session_id = Uuid::new_v4().to_string();

// Headless mode
Command::new("claude")
    .arg("--print")
    .arg("--session-id")
    .arg(&cli_session_id)
    .arg(&command)
    .spawn()?

// Resume
Command::new("claude")
    .arg("--print")
    .arg("--resume")
    .arg(&cli_session_id)
    .arg(&next_command)
    .spawn()?

// Interactive handover
Command::new("claude")
    .arg("--resume")
    .arg(&cli_session_id)
    // No --print = interactive
    .spawn()?
```

**Gemini CLI (Alternative):**
```rust
// Create session (auto-generates UUID)
let output = Command::new("gemini")
    .arg("--prompt")
    .arg(&command)
    .output()?;

// Extract UUID from --list-sessions (parse output)
let cli_session_id = extract_latest_session_uuid()?;

// Resume by UUID (or "latest")
Command::new("gemini")
    .arg("--resume")
    .arg(&cli_session_id) // or "latest"
    .arg("--prompt")
    .arg(&next_command)
    .spawn()?

// Interactive handover
Command::new("gemini")
    .arg("--resume")
    .arg(&cli_session_id) // or "latest"
    // No --prompt = interactive
    .spawn()?
```

---

## Comparison Matrix

| Capability | Claude Code | Gemini CLI |
|------------|-------------|------------|
| **Session ID Control** | ✅ User-specified UUID | ❌ Auto-generated only |
| **Resume by Index** | ❌ UUID only | ✅ Index or "latest" |
| **Resume by UUID** | ✅ Direct | ✅ Supported |
| **Session Picker** | ✅ Interactive fuzzy search | ✅ List with descriptions |
| **Session Descriptions** | ❌ UUID only | ✅ First prompt as title |
| **Headless Mode** | `--print` | `--prompt` |
| **Interactive Mode** | Default (no `--print`) | Default (no `--prompt`) |
| **State Preservation** | ✅ Full | ✅ Full |
| **Session Forking** | ✅ `--fork-session` | ❌ Not available |
| **Continue Recent** | ✅ `--continue` | ✅ `--resume latest` |
| **Multi-Session Orchestration** | ✅ Excellent (UUID tracking) | ⚠️ Limited (index unstable) |
| **User-Friendly Browsing** | ⚠️ UUID-based | ✅ Excellent (titles) |

---

## Recommendation for bp6-643.7

### Primary: Use Claude Code

**Rationale:**
- ✅ Explicit UUID control for multi-session tracking
- ✅ Predictable resumption across session manager restarts
- ✅ Easier to map `SessionState.cli_session_id` to backend
- ✅ Session forking for advanced features

### Optional: Support Gemini CLI

**If Gemini support is needed:**
1. Use `--list-sessions` to extract latest UUID after creation
2. Store UUID in `SessionState.cli_session_id`
3. Resume using UUID (not index, for stability)
4. Accept trade-off: no pre-specified session IDs

**Hybrid Approach:**
```rust
pub enum BackendId {
    ClaudeCode,
    Gemini,
}

impl SessionManager {
    async fn start_headless(
        &self,
        backend: BackendId,
        commands: Vec<String>,
    ) -> Result<SessionInfo> {
        let cli_session_id = match backend {
            BackendId::ClaudeCode => {
                // User-specified UUID
                let uuid = Uuid::new_v4().to_string();
                self.spawn_claude_with_id(&uuid, &commands[0])?;
                uuid
            }
            BackendId::Gemini => {
                // Auto-generated, extract after spawn
                self.spawn_gemini_and_extract_id(&commands[0])?
            }
        };

        // Store in SessionState for resume
        Ok(SessionInfo { cli_session_id, ... })
    }
}
```

---

## Testing Summary

### Claude Code
- ✅ Session creation with `--session-id`
- ✅ Multi-command resume with `--resume`
- ✅ Context preservation verified
- ✅ Headless→interactive transition tested

### Gemini CLI
- ✅ Auto-session creation with `--prompt`
- ✅ Resume by index and "latest"
- ✅ Context preservation verified
- ✅ Headless→interactive transition tested

**Both CLIs are production-ready for bp6-643.7.**

---

## Final Verdict

**Recommended Primary:** Claude Code
- Best for multi-session orchestration
- Explicit UUID control
- Predictable behavior

**Recommended Secondary:** Gemini CLI support
- Fallback for flexibility
- User preference option
- Leverage auto-generated sessions

**No workarounds needed** - both CLIs have native, robust session resumption capabilities suitable for bp6-643.7 requirements.

---

## Next Steps

1. ✅ Investigation complete (bp6-643.7.1)
2. → Implement `SessionState` extensions (bp6-643.7.2)
3. → Build headless launcher with backend abstraction (bp6-643.7.3)
4. → Add queue executor with backend-specific resume logic (bp6-643.7.4)
5. → Implement handover with mode detection (bp6-643.7.5)
6. → Polish UI with session indicators (bp6-643.7.6)
