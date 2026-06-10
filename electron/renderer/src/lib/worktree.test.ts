import { describe, it, expect } from 'vitest';
import { parseGitWorktreeList, sanitizeWorktreeSlug } from './worktree';

describe('parseGitWorktreeList', () => {
  it('parses normal worktrees', () => {
    const input = `worktree /path/to/repo
HEAD abcd1234
branch refs/heads/main

worktree /path/to/repo/wt1
HEAD efgh5678
branch refs/heads/feature`;

    const result = parseGitWorktreeList(input);
    expect(result).toHaveLength(2);
    expect(result[0]).toEqual({
      path: '/path/to/repo',
      branch: 'main',
      head: 'abcd1234',
      detached: false,
      locked: false,
      prunable: false,
    });
    expect(result[1]).toEqual({
      path: '/path/to/repo/wt1',
      branch: 'feature',
      head: 'efgh5678',
      detached: false,
      locked: false,
      prunable: false,
    });
  });

  it('parses detached worktree', () => {
    const input = `worktree /path/to/detached
HEAD abcd1234
detached`;

    const result = parseGitWorktreeList(input);
    expect(result).toHaveLength(1);
    expect(result[0]).toEqual({
      path: '/path/to/detached',
      branch: '',
      head: 'abcd1234',
      detached: true,
      locked: false,
      prunable: false,
    });
  });

  it('parses locked worktree', () => {
    const input = `worktree /path/to/locked
HEAD abcd1234
branch refs/heads/main
locked reason`;

    const result = parseGitWorktreeList(input);
    expect(result).toHaveLength(1);
    expect(result[0].locked).toBe(true);
  });

  it('parses prunable worktree', () => {
    const input = `worktree /path/to/prunable
HEAD abcd1234
branch refs/heads/main
prunable`;

    const result = parseGitWorktreeList(input);
    expect(result).toHaveLength(1);
    expect(result[0].prunable).toBe(true);
  });

  it('returns empty array for empty input', () => {
    const result = parseGitWorktreeList('');
    expect(result).toHaveLength(0);
  });

  it('sanitizes worktree slugs like the Rust app', () => {
    expect(sanitizeWorktreeSlug('Feature/Foo Bar')).toBe('feature-foo-bar');
    expect(sanitizeWorktreeSlug('release.v1')).toBe('release.v1');
    expect(sanitizeWorktreeSlug('fix..bug')).toBe('fix-bug');
    expect(sanitizeWorktreeSlug('özellik/çağrı')).toBe('özellik-çağrı');
  });
});
