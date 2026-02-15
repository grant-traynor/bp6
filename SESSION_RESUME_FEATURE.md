# Session Resume Feature (bp6-6nj.20)

## Overview

This feature allows users to automatically resume previous conversations when reopening a chat for the same bead/persona combination, even after restarting the app.

## How It Works

### 1. Session Index Persistence

A new file `~/.bp6/session_index.json` stores mappings of:
```
"{bead_id}-{persona}" → SessionMetadata
```

Where `SessionMetadata` contains:
- `session_id`: The app's internal session UUID
- `cli_session_id`: The CLI-provided session ID (for Gemini/Claude resume)
- `last_active`: Timestamp of last activity
- `backend_id`: Which backend was used (gemini, claude-code)

### 2. Automatic Resume Flow

When you open a chat dialog:

1. **Check for existing session**: `useAgentSession` hook calls `findRecentSession(beadId, persona)`
2. **If found**:
   - Loads conversation history from `~/.bp6/sessions/{bead-id}/{session-id}-{timestamp}.jsonl`
   - Attaches to the existing session (shows all previous messages)
   - Ready to continue the conversation
3. **If not found**:
   - Creates a new session as usual
   - Records it in the session index for future resumption

### 3. Session Tracking

- **On session start**: `recordSessionForResume()` saves the session to the index
- **On message send**: `touchSession()` updates the last_active timestamp
- **On CLI session ID capture**: Updates the index with Gemini/Claude's session ID

### 4. Conversation History

Conversation history is already persisted in JSONL files:
```
~/.bp6/sessions/
  ├── bp6-6nj/
  │   ├── session-uuid-1-timestamp.jsonl
  │   └── session-uuid-2-timestamp.jsonl
  ├── untracked/
  │   └── session-uuid-3-timestamp.jsonl
```

Each JSONL file contains:
- Session start/end events
- User messages
- Assistant responses (chunks)

## User Experience

### Before:
1. Start chat with Product Manager for bead-6nj
2. Have a conversation
3. Restart the app
4. Click chat again → **New empty conversation** ❌

### After:
1. Start chat with Product Manager for bead-6nj
2. Have a conversation
3. Restart the app
4. Click chat again → **Previous conversation loads automatically** ✅

## Files Modified

### Backend (Rust)
- `session_index.rs` - New module for session persistence
- `session.rs` - Added 3 new Tauri commands:
  - `find_recent_session` - Check for existing session
  - `record_session_for_resume` - Save session to index
  - `touch_session` - Update last active timestamp
- `mod.rs` - Added session_index module
- `lib.rs` - Registered new Tauri commands

### Frontend (TypeScript)
- `api.ts` - Added TypeScript bindings for new commands
- `useAgentSession.ts` - Modified to:
  - Check for recent sessions before creating new ones
  - Record new sessions in the index
  - Touch sessions when messages are sent

## Testing

1. Start the app
2. Open a chat for a bead (e.g., bp6-6nj with Product Manager persona)
3. Send a message
4. Close the chat dialog
5. Restart the app
6. Open the same chat again
7. ✅ Previous conversation should load automatically

## Notes

- Sessions older than 30 days can be cleaned up (method exists but not auto-triggered yet)
- The session index is separate from the session history JSONL files
- If a session index entry exists but the JSONL file is missing, a new session will be created
- CLI session IDs (from Gemini/Claude) are captured and stored for proper resume capability

## Future Enhancements

- Add UI to view/manage saved sessions
- Add "Clear session history" button
- Auto-cleanup of old sessions (>30 days)
- Session search by bead title or conversation content
