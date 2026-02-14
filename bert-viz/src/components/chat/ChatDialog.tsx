import React, { useState, useEffect, useRef, useCallback } from 'react';
import {
  startAgentSession,
  interruptAgentSession,
  sendAgentMessage,
  approveSuggestion,
  switchActiveSession,
  listActiveSessions,
  terminateSession,
  loadSessionHistory,
  markSessionRead,
  AgentChunk,
  CliBackend
} from '../../api';
import { listen } from '@tauri-apps/api/event';
import { Terminal, MessageSquare, Trash2, X, Square } from 'lucide-react';
import { SessionList } from '../session/SessionList';

interface Message {
  role: 'user' | 'assistant';
  content: string;
}

interface ChatDialogProps {
  isOpen: boolean;
  onClose: () => void;
  persona: string;
  task: string | null;
  beadId: string | null;
  cliBackend: CliBackend;
}

const ChatDialog: React.FC<ChatDialogProps> = ({ isOpen, onClose, persona, task, beadId, cliBackend }) => {
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [streamingMessage, setStreamingMessage] = useState('');
  const [showDebug, setShowDebug] = useState(false);
  const [debugLogs, setDebugLogs] = useState<string[]>([]);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const debugEndRef = useRef<HTMLDivElement>(null);
  const unlistenChunkRef = useRef<(() => void) | undefined>(undefined);
  const unlistenStderrRef = useRef<(() => void) | undefined>(undefined);
  const isSwitchingSession = useRef(false);

  // Drag state
  const [position, setPosition] = useState({ x: 100, y: 100 });
  const [isDragging, setIsDragging] = useState(false);
  const [dragOffset, setDragOffset] = useState({ x: 0, y: 0 });

  const scrollToBottom = (behavior: ScrollBehavior = 'auto') => {
    messagesEndRef.current?.scrollIntoView({ behavior });
  };

  const scrollDebugToBottom = () => {
    debugEndRef.current?.scrollIntoView({ behavior: 'auto' });
  };

  // Only auto-scroll if user is already at bottom (within 100px threshold)
  const isNearBottom = (element: HTMLElement | null | undefined) => {
    if (!element) return true;
    const threshold = 100;
    return element.scrollHeight - element.scrollTop - element.clientHeight < threshold;
  };

  // Drag event handlers
  const handleMouseDown = (e: React.MouseEvent) => {
    setIsDragging(true);
    setDragOffset({
      x: e.clientX - position.x,
      y: e.clientY - position.y
    });
  };

  const handleMouseMove = useCallback((e: MouseEvent) => {
    if (!isDragging) return;

    const newX = Math.max(0, Math.min(e.clientX - dragOffset.x, window.innerWidth - 800));
    const newY = Math.max(0, Math.min(e.clientY - dragOffset.y, window.innerHeight - 600));

    setPosition({ x: newX, y: newY });
  }, [isDragging, dragOffset]);

  const handleMouseUp = useCallback(() => {
    setIsDragging(false);
  }, []);

  useEffect(() => {
    // Don't auto-scroll while switching sessions to prevent aggressive scrolling
    if (!showDebug && !isSwitchingSession.current) {
      const scrollContainer = messagesEndRef.current?.parentElement;
      if (isNearBottom(scrollContainer)) {
        scrollToBottom('auto'); // Instant scroll, no animation
      }
    }
  }, [messages, streamingMessage, showDebug]);

  useEffect(() => {
    if (showDebug) scrollDebugToBottom();
  }, [debugLogs, showDebug]);

  // Drag event listeners
  useEffect(() => {
    if (isDragging) {
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);
    }
    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isDragging, dragOffset, handleMouseMove, handleMouseUp]);

  // Track previous bead/persona to prevent unnecessary resets
  const prevContextRef = useRef<string>('');

  useEffect(() => {
    if (isOpen) {
      // Reset drag state when dialog opens to prevent layout issues
      setIsDragging(false);

      // Only clear state if context changed (different bead/persona combination)
      const currentContext = `${beadId}-${persona}-${task}`;
      const shouldReset = prevContextRef.current !== currentContext;
      prevContextRef.current = currentContext;

      if (shouldReset) {
        setMessages([]);
        setStreamingMessage('');
        setDebugLogs(['[System] Starting agent session...']);
      }

      const setup = async () => {
        // Skip if we already have a session for this context
        if (!shouldReset && sessionId) {
          console.log('♻️  ChatDialog: Reusing existing session:', sessionId);
          return;
        }

        try {
          console.log('🚀 ChatDialog: Starting session with:', { persona, task, beadId, cliBackend, isOpen });
          setIsLoading(true);
          const newSessionId = await startAgentSession(persona, task || undefined, beadId || undefined, cliBackend);
          console.log('✅ ChatDialog: Session started:', newSessionId);
          setSessionId(newSessionId);
          setDebugLogs(prev => [...prev, `[System] Session ID: ${newSessionId}`]);

          // Load conversation history from JSONL (will be empty for new sessions)
          if (beadId) {
            const history = await loadSessionHistory(newSessionId, beadId);
            const messages: Message[] = history.map(m => ({
              role: m.role as 'user' | 'assistant',
              content: m.content
            }));
            setMessages(messages);
            setDebugLogs(prev => [...prev, `[System] Loaded ${messages.length} messages from history`]);
          }

          // Subscribe to session-specific event channel for live updates
          const eventName = `agent-chunk-${newSessionId}`;

          // Clean up any existing listener first to prevent duplicates
          if (unlistenChunkRef.current) {
            unlistenChunkRef.current();
            unlistenChunkRef.current = undefined;
          }

          unlistenChunkRef.current = await listen<AgentChunk>(eventName, (event) => {
            const { content, isDone } = event.payload;

            if (content) {
              setDebugLogs(prev => [...prev, `[Stdout] ${content}`]);
            }

            if (isDone) {
              // First, capture the current streaming message
              setStreamingMessage((current) => {
                if (current) {
                  // Add to messages array only if it's not a duplicate
                  setMessages(prev => {
                    // Check if the last message is already this content (duplicate prevention)
                    const lastMsg = prev[prev.length - 1];
                    if (lastMsg && lastMsg.role === 'assistant' && lastMsg.content === current) {
                      console.warn('Prevented duplicate message from being added');
                      return prev;
                    }
                    return [...prev, { role: 'assistant', content: current }];
                  });
                }
                // Clear streaming message
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

        } catch (error) {
          console.error('Failed to start agent session:', error);
          setDebugLogs(prev => [...prev, `[Error] ${error}`]);
        }
      };
      
      setup();
    }

    return () => {
      // Clean up event listeners when switching sessions
      if (unlistenChunkRef.current) unlistenChunkRef.current();
      if (unlistenStderrRef.current) unlistenStderrRef.current();
      // DO NOT stop the session here - this would kill it when switching epics!
      // Sessions should only be terminated via explicit user action (SessionItem terminate button)
    };
  }, [isOpen, persona, task, beadId]);

  const handleStop = async () => {
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
  };

  const handleSend = async () => {
    if (!input.trim() || isLoading || !sessionId) return;

    const userMessage = input.trim();
    setMessages((prev) => [...prev, { role: 'user', content: userMessage }]);
    setInput('');
    setIsLoading(true);
    setDebugLogs(prev => [...prev, `[Input] ${userMessage}`]);

    try {
      await sendAgentMessage(sessionId, userMessage);
    } catch (error) {
      console.error('Failed to send message:', error);
      setIsLoading(false);
      setDebugLogs(prev => [...prev, `[Error] Failed to send: ${error}`]);
    }
  };

  const handleApprove = async (command: string) => {
    try {
      setIsLoading(true);
      setDebugLogs(prev => [...prev, `[System] Executing approved command: ${command}`]);
      const result = await approveSuggestion(command);
      setMessages(prev => [...prev, { role: 'assistant', content: `✅ Executed: ${command}\nResult: ${result}` }]);
      setDebugLogs(prev => [...prev, `[System] Command result: ${result}`]);
    } catch (error) {
      console.error('Failed to approve suggestion:', error);
      setMessages(prev => [...prev, { role: 'assistant', content: `❌ Error executing command: ${error}` }]);
      setDebugLogs(prev => [...prev, `[Error] Command failed: ${error}`]);
    } finally {
      setIsLoading(false);
    }
  };

  const handleSessionSwitch = async (targetSessionId: string, targetBeadId: string | null) => {
    if (targetSessionId === sessionId) return; // Same session check

    try {
      // Set flag to prevent aggressive scrolling during session switch
      isSwitchingSession.current = true;
      setIsLoading(true);

      // Clean up old listeners
      if (unlistenChunkRef.current) unlistenChunkRef.current();
      if (unlistenStderrRef.current) unlistenStderrRef.current();

      // CRITICAL: Clear messages immediately to prevent race condition
      // where new chunks append to old session's messages
      setMessages([]);
      setStreamingMessage('');

      await switchActiveSession(targetSessionId);

      // Set up new listeners for the target session
      const eventName = `agent-chunk-${targetSessionId}`;

      // Ensure old listener is fully cleaned up before creating new one
      if (unlistenChunkRef.current) {
        unlistenChunkRef.current();
        unlistenChunkRef.current = undefined;
      }

      unlistenChunkRef.current = await listen<AgentChunk>(eventName, (event) => {
        const { content, isDone } = event.payload;

        if (content) {
          setDebugLogs(prev => [...prev, `[Stdout] ${content}`]);
        }

        if (isDone) {
          // First, capture the current streaming message
          setStreamingMessage((current) => {
            if (current) {
              // Add to messages array only if it's not a duplicate
              setMessages(prev => {
                // Check if the last message is already this content (duplicate prevention)
                const lastMsg = prev[prev.length - 1];
                if (lastMsg && lastMsg.role === 'assistant' && lastMsg.content === current) {
                  console.warn('Prevented duplicate message from being added');
                  return prev;
                }
                return [...prev, { role: 'assistant', content: current }];
              });
            }
            // Clear streaming message
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

      // Load conversation history from JSONL using the target session's bead_id
      if (targetBeadId) {
        const history = await loadSessionHistory(targetSessionId, targetBeadId);

        // Convert ConversationMessage[] to Message[]
        const messages: Message[] = history.map(m => ({
          role: m.role,
          content: m.content
        }));

        setMessages(messages);
      } else {
        setMessages([]);
      }

      setSessionId(targetSessionId);

      // Mark the session as read (clear unread indicator)
      try {
        await markSessionRead(targetSessionId);
      } catch (error) {
        console.error('Failed to mark session as read:', error);
        // Non-critical error, don't block the UI
      }

      setDebugLogs(prev => [...prev, `[System] Switched to session ${targetSessionId}`]);
    } catch (error) {
      console.error('Failed to switch session:', error);
      setDebugLogs(prev => [...prev, `[Error] Switch failed: ${error}`]);
    } finally {
      setIsLoading(false);
      // Re-enable auto-scroll after a short delay to let history load settle
      setTimeout(() => {
        isSwitchingSession.current = false;
      }, 500);
    }
  };

  // @ts-ignore - Used in bp6-643.004.6 SessionList integration
  const handleSessionTerminate = async (targetSessionId: string) => {
    // Confirmation already handled by SessionItem, but add here for safety
    const confirmed = window.confirm(
      `Terminate session ${targetSessionId}? This will stop the agent and close the session.`
    );
    if (!confirmed) return;

    try {
      await terminateSession(targetSessionId);

      // If we terminated the active session, switch to another or clear
      if (targetSessionId === sessionId) {
        const sessions = await listActiveSessions();
        if (sessions.length > 0) {
          await handleSessionSwitch(sessions[0].sessionId, sessions[0].beadId);
        } else {
          setMessages([]);
          setSessionId(null);
        }
      }

      setDebugLogs(prev => [...prev, `[System] Terminated session ${targetSessionId}`]);
    } catch (error) {
      console.error('Failed to terminate session:', error);
      setDebugLogs(prev => [...prev, `[Error] Termination failed: ${error}`]);
    }
  };

  const renderContent = (content: string) => {
    // Check if content contains `bd` commands that need special handling
    const bdCommandRegex = /<code>bd\s+[^<]+<\/code>|`bd\s+[^`]+`/g;
    const hasBdCommand = bdCommandRegex.test(content);

    if (hasBdCommand) {
      // Extract and render bd commands with approve/edit buttons
      const parts = content.split(/(<code>bd\s+[^<]+<\/code>|`bd\s+[^`]+`)/g);

      return parts.map((part, index) => {
        const bdMatch = part.match(/<code>(bd\s+[^<]+)<\/code>/) || part.match(/`(bd\s+[^`]+)`/);

        if (bdMatch) {
          const command = bdMatch[1].trim();
          return (
            <div key={index} className="my-2 p-3 bg-slate-200 dark:bg-slate-900 rounded border border-indigo-500/30">
              <code className="text-indigo-600 dark:text-indigo-400 font-mono text-sm">{command}</code>
              <div className="mt-2 flex gap-2">
                <button
                  onClick={() => handleApprove(command)}
                  className="text-[10px] font-black uppercase tracking-wider bg-indigo-600 hover:bg-indigo-700 text-white px-2 py-1 rounded"
                >
                  Approve
                </button>
                <button
                  className="text-[10px] font-black uppercase tracking-wider bg-slate-300 dark:bg-slate-700 hover:bg-slate-400 dark:hover:bg-slate-600 text-slate-800 dark:text-slate-200 px-2 py-1 rounded"
                  onClick={() => setInput(command)}
                >
                  Edit
                </button>
              </div>
            </div>
          );
        }

        // Render other HTML parts
        return <span key={index} dangerouslySetInnerHTML={{ __html: part }} />;
      });
    }

    // No bd commands - render HTML directly with contained styling
    return <div dangerouslySetInnerHTML={{ __html: content }} className="prose prose-sm dark:prose-invert max-w-none" style={{ contain: 'layout style' }} />;
  };

  if (!isOpen) return null;

  return (
    <div
      className="fixed w-[650px] h-[600px] bg-white dark:bg-slate-800 border-2 border-indigo-500/50 rounded-2xl shadow-2xl flex flex-col z-50 overflow-hidden"
      style={{
        left: `${position.x}px`,
        top: `${position.y}px`
      }}
    >
      {/* Header */}
      <div
        className={`p-4 border-b-2 border-slate-200 dark:border-slate-700 flex justify-between items-center bg-indigo-600 text-white ${isDragging ? 'cursor-grabbing' : 'cursor-grab'}`}
        onMouseDown={handleMouseDown}
      >
        <div className="flex items-center gap-3">
          <span className="text-lg">🤖</span>
          <div className="flex flex-col">
            <h3 className="font-black text-xs uppercase tracking-[0.2em]">
              {persona === 'product-manager' ? 'Product Manager' : 
               persona === 'qa-engineer' ? 'QA Engineer' : 'AI Assistant'}
            </h3>
            <span className="text-[10px] opacity-70 font-bold uppercase tracking-widest">Active Session</span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {isLoading && (
            <button
              onClick={handleStop}
              className="p-2 bg-rose-500 hover:bg-rose-400 text-white rounded-lg transition-all"
              title="Stop Agent"
            >
              <Square size={18} fill="currentColor" />
            </button>
          )}
          <button 
            onClick={() => setShowDebug(!showDebug)}
            className={`p-2 rounded-lg transition-all ${showDebug ? 'bg-white/20 text-white' : 'hover:bg-white/10 text-white/70'}`}
            title="Toggle Debug Logs"
          >
            <Terminal size={18} />
          </button>
          <button 
            onClick={() => { setDebugLogs([]); setMessages([]); }}
            className="p-2 hover:bg-white/10 text-white/70 rounded-lg transition-all"
            title="Clear Chat"
          >
            <Trash2 size={18} />
          </button>
          <div className="w-px h-4 bg-white/20 mx-1" />
          <button 
            onClick={onClose}
            className="hover:bg-rose-500 p-2 rounded-lg transition-colors"
          >
            <X size={18} />
          </button>
        </div>
      </div>

      {/* Main Content Area with SessionList */}
      <div className="flex-1 overflow-hidden relative flex">
        {/* Session List Sidebar */}
        <SessionList
          activeSessionId={sessionId}
          onSessionSelect={handleSessionSwitch}
          onSessionTerminate={handleSessionTerminate}
        />

        {/* Chat Content */}
        <div className="flex-1 overflow-hidden relative">
        {/* Chat View */}
        <div className={`absolute inset-0 overflow-y-auto custom-scrollbar transition-opacity duration-300 ${showDebug ? 'opacity-0 pointer-events-none' : 'opacity-100'}`}>
          <div className="min-h-full flex flex-col justify-end gap-4 p-4">
          {messages.length === 0 && !streamingMessage && (
            <div className="flex-1 flex flex-col items-center justify-center text-slate-500 dark:text-slate-400 space-y-4">
              <div className="w-16 h-16 rounded-full bg-slate-100 dark:bg-slate-700 flex items-center justify-center">
                <MessageSquare size={32} className="opacity-50" />
              </div>
              <p className="font-bold italic text-sm text-center px-8">Ready to help with your project plan. Ask me to elaborate an epic or breakdown features.</p>
            </div>
          )}
          {messages.map((msg, i) => (
            <div 
              key={i} 
              className={`flex ${msg.role === 'user' ? 'justify-end' : 'justify-start'}`}
            >
              <div 
                className={`max-w-[85%] p-3 rounded-2xl text-sm font-bold leading-relaxed ${
                  msg.role === 'user' 
                    ? 'bg-indigo-600 text-white rounded-br-none shadow-md' 
                    : 'bg-slate-100 dark:bg-slate-700 text-slate-800 dark:text-slate-200 rounded-bl-none shadow-sm border border-slate-200 dark:border-slate-600'
                }`}
              >
                {renderContent(msg.content)}
              </div>
            </div>
          ))}
          {streamingMessage && (
            <div className="flex justify-start">
              <div className="max-w-[85%] p-3 rounded-2xl bg-slate-100 dark:bg-slate-700 text-slate-800 dark:text-slate-200 rounded-bl-none shadow-sm border border-slate-200 dark:border-slate-600 text-sm font-bold leading-relaxed">
                {renderContent(streamingMessage)}
                <span className="inline-block w-2 h-4 ml-1 bg-indigo-400 opacity-50"></span>
              </div>
            </div>
          )}
          {isLoading && !streamingMessage && (
            <div className="flex justify-start">
              <div className="p-3 rounded-xl bg-slate-100 dark:bg-slate-700 border border-slate-200 dark:border-slate-600">
                <div className="flex space-x-1">
                  <div className="w-2 h-2 bg-indigo-400 rounded-full opacity-50"></div>
                  <div className="w-2 h-2 bg-indigo-400 rounded-full opacity-50"></div>
                  <div className="w-2 h-2 bg-indigo-400 rounded-full opacity-50"></div>
                </div>
              </div>
            </div>
          )}
          <div ref={messagesEndRef} />
          </div>
        </div>

        {/* Debug View */}
        <div className={`absolute inset-0 bg-slate-900 text-emerald-400 p-4 font-mono text-[10px] overflow-y-auto custom-scrollbar transition-opacity duration-300 ${showDebug ? 'opacity-100' : 'opacity-0 pointer-events-none'}`}>
          <div className="space-y-1">
            {debugLogs.map((log, i) => {
              let color = 'text-slate-400';
              if (log.startsWith('[Stderr]')) color = 'text-amber-400';
              if (log.startsWith('[Input]')) color = 'text-blue-400';
              if (log.startsWith('[Stdout]')) color = 'text-emerald-400';
              if (log.startsWith('[Error]')) color = 'text-rose-400';
              if (log.startsWith('[System]')) color = 'text-indigo-400 font-bold';
              
              return <div key={i} className={color}>{log}</div>;
            })}
            <div ref={debugEndRef} />
          </div>
        </div>
        </div>
      </div>

      {/* Input Area */}
      <div className="p-4 border-t-2 border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-900/50">
        <div className="flex space-x-2">
          <input
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSend()}
            placeholder={showDebug ? "Debug input..." : "Type a message..."}
            className="flex-1 p-3 bg-white dark:bg-slate-800 border-2 border-slate-200 dark:border-slate-700 rounded-xl text-sm font-bold text-slate-800 dark:text-slate-200 focus:outline-none focus:border-indigo-500 transition-all shadow-inner"
            disabled={isLoading}
          />
          <button
            onClick={isLoading ? handleStop : handleSend}
            disabled={!isLoading && !input.trim()}
            className={`${
              isLoading 
                ? 'bg-rose-600 hover:bg-rose-700' 
                : 'bg-indigo-600 hover:bg-indigo-700 disabled:bg-slate-400'
            } text-white px-5 py-2 rounded-xl transition-all active:scale-95 shadow-lg font-black text-xs uppercase tracking-widest flex items-center justify-center gap-2`}
          >
            {isLoading ? (
              <>
                <div className="w-2 h-2 bg-white rounded-sm" />
                Stop
              </>
            ) : 'Send'}
          </button>
        </div>
      </div>
    </div>
  );
};

export default ChatDialog;
