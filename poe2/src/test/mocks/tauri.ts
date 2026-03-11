import { vi } from 'vitest';

// ── @tauri-apps/api/core ──────────────────────────────────────────────────────

export const mockInvoke = vi.fn().mockResolvedValue(undefined);

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mockInvoke,
}));

// ── @tauri-apps/api/event ─────────────────────────────────────────────────────

export const mockListen = vi.fn().mockResolvedValue(() => {});
export const mockEmit = vi.fn().mockResolvedValue(undefined);

vi.mock('@tauri-apps/api/event', () => ({
  listen: mockListen,
  emit: mockEmit,
}));

// ── @tauri-apps/plugin-dialog ─────────────────────────────────────────────────

export const mockOpen = vi.fn().mockResolvedValue(null);

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: mockOpen,
}));

// ── Helper: configure invoke return value per command ─────────────────────────

/**
 * Configure mockInvoke to return `response` when called with `command` as the
 * first argument.  Subsequent calls with the same command will also return
 * `response` unless you call mockResolvedValueOnce manually.
 *
 * Example:
 *   setInvokeResponse('list_projects', [makeProject()]);
 */
export function setInvokeResponse(command: string, response: unknown): void {
  mockInvoke.mockImplementation((cmd: string, ..._args: unknown[]) => {
    if (cmd === command) return Promise.resolve(response);
    return Promise.resolve(undefined);
  });
}
