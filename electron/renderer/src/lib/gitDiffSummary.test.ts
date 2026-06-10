import { describe, expect, it } from 'vitest';
import {
  gitDiffSummaryLabel,
  parseGitNumstatLine,
  parseGitNumstatTotals,
  type GitDiffSummary,
} from '../../../shared/gitDiffSummary';

describe('gitDiffSummary', () => {
  it('parses numstat totals and skips binary rows', () => {
    const totals = parseGitNumstatTotals([
      '3\t1\tsrc/app.ts',
      '-\t-\tassets/logo.png',
      '10\t0\tREADME.md',
    ].join('\n'));

    expect(totals).toEqual({ addedLines: 13, removedLines: 1 });
  });

  it('parses malformed numstat rows as zero', () => {
    expect(parseGitNumstatLine('not-a-numstat-row')).toEqual({ added: 0, removed: 0 });
    expect(parseGitNumstatLine('x\ty\tfile.txt')).toEqual({ added: 0, removed: 0 });
  });

  it('formats loading, error, clean, and changed labels', () => {
    const changed: GitDiffSummary = { status: 'ready', addedLines: 8, removedLines: 3 };
    const clean: GitDiffSummary = { status: 'ready', addedLines: 0, removedLines: 0 };
    const error: GitDiffSummary = { status: 'error', addedLines: 0, removedLines: 0 };

    expect(gitDiffSummaryLabel(undefined, true)).toBe('...');
    expect(gitDiffSummaryLabel(undefined, false)).toBe('--');
    expect(gitDiffSummaryLabel(error, false)).toBe('--');
    expect(gitDiffSummaryLabel(clean, false)).toBe('');
    expect(gitDiffSummaryLabel(changed, false)).toBe('+8 -3');
  });
});
