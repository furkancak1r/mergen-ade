import { describe, expect, it } from 'vitest';
import {
  acpChangeStatusAbbreviation,
  acpDiffLineClass,
  acpDiffTotals,
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
});
