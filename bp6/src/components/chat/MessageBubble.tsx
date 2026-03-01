import { memo, useState } from 'react';
import { sanitizeAgentHtml } from '../../utils/sanitizeAgentHtml';
import { RawHtml } from '../shared/Markdown';
import { markdownToHtml } from '../../utils/markdown';
import { DiffView } from './DiffView';
import { ToolUseData } from '../../api';

interface MessageBubbleProps {
  role: 'user' | 'assistant' | 'terminal';
  content: string;
  toolUse?: ToolUseData;
  onApprove?: (command: string) => void;
  onEdit?: (command: string) => void;
}

/**
 * Render message content with shared styles and bd command support
 */
function renderContent(
  content: string,
  onApprove?: (command: string) => void,
  onEdit?: (command: string) => void
): React.ReactNode {
  const safeContent = sanitizeAgentHtml(markdownToHtml(content));

  // Check if content contains bd commands
  const bdCommandRegex = /<code>bd\s+[^<]+<\/code>|`bd\s+[^`]+`/g;
  const hasBdCommand = bdCommandRegex.test(safeContent);

  if (hasBdCommand && onApprove && onEdit) {
    // Extract and render bd commands with approve/edit buttons
    const parts = safeContent.split(/(<code>bd\s+[^<]+<\/code>|`bd\s+[^`]+`)/g);

    return parts.map((part, index) => {
      const bdMatch = part.match(/<code>(bd\s+[^<]+)<\/code>/) || part.match(/`(bd\s+[^`]+)`/);

      if (bdMatch) {
        const command = bdMatch[1].trim();
        return (
          <div key={index} className="my-3 p-4 bg-[var(--background-secondary)] rounded-2xl border-2 border-[var(--border-primary)] shadow-sm">
            <div className="flex items-center gap-2 mb-2">
              <span className="text-[10px] font-black uppercase tracking-widest text-[var(--text-muted)]">Suggested Command</span>
            </div>
            <code className="text-indigo-600 dark:text-indigo-400 font-mono text-sm block bg-[var(--background-tertiary)] p-2 rounded-lg mb-3">{command}</code>
            <div className="flex gap-2">
              <button
                onClick={() => onApprove(command)}
                className="text-[10px] font-black uppercase tracking-widest bg-indigo-600 hover:bg-indigo-700 text-white px-4 py-1.5 rounded-xl shadow-sm active:scale-95 transition-all"
              >
                Approve
              </button>
              <button
                className="text-[10px] font-black uppercase tracking-widest bg-[var(--background-tertiary)] hover:bg-[var(--border-primary)] text-[var(--text-primary)] px-4 py-1.5 rounded-xl border border-[var(--border-primary)] active:scale-95 transition-all"
                onClick={() => onEdit(command)}
              >
                Edit
              </button>
            </div>
          </div>
        );
      }

      // Render other parts as RawHtml
      return <RawHtml key={index} html={part} />;
    });
  }

  // No bd commands - render as RawHtml
  return <RawHtml html={safeContent} />;
}

/**
 * TerminalBlock - Collapsible block showing captured PTY/terminal output
 */
const TerminalBlock = memo<{ content: string }>(({ content }) => {
  const [expanded, setExpanded] = useState(false);
  const lineCount = content.split('\n').length;
  const preview = content.split('\n').slice(0, 3).join('\n');

  return (
    <div className="mb-4 rounded-2xl border border-[var(--border-primary)] overflow-hidden">
      <button
        className="w-full flex items-center justify-between px-4 py-2 bg-[var(--background-secondary)] hover:bg-[var(--background-tertiary)] transition-colors text-left"
        onClick={() => setExpanded(v => !v)}
      >
        <div className="flex items-center gap-2">
          <span className="text-[10px] font-black uppercase tracking-widest text-[var(--text-muted)]">Terminal Session</span>
          <span className="text-[10px] text-[var(--text-muted)] opacity-60">{lineCount} lines</span>
        </div>
        <span className="text-[10px] text-[var(--text-muted)] opacity-60">{expanded ? '▲ collapse' : '▼ expand'}</span>
      </button>
      <pre className={`px-4 py-3 font-mono text-xs text-[var(--text-secondary)] bg-[var(--background-primary)] overflow-x-auto whitespace-pre-wrap break-words ${expanded ? '' : 'max-h-20 overflow-y-hidden'}`}>
        {expanded ? content : preview}
      </pre>
      {!expanded && lineCount > 3 && (
        <div className="px-4 py-1 bg-[var(--background-primary)] border-t border-[var(--border-primary)]">
          <button
            className="text-[10px] text-indigo-500 hover:text-indigo-400 font-bold uppercase tracking-widest"
            onClick={() => setExpanded(true)}
          >
            Show all {lineCount} lines
          </button>
        </div>
      )}
    </div>
  );
});
TerminalBlock.displayName = 'TerminalBlock';

/**
 * MessageBubble - Displays a single message with role-based styling
 */
export const MessageBubble = memo<MessageBubbleProps>(({
  role,
  content,
  toolUse,
  onApprove,
  onEdit
}) => {
  // Tool use message: render as diff view, no bubble chrome needed
  if (toolUse) {
    return (
      <div className="mb-2">
        <DiffView
          filePath={toolUse.filePath}
          oldString={toolUse.oldString}
          newString={toolUse.newString}
        />
      </div>
    );
  }

  // Terminal block: collapsible monospace view of captured PTY output
  if (role === 'terminal') {
    return <TerminalBlock content={content} />;
  }

  return (
    <div className={`flex ${role === 'user' ? 'justify-end' : 'justify-start'} mb-4`}>
      <div
        className={`max-w-[90%] ${
          role === 'user'
            ? 'bg-[var(--background-tertiary)] text-[var(--text-primary)] p-4 rounded-3xl rounded-tr-none border-2 border-[var(--border-primary)] shadow-sm whitespace-pre-wrap font-bold text-sm'
            : 'text-[var(--text-secondary)] w-full'
        }`}
      >
        {role === 'assistant' && (
          <div className="flex items-center gap-2 mb-2 opacity-50">
            <span className="text-[10px] font-black uppercase tracking-[0.2em] text-[var(--text-muted)]">Assistant</span>
          </div>
        )}
        {renderContent(content, onApprove, onEdit)}
      </div>
    </div>
  );
});

MessageBubble.displayName = 'MessageBubble';
