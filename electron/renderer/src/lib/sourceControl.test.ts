import { describe, expect, it } from 'vitest';
import {
  parseBranchHeader,
  parseSourceControlStatusLine,
  sourceControlFileAbsolutePath,
  sourceControlBranchLine,
  sourceControlStatusLabel,
} from '../../../shared/sourceControl';

describe('sourceControl helpers', () => {
  it('parses branch headers with ahead and behind counts', () => {
    expect(parseBranchHeader('main...origin/main [ahead 2, behind 1]')).toEqual({
      branch: 'main',
      ahead: 2,
      behind: 1,
    });
  });

  it('parses branch headers without tracking data', () => {
    expect(parseBranchHeader('feature/foo')).toEqual({
      branch: 'feature/foo',
      ahead: 0,
      behind: 0,
    });
  });

  it('formats branch line like the Rust source control panel', () => {
    expect(sourceControlBranchLine({ branch: 'main', ahead: 2, behind: 1 })).toBe('main  ahead:2 behind:1');
    expect(sourceControlBranchLine({ branch: 'main', ahead: 0, behind: 0 })).toBe('main');
    expect(sourceControlBranchLine({ branch: '' })).toBeUndefined();
  });

  it('parses renamed status lines using the new path', () => {
    expect(parseSourceControlStatusLine('R  old/name.ts -> src/name.ts')).toEqual({
      path: 'src/name.ts',
      status: 'Renamed',
      staged: true,
    });
  });

  it('parses unstaged and untracked status lines like Rust', () => {
    expect(parseSourceControlStatusLine(' M src/app.ts')).toEqual({
      path: 'src/app.ts',
      status: 'Modified',
      staged: false,
    });
    expect(parseSourceControlStatusLine('?? new.txt')).toEqual({
      path: 'new.txt',
      status: 'Untracked',
      staged: false,
    });
  });

  it('maps conflict and ignored status labels', () => {
    expect(sourceControlStatusLabel('U')).toBe('Conflicted');
    expect(sourceControlStatusLabel('!')).toBe('Ignored');
    expect(sourceControlStatusLabel('X')).toBe('Changed');
  });

  it('builds platform-shaped absolute file paths from git relative paths', () => {
    expect(sourceControlFileAbsolutePath('C:\\repo\\', 'src/app.ts')).toBe('C:\\repo\\src/app.ts');
    expect(sourceControlFileAbsolutePath('/repo/', '/src/app.ts')).toBe('/repo/src/app.ts');
  });
});
