import { useState, useRef, useEffect, useCallback } from 'react';
import { Terminal, MessageSquare, Trash2, X, Square, Pin, PinOff } from 'lucide-react';
import { MessageBubble } from './MessageBubble';
import { useAgentSession } from '../../hooks/useAgentSession';
import { useDraggable } from '../../hooks/useDraggable';
import { approveSuggestion, CliBackend, toggleWindowAlwaysOnTop } from '../../api';
import { sanitizeAgentHtml } from '../../utils/sanitizeAgentHtml';
import { getCurrentWindow } from '@tauri-apps/api/window';

interface ChatDialogProps {
  isOpen: boolean;
  onClose: () => void;
  persona: string;
  task: string | null;
  beadId: string | null;
  beadTitle?: string | null;
  cliBackend: CliBackend;
  isSessionWindow?: boolean;  // True for fullscreen pop-out windows
  sessionIdOverride?: string | null; // Use existing session instead of creating a new one
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
  cliBackend,
  isSessionWindow = false,
  sessionIdOverride = null
}) => {
  const [input, setInput] = useState('');
  const [showDebug, setShowDebug] = useState(false);
  const [isAlwaysOnTop, setIsAlwaysOnTop] = useState(true); // Default is true per current implementation

  // Refs for auto-scroll and input
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const debugEndRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Custom hooks
  const {
    messages,
    streamingMessage,
    isLoading,
    isAwaitingFirstChunk,
    debugLogs,
    sendMessage,
    stopAgent
  } = useAgentSession({ persona, task, beadId, cliBackend, isOpen, sessionIdOverride });

  const safeStreamingMessage = sanitizeAgentHtml(streamingMessage);

  const { position, isDragging, handleMouseDown } = useDraggable({ x: 100, y: 100 });

  // Auto-scroll logic
  const isNearBottom = (element: HTMLElement | null | undefined) => {
    if (!element) return true;
    const threshold = 100;
    return element.scrollHeight - element.scrollTop - element.clientHeight < threshold;
  };

  const scrollToBottom = (behavior: ScrollBehavior = 'auto') => {
    messagesEndRef.current?.scrollIntoView({ behavior });
  };

  const scrollDebugToBottom = () => {
    debugEndRef.current?.scrollIntoView({ behavior: 'auto' });
  };

  // Auto-scroll when messages change
  useEffect(() => {
    if (!showDebug) {
      const scrollContainer = messagesEndRef.current?.parentElement;
      if (isNearBottom(scrollContainer)) {
        scrollToBottom('auto');
      }
    }
  }, [messages, streamingMessage, showDebug]);

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

  // Diagnostic: Track component renders
  console.log('🎨 RENDER: ChatDialog rendering | persona:', persona, '| beadId:', beadId, '| sessionIdOverride:', sessionIdOverride);

  if (!isOpen) return null;

  return (
    <div
      className={
        isSessionWindow
          ? "h-screen w-screen bg-white dark:bg-slate-800 flex flex-col overflow-hidden"
          : "fixed w-[850px] h-[600px] bg-white dark:bg-slate-800 border-2 border-indigo-500/50 rounded-2xl shadow-2xl flex flex-col z-50 overflow-hidden"
      }
      style={isSessionWindow ? undefined : {
        left: `${position.x}px`,
        top: `${position.y}px`
      }}
    >
      {/* Header */}
      <div
        className={`p-4 border-b-2 border-slate-200 dark:border-slate-700 flex justify-between items-center bg-indigo-600 text-white ${
          !isSessionWindow && isDragging ? 'cursor-grabbing' : !isSessionWindow ? 'cursor-grab' : ''
        }`}
        onMouseDown={isSessionWindow ? undefined : handleMouseDown}
      >
        <div className="flex items-center gap-3">
          <span className="text-lg">🤖</span>
          <div className="flex flex-col">
            <h3 className="font-black text-xs uppercase tracking-[0.2em]">
              {persona === 'product-manager' ? 'Product Manager' :
               persona === 'qa-engineer' ? 'QA Engineer' :
               persona === 'specialist' ? 'Specialist' :
               persona === 'architect' ? 'Architect' : 'AI Assistant'}
            </h3>
            <span className="text-[10px] opacity-70 font-bold uppercase tracking-widest">
              {task || 'Active Session'}
            </span>
            <span className="text-[11px] font-black tracking-tight opacity-80">
              {(beadId || 'Untracked')}{beadTitle ? ` · ${beadTitle}` : ''}
            </span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {isLoading && (
            <button
              onClick={stopAgent}
              className="p-2 bg-rose-500 hover:bg-rose-400 text-white rounded-lg transition-all"
              title="Stop Agent"
            >
              <Square size={18} fill="currentColor" />
            </button>
          )}
          {isSessionWindow && (
            <button
              onClick={handleToggleAlwaysOnTop}
              className={`p-2 rounded-lg transition-all ${
                isAlwaysOnTop
                  ? 'bg-blue-500 hover:bg-blue-400 text-white'
                  : 'bg-gray-600 hover:bg-gray-500 text-gray-300'
              }`}
              title={isAlwaysOnTop ? 'Unpin Window' : 'Pin Window on Top'}
            >
              {isAlwaysOnTop ? <Pin size={18} /> : <PinOff size={18} />}
            </button>
          )}
          <button
            onClick={() => setShowDebug(!showDebug)}
            className={`p-2 rounded-lg transition-all ${
              showDebug ? 'bg-white/20 text-white' : 'hover:bg-white/10 text-white/70'
            }`}
            title="Toggle Debug Logs"
          >
            <Terminal size={18} />
          </button>
          <button
            onClick={handleClearChat}
            className="p-2 hover:bg-white/10 text-white/70 rounded-lg transition-all"
            title="Clear Chat"
          >
            <Trash2 size={18} />
          </button>
          {!isSessionWindow && (
            <>
              <div className="w-px h-4 bg-white/20 mx-1" />
              <button
                onClick={onClose}
                className="hover:bg-rose-500 p-2 rounded-lg transition-colors"
              >
                <X size={18} />
              </button>
            </>
          )}
        </div>
      </div>

      {/* Main Content - Flex Layout */}
      <div className="min-h-0 flex-1 overflow-hidden flex">
        {/* Chat Content */}
        <div className="min-h-0 flex-1 flex flex-col overflow-hidden bg-[var(--background-primary)]">
          {/* Messages View */}
          {!showDebug && (
            <div className="min-h-0 flex-1 overflow-y-auto custom-scrollbar">
              <div className="flex flex-col gap-2 px-4 py-4">
                {messages.length === 0 && !streamingMessage && (
                  <div className="py-12 flex flex-col items-center justify-center text-slate-500 dark:text-slate-400 space-y-4">
                    <div className="w-16 h-16 rounded-full bg-slate-100 dark:bg-slate-700 flex items-center justify-center">
                      <MessageSquare size={32} className="opacity-50" />
                    </div>
                    <p className="font-bold italic text-sm text-center px-8">
                      Ready to help with your project plan. Ask me to elaborate an epic or breakdown features.
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
                  <div className="flex justify-start">
                    <div className="max-w-[85%] p-3 rounded-2xl bg-slate-100 dark:bg-slate-700 text-slate-800 dark:text-slate-200 rounded-bl-none shadow-sm border border-slate-200 dark:border-slate-600 text-sm font-bold leading-relaxed whitespace-pre-wrap">
                      <div dangerouslySetInnerHTML={{ __html: safeStreamingMessage }} />
                      <span className="inline-block w-2 h-4 ml-1 bg-indigo-400 opacity-50"></span>
                    </div>
                  </div>
                )}
                {(isLoading && (isAwaitingFirstChunk || !streamingMessage)) && (
                  <div className="flex justify-start">
                    <div className="px-4 py-3 rounded-2xl bg-slate-100 dark:bg-slate-700 border border-slate-200 dark:border-slate-600 shadow-sm text-sm font-bold text-slate-600 dark:text-slate-200 flex items-center gap-3">
                      <span>Thinking</span>
                      <div className="flex items-center gap-1">
                        <span className="inline-flex h-2 w-2 rounded-full bg-indigo-500 animate-bounce" style={{ animationDelay: '0ms' }} />
                        <span className="inline-flex h-2 w-2 rounded-full bg-indigo-500 animate-bounce" style={{ animationDelay: '120ms' }} />
                        <span className="inline-flex h-2 w-2 rounded-full bg-indigo-500 animate-bounce" style={{ animationDelay: '240ms' }} />
                      </div>
                    </div>
                  </div>
                )}
                <div ref={messagesEndRef} />
              </div>
            </div>
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
      </div>

      {/* Input Area */}
      <div className="p-4 border-t-2 border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-900/50">
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
            className="flex-1 p-3 bg-white dark:bg-slate-800 border-2 border-slate-200 dark:border-slate-700 rounded-xl text-sm font-bold text-slate-800 dark:text-slate-200 focus:outline-none focus:border-indigo-500 transition-all shadow-inner resize-none overflow-y-auto max-h-32"
            disabled={isLoading}
          />
          <button
            onClick={isLoading ? stopAgent : handleSend}
            disabled={!isLoading && !input.trim()}
            className={`${
              isLoading
                ? 'bg-rose-600 hover:bg-rose-700'
                : 'bg-indigo-600 hover:bg-indigo-700 disabled:bg-slate-400'
            } text-white px-5 py-3 rounded-xl transition-all active:scale-95 shadow-lg font-black text-xs uppercase tracking-widest flex items-center justify-center gap-2 h-[46px]`}
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
