import path from 'path';
import { describe, expect, it } from 'vitest';
import { resolveChildPath } from './safePath';

describe('resolveChildPath', () => {
  it('resolves children but rejects traversal outside the configured root', () => {
    const root = process.cwd();
    expect(path.relative(root, resolveChildPath(root, 'child', 'file.txt'))).toBe(path.join('child', 'file.txt'));
    expect(() => resolveChildPath(root, '..', 'secret.txt')).toThrow('Path escapes its configured root');
  });
});
