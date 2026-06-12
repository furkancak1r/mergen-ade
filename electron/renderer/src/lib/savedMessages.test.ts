import { describe, expect, it } from 'vitest';
import type { ProjectRecord } from '../../../shared/types';
import {
  addSavedMessage,
  removeSavedMessage,
  replaceProjectSavedMessages,
  savedMessageOwnerProjectId,
  updateSavedMessage,
} from './savedMessages';

const project = (partial: Partial<ProjectRecord>): ProjectRecord => ({
  id: partial.id ?? 1,
  name: partial.name ?? 'Project',
  path: partial.path ?? 'C:\\repo',
  savedMessages: partial.savedMessages ?? [],
  aiConfig: partial.aiConfig ?? {},
  checklist: partial.checklist ?? [],
  repoRoot: partial.repoRoot,
  isWorktree: partial.isWorktree ?? false,
});

describe('savedMessages helpers', () => {
  it('trims and appends unique saved messages', () => {
    expect(addSavedMessage(['one'], ' two ')).toEqual(['one', 'two']);
    expect(addSavedMessage(['one'], 'one')).toEqual(['one']);
    expect(addSavedMessage(['one'], '   ')).toEqual(['one']);
  });

  it('updates and removes by index without mutating invalid positions', () => {
    expect(updateSavedMessage(['one', 'two'], 1, 'changed')).toEqual(['one', 'changed']);
    expect(updateSavedMessage(['one'], 4, 'changed')).toEqual(['one']);
    expect(removeSavedMessage(['one', 'two'], 0)).toEqual(['two']);
    expect(removeSavedMessage(['one'], -1)).toEqual(['one']);
  });

  it('uses the root project as saved-message owner for worktrees', () => {
    const root = project({ id: 1, path: 'C:\\repo', isWorktree: false });
    const worktree = project({ id: 2, path: 'C:\\worktrees\\feature', repoRoot: 'C:\\repo', isWorktree: true });
    expect(savedMessageOwnerProjectId([root, worktree], worktree)).toBe(1);
  });

  it('replaces saved messages across the root/worktree family', () => {
    const root = project({ id: 1, path: 'C:\\repo', isWorktree: false, savedMessages: ['old'] });
    const worktree = project({ id: 2, path: 'C:\\worktrees\\feature', repoRoot: 'C:\\repo', isWorktree: true, savedMessages: ['old'] });
    const unrelated = project({ id: 3, path: 'C:\\other', isWorktree: false, savedMessages: ['keep'] });
    const next = replaceProjectSavedMessages([root, worktree, unrelated], 1, ['new']);
    expect(next[0].savedMessages).toEqual(['new']);
    expect(next[1].savedMessages).toEqual(['new']);
    expect(next[2].savedMessages).toEqual(['keep']);
  });
});
