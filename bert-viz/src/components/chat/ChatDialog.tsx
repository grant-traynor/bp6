import { useState, useRef, useEffect, useCallback } from 'react';
import { Terminal, MessageSquare, Trash2, X, Square, Hand, ChevronDown, Pin, PinOff } from 'lucide-react';
import { MessageBubble } from './MessageBubble';
import { useAgentSession } from '../../hooks/useAgentSession';
import { useDraggable } from '../../hooks/useDraggable';
import { approveSuggestion, CliBackend, handoverToInteractive, PERSONA_ICONS, toggleWindowAlwaysOnTop } from '../../api';
import { sanitizeAgentHtml } from '../../utils/sanitizeAgentHtml';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useSessionStore } from '../../stores/sessionStore';
import { Terminal as TerminalComponent } from '../terminal/Terminal';
import { RawHtml } from '../shared/Markdown';
import { invoke } from '@tauri-apps/api/core';

interface ChatDialogProps {
  isOpen: boolean;
  onClose: () => void;
  persona: string;
  task: string | null;
  beadId: string | null;
  beadTitle?: string | null;
  beadDescription?: string | null;
  cliBackend: CliBackend;
  isSessionWindow?: boolean;  // True for fullscreen pop-out windows
  sessionIdOverride?: string | null; // Use existing session instead of creating a new one
  role?: string | null;
}

/**
 * ChatDialog - Clean rewrite with proper architecture
 *
 * Features:
 * - Clean flex-col layout (no absolute positioning)
 * - Custom hooks for session, streaming, and dragging
 * - Separate components for messages and commands
 * - Conditional rendering for debug view (not opacity)
 * - Proper event listener management
 * - Auto-scroll with smart detection
 */
const ChatDialog: React.FC<ChatDialogProps> = ({
  isOpen,
  onClose,
  persona,
  task,
  beadId,
  beadTitle = null,
  beadDescription = null,
  cliBackend,
  isSessionWindow = false,
  sessionIdOverride = null,
  role = null
}) => {
  const [input, setInput] = useState('');
  const [showDebug, setShowDebug] = useState(false);
  const [isAlwaysOnTop, setIsAlwaysOnTop] = useState(true);
  const [isHandingOver, setIsHandingOver] = useState(false);
  const [viewMode, setViewMode] = useState<'chat' | 'cli'>('chat');

  // Refs for auto-scroll and input
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const debugEndRef = useRef<HTMLDivElement>(null);
  const chatContainerRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const [isAtBottom, setIsAtBottom] = useState(true);

  // Custom hooks
  const {
    sessionId,
    messages,
    streamingMessage,
    isLoading,
    isAwaitingFirstChunk,
    debugLogs,
    sendMessage,
    stopAgent
  } = useAgentSession({ persona, task, beadId, cliBackend, isOpen, sessionIdOverride, role });

  // Get session info from store to check execution mode
  const sessions = useSessionStore(state => state.sessions);
  const currentSession = sessionId ? sessions.find(s => s.sessionId === sessionId) : null;
  const isHeadless = currentSession?.executionMode === 'headless';

  const safeStreamingMessage = sanitizeAgentHtml(streamingMessage);

  const { position, isDragging, handleMouseDown } = useDraggable({ x: 100, y: 100 });

  // Auto-scroll logic
  const checkIsAtBottom = useCallback(() => {
    if (!chatContainerRef.current) return;
    const { scrollHeight, scrollTop, clientHeight } = chatContainerRef.current;
    // Use a smaller threshold (30px) to determine if we're "at bottom"
    const threshold = 30;
    const nearBottom = scrollHeight - scrollTop - clientHeight < threshold;
    setIsAtBottom(nearBottom);
  }, []);

  const scrollToBottom = (behavior: ScrollBehavior = 'auto') => {
    messagesEndRef.current?.scrollIntoView({ behavior });
  };

  const scrollDebugToBottom = () => {
    debugEndRef.current?.scrollIntoView({ behavior: 'auto' });
  };

  // Handle manual scroll to update isAtBottom state
  useEffect(() => {
    const container = chatContainerRef.current;
    if (container) {
      container.addEventListener('scroll', checkIsAtBottom);
      return () => container.removeEventListener('scroll', checkIsAtBottom);
    }
  }, [checkIsAtBottom, showDebug]);

  // Auto-scroll when messages change - but only if we were already at bottom
  useEffect(() => {
    if (!showDebug && isAtBottom) {
      scrollToBottom('auto');
    }
  }, [messages, streamingMessage, showDebug, isAtBottom]);

  // Auto-scroll debug logs
  useEffect(() => {
    if (showDebug) scrollDebugToBottom();
  }, [debugLogs, showDebug]);

  // Auto-expand textarea
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = 'auto';
      textareaRef.current.style.height = `${Math.min(textareaRef.current.scrollHeight, 128)}px`;
    }
  }, [input]);

  // Auto-focus textarea on mount or when opening
  useEffect(() => {
    if (isOpen && !isLoading) {
      // Small timeout to ensure the dialog is fully rendered/positioned
      const timer = setTimeout(() => textareaRef.current?.focus(), 50);
      return () => clearTimeout(timer);
    }
  }, [isOpen, isLoading]);

  // Keyboard shortcut: Ctrl+` (or Cmd+` on Mac) to toggle terminal
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = async (e: KeyboardEvent) => {
      // Check for Ctrl+` or Cmd+` (backtick key)
      if ((e.ctrlKey || e.metaKey) && e.key === '`') {
        e.preventDefault();

        if (!sessionId) {
          console.error('No session ID available for terminal toggle');
          return;
        }

        // Toggle between chat and CLI modes
        if (viewMode === 'chat') {
          try {
            await invoke('spawn_pty_for_session', { sessionId: sessionId });
            setViewMode('cli');
          } catch (error) {
            console.error('Failed to switch to CLI mode:', error);
          }
        } else {
          try {
            await invoke('detach_pty_from_session', { sessionId: sessionId });
            setViewMode('chat');
          } catch (error) {
            console.error('Failed to switch to chat mode:', error);
            // If PTY wasn't attached, just switch view
            setViewMode('chat');
          }
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, sessionId, viewMode]);

  // Event handlers
  const handleSend = useCallback(async () => {
    if (!input.trim() || isLoading) return;

    const userMessage = input.trim();
    setInput('');
    await sendMessage(userMessage);
  }, [input, isLoading, sendMessage]);

  const handleApprove = useCallback(async (command: string) => {
    try {
      const result = await approveSuggestion(command);
      await sendMessage(`✅ Executed: ${command}\nResult: ${result}`);
    } catch (error) {
      console.error('Failed to approve suggestion:', error);
      await sendMessage(`❌ Error executing command: ${error}`);
    }
  }, [sendMessage]);

  const handleEdit = useCallback((command: string) => {
    setInput(command);
  }, []);

  const handleClearChat = useCallback(() => {
    // Note: This only clears the UI, not the actual session history
    // The original implementation had setDebugLogs([]) and setMessages([])
    // but those are managed by the hook now
    console.log('Clear chat requested - consider adding this to useAgentSession hook');
  }, []);


  const handleToggleAlwaysOnTop = useCallback(async () => {
    try {
      const currentWindow = getCurrentWindow();
      const windowLabel = currentWindow.label;
      const newState = !isAlwaysOnTop;
      await toggleWindowAlwaysOnTop(windowLabel, newState);
      setIsAlwaysOnTop(newState);
    } catch (error) {
      console.error("Failed to toggle always-on-top:", error);
    }
  }, [isAlwaysOnTop]);

  const handleTakeOver = useCallback(async () => {
    if (!sessionId) return;

    setIsHandingOver(true);
    try {
      await handoverToInteractive(sessionId);
      console.log(`Handed over session ${sessionId} to interactive mode`);
    } catch (error) {
      console.error('Failed to handover session:', error);
      alert(`Failed to take over session: ${error}`);
    } finally {
      setIsHandingOver(false);
    }
  }, [sessionId]);

  if (!isOpen) return null;

  const beadDescriptionExcerpt = beadDescription
    ? beadDescription.trim().split('\n').filter(l => l.trim()).slice(0, 2).join(' · ')
    : null;

  return (
    <div
      className={
        isSessionWindow
          ? "h-screen w-screen bg-[var(--background-primary)] flex flex-col overflow-hidden"
          : "fixed w-[850px] h-[700px] bg-[var(--background-primary)] border-2 border-[var(--border-primary)] rounded-3xl shadow-2xl flex flex-col z-50 overflow-hidden"
      }
      style={isSessionWindow ? undefined : {
        left: `${position.x}px`,
        top: `${position.y}px`
      }}
    >
      {/* Header */}
      <div
        className={`px-5 py-3 border-b-2 border-[var(--border-primary)] flex justify-between items-center bg-[var(--background-secondary)] ${
          !isSessionWindow && isDragging ? 'cursor-grabbing' : !isSessionWindow ? 'cursor-grab' : ''
        }`}
        onMouseDown={isSessionWindow ? undefined : handleMouseDown}
      >
        <div className="flex items-center gap-3">
          <span className="text-xl">{PERSONA_ICONS[role || persona] || '🤖'}</span>
          <div className="flex flex-col">
            <div className="flex items-center gap-2">
              <span className="text-[10px] font-black font-mono text-[var(--text-muted)] uppercase tracking-widest">
                {beadId || 'Session'}
              </span>
              <span className="text-[10px] font-black uppercase tracking-widest text-indigo-600 dark:text-indigo-400">
                · {role ? role.split('-').map(s => s.charAt(0).toUpperCase() + s.slice(1)).join(' ') :
                   persona.split('-').map(s => s.charAt(0).toUpperCase() + s.slice(1)).join(' ')}
              </span>
            </div>
            <h3 className="text-sm font-black text-[var(--text-primary)] truncate leading-tight">
              {task || (beadTitle ? beadTitle : 'AI Conversation')}
            </h3>
          </div>
        </div>

        {/* View Mode Toggle */}
        <div className="flex items-center gap-1 bg-[var(--background-tertiary)] p-1 rounded-xl border border-[var(--border-primary)]">
          <button
            onClick={async () => {
              if (!sessionId) return;
              try {
                await invoke('detach_pty_from_session', { sessionId: sessionId });
                setViewMode('chat');
              } catch (error) {
                console.error('Failed to switch to chat mode:', error);
                setViewMode('chat');
              }
            }}
            className={`px-3 py-1 text-[10px] font-black uppercase tracking-widest rounded-lg transition-all ${
              viewMode === 'chat'
                ? 'bg-[var(--background-primary)] text-indigo-600 shadow-sm'
                : 'text-[var(--text-muted)] hover:text-[var(--text-primary)]'
            }`}
          >
            Chat
          </button>
          <button
            onClick={async () => {
              if (!sessionId) return;
              try {
                await invoke('spawn_pty_for_session', { sessionId: sessionId });
                setViewMode('cli');
              } catch (error) {
                console.error('Failed to switch to CLI mode:', error);
              }
            }}
            className={`px-3 py-1 text-[10px] font-black uppercase tracking-widest rounded-lg transition-all ${
              viewMode === 'cli'
                ? 'bg-[var(--background-primary)] text-indigo-600 shadow-sm'
                : 'text-[var(--text-muted)] hover:text-[var(--text-primary)]'
            }`}
          >
            CLI
          </button>
        </div>

        <div className="flex items-center gap-1">
          {isLoading && (
            <button
              onClick={stopAgent}
              className="p-1.5 hover:bg-rose-500/10 text-rose-500 rounded-xl transition-all active:scale-90"
              title="Stop Agent"
            >
              <Square size={16} fill="currentColor" strokeWidth={2.5} />
            </button>
          )}
          <button
            onClick={() => setShowDebug(!showDebug)}
            className={`p-1.5 rounded-xl transition-all active:scale-90 ${
              showDebug ? 'bg-indigo-500/10 text-indigo-600' : 'text-[var(--text-muted)] hover:text-[var(--text-primary)]'
            }`}
            title="Toggle Debug Logs"
          >
            <Terminal size={16} strokeWidth={2.5} />
          </button>
          <button
            onClick={handleClearChat}
            className="p-1.5 text-[var(--text-muted)] hover:text-[var(--text-primary)] rounded-xl transition-all active:scale-90"
            title="Clear Chat"
          >
            <Trash2 size={16} strokeWidth={2.5} />
          </button>
          {isSessionWindow && (
            <button
              onClick={handleToggleAlwaysOnTop}
              className={`p-1.5 rounded-xl transition-all active:scale-90 ${
                isAlwaysOnTop ? 'text-indigo-500 hover:text-indigo-400' : 'text-[var(--text-muted)] hover:text-[var(--text-primary)]'
              }`}
              title={isAlwaysOnTop ? "Always on top (click to disable)" : "Not always on top (click to enable)"}
            >
              {isAlwaysOnTop ? <Pin size={16} strokeWidth={2.5} /> : <PinOff size={16} strokeWidth={2.5} />}
            </button>
          )}
          {!isSessionWindow && (
            <button
              onClick={onClose}
              className="p-1.5 text-[var(--text-muted)] hover:text-rose-500 rounded-xl transition-all active:scale-90"
            >
              <X size={18} strokeWidth={2.5} />
            </button>
          )}
        </div>
      </div>

      {/* Bead Description Strip */}
      {beadId && beadDescriptionExcerpt && (
        <div className="px-5 py-1.5 border-b border-[var(--border-primary)] bg-[var(--background-secondary)] flex items-center gap-2 min-w-0">
          <span className="text-[10px] font-black font-mono text-[var(--text-muted)] uppercase tracking-widest shrink-0">{beadId}</span>
          <span className="text-[var(--border-primary)] text-[10px] shrink-0">·</span>
          <span className="text-[10px] font-medium text-[var(--text-muted)] truncate leading-tight">{beadDescriptionExcerpt}</span>
        </div>
      )}

      {/* Main Content - Flex Layout */}
      <div className="min-h-0 flex-1 overflow-hidden flex bg-[var(--background-primary)]">
        {viewMode === 'chat' ? (
          /* Chat Content */
          <div className="min-h-0 flex-1 flex flex-col relative overflow-hidden">
            {/* Messages View */}
            {!showDebug && (
            <div ref={chatContainerRef} className="min-h-0 flex-1 overflow-y-auto custom-scrollbar">
              <div className="flex flex-col px-6 py-6">
                {messages.length === 0 && !streamingMessage && (
                  <div className="py-12 flex flex-col items-center justify-center text-[var(--text-muted)] space-y-4">
                    <div className="w-16 h-16 rounded-3xl bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] flex items-center justify-center">
                      <MessageSquare size={32} className="opacity-30" />
                    </div>
                    <p className="font-black uppercase tracking-widest text-[10px] text-center px-8">
                      Session Started · Waiting for input
                    </p>
                  </div>
                )}
                {messages.map((msg, i) => (
                  <MessageBubble
                    key={i}
                    role={msg.role}
                    content={msg.content}
                    onApprove={msg.role === 'assistant' ? handleApprove : undefined}
                    onEdit={msg.role === 'assistant' ? handleEdit : undefined}
                  />
                ))}
                {streamingMessage && (
                  <div className="flex justify-start mb-4">
                    <div className="w-full">
                      <div className="flex items-center gap-2 mb-2 opacity-50">
                        <span className="text-[10px] font-black uppercase tracking-[0.2em] text-[var(--text-muted)]">Assistant Thinking</span>
                      </div>
                      <RawHtml html={safeStreamingMessage} />
                      <span className="inline-block w-2 h-4 ml-1 bg-indigo-400 animate-pulse"></span>
                    </div>
                  </div>
                )}
                {(isLoading && (isAwaitingFirstChunk || !streamingMessage)) && (
                  <div className="flex justify-start mb-4">
                    <div className="flex items-center gap-3">
                      <div className="flex items-center gap-1.5">
                        <span className="inline-flex h-2 w-2 rounded-full bg-indigo-500 animate-bounce" style={{ animationDelay: '0ms' }} />
                        <span className="inline-flex h-2 w-2 rounded-full bg-indigo-500 animate-bounce" style={{ animationDelay: '120ms' }} />
                        <span className="inline-flex h-2 w-2 rounded-full bg-indigo-500 animate-bounce" style={{ animationDelay: '240ms' }} />
                      </div>
                      <span className="text-[10px] font-black uppercase tracking-widest text-[var(--text-muted)]">Agent Processing</span>
                    </div>
                  </div>
                )}
                <div ref={messagesEndRef} />
              </div>
            </div>
          )}


          {/* Scroll to Bottom Button Overlay */}
          {!isAtBottom && !showDebug && (
            <button
              onClick={() => {
                setIsAtBottom(true);
                scrollToBottom('smooth');
              }}
              className="absolute bottom-4 right-8 p-2 bg-indigo-600/90 text-white rounded-full shadow-lg hover:bg-indigo-700 transition-all active:scale-95 flex items-center gap-2 pr-4 z-10 animate-in fade-in slide-in-from-bottom-2 duration-300"
            >
              <div className="relative">
                <ChevronDown size={20} />
                {streamingMessage && (
                  <span className="absolute -top-1 -right-1 flex h-3 w-3">
                    <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-white opacity-75"></span>
                    <span className="relative inline-flex rounded-full h-3 w-3 bg-white"></span>
                  </span>
                )}
              </div>
              <span className="text-[10px] font-black uppercase tracking-widest">New Messages</span>
            </button>
          )}

          {/* Debug View */}
          {showDebug && (
            <div className="min-h-0 flex-1 bg-slate-900 text-emerald-400 p-4 font-mono text-[10px] overflow-y-auto custom-scrollbar">
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
          )}
        </div>
        ) : (
          /* Terminal Content */
          <div className="min-h-0 flex-1 flex flex-col overflow-hidden">
            {sessionId && <TerminalComponent sessionId={sessionId} />}
          </div>
        )}
      </div>

      {/* Input Area - only show in chat mode */}
      {viewMode === 'chat' && (
        <div className="p-4 border-t-2 border-[var(--border-primary)] bg-[var(--background-secondary)]">
          {isHeadless ? (
          <div className="flex flex-col items-center justify-center gap-3 py-4 px-6 bg-amber-500/10 border-2 border-amber-500/30 rounded-2xl">
            <div className="flex items-center gap-2 text-amber-600 dark:text-amber-400">
              <span className="text-2xl">🤖</span>
              <div className="flex flex-col">
                <span className="font-black text-[10px] uppercase tracking-widest">Headless Mode</span>
                <span className="text-[10px] font-bold opacity-80">Commands are executing automatically</span>
              </div>
            </div>
            {currentSession?.commandsRemaining !== undefined && currentSession?.commandsRemaining !== null && currentSession?.totalCommands && (() => {
              const remaining = currentSession.commandsRemaining ?? 0;
              const total = currentSession.totalCommands;
              return (
                <div className="text-[10px] text-amber-600 dark:text-amber-400 font-black uppercase tracking-widest">
                  Command {(total - remaining)}/{total}
                  {remaining > 0 && ` • ${remaining} remaining`}
                </div>
              );
            })()}
            <button
              onClick={handleTakeOver}
              disabled={isHandingOver}
              className="flex items-center gap-2 px-4 py-2 bg-indigo-600 hover:bg-indigo-700 disabled:bg-[var(--background-tertiary)] text-white rounded-xl font-black text-[10px] uppercase tracking-widest transition-all active:scale-95 shadow-sm"
            >
              <Hand size={12} />
              {isHandingOver ? 'Taking Over...' : 'Take Over Session'}
            </button>
          </div>
        ) : (
          <div className="flex items-end space-x-2">
            <textarea
              ref={textareaRef}
              rows={1}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey) {
                  e.preventDefault();
                  handleSend();
                }
              }}
              placeholder={showDebug ? "Debug input..." : "Type a message..."}
              className="flex-1 p-3 bg-[var(--background-primary)] border-2 border-[var(--border-primary)] rounded-2xl text-sm font-bold text-[var(--text-primary)] focus:outline-none focus:border-indigo-500 transition-all shadow-inner resize-none overflow-y-auto max-h-32 placeholder:text-[var(--text-muted)]"
              disabled={isLoading}
              spellCheck={true}
              autoCorrect="on"
            />
            <button
              onClick={isLoading ? stopAgent : handleSend}
              disabled={!isLoading && !input.trim()}
              className={`${
                isLoading
                  ? 'bg-rose-600 hover:bg-rose-700'
                  : 'bg-indigo-600 hover:bg-indigo-700 disabled:bg-[var(--background-tertiary)] disabled:text-[var(--text-muted)]'
              } text-white px-6 py-3 rounded-2xl transition-all active:scale-95 shadow font-black text-[10px] uppercase tracking-widest flex items-center justify-center gap-2 h-[46px]`}
            >
              {isLoading ? (
                <>
                  <Square size={12} fill="currentColor" />
                  Stop
                </>
              ) : 'Send'}
            </button>
          </div>
        )}
      </div>
      )}
    </div>
  );
};

export default ChatDialog;
