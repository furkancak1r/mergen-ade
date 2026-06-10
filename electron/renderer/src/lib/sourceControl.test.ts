import { describe, expect, it } from 'vitest';
import {
  parseBranchHeader,
  parseSourceControlStatusLine,
  sourceControlFileAbsolutePath,
  sourceControlFileMenuActionMeta,
  sourceControlBranchLine,
  sourceControlSnapshotHasDisplayData,
  sourceControlStatusLabel,
  sourceControlNoMatchesMessage,
  sourceControlMenuLabel,
  sourceControlToolbarButtonMeta,
  sourceControlWorktreeRowModel,
  sourceControlWorktreeLabel,
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

  it('detects source control display data like the Rust panel', () => {
    expect(sourceControlSnapshotHasDisplayData({ files: [] })).toBe(false);
    expect(sourceControlSnapshotHasDisplayData({ branch: 'main', files: [] })).toBe(true);
    expect(sourceControlSnapshotHasDisplayData({ ahead: 1, files: [] })).toBe(true);
    expect(sourceControlSnapshotHasDisplayData({ behind: 1, files: [] })).toBe(true);
    expect(sourceControlSnapshotHasDisplayData({ files: [{ path: 'src/app.ts', status: 'Modified', staged: false }] })).toBe(true);
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

  it('formats worktree labels like the Rust source control panel', () => {
    expect(sourceControlWorktreeLabel({
      path: 'C:\\repo\\worktrees\\feature',
      branch: 'refs/heads/feature/foo',
      detached: false,
    })).toBe('feature/foo');
    expect(sourceControlWorktreeLabel({
      path: '/repo/wt',
      branch: '',
      head: '1234567890abcdef',
      detached: true,
    })).toBe('detached@90abcdef');
    expect(sourceControlWorktreeLabel({
      path: '/repo/worktrees/fallback',
      branch: '',
      detached: false,
    })).toBe('fallback');
  });

  it('uses Rust source control toolbar icon metadata', () => {
    expect(sourceControlToolbarButtonMeta('refreshStatus')).toMatchObject({
      icon: '↻',
      tooltip: 'Refresh Status',
      ariaLabel: 'Refresh Status',
      accent: false,
    });
    expect(sourceControlToolbarButtonMeta('fetchAndRefresh')).toMatchObject({
      icon: '↓',
      tooltip: 'Fetch and Refresh',
      ariaLabel: 'Fetch and Refresh',
      accent: false,
    });
    expect(sourceControlToolbarButtonMeta('openProjectFolder')).toMatchObject({
      icon: '📂',
      tooltip: 'Open Project Folder',
      ariaLabel: 'Open Project Folder',
      accent: false,
    });
    expect(sourceControlToolbarButtonMeta('createWorktree')).toMatchObject({
      icon: '+',
      tooltip: 'Create Worktree',
      ariaLabel: 'Create Worktree',
      accent: true,
    });
  });

  it('uses Rust source control no-match copy without a trailing period', () => {
    expect(sourceControlNoMatchesMessage()).toBe('No matching files or worktrees');
  });

  it('builds Rust-style worktree row state for clickable unregistered rows', () => {
    const model = sourceControlWorktreeRowModel({
      path: 'C:\\repo\\worktrees\\feature',
      branch: 'refs/heads/feature/foo',
      head: 'abc',
      detached: false,
    }, 'C:\\repo', []);
    expect(model).toEqual({
      label: 'feature/foo',
      tooltip: 'feature/foo\nC:\\repo\\worktrees\\feature',
      branchNameForCopy: 'feature/foo',
      isCurrent: false,
      alreadyAdded: false,
      canAdd: true,
    });
  });

  it('marks registered or current worktree rows as non-clickable like Rust', () => {
    expect(sourceControlWorktreeRowModel({
      path: '/repo/wt',
      branch: 'refs/heads/wt',
      detached: false,
    }, '/repo', ['/repo/wt'])).toMatchObject({
      alreadyAdded: true,
      canAdd: false,
      isCurrent: false,
    });
    expect(sourceControlWorktreeRowModel({
      path: '/repo',
      branch: 'refs/heads/main',
      detached: false,
    }, '/repo', [])).toMatchObject({
      tooltip: 'main ●\n/repo',
      alreadyAdded: true,
      canAdd: false,
      isCurrent: true,
    });
  });

  it('uses icon-prefixed file context menu labels like Rust', () => {
    const open = sourceControlFileMenuActionMeta('openInFolder');
    const copy = sourceControlFileMenuActionMeta('copyRelativePath');
    expect(sourceControlMenuLabel(open)).toBe('📂 Open in Folder');
    expect(sourceControlMenuLabel(copy)).toBe('⧉ Copy Relative Path');
  });
});
