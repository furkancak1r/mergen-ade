import { execFileSync } from 'child_process';
import fs from 'fs';
import os from 'os';
import path from 'path';
import { describe, expect, it } from 'vitest';
import { getGitDiffSummary } from '../../../main/worktree';
import {
  countTextLineBytes,
  gitDiffSummaryLabel,
  parseGitPathList,
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

  it('includes untracked text file lines in the main-process diff summary', async () => {
    const repoPath = fs.mkdtempSync(path.join(os.tmpdir(), 'mergen-diff-summary-'));
    const git = (args: string[]) => execFileSync('git', args, { cwd: repoPath });

    try {
      git(['init']);
      git(['-c', 'user.name=Test', '-c', 'user.email=test@example.com', 'commit', '--allow-empty', '--quiet', '-m', 'init']);
      fs.mkdirSync(path.join(repoPath, 'nested', 'deep'), { recursive: true });
      fs.writeFileSync(path.join(repoPath, 'nested', 'deep', 'a.txt'), 'one\ntwo\n');
      fs.writeFileSync(path.join(repoPath, 'nested', 'b.txt'), 'three');
      fs.writeFileSync(path.join(repoPath, 'nested', 'logo.bin'), Buffer.from([0, 159, 146, 150]));

      await expect(getGitDiffSummary(repoPath)).resolves.toEqual({
        status: 'ready',
        addedLines: 3,
        removedLines: 0,
      });
    } finally {
      fs.rmSync(repoPath, { recursive: true, force: true });
    }
  });

  it('parses nul-delimited git path lists', () => {
    expect(parseGitPathList('nested/a.txt\0nested/b.txt\0')).toEqual([
      'nested/a.txt',
      'nested/b.txt',
    ]);
  });

  it('counts text lines in utf8 and utf16 variants', () => {
    expect(countTextLineBytes(new TextEncoder().encode('alpha\nbeta\n'))).toBe(2);
    expect(countTextLineBytes(new Uint8Array([0xff, 0xfe, 97, 0, 13, 0, 10, 0, 98, 0]))).toBe(2);
    expect(countTextLineBytes(new Uint8Array([0xfe, 0xff, 0, 97, 0, 10, 0, 98]))).toBe(2);
    expect(countTextLineBytes(new Uint8Array([97, 0, 10, 0, 98, 0]))).toBe(2);
  });

  it('skips binary content with embedded nuls', () => {
    expect(countTextLineBytes(new Uint8Array([0, 159, 146, 150]))).toBeUndefined();
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
