import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { cn } from "../../utils";

export const mdComponents = {
  h1: ({ children }: any) => (
    <h1 className="text-lg font-black text-[var(--text-primary)] mb-3 pb-2 border-b-2 border-[var(--border-primary)]">
      {children}
    </h1>
  ),
  h2: ({ children }: any) => (
    <h2 className="text-[10px] font-black uppercase tracking-[0.25em] text-[var(--text-muted)] mt-6 mb-2">
      {children}
    </h2>
  ),
  h3: ({ children }: any) => (
    <h3 className="text-[11px] font-black uppercase tracking-wider text-[var(--text-primary)] mt-4 mb-1">
      {children}
    </h3>
  ),
  p: ({ children }: any) => (
    <p className="text-sm text-[var(--text-secondary)] leading-relaxed mb-3 last:mb-0">{children}</p>
  ),
  strong: ({ children }: any) => (
    <strong className="font-black text-[var(--text-primary)]">{children}</strong>
  ),
  ul: ({ children }: any) => <ul className="list-none pl-0 mb-3 space-y-1">{children}</ul>,
  li: ({ children }: any) => (
    <li className="flex items-start gap-2 text-sm text-[var(--text-secondary)]">
      <span className="mt-1.5 w-1.5 h-1.5 rounded-full bg-[var(--text-muted)] opacity-50 shrink-0" />
      <span className="flex-1">{children}</span>
    </li>
  ),
  ol: ({ children }: any) => <ol className="list-decimal pl-5 mb-3 space-y-1 text-sm text-[var(--text-secondary)]">{children}</ol>,
  input: ({ checked }: any) => (
    <input type="checkbox" checked={!!checked} readOnly
      className="mt-0.5 shrink-0 accent-indigo-500 cursor-default" />
  ),
  code: ({ children, className }: any) => (
    <code className={cn("font-mono text-xs px-1.5 py-0.5 bg-[var(--background-tertiary)] rounded text-[var(--text-primary)]", className)}>
      {children}
    </code>
  ),
  pre: ({ children }: any) => (
    <pre className="p-3 bg-[var(--background-secondary)] border-2 border-[var(--border-primary)] rounded-xl overflow-x-auto mb-3 custom-scrollbar">
      {children}
    </pre>
  ),
};

export const Markdown = ({ content, className }: { content: string; className?: string }) => (
  <div className={cn("markdown-content", className)}>
    <ReactMarkdown remarkPlugins={[remarkGfm]} components={mdComponents}>
      {content}
    </ReactMarkdown>
  </div>
);

/**
 * Renders raw HTML (from backend) with the same styling as Markdown component
 */
export const RawHtml = ({ html, className }: { html: string; className?: string }) => (
  <div
    className={cn(
      "raw-html-content",
      // Map our brutalist styles to raw HTML tags
      "[&_h1]:text-lg [&_h1]:font-black [&_h1]:text-[var(--text-primary)] [&_h1]:mb-3 [&_h1]:pb-2 [&_h1]:border-b-2 [&_h1]:border-[var(--border-primary)]",
      "[&_h2]:text-[10px] [&_h2]:font-black [&_h2]:uppercase [&_h2]:tracking-[0.25em] [&_h2]:text-[var(--text-muted)] [&_h2]:mt-6 [&_h2]:mb-2",
      "[&_h3]:text-[11px] [&_h3]:font-black [&_h3]:uppercase [&_h3]:tracking-wider [&_h3]:text-[var(--text-primary)] [&_h3]:mt-4 [&_h3]:mb-1",
      "[&_p]:text-sm [&_p]:text-[var(--text-secondary)] [&_p]:leading-relaxed [&_p]:mb-3 last:[&_p]:mb-0",
      "[&_strong]:font-black [&_strong]:text-[var(--text-primary)]",
      "[&_ul]:list-none [&_ul]:pl-0 [&_ul]:mb-3 [&_ul]:space-y-1",
      "[&_li]:flex [&_li]:items-start [&_li]:gap-2 [&_li]:text-sm [&_li]:text-[var(--text-secondary)]",
      "[&_li]:before:content-[''] [&_li]:before:mt-1.5 [&_li]:before:w-1.5 [&_li]:before:h-1.5 [&_li]:before:rounded-full [&_li]:before:bg-[var(--text-muted)] [&_li]:before:opacity-50 [&_li]:before:shrink-0",
      "[&_code]:font-mono [&_code]:text-xs [&_code]:px-1.5 [&_code]:py-0.5 [&_code]:bg-[var(--background-tertiary)] [&_code]:rounded [&_code]:text-[var(--text-primary)]",
      "[&_pre]:p-3 [&_pre]:bg-[var(--background-secondary)] [&_pre]:border-2 [&_pre]:border-[var(--border-primary)] [&_pre]:rounded-xl [&_pre]:overflow-x-auto [&_pre]:mb-3 [&_pre]:custom-scrollbar",
      className
    )}
    dangerouslySetInnerHTML={{ __html: html }}
  />
);
