/**
 * Strip styles and unsafe attributes from agent-generated HTML so it cannot
 * bleed layout or custom styling into the chat container.
 */
export function sanitizeAgentHtml(html: string): string {
  if (!html) return '';

  // Guard for non-browser environments (tests, SSR). In those cases just
  // return the original string so rendering can fall back to plain HTML.
  if (typeof document === 'undefined') return html;

  const container = document.createElement('div');
  container.innerHTML = html;

  // Remove entire elements that can inject global styles or external content
  container.querySelectorAll('style,script,link,meta,iframe,object,embed').forEach((el) => {
    el.remove();
  });

  // Strip inline styles and event handlers from all remaining nodes
  container.querySelectorAll('*').forEach((el) => {
    Array.from(el.attributes).forEach((attr) => {
      const name = attr.name.toLowerCase();
      if (name === 'style' || name.startsWith('on')) {
        el.removeAttribute(attr.name);
      }
    });
  });

  return container.innerHTML;
}
