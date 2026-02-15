import { useState, useEffect, useRef, useCallback } from 'react';
import {
  startAgentSession,
  interruptAgentSession,
  sendAgentMessage,
  switchActiveSession,
  loadSessionHistory,
  markSessionRead,
  listActiveSessions,
  findRecentSession,
  recordSessionForResume,
  touchSession,
  CliBackend,
  AgentChunk
} from '../api';
import { listen } from '@tauri-apps/api/event';

interface Message {
  role: 'user' | 'assistant';
  content: string;
}

interface UseAgentSessionOptions {
  persona: string;
  task: string | null;
  beadId: string | null;
  cliBackend: CliBackend;
  isOpen: boolean;
  sessionIdOverride?: string | null;
}

interface UseAgentSessionReturn {
  sessionId: string | null;
  messages: Message[];
  streamingMessage: string;
  isLoading: boolean;
  isAwaitingFirstChunk: boolean;
  debugLogs: string[];
  sendMessage: (message: string) => Promise<void>;
  stopAgent: () => Promise<void>;
  switchSession: (targetSessionId: string, targetBeadId: string | null) => Promise<void>;
}

/**
 * Custom hook to manage agent session lifecycle and events
 *
 * Handles:
 * - Session creation and cleanup
 * - Event listeners for streaming chunks and stderr
 * - Message history loading
 * - Session switching without race conditions
 */
export function useAgentSession({
  persona,
  task,
  beadId,
  cliBackend,
  isOpen,
  sessionIdOverride = null
}: UseAgentSessionOptions): UseAgentSessionReturn {
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [streamingMessage, setStreamingMessage] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [isAwaitingFirstChunk, setIsAwaitingFirstChunk] = useState(false);
  const [debugLogs, setDebugLogs] = useState<string[]>(['[System] Starting agent session...']);

  const unlistenChunkRef = useRef<(() => void) | undefined>(undefined);
  const unlistenStderrRef = useRef<(() => void) | undefined>(undefined);
  const currentListenerSessionRef = useRef<string | null>(null); // Track current listener's session
  const isSwitchingSession = useRef(false);
  const prevContextRef = useRef<string>('');
  const hasCheckedForResumeRef = useRef(false);

  // Setup event listeners for a session
  const setupEventListeners = useCallback(async (sid: string) => {
    // Skip if we're already listening to this exact session
    if (currentListenerSessionRef.current === sid && unlistenChunkRef.current) {
      console.log('✓ Already listening to session:', sid);
      return;
    }

    console.log('🎧 Setting up listeners for session:', sid);

    // Clean up existing listeners - ensure cleanup completes
    if (unlistenChunkRef.current) {
      unlistenChunkRef.current();
      unlistenChunkRef.current = undefined;
    }
    if (unlistenStderrRef.current) {
      unlistenStderrRef.current();
      unlistenStderrRef.current = undefined;
    }

    currentListenerSessionRef.current = null; // Clear before setting up new

    const eventName = `agent-chunk-${sid}`;

    unlistenChunkRef.current = await listen<AgentChunk>(eventName, (event) => {
      const { content, isDone } = event.payload;

      if (content) {
        setDebugLogs(prev => [...prev, `[Stdout] ${content}`]);
      }

      // First chunk received — clear thinking state
      setIsAwaitingFirstChunk(false);

      if (isDone) {
        setStreamingMessage((current) => {
          if (current) {
            setMessages(prev => {
              // Prevent duplicate messages
              const lastMsg = prev[prev.length - 1];
              if (lastMsg && lastMsg.role === 'assistant' && lastMsg.content === current) {
                console.warn('Prevented duplicate message from being added');
                return prev;
              }
              return [...prev, { role: 'assistant', content: current }];
            });
          }
          return '';
        });
        setIsLoading(false);
        setDebugLogs(prev => [...prev, '[System] Agent finished response.']);
      } else if (content) {
        setStreamingMessage((prev) => prev + content);
      }
    });

    unlistenStderrRef.current = await listen<string>('agent-stderr', (event) => {
      setDebugLogs(prev => [...prev, `[Stderr] ${event.payload}`]);
    });

    // Mark this session as having active listeners
    currentListenerSessionRef.current = sid;
    console.log('✓ Listeners ready for session:', sid);
  }, []);

  // Initialize session on mount or context change
  useEffect(() => {
    if (!isOpen) return;

    const currentContext = sessionIdOverride
      ? `session-${sessionIdOverride}`
      : `${beadId}-${persona}-${task}`;
    const shouldReset = prevContextRef.current !== currentContext;
    prevContextRef.current = currentContext;

    if (shouldReset) {
      setMessages([]);
      setStreamingMessage('');
      setDebugLogs(['[System] Starting agent session...']);
    }

    const setup = async () => {
      if (!shouldReset && sessionId) {
        console.log('♻️  Reusing existing session:', sessionId);
        return;
      }

      if (sessionIdOverride) {
        console.log('🔗 Attaching to existing session:', sessionIdOverride);
        setSessionId(sessionIdOverride);
        setIsLoading(false);
        setDebugLogs(prev => [...prev, `[System] Attaching to session ${sessionIdOverride}`]);

        // Load conversation history for existing session (discover beadId if not provided)
        let targetBeadId = beadId;
        if (!targetBeadId) {
          try {
            const sessions = await listActiveSessions();
            targetBeadId = sessions.find(s => s.sessionId === sessionIdOverride)?.beadId || null;
          } catch (error) {
            console.error('Failed to fetch sessions while attaching:', error);
          }
        }

        if (targetBeadId !== undefined) {
          const history = await loadSessionHistory(sessionIdOverride, targetBeadId || null);
          const historyMessages: Message[] = history.map(m => ({
            role: m.role as 'user' | 'assistant',
            content: m.content
          }));
          setMessages(historyMessages);
          setDebugLogs(prev => [...prev, `[System] Loaded ${historyMessages.length} messages from history`]);
        }

        await setupEventListeners(sessionIdOverride);
        return;
      }

      // Check for a recent session to resume (only check once per context)
      // Note: This loads the OLD conversation history, but starts a NEW CLI session
      let historyToLoad: Message[] = [];
      if (!hasCheckedForResumeRef.current) {
        hasCheckedForResumeRef.current = true;
        console.log('🔍 Checking for recent session:', { beadId: beadId || 'untracked', persona });
        const recentSession = await findRecentSession(beadId || null, persona);
        console.log('🔍 findRecentSession result:', recentSession);
        if (recentSession) {
          console.log('🔄 Found recent session - loading history:', recentSession.sessionId);
          setDebugLogs(prev => [...prev, `[System] Loading history from session ${recentSession.sessionId}`]);

          // Load conversation history from the old session
          const history = await loadSessionHistory(recentSession.sessionId, beadId || null);
          console.log('📜 Loaded history:', history.length, 'messages');
          historyToLoad = history.map(m => ({
            role: m.role as 'user' | 'assistant',
            content: m.content
          }));
          setMessages(historyToLoad);
          setDebugLogs(prev => [...prev, `[System] Loaded ${historyToLoad.length} messages from previous session`]);
        } else {
          console.log('❌ No recent session found');
        }
      }

      try {
        console.log('🚀 Starting new CLI session with:', { persona, task, beadId, cliBackend, isOpen });
        setIsLoading(true);
        setIsAwaitingFirstChunk(true);
        const newSessionId = await startAgentSession(persona, task || undefined, beadId || undefined, cliBackend);
        console.log('✅ Session started:', newSessionId);
        setSessionId(newSessionId);
        setDebugLogs(prev => [...prev, `[System] New session ID: ${newSessionId}`]);

        // Record this session for future resumption
        console.log('📝 Recording session for resume:', { beadId: beadId || 'untracked', persona, newSessionId, cliBackend });
        try {
          await recordSessionForResume(beadId || null, persona, newSessionId, null, cliBackend);
          console.log('✅ Session recorded successfully');
        } catch (error) {
          console.error('❌ Failed to record session:', error);
        }

        // If we already loaded history from a previous session, keep it
        // Otherwise load history for this session (in case it's a resumed session)
        if (historyToLoad.length === 0 && beadId) {
          const history = await loadSessionHistory(newSessionId, beadId);
          const historyMessages: Message[] = history.map(m => ({
            role: m.role as 'user' | 'assistant',
            content: m.content
          }));
          setMessages(historyMessages);
          setDebugLogs(prev => [...prev, `[System] Loaded ${historyMessages.length} messages from current session`]);
        }

        // Setup event listeners
        await setupEventListeners(newSessionId);
      } catch (error) {
        console.error('Failed to start agent session:', error);
        setDebugLogs(prev => [...prev, `[Error] ${error}`]);
      }
    };

    setup();

    return () => {
      // Cleanup event listeners when switching sessions
      if (unlistenChunkRef.current) {
        unlistenChunkRef.current();
        unlistenChunkRef.current = undefined;
      }
      if (unlistenStderrRef.current) {
        unlistenStderrRef.current();
        unlistenStderrRef.current = undefined;
      }
      currentListenerSessionRef.current = null; // Clear tracking ref
    };
  }, [isOpen, persona, task, beadId, cliBackend, sessionId, setupEventListeners]);

  const sendMessage = useCallback(async (message: string) => {
    if (!sessionId) {
      console.error('Cannot send message: no active session');
      return;
    }

    setMessages((prev) => [...prev, { role: 'user', content: message }]);
    setIsLoading(true);
    setIsAwaitingFirstChunk(true);
    setDebugLogs(prev => [...prev, `[Input] ${message}`]);

    try {
      await sendAgentMessage(sessionId, message);
      // Update last active timestamp for this session
      await touchSession(beadId || null, persona);
    } catch (error) {
      console.error('Failed to send message:', error);
      setIsLoading(false);
      setIsAwaitingFirstChunk(false);
      setDebugLogs(prev => [...prev, `[Error] Failed to send: ${error}`]);
    }
  }, [sessionId, beadId, persona]);

  const stopAgent = useCallback(async () => {
    if (!sessionId) return;

    try {
      setDebugLogs(prev => [...prev, '[System] Interrupting agent...']);
      await interruptAgentSession(sessionId);
      setIsLoading(false);
      setStreamingMessage('');
      setDebugLogs(prev => [...prev, '[System] Agent interrupted (session remains alive).']);
    } catch (error) {
      console.error('Failed to interrupt agent session:', error);
    }
  }, [sessionId]);

  const switchSession = useCallback(async (targetSessionId: string, targetBeadId: string | null) => {
    if (targetSessionId === sessionId) {
      console.log('Already on session:', targetSessionId);
      return;
    }

    console.log('🔄 Switching from', sessionId, 'to', targetSessionId);

    try {
      isSwitchingSession.current = true;
      setIsLoading(true);

      // Clean up old listeners
      if (unlistenChunkRef.current) {
        unlistenChunkRef.current();
        unlistenChunkRef.current = undefined;
      }
      if (unlistenStderrRef.current) {
        unlistenStderrRef.current();
        unlistenStderrRef.current = undefined;
      }
      currentListenerSessionRef.current = null; // Clear tracking ref

      // Clear messages immediately to prevent race condition
      setMessages([]);
      setStreamingMessage('');

      await switchActiveSession(targetSessionId);

      // Setup listeners for new session
      await setupEventListeners(targetSessionId);

      // Load conversation history
      if (targetBeadId) {
        const history = await loadSessionHistory(targetSessionId, targetBeadId);
        const historyMessages: Message[] = history.map(m => ({
          role: m.role as 'user' | 'assistant',
          content: m.content
        }));
        setMessages(historyMessages);
      } else {
        setMessages([]);
      }

      setSessionId(targetSessionId);

      // Mark session as read
      try {
        await markSessionRead(targetSessionId);
      } catch (error) {
        console.error('Failed to mark session as read:', error);
      }

      setDebugLogs(prev => [...prev, `[System] Switched to session ${targetSessionId}`]);
    } catch (error) {
      console.error('Failed to switch session:', error);
      setDebugLogs(prev => [...prev, `[Error] Switch failed: ${error}`]);
    } finally {
      setIsLoading(false);
      setTimeout(() => {
        isSwitchingSession.current = false;
      }, 500);
    }
  }, [sessionId, setupEventListeners]);

  return {
    sessionId,
    messages,
    streamingMessage,
    isLoading,
    isAwaitingFirstChunk,
    debugLogs,
    sendMessage,
    stopAgent,
    switchSession
  };
}
