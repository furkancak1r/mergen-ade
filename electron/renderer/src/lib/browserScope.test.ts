import { describe, it, expect } from 'vitest';
import { BrowserScopeKeyType, type BrowserTab } from '../../../shared/types';
import {
  scopeKeyString,
  activeBrowserScope,
  shouldPersistUrl,
  isTerminalScope,
  browserLastUrlForProjectFamily,
  browserUrlForProjectFamily,
  withBrowserLastUrlForProjectFamily,
  withoutBrowserLastUrlForProjectFamily,
  type ProjectFamilyRecord,
} from './browserScope';

function project(partial: Partial<ProjectFamilyRecord> & Pick<ProjectFamilyRecord, 'id' | 'path'>): ProjectFamilyRecord {
  return {
    name: `Project ${partial.id}`,
    isWorktree: false,
    ...partial,
  } as ProjectFamilyRecord;
}

function tabsByScope(entries: Array<[string, BrowserTab[]]>): Map<string, BrowserTab[]> {
  return new Map(entries);
}

function activeTabs(entries: Array<[string, string]>): Map<string, string> {
  return new Map(entries);
}

describe('browserScope', () => {
  it('scopeKeyString for project', () => {
    expect(scopeKeyString({ type: BrowserScopeKeyType.Project, projectId: 5 })).toBe('project:5');
  });

  it('scopeKeyString for terminal', () => {
    expect(scopeKeyString({ type: BrowserScopeKeyType.Terminal, projectId: 5, terminalId: 12 })).toBe('terminal:5:12');
  });

  it('activeBrowserScope uses visible override first', () => {
    const override = { type: BrowserScopeKeyType.Terminal, projectId: 1, terminalId: 99 };
    const result = activeBrowserScope(1, 2, override, () => false, () => false);
    expect(result).toEqual(override);
  });

  it('activeBrowserScope falls back to terminal scope when tabs exist', () => {
    const result = activeBrowserScope(1, 2, undefined, () => true, () => false);
    expect(result).toEqual({ type: BrowserScopeKeyType.Terminal, projectId: 1, terminalId: 2 });
  });

  it('activeBrowserScope falls back to project scope when no terminal tabs', () => {
    const result = activeBrowserScope(1, 2, undefined, () => false, () => true);
    expect(result).toEqual({ type: BrowserScopeKeyType.Project, projectId: 1 });
  });

  it('activeBrowserScope returns undefined when no tabs anywhere', () => {
    const result = activeBrowserScope(1, 2, undefined, () => false, () => false);
    expect(result).toBeUndefined();
  });

  it('shouldPersistUrl true only for project scope', () => {
    expect(shouldPersistUrl({ type: BrowserScopeKeyType.Project, projectId: 1 })).toBe(true);
    expect(shouldPersistUrl({ type: BrowserScopeKeyType.Terminal, projectId: 1, terminalId: 2 })).toBe(false);
  });

  it('isTerminalScope true only for terminal scope', () => {
    expect(isTerminalScope({ type: BrowserScopeKeyType.Project, projectId: 1 })).toBe(false);
    expect(isTerminalScope({ type: BrowserScopeKeyType.Terminal, projectId: 1, terminalId: 2 })).toBe(true);
  });

  it('browserUrlForProjectFamily uses the target visible active tab first', () => {
    const target = project({ id: 2, path: 'C:/repo-worktrees/feature', repoRoot: 'C:/repo', isWorktree: true });
    const projects = [
      project({ id: 1, path: 'C:/repo', browserLastUrl: 'https://root-last.example.com' }),
      target,
    ];
    const visibleScopes = new Map([
      [2, { type: BrowserScopeKeyType.Terminal, projectId: 2, terminalId: 9 }],
    ]);

    expect(browserUrlForProjectFamily(
      target,
      projects,
      tabsByScope([
        ['terminal:2:9', [{ id: 'terminal-active', url: 'https://visible.example.com' }]],
        ['project:1', [{ id: 'root-active', url: 'https://root-active.example.com' }]],
      ]),
      activeTabs([
        ['terminal:2:9', 'terminal-active'],
        ['project:1', 'root-active'],
      ]),
      visibleScopes,
    )).toBe('https://visible.example.com');
  });

  it('browserUrlForProjectFamily falls back to project-family active tabs', () => {
    const target = project({ id: 2, path: 'C:/repo-worktrees/feature', repoRoot: 'C:/repo', isWorktree: true });
    const projects = [
      project({ id: 1, path: 'C:/repo' }),
      target,
      project({ id: 3, path: 'C:/other', browserLastUrl: 'https://unrelated.example.com' }),
    ];

    expect(browserUrlForProjectFamily(
      target,
      projects,
      tabsByScope([
        ['project:1', [{ id: 'root-active', url: 'https://root-active.example.com' }]],
        ['project:3', [{ id: 'other-active', url: 'https://other-active.example.com' }]],
      ]),
      activeTabs([
        ['project:1', 'root-active'],
        ['project:3', 'other-active'],
      ]),
    )).toBe('https://root-active.example.com');
  });

  it('browserLastUrlForProjectFamily prefers the registered root project URL', () => {
    const target = project({
      id: 2,
      path: 'C:/repo-worktrees/feature',
      repoRoot: 'C:/repo',
      isWorktree: true,
      browserLastUrl: 'https://worktree.example.com',
    });
    const projects = [
      project({ id: 1, path: 'C:/repo', browserLastUrl: 'https://root.example.com' }),
      target,
    ];

    expect(browserLastUrlForProjectFamily(target, projects)).toBe('https://root.example.com');
  });

  it('browserUrlForProjectFamily ignores unrelated active tabs and last URLs', () => {
    const target = project({ id: 2, path: 'C:/repo-worktrees/feature', repoRoot: 'C:/repo', isWorktree: true });
    const projects = [
      project({ id: 1, path: 'C:/repo' }),
      target,
      project({ id: 3, path: 'C:/other', browserLastUrl: 'https://unrelated-last.example.com' }),
    ];

    expect(browserUrlForProjectFamily(
      target,
      projects,
      tabsByScope([
        ['project:3', [{ id: 'other-active', url: 'https://other-active.example.com' }]],
      ]),
      activeTabs([['project:3', 'other-active']]),
    )).toBeUndefined();
  });

  it('withBrowserLastUrlForProjectFamily syncs root and worktree URLs only', () => {
    const projects = [
      project({ id: 1, path: 'C:/repo', browserLastUrl: 'https://old.example.com' }),
      project({ id: 2, path: 'C:/repo-worktrees/feature', repoRoot: 'C:/repo', isWorktree: true }),
      project({ id: 3, path: 'C:/other', browserLastUrl: 'https://other.example.com' }),
    ];

    const updated = withBrowserLastUrlForProjectFamily(projects, 2, 'https://shared.example.com');

    expect(updated[0].browserLastUrl).toBe('https://shared.example.com');
    expect(updated[1].browserLastUrl).toBe('https://shared.example.com');
    expect(updated[2].browserLastUrl).toBe('https://other.example.com');
  });

  it('withoutBrowserLastUrlForProjectFamily clears root and worktree URLs only', () => {
    const projects = [
      project({ id: 1, path: 'C:/repo', browserLastUrl: 'https://root.example.com' }),
      project({ id: 2, path: 'C:/repo-worktrees/feature', repoRoot: 'C:/repo', isWorktree: true, browserLastUrl: 'https://worktree.example.com' }),
      project({ id: 3, path: 'C:/other', browserLastUrl: 'https://other.example.com' }),
    ];

    const updated = withoutBrowserLastUrlForProjectFamily(projects, 2);

    expect(updated[0].browserLastUrl).toBeUndefined();
    expect(updated[1].browserLastUrl).toBeUndefined();
    expect(updated[2].browserLastUrl).toBe('https://other.example.com');
  });
});
