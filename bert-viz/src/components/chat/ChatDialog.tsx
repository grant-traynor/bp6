import React, { useState, useEffect, useRef } from 'react';
import { startAgentSession, stopAgentSession, sendAgentMessage, approveSuggestion, AgentChunk } from '../../api';
import { listen } from '@tauri-apps/api/event';

interface Message {
  role: 'user' | 'assistant';
  content: string;
}

interface ChatDialogProps {
  isOpen: boolean;
  onClose: () => void;
  persona: string;
}

const ChatDialog: React.FC<ChatDialogProps> = ({ isOpen, onClose, persona }) => {
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [streamingMessage, setStreamingMessage] = useState('');
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const scrollToBottom = () => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  };

  useEffect(() => {
    scrollToBottom();
  }, [messages, streamingMessage]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    if (isOpen) {
      setMessages([]);
      setStreamingMessage('');
      
      const setup = async () => {
        try {
          await startAgentSession(persona);
          
          unlisten = await listen<AgentChunk>('agent-chunk', (event) => {
            const { content, isDone } = event.payload;
            
            if (isDone) {
              setStreamingMessage((current) => {
                if (current) {
                   setMessages(prev => [...prev, { role: 'assistant', content: current }]);
                }
                return '';
              });
              setIsLoading(false);
            } else {
              setStreamingMessage((prev) => prev + content);
            }
          });
        } catch (error) {
          console.error('Failed to start agent session:', error);
        }
      };
      
      setup();
    }

    return () => {
      if (unlisten) unlisten();
      stopAgentSession();
    };
  }, [isOpen, persona]);

  const handleSend = async () => {
    if (!input.trim() || isLoading) return;

    const userMessage = input.trim();
    setMessages((prev) => [...prev, { role: 'user', content: userMessage }]);
    setInput('');
    setIsLoading(true);

    try {
      await sendAgentMessage(userMessage);
    } catch (error) {
      console.error('Failed to send message:', error);
      setIsLoading(false);
    }
  };

  const handleApprove = async (command: string) => {
    try {
      setIsLoading(true);
      const result = await approveSuggestion(command);
      setMessages(prev => [...prev, { role: 'assistant', content: `✅ Executed: ${command}\nResult: ${result}` }]);
    } catch (error) {
      console.error('Failed to approve suggestion:', error);
      setMessages(prev => [...prev, { role: 'assistant', content: `❌ Error executing command: ${error}` }]);
    } finally {
      setIsLoading(false);
    }
  };

  const renderContent = (content: string) => {
    // Basic detection of bd commands in backticks or as separate lines
    const parts = content.split(/(```[\s\S]*?```|`bd .*?`|^bd .*$)/m);
    
    return parts.map((part, index) => {
      const bdMatch = part.match(/^`?(bd\s+.*?)`?$/m) || part.match(/```(?:\w+)?\n?(bd\s+[\s\S]*?)```/);
      
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
      
      return <span key={index} className="whitespace-pre-wrap">{part}</span>;
    });
  };

  if (!isOpen) return null;

  return (
    <div className="fixed bottom-4 right-4 w-96 h-[500px] bg-white dark:bg-slate-800 border-2 border-indigo-500/50 rounded-2xl shadow-2xl flex flex-col z-50 overflow-hidden animate-in slide-in-from-bottom-4 duration-300">
      {/* Header */}
      <div className="p-4 border-b-2 border-slate-200 dark:border-slate-700 flex justify-between items-center bg-indigo-600 text-white">
        <h3 className="font-black text-xs uppercase tracking-[0.2em] flex items-center">
          <span className="mr-2 text-lg">🤖</span>
          {persona === 'product-manager' ? 'Product Manager' : 'AI Assistant'}
        </h3>
        <button 
          onClick={onClose}
          className="hover:bg-white/20 p-1 rounded-lg transition-colors"
        >
          ✕
        </button>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4 custom-scrollbar">
        {messages.length === 0 && !streamingMessage && (
          <div className="text-center text-slate-500 dark:text-slate-400 mt-10 font-bold italic text-sm">
            Ready to help with your project plan.
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
              <span className="inline-block w-2 h-4 ml-1 bg-indigo-400 animate-pulse"></span>
            </div>
          </div>
        )}
        {isLoading && !streamingMessage && (
          <div className="flex justify-start">
            <div className="p-3 rounded-xl bg-slate-100 dark:bg-slate-700">
              <div className="flex space-x-1">
                <div className="w-2 h-2 bg-indigo-400 rounded-full animate-bounce"></div>
                <div className="w-2 h-2 bg-indigo-400 rounded-full animate-bounce [animation-delay:-.3s]"></div>
                <div className="w-2 h-2 bg-indigo-400 rounded-full animate-bounce [animation-delay:-.5s]"></div>
              </div>
            </div>
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Input */}
      <div className="p-4 border-t-2 border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-slate-900/50">
        <div className="flex space-x-2">
          <input
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && handleSend()}
            placeholder="Type a message..."
            className="flex-1 p-3 bg-white dark:bg-slate-800 border-2 border-slate-200 dark:border-slate-700 rounded-xl text-sm font-bold text-slate-800 dark:text-slate-200 focus:outline-none focus:border-indigo-500 transition-all shadow-inner"
            disabled={isLoading}
          />
          <button
            onClick={handleSend}
            disabled={isLoading || !input.trim()}
            className="bg-indigo-600 hover:bg-indigo-700 disabled:bg-slate-400 text-white px-5 py-2 rounded-xl transition-all active:scale-95 shadow-lg font-black text-xs uppercase tracking-widest"
          >
            Send
          </button>
        </div>
      </div>
    </div>
  );
};

export default ChatDialog;
