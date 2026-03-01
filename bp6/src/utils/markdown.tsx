import { marked } from 'marked';

/**
 * Configure marked for secure rendering
 */
marked.setOptions({
  gfm: true, // GitHub Flavored Markdown
  breaks: true, // Convert \n to <br>
});

/**
 * Renders markdown HTML content safely
 *
 * Note: The content is already converted to HTML by the Rust backend,
 * so we just need to render it with proper styling.
 *
 * @param content - HTML string (pre-converted from markdown)
 * @param className - Additional CSS classes
 * @returns React element with rendered HTML
 */
export function MarkdownContent({
  content,
  className = ''
}: {
  content: string | null | undefined;
  className?: string;
}) {
  if (!content) {
    return <span className="text-slate-400 dark:text-slate-500 italic">No content</span>;
  }

  return (
    <div
      className={`prose prose-sm dark:prose-invert max-w-none ${className}`}
      dangerouslySetInnerHTML={{ __html: content }}
    />
  );
}

/**
 * Renders markdown for bead fields (description, notes, design, etc.)
 * Converts markdown to HTML on the frontend using marked.
 *
 * @param content - The markdown content (as plain text)
 * @param placeholder - Placeholder text when empty
 * @returns React element with rendered markdown
 */
export function BeadFieldMarkdown({
  content,
  placeholder = 'No content',
  className = ''
}: {
  content: string | null | undefined;
  placeholder?: string;
  className?: string;
}) {
  if (!content || content.trim() === '') {
    return <span className="text-slate-400 dark:text-slate-500 italic text-sm">{placeholder}</span>;
  }

  // Convert markdown to HTML
  const html = marked.parse(content, { async: false }) as string;

  return (
    <div
      className={`prose prose-sm dark:prose-invert max-w-none text-sm ${className}`}
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
