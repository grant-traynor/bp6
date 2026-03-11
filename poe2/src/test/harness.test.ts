import { describe, it, expect } from 'vitest';
import { makeProject } from './factories';

describe('test harness', () => {
  it('factories produce valid objects', () => {
    const p = makeProject({ name: 'My Project' });
    expect(p.name).toBe('My Project');
    expect(p.id).toBeDefined();
  });
});
