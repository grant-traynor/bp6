import type { TestingLibraryMatchers } from '@testing-library/jest-dom/matchers';

declare module 'vitest' {
  interface Matchers<R = void>
    extends TestingLibraryMatchers<typeof expect.stringContaining, R> {}
}
