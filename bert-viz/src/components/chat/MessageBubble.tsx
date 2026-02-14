import { memo } from 'react';

interface MessageBubbleProps {
  role: 'user' | 'assistant';
  content: string;
  onApprove?: (command: string) => void;
  onEdit?: (command: string) => void;
}

/**
 * Render message content with HTML and bd command support
 */
function renderContent(
  content: string,
  onApprove?: (command: string) => void,
  onEdit?: (command: string) => void
): React.ReactNode {
  // Check if content contains bd commands
  const bdCommandRegex = /<code>bd\s+[^<]+<\/code>|`bd\s+[^`]+`/g;
  const hasBdCommand = bdCommandRegex.test(content);

  if (hasBdCommand && onApprove && onEdit) {
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
                onClick={() => onApprove(command)}
                className="text-[10px] font-black uppercase tracking-wider bg-indigo-600 hover:bg-indigo-700 text-white px-2 py-1 rounded"
              >
                Approve
              </button>
              <button
                className="text-[10px] font-black uppercase tracking-wider bg-slate-300 dark:bg-slate-700 hover:bg-slate-400 dark:hover:bg-slate-600 text-slate-800 dark:text-slate-200 px-2 py-1 rounded"
                onClick={() => onEdit(command)}
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
  return (
    <div
      dangerouslySetInnerHTML={{ __html: content }}
      className="prose prose-sm dark:prose-invert max-w-none"
      style={{ contain: 'layout style' }}
    />
  );
}

/**
 * MessageBubble - Displays a single message with role-based styling
 *
 * Features:
 * - User/assistant bubble styling
 * - HTML content rendering
 * - bd command detection and UI
 * - Command approve/edit callbacks
 */
export const MessageBubble = memo<MessageBubbleProps>(({
  role,
  content,
  onApprove,
  onEdit
}) => {
  return (
    <div className={`flex ${role === 'user' ? 'justify-end' : 'justify-start'}`}>
      <div
        className={`max-w-[85%] p-3 rounded-2xl text-sm font-bold leading-relaxed ${
          role === 'user'
            ? 'bg-indigo-600 text-white rounded-br-none shadow-md'
            : 'bg-slate-100 dark:bg-slate-700 text-slate-800 dark:text-slate-200 rounded-bl-none shadow-sm border border-slate-200 dark:border-slate-600'
        }`}
      >
        {renderContent(content, onApprove, onEdit)}
      </div>
    </div>
  );
});

MessageBubble.displayName = 'MessageBubble';
