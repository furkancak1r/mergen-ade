import { describe, expect, it } from 'vitest';
import {
  acpChangeSummarySignature,
  acpChangeSummaryTotals,
  acpChangeStatusAbbreviation,
  acpDiffLineClass,
  acpDiffTotals,
  buildAcpChangeSummaryFiles,
  groupAcpChanges,
  nextSelectedChangePath,
} from './acpChanges';

describe('acpChanges helpers', () => {
  it('groups staged and unstaged files', () => {
    const groups = groupAcpChanges([
      { path: 'a.ts', status: 'Modified', staged: true },
      { path: 'b.ts', status: 'Untracked', staged: false },
    ]);

    expect(groups.staged.map((file) => file.path)).toEqual(['a.ts']);
    expect(groups.unstaged.map((file) => file.path)).toEqual(['b.ts']);
  });

  it('preserves selected file when still present and falls back to first change', () => {
    const files = [
      { path: 'a.ts', status: 'Modified', staged: false },
      { path: 'b.ts', status: 'Deleted', staged: false },
    ];

    expect(nextSelectedChangePath('b.ts', files)).toBe('b.ts');
    expect(nextSelectedChangePath('missing.ts', files)).toBe('a.ts');
    expect(nextSelectedChangePath(undefined, [])).toBeUndefined();
  });

  it('formats diff totals only for ready diffs', () => {
    expect(acpDiffTotals({ status: 'ready', filePath: 'a.ts', patch: '', addedLines: 2, removedLines: 1, binary: false })).toBe('+2 -1');
    expect(acpDiffTotals({ status: 'error', filePath: 'a.ts', patch: '', addedLines: 0, removedLines: 0, binary: false })).toBeUndefined();
  });

  it('classifies diff lines', () => {
    expect(acpDiffLineClass('+++ b/a.ts')).toBe('is-file');
    expect(acpDiffLineClass('@@ -1 +1 @@')).toBe('is-hunk');
    expect(acpDiffLineClass('+added')).toBe('is-added');
    expect(acpDiffLineClass('-removed')).toBe('is-removed');
    expect(acpDiffLineClass(' context')).toBe('');
  });

  it('maps statuses to compact VS Code-like badges', () => {
    expect(acpChangeStatusAbbreviation('Modified')).toBe('M');
    expect(acpChangeStatusAbbreviation('Untracked')).toBe('U');
    expect(acpChangeStatusAbbreviation('Conflicted')).toBe('!');
  });

  it('builds change summary files from git diffs', () => {
    const diffs = new Map([
      ['src/app.ts', { status: 'ready' as const, filePath: 'src/app.ts', patch: '', addedLines: 4, removedLines: 2, binary: false }],
      ['assets/logo.png', { status: 'ready' as const, filePath: 'assets/logo.png', patch: '', addedLines: 0, removedLines: 0, binary: true }],
      ['bad.ts', { status: 'error' as const, filePath: 'bad.ts', patch: '', addedLines: 0, removedLines: 0, binary: false, error: 'diff failed' }],
    ]);

    const summary = buildAcpChangeSummaryFiles([
      { path: 'src/app.ts', status: 'Modified', staged: false },
      { path: 'assets/logo.png', status: 'Added', staged: true },
      { path: 'bad.ts', status: 'Modified', staged: false },
    ], diffs);

    expect(summary).toEqual([
      { path: 'src/app.ts', status: 'Modified', staged: false, addedLines: 4, removedLines: 2, binary: false, error: undefined },
      { path: 'assets/logo.png', status: 'Added', staged: true, addedLines: 0, removedLines: 0, binary: true, error: undefined },
      { path: 'bad.ts', status: 'Modified', staged: false, addedLines: 0, removedLines: 0, binary: false, error: 'diff failed' },
    ]);
    expect(acpChangeSummaryTotals(summary)).toEqual({ addedLines: 4, removedLines: 2 });
    expect(acpChangeSummarySignature(summary)).toContain('src/app.ts:Modified:unstaged:4:2:text:');
  });
});
