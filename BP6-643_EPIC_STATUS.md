# Epic bp6-643: Concurrent Multi-Agent Session Management - Status Report

## Overview
Implementation of multi-session agent system to enable multiple concurrent agent processes with UI indicators, session management, and conversation logging.

**CRITICAL UPDATE (2026-02-13):** Epic 6dk (Plugin Architecture Refactor) completed, requiring redesign of bp6-643.001. Implementation guide updated to v2.0.

## Current Status: In Progress (Redesign Phase Complete)

### Completed Work

#### Epic 6dk Impact Assessment (2026-02-13)
**Status:** Complete
**Outcome:** Implementation guide v1.0 is DEPRECATED and incompatible with new plugin architecture

**Key Findings:**
- ✅ Plugin architecture **improves** multi-session design (BackendRegistry pattern)
- ⚠️ Old implementation doc targets flat `agent.rs` (now modular structure)
- 🎯 Need to align with: BackendId, PersonaPlugin, TemplateLoader, modular files
- 💡 Can leverage BackendRegistry as blueprint for SessionRegistry

**Actions Taken:**
1. Created comprehensive impact assessment
2. Archived v1.0 doc: `archive/MULTI_SESSION_IMPLEMENTATION_V1_DEPRECATED.md`
3. Created v2.0 implementation guide: `MULTI_SESSION_IMPLEMENTATION_V2.md`
4. Updated epic design and notes
5. Updated bp6-643.001 feature design

#### bp6-643.001: Backend Multi-Session Management (v2.0)
**Status:** Design Complete, Ready for Implementation
**Location:** `/MULTI_SESSION_IMPLEMENTATION_V2.md`

**Design Changes from v1.0:**
- **File Structure:** Modular (agent/session.rs, agent/plugin.rs) not flat agent.rs
- **Backend Enum:** BackendId + BackendRegistry (not CliBackend)
- **API Preservation:** Keep run_cli_command, add wrapper (not replace)
- **Session Tracking:** Build on existing current_session_id field
- **Registry Pattern:** Mirror BackendRegistry for SessionRegistry

**Implementation Plan (11 Steps):**
1. ✅ Update AgentChunk in agent/plugin.rs (add session_id field)
2. ✅ Define SessionState, SessionStatus, SessionInfo structs
3. ✅ Refactor AgentState (sessions HashMap)
4. ✅ Create run_cli_command_for_session wrapper
5. ✅ Add helper functions (emit_session_list_changed, etc.)
6. ✅ Refactor start_agent_session (return session_id)
7. ✅ Refactor send_agent_message (add session_id param)
8. ✅ Refactor stop_agent_session (add session_id param)
9. ✅ Add 4 new Tauri commands
10. ✅ Update agent/mod.rs exports
11. ✅ Register commands in lib.rs

**Technical Highlights:**
- UUID v4 session IDs (internal, distinct from CLI backend session IDs)
- Thread-safe HashMap<String, SessionState> with Mutex
- Preserved existing APIs for backwards compatibility
- Event emission: session-created, session-terminated, session-list-changed, active-session-changed
- AgentChunk includes optional session_id for UI routing

**Dependencies:** ✅ All installed (uuid v1.11+)

**Next Steps:**
1. Apply v2.0 changes sequentially (11 steps)
2. Test compilation: `cd bert-viz/src-tauri && cargo build`
3. Test multi-session creation and switching
4. Verify event emission
5. Proceed to bp6-643.002 (Logging)

---

### Remaining Features

#### bp6-643.002: Conversation Logging System
**Status:** Ready to Start (after bp6-643.001 v2.0 implemented)
**Priority:** P1
**Dependencies:** bp6-643.001

**Scope:**
- Implement JSONL logging to .bp6/sessions/<bead-id>/<session-id>-<timestamp>.jsonl
- Create logger.rs module with SessionLogger struct
- Hook into agent message flow
- Append to log file on each message chunk
- Close and finalize log on session termination

**Files to Create:**
- bert-viz/src-tauri/src/agent/logger.rs

**Files to Modify:**
- bert-viz/src-tauri/src/agent/session.rs (add logging hooks)
- bert-viz/src-tauri/src/agent/mod.rs (declare logger module)
- bert-viz/src-tauri/src/lib.rs (if needed for logger init)
- .gitignore (add .bp6/ entry)

**Alignment with v2.0:**
- Logger receives session_id from AgentChunk
- Uses SessionInfo for metadata
- Integrates with run_cli_command_for_session threads

---

#### bp6-643.003: WBS Tree UI Indicators
**Status:** Ready to Start (after bp6-643.001 v2.0 implemented)
**Priority:** P1
**Dependencies:** bp6-643.001

**Scope:**
- Add visual indicators to WBS tree showing active sessions
- Pulsing animation with persona icons (📋 PM, 🧪 QA, ⚡ Specialist)
- Subscribe to session-list-changed events
- Count badge for multiple sessions on same bead

**Files to Modify:**
- bert-viz/src/App.tsx (session state management)
- bert-viz/src/components/WbsNode.tsx (indicator rendering)
- bert-viz/src/index.css (@keyframes pulse animation)
- bert-viz/src/api.ts (session event listeners)

**Integration Notes:**
- Listen for session-list-changed events (payload: Vec<SessionInfo>)
- Map session_id to bead_id for tree node lookup
- Display persona icon based on SessionInfo.persona field
- Show count badge if sessions.filter(s => s.bead_id === beadId).length > 1

---

#### bp6-643.004: Session Selection & Switching UI
**Status:** Ready to Start (after bp6-643.001 v2.0 implemented)
**Priority:** P1
**Dependencies:** bp6-643.001

**Scope:**
- SessionList component in ChatDialog
- Session switching functionality
- Load conversation history
- Terminate session controls

**Files to Create:**
- bert-viz/src/components/SessionList.tsx
- bert-viz/src/components/SessionItem.tsx

**Files to Modify:**
- bert-viz/src/components/ChatDialog.tsx (integrate session list)
- bert-viz/src/api.ts (switchSession, terminateSession functions)
- bert-viz/src/index.css (session list styles)

**Integration Notes:**
- Call listActiveSessions() on component mount
- Subscribe to session-list-changed for updates
- Use switchActiveSession(sessionId) for switching
- Pass session_id to sendAgentMessage for targeted messaging

---

#### bp6-643.005: Multi-Window Chat Support
**Status:** Not Started
**Priority:** P2 (Optional enhancement)

**Scope:**
- Enable opening multiple chat windows via Tauri
- Independent window per session
- Window management

**Decision:** Defer until P1 features complete

---

## Implementation Strategy

### Recommended Execution Order:
1. **✅ PHASE 0: Impact Assessment & Redesign** (COMPLETE)
   - Assess epic 6dk impact
   - Create v2.0 implementation guide
   - Update epic and feature designs

2. **PHASE 1: Backend Implementation** (NEXT)
   - Apply bp6-643.001 v2.0 changes (11 steps)
   - Test compilation and basic functionality
   - Verify event emission

3. **PHASE 2: Logging** (After Phase 1)
   - Implement bp6-643.002
   - Test log file creation and appending

4. **PHASE 3: UI Implementation** (After Phase 2)
   - Implement bp6-643.003 & bp6-643.004 in parallel
   - Tree indicators (bp6-643.003)
   - Session list UI (bp6-643.004)

5. **PHASE 4: Optional Enhancements** (If needed)
   - Implement bp6-643.005 (multi-window support)

---

## Testing Checklist

### Backend (bp6-643.001):
- [ ] Code compiles without errors or warnings
- [ ] Multiple sessions can be created concurrently
- [ ] Sessions are tracked with unique UUIDs
- [ ] Active session can be switched
- [ ] Individual sessions can be terminated
- [ ] All sessions can be listed
- [ ] Events are emitted correctly (session-created, session-terminated, session-list-changed, active-session-changed)
- [ ] Agent messages route to correct session
- [ ] AgentChunk includes session_id in payload
- [ ] Session cleanup works properly on termination

### Logging (bp6-643.002):
- [ ] Log files created in .bp6/sessions/<bead-id>/<session-id>-<timestamp>.jsonl
- [ ] JSONL format is valid (one JSON object per line)
- [ ] Messages appended in real-time
- [ ] Log file closed on session termination
- [ ] .bp6/ directory added to .gitignore

### UI (bp6-643.003 & bp6-643.004):
- [ ] Tree nodes show session indicators
- [ ] Pulsing animation works
- [ ] Persona icons display correctly
- [ ] Count badge appears for multiple sessions
- [ ] Session list loads on chat open
- [ ] Session switching works
- [ ] Terminate session button functions
- [ ] UI updates on session-list-changed events

---

## Technical Architecture

### Session Management Flow (v2.0):
```
User → startAgentSession → Generate UUID → Spawn Child Process → Store in HashMap
                                          ↓
                                   Emit session-created
                                          ↓
                                   Set as active_session_id
                                          ↓
                                   Emit session-list-changed
                                          ↓
                                   Return session_id to frontend

User → sendAgentMessage → Get active_session_id (or use provided)
                                          ↓
                                   Get SessionState from HashMap
                                          ↓
                                   Spawn new process (resume=true)
                                          ↓
                                   Replace process in SessionState
                                          ↓
                                   Emit agent-chunk with session_id

User → switchActiveSession → Verify session exists
                                          ↓
                                   Update active_session_id
                                          ↓
                                   Emit active-session-changed

User → terminateSession → Remove from HashMap
                                          ↓
                                   Kill process group
                                          ↓
                                   Clear active_session_id if matches
                                          ↓
                                   Emit session-terminated
                                          ↓
                                   Emit session-list-changed
```

### Event System:
- **session-created**: Payload is session_id (String)
- **session-terminated**: Payload is session_id (String)
- **session-list-changed**: Payload is Vec<SessionInfo>
- **active-session-changed**: Payload is session_id (String)
- **agent-chunk**: Now includes optional session_id field (String)

### Storage Structure (for bp6-643.002):
```
.bp6/
└── sessions/
    └── <bead-id>/
        └── <session-id>-<timestamp>.jsonl
```

### Data Structures (v2.0):
```rust
// In agent/plugin.rs
pub struct AgentChunk {
    pub content: String,
    pub is_done: bool,
    pub session_id: Option<String>, // NEW
}

// In agent/session.rs
pub enum SessionStatus {
    Running,
    Paused,
}

pub struct SessionState {
    pub process: Child,
    pub bead_id: String,
    pub persona: String,
    pub backend_id: BackendId,
    pub status: SessionStatus,
    pub created_at: SystemTime,
    pub cli_session_id: Option<String>,
}

pub struct SessionInfo {
    pub session_id: String,
    pub bead_id: String,
    pub persona: String,
    pub backend_id: String,
    pub status: SessionStatus,
    pub created_at: u64,
}

pub struct AgentState {
    pub sessions: Mutex<HashMap<String, SessionState>>,
    pub active_session_id: Mutex<Option<String>>,
    pub backend_registry: BackendRegistry,
    pub persona_registry: PersonaRegistry,
    pub template_loader: TemplateLoader,
}
```

---

## Timeline Estimate (Updated)

- ✅ Phase 0 (Redesign): 2 hours (COMPLETE)
- Phase 1 (bp6-643.001 v2.0): 2-3 hours (apply + test)
- Phase 2 (bp6-643.002): 2-3 hours (logging system)
- Phase 3 (bp6-643.003): 2-3 hours (tree indicators)
- Phase 3 (bp6-643.004): 3-4 hours (session list UI)
- Phase 4 (bp6-643.005): 4-6 hours (multi-window, optional)

**Total**: 13.5-18.5 hours for complete epic (including redesign)

---

## Current Blockers

~~1. File locking preventing backend changes from persisting~~ (Resolved: v2.0 guide created)
~~2. Implementation guide v1.0 incompatible with plugin architecture~~ (Resolved: v2.0 complete)

**No current blockers.** Ready to proceed with implementation.

---

## Documentation

- **Impact Assessment**: This document (sections above)
- **Implementation Guide v2.0**: `/MULTI_SESSION_IMPLEMENTATION_V2.md` ✅
- **Implementation Guide v1.0 (DEPRECATED)**: `/archive/MULTI_SESSION_IMPLEMENTATION_V1_DEPRECATED.md` ⚠️ DO NOT USE
- **Epic Design**: `bd show bp6-643` (updated 2026-02-13)
- **Feature Design**: `bd show bp6-643.001` (updated to v2.0)

---

## Next Session Recommendations

### Option A: Implement v2.0 Backend (Recommended)
1. Review `MULTI_SESSION_IMPLEMENTATION_V2.md`
2. Apply changes step-by-step (11 steps)
3. Test compilation after each major change
4. Verify events emit correctly
5. Test multi-session creation manually
6. Proceed to bp6-643.002 (logging)

### Option B: Parallel Development
1. Start bp6-643.002 (logging) in separate branch
2. Implement backend (bp6-643.001) in main
3. Merge after both complete
4. Proceed to UI features

### Option C: Full Stack Iteration
1. Implement minimal bp6-643.001 (basic multi-session)
2. Implement minimal bp6-643.003 (tree indicators)
3. Test end-to-end
4. Enhance with bp6-643.002 (logging) and bp6-643.004 (session UI)

**Recommendation:** Option A - complete backend first, then layer on features

---

## Git Workflow

### Before Starting Implementation:
```bash
git status                    # Check clean state
bd sync                       # Sync beads changes
git checkout -b feature/multi-session-v2
```

### During Implementation:
```bash
# After each step compiles successfully
git add <modified-files>
git commit -m "feat(agent): step N - <description>"
```

### After Feature Complete:
```bash
bd close bp6-643.001          # Close feature
bd sync                       # Sync bead changes
git push origin feature/multi-session-v2
# Create PR for review
```

---

## Notes

This epic represents a significant architectural change from single-process to multi-process agent management. Epic 6dk (plugin architecture) was completed 2026-02-13, requiring redesign of bp6-643.001 to align with modular structure, BackendId pattern, and PersonaPlugin system.

**v2.0 design is complete and ready for implementation.** All architectural conflicts with epic 6dk have been resolved. Implementation guide provides step-by-step instructions aligned with current codebase structure.

---

*Last Updated: 2026-02-13*
*Epic Owner: Grant Traynor*
*Status: In Progress - Phase 0 (Redesign) complete, Phase 1 (Implementation) ready to start*
