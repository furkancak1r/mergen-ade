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
