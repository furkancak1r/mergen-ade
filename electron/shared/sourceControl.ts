import type { GitWorktreeInfo, SourceControlFile, SourceControlSnapshot } from './types';

export interface BranchStatus {
  branch: string;
  ahead: number;
  behind: number;
}

export function parseBranchHeader(header: string): BranchStatus {
  const splitIndex = header.indexOf('...');
  const branchPart = splitIndex >= 0
    ? header.slice(0, splitIndex).trim()
    : (header.trim().split(/\s+/)[0] || 'detached');
  const trackingPart = splitIndex >= 0 ? header.slice(splitIndex + 3) : '';

  let ahead = 0;
  let behind = 0;
  const flagsStart = trackingPart.indexOf('[');
  if (flagsStart >= 0) {
    const flagsEnd = trackingPart.indexOf(']', flagsStart);
    if (flagsEnd > flagsStart) {
      const flags = trackingPart.slice(flagsStart + 1, flagsEnd);
      for (const part of flags.split(',')) {
        const piece = part.trim();
        if (piece.startsWith('ahead ')) {
          ahead = Number.parseInt(piece.slice('ahead '.length), 10) || 0;
        } else if (piece.startsWith('behind ')) {
          behind = Number.parseInt(piece.slice('behind '.length), 10) || 0;
        }
      }
    }
  }

  return {
    branch: branchPart || 'detached',
    ahead,
    behind,
  };
}

export function sourceControlBranchLine(status: Partial<BranchStatus>): string | undefined {
  if (!status.branch) return undefined;
  const ahead = status.ahead ?? 0;
  const behind = status.behind ?? 0;
  if (ahead > 0 || behind > 0) {
    return `${status.branch}  ahead:${ahead} behind:${behind}`;
  }
  return status.branch;
}

export function sourceControlSnapshotHasDisplayData(snapshot: Pick<SourceControlSnapshot, 'branch' | 'ahead' | 'behind' | 'files'>): boolean {
  return Boolean(snapshot.branch)
    || (snapshot.ahead ?? 0) > 0
    || (snapshot.behind ?? 0) > 0
    || snapshot.files.length > 0;
}

export function sourceControlStatusLabel(statusCode: string): string {
  switch (statusCode) {
    case 'M':
      return 'Modified';
    case 'A':
      return 'Added';
    case 'D':
      return 'Deleted';
    case 'R':
      return 'Renamed';
    case 'C':
      return 'Copied';
    case 'U':
      return 'Conflicted';
    case '?':
      return 'Untracked';
    case '!':
      return 'Ignored';
    default:
      return 'Changed';
  }
}

export function parseSourceControlStatusLine(line: string): SourceControlFile | undefined {
  if (line.length < 3 || line.startsWith('## ')) {
    return undefined;
  }

  const statusCode = line.slice(0, 2);
  const pathPart = line.slice(3).trim();
  if (!pathPart) return undefined;

  const renamedPath = pathPart.includes(' -> ')
    ? pathPart.split(' -> ').at(-1)?.trim() || pathPart
    : pathPart;
  const x = statusCode[0] ?? ' ';
  const y = statusCode[1] ?? ' ';
  const statusChar = x !== ' ' && x !== '?' ? x : y;

  return {
    path: renamedPath,
    status: sourceControlStatusLabel(statusChar),
    staged: x !== ' ' && x !== '?',
  };
}

export function sourceControlFileAbsolutePath(projectPath: string, filePath: string): string {
  const separator = projectPath.includes('\\') ? '\\' : '/';
  const root = projectPath.replace(/[\\/]+$/, '');
  const relative = filePath.replace(/^[\\/]+/, '');
  return `${root}${separator}${relative}`;
}

export function sourceControlWorktreeLabel(worktree: Pick<GitWorktreeInfo, 'path' | 'branch' | 'head' | 'detached'>): string {
  const branch = worktree.branch.replace(/^refs\/heads\//, '').trim();
  if (branch) return branch;
  if (worktree.detached) {
    const short = worktree.head ? worktree.head.slice(-8) : 'detached';
    return `detached@${short}`;
  }
  return worktree.path.split(/[\\/]/).filter(Boolean).at(-1) || worktree.path;
}
