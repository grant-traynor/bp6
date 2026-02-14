import { useState, useEffect, useRef, useCallback } from 'react';
import {
  startAgentSession,
  interruptAgentSession,
  sendAgentMessage,
  switchActiveSession,
  loadSessionHistory,
  markSessionRead,
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
}

interface UseAgentSessionReturn {
  sessionId: string | null;
  messages: Message[];
  streamingMessage: string;
  isLoading: boolean;
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
  isOpen
}: UseAgentSessionOptions): UseAgentSessionReturn {
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [streamingMessage, setStreamingMessage] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [debugLogs, setDebugLogs] = useState<string[]>(['[System] Starting agent session...']);

  const unlistenChunkRef = useRef<(() => void) | undefined>(undefined);
  const unlistenStderrRef = useRef<(() => void) | undefined>(undefined);
  const isSwitchingSession = useRef(false);
  const prevContextRef = useRef<string>('');

  // Setup event listeners for a session
  const setupEventListeners = useCallback(async (sid: string) => {
    // Clean up existing listeners
    if (unlistenChunkRef.current) {
      unlistenChunkRef.current();
      unlistenChunkRef.current = undefined;
    }
    if (unlistenStderrRef.current) {
      unlistenStderrRef.current();
      unlistenStderrRef.current = undefined;
    }

    const eventName = `agent-chunk-${sid}`;

    unlistenChunkRef.current = await listen<AgentChunk>(eventName, (event) => {
      const { content, isDone } = event.payload;

      if (content) {
        setDebugLogs(prev => [...prev, `[Stdout] ${content}`]);
      }

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
  }, []);

  // Initialize session on mount or context change
  useEffect(() => {
    if (!isOpen) return;

    const currentContext = `${beadId}-${persona}-${task}`;
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

      try {
        console.log('🚀 Starting session with:', { persona, task, beadId, cliBackend, isOpen });
        setIsLoading(true);
        const newSessionId = await startAgentSession(persona, task || undefined, beadId || undefined, cliBackend);
        console.log('✅ Session started:', newSessionId);
        setSessionId(newSessionId);
        setDebugLogs(prev => [...prev, `[System] Session ID: ${newSessionId}`]);

        // Load conversation history
        if (beadId) {
          const history = await loadSessionHistory(newSessionId, beadId);
          const historyMessages: Message[] = history.map(m => ({
            role: m.role as 'user' | 'assistant',
            content: m.content
          }));
          setMessages(historyMessages);
          setDebugLogs(prev => [...prev, `[System] Loaded ${historyMessages.length} messages from history`]);
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
      if (unlistenChunkRef.current) unlistenChunkRef.current();
      if (unlistenStderrRef.current) unlistenStderrRef.current();
    };
  }, [isOpen, persona, task, beadId, cliBackend, sessionId, setupEventListeners]);

  const sendMessage = useCallback(async (message: string) => {
    if (!sessionId) {
      console.error('Cannot send message: no active session');
      return;
    }

    setMessages((prev) => [...prev, { role: 'user', content: message }]);
    setIsLoading(true);
    setDebugLogs(prev => [...prev, `[Input] ${message}`]);

    try {
      await sendAgentMessage(sessionId, message);
    } catch (error) {
      console.error('Failed to send message:', error);
      setIsLoading(false);
      setDebugLogs(prev => [...prev, `[Error] Failed to send: ${error}`]);
    }
  }, [sessionId]);

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
    if (targetSessionId === sessionId) return;

    try {
      isSwitchingSession.current = true;
      setIsLoading(true);

      // Clean up old listeners
      if (unlistenChunkRef.current) unlistenChunkRef.current();
      if (unlistenStderrRef.current) unlistenStderrRef.current();

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
    debugLogs,
    sendMessage,
    stopAgent,
    switchSession
  };
}
