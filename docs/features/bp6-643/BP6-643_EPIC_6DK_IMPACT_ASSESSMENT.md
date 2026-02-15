# Impact Assessment: Epic 6dk on bp6-643

**Date:** 2026-02-13
**Epic:** bp6-643 - Concurrent Multi-Agent Session Management
**Completed Epic:** 6dk - Agent.rs Plugin Architecture Refactor
**Assessment By:** Product Manager AI

---

## Executive Summary

Epic 6dk introduced a plugin-based architecture that **fundamentally changes** how the agent system is structured. This impacts all 4 features in bp6-643, requiring design updates but **no scope changes**. The core functionality remains the same; the implementation approach has shifted.

**Impact Level:** 🟡 **MEDIUM** - Design changes required, no scope changes

---

## What Changed in Epic 6dk

### 1. **Modular File Structure**
- **Before:** Flat `agent.rs` file with all logic
- **After:** Modular structure:
  - `agent/mod.rs` - Module exports
  - `agent/session.rs` - Session management
  - `agent/plugin.rs` - Plugin trait definitions
  - `agent/backends/` - Backend implementations
  - `agent/personas/` - Persona implementations
  - `agent/registry.rs` - Backend/persona registries
  - `agent/templates.rs` - Template loading from markdown files

### 2. **Plugin Architecture**
- **CliBackendPlugin trait** - Common interface for backends
- **BackendRegistry** - Dynamic backend registration
- **PersonaPlugin trait** - Common interface for personas
- **PersonaRegistry** - Dynamic persona registration
- **TemplateLoader** - Loads templates from `.agent/templates/personas/*.md`

### 3. **Type System Changes**
- `BackendId` enum replaces `CliBackend` enum
- `AgentChunk` moved to `agent/plugin.rs`
- `PersonaType` enum for persona identification

### 4. **AgentState Refactoring**
- Added `backend_registry: BackendRegistry`
- Added `persona_registry: PersonaRegistry`
- Added `template_loader: TemplateLoader`
- Kept `current_process: Mutex<Option<Child>>`
- Kept `current_backend: Mutex<BackendId>`
- Kept `current_session_id: Arc<Mutex<Option<String>>>`

---

## Impact on bp6-643 Features

### ✅ bp6-643.001 - Backend Multi-Session Management
**Status:** IN_PROGRESS
**Impact:** 🟢 **LOW** - Already updated to v2.0

**Changes Made:**
- Implementation guide updated to MULTI_SESSION_IMPLEMENTATION_V2.md
- Design aligned with plugin architecture
- Uses `BackendId` instead of `CliBackend`
- Uses modular file structure (`agent/session.rs`)
- Preserves existing `run_cli_command` API
- Adds `run_cli_command_for_session` wrapper

**Action Required:** ✅ None - Already aligned with 6dk

---

### 🔴 bp6-643.002 - Conversation Logging System
**Status:** OPEN
**Impact:** 🟡 **MEDIUM** - Design update needed

**Current Design Issues:**
1. References old flat `agent.rs` structure
2. No mention of how to integrate with `AgentChunk` in `plugin.rs`
3. Storage location defined but implementation approach unclear

**Required Design Updates:**
1. **Session ID Source:** Use UUID from bp6-643.001's SessionState
2. **Integration Point:** Hook into `run_cli_command_for_session` streaming loop
3. **Data Structure:** Log `AgentChunk` objects with session metadata
4. **File Location:** Logging code should live in `agent/session.rs`
5. **Template Context:** Include persona template (from TemplateLoader) in session header

**Proposed Design Changes:**
```
Storage: ~/.bp6/sessions/<bead-id>/<session-id>-<timestamp>.jsonl

Log Entry Format:
{
  "timestamp": "2026-02-13T10:30:00Z",
  "session_id": "uuid-v4",
  "bead_id": "bp6-643",
  "persona": "product_manager",
  "backend": "gemini",
  "event_type": "message" | "chunk" | "session_start" | "session_end",
  "content": "...",
  "metadata": { ... }
}

Implementation Location: agent/session.rs
- Add SessionLogger struct
- Hook into run_cli_command_for_session
- Log all AgentChunk events
- Include template content in session_start event
```

**Action Required:** 🔧 Update design field with integration details

---

### 🔴 bp6-643.003 - WBS Tree UI Indicators
**Status:** OPEN
**Impact:** 🟢 **LOW** - Minor clarification needed

**Current Design Issues:**
1. References `SessionInfo` structure not yet defined in exports
2. No consideration of persona types from PersonaRegistry

**Required Design Updates:**
1. **SessionInfo Import:** Ensure `SessionInfo` is exported from `agent/mod.rs`
2. **Persona Icons:** Use `PersonaType` enum for icon mapping
3. **Event Payload:** Clarify that `session-list-changed` event includes full `Vec<SessionInfo>`

**Proposed Design Additions:**
```typescript
// Persona icon mapping aligned with PersonaType enum
const PERSONA_ICONS: Record<string, string> = {
  'product_manager': '📋',
  'qa_engineer': '🧪',
  'specialist': '⚡',
  // Future: 'architect', 'security', etc.
};

// SessionInfo type (matches Rust SessionInfo from bp6-643.001)
interface SessionInfo {
  session_id: string;
  bead_id: string;
  persona: string;
  backend_id: string;
  status: 'running' | 'paused';
  created_at: number; // Unix timestamp
}
```

**Action Required:** 🔧 Add SessionInfo type definition and PersonaType mapping

---

### 🔴 bp6-643.004 - Session Selection & Switching UI
**Status:** OPEN
**Impact:** 🟢 **LOW** - Minor clarification needed

**Current Design Issues:**
1. References session APIs not yet defined
2. No mention of how to handle persona template display
3. Conversation history loading details vague

**Required Design Updates:**
1. **Session APIs:** Document exact Tauri command signatures
2. **Template Display:** Show persona name (not raw template) in session list
3. **History Loading:** Read from `~/.bp6/sessions/` JSONL files (bp6-643.002)
4. **Backend Display:** Show backend name (Gemini/Claude Code) in session item

**Proposed Design Additions:**
```typescript
// Tauri command signatures (from bp6-643.001)
await invoke('list_active_sessions'): Promise<SessionInfo[]>
await invoke('switch_active_session', { sessionId: string }): Promise<void>
await invoke('get_active_session_id'): Promise<string | null>
await invoke('terminate_session', { sessionId: string }): Promise<void>

// Session display format
Session Item:
┌─────────────────────────┐
│ 📋 PM · bp6-643         │ <- Persona icon + bead title
│ Gemini · 5m 32s         │ <- Backend + elapsed time
│ [Running]               │ <- Status
└─────────────────────────┘

// Load conversation history on switch
async function loadSessionHistory(sessionId: string, beadId: string) {
  const logFiles = await readDir(`~/.bp6/sessions/${beadId}/`);
  const sessionLogs = logFiles.filter(f => f.startsWith(sessionId));
  // Parse JSONL and reconstruct conversation
}
```

**Action Required:** 🔧 Add API signatures and history loading details

---

## Risk Assessment

### 🟢 Low Risk
- **Architectural Alignment:** v2.0 design already aligned with 6dk
- **No Breaking Changes:** Plugin architecture is additive, preserves existing APIs
- **Clear Migration Path:** Each feature has clear integration points

### 🟡 Medium Risk
- **Incomplete Exports:** Need to ensure `SessionInfo`, `SessionState` exported from `agent/mod.rs`
- **Event Payload Schema:** Need to define exact JSON structure for Tauri events
- **Logging Integration:** Need to ensure minimal performance impact from JSONL writes

### 🔴 High Risk
- **None identified**

---

## Recommended Actions

### Immediate (Before Implementation)
1. ✅ **bp6-643.001:** Already aligned - proceed with implementation
2. 🔧 **bp6-643.002:** Update design with SessionLogger integration details
3. 🔧 **bp6-643.003:** Add SessionInfo TypeScript type and PersonaType mapping
4. 🔧 **bp6-643.004:** Add Tauri command signatures and history loading spec

### During Implementation
1. Export `SessionInfo` and `SessionState` from `agent/mod.rs`
2. Define Tauri event payload schemas in documentation
3. Add performance monitoring for JSONL logging (bp6-643.002)
4. Test multi-session with different backend/persona combinations

### Post-Implementation
1. Update MULTI_SESSION_IMPLEMENTATION_V2.md with final implementation notes
2. Document any deviations from original design
3. Create migration guide if any breaking changes introduced

---

## Dependencies & Blockers

### Unblocked
- ✅ bp6-643.001 can proceed immediately (v2.0 design complete)

### Sequential Dependencies
- bp6-643.002 depends on bp6-643.001 (needs SessionState for logging)
- bp6-643.003 depends on bp6-643.001 (needs session-list-changed event)
- bp6-643.004 depends on bp6-643.001 + bp6-643.002 (needs session APIs + history logs)

### Recommended Implementation Order
1. **bp6-643.001** - Core session management (foundation)
2. **bp6-643.002** - Logging (enables history)
3. **bp6-643.003** + **bp6-643.004** in parallel (both depend on 001+002)

---

## Conclusion

Epic 6dk's plugin architecture **improves** the implementation path for bp6-643 by:
- ✅ Providing cleaner modular structure
- ✅ Enabling easier testing (isolated backends/personas)
- ✅ Supporting future extensibility (new backends/personas)

**No scope changes required.** All features remain valid; only implementation details need updates.

**Next Steps:**
1. Update bp6-643.002, .003, .004 design fields with integration details
2. Proceed with bp6-643.001 implementation using v2.0 guide
3. Monitor for any additional alignment issues during implementation
