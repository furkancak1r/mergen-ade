import type { GitWorktreeInfo } from '../../../shared/types';

export function parseGitWorktreeList(output: string): GitWorktreeInfo[] {
  const worktrees: GitWorktreeInfo[] = [];
  const lines = output.split('\n');
  let current: Partial<GitWorktreeInfo> | null = null;

  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed) {
      if (current && current.path) {
        worktrees.push({
          path: current.path,
          branch: current.branch ?? '',
          head: current.head,
          detached: current.detached ?? false,
          locked: current.locked ?? false,
          prunable: current.prunable ?? false,
        });
      }
      current = null;
      continue;
    }

    if (trimmed.startsWith('worktree ')) {
      current = { path: trimmed.slice(9).trim() };
    } else if (current) {
      if (trimmed.startsWith('HEAD ')) {
        current.head = trimmed.slice(5).trim();
      } else if (trimmed.startsWith('branch ')) {
        current.branch = trimmed.slice(7).trim().replace(/^refs\/heads\//, '');
      } else if (trimmed === 'detached') {
        current.detached = true;
      } else if (trimmed.startsWith('locked ')) {
        current.locked = true;
      } else if (trimmed === 'locked') {
        current.locked = true;
      } else if (trimmed === 'prunable') {
        current.prunable = true;
      }
    }
  }

  if (current && current.path) {
    worktrees.push({
      path: current.path,
      branch: current.branch ?? '',
      head: current.head,
      detached: current.detached ?? false,
      locked: current.locked ?? false,
      prunable: current.prunable ?? false,
    });
  }

  return worktrees;
}

export function sanitizeWorktreeSlug(branchName: string): string {
  return branchName
    .toLowerCase()
    .replace(/[\\/ :*?"<>|]/g, '-')
    .replace(/\.\./g, '-')
    .replace(/^-+|-+$/g, '');
}

function pathSeparatorFor(repoPath: string): '/' | '\\' {
  const lastSlash = repoPath.lastIndexOf('/');
  const lastBackslash = repoPath.lastIndexOf('\\');
  return lastBackslash > lastSlash ? '\\' : '/';
}

function trimTrailingSeparators(path: string): string {
  if (path === '/' || /^[A-Za-z]:[\\/]$/.test(path)) {
    return path;
  }
  return path.replace(/[\\/]+$/g, '');
}

function parentPathOf(repoPath: string): string {
  const trimmed = trimTrailingSeparators(repoPath);
  const lastSlash = trimmed.lastIndexOf('/');
  const lastBackslash = trimmed.lastIndexOf('\\');
  const idx = Math.max(lastSlash, lastBackslash);

  if (idx < 0) {
    return trimmed;
  }
  if (idx === 0) {
    return trimmed.slice(0, 1);
  }
  if (idx === 2 && /^[A-Za-z]:/.test(trimmed)) {
    return trimmed.slice(0, 3);
  }
  return trimmed.slice(0, idx);
}

function joinPath(base: string, separator: '/' | '\\', ...segments: string[]): string {
  return segments.reduce((current, segment) => {
    if (!current) {
      return segment;
    }
    if (current.endsWith('/') || current.endsWith('\\')) {
      return `${current}${segment}`;
    }
    return `${current}${separator}${segment}`;
  }, base);
}

export function defaultWorktreePathForBranch(repoPath: string, branchName: string): string {
  const separator = pathSeparatorFor(repoPath);
  const parent = parentPathOf(repoPath);
  return joinPath(parent, separator, 'worktrees', sanitizeWorktreeSlug(branchName));
}
