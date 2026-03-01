import { twMerge } from "tailwind-merge";
import { clsx, type ClassValue } from "clsx";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/**
 * Replace literal \n escape sequences with real newlines.
 * Beads created via CLI or agent shell commands sometimes store "\\n"
 * instead of actual newline characters in text fields.
 */
export function unescapeNewlines(s: string | null | undefined): string | undefined {
  if (s == null) return undefined;
  return s.replace(/\\n/g, '\n');
}

/**
 * Apply unescapeNewlines to all freeform text fields on a bead object.
 */
export function unescapeBeadText<T extends { description?: string; notes?: string; design?: string }>(bead: T): T {
  return {
    ...bead,
    description: unescapeNewlines(bead.description),
    notes: unescapeNewlines(bead.notes),
    design: unescapeNewlines(bead.design),
  };
}
