import type { GitFileDiff, SourceControlFile } from '../../../shared/types';

export interface AcpChangeGroups {
  staged: SourceControlFile[];
  unstaged: SourceControlFile[];
}

export function groupAcpChanges(files: readonly SourceControlFile[]): AcpChangeGroups {
  return {
    staged: files.filter((file) => file.staged),
    unstaged: files.filter((file) => !file.staged),
  };
}

export function acpDiffTotals(diff: GitFileDiff | undefined): string | undefined {
  if (!diff || diff.status !== 'ready') return undefined;
  return `+${diff.addedLines} -${diff.removedLines}`;
}

export function nextSelectedChangePath(
  previousPath: string | undefined,
  files: readonly SourceControlFile[],
): string | undefined {
  if (previousPath && files.some((file) => file.path === previousPath)) return previousPath;
  return files[0]?.path;
}

export function acpChangeStatusAbbreviation(status: string): string {
  const normalized = status.trim().toLowerCase();
  if (normalized.startsWith('modified')) return 'M';
  if (normalized.startsWith('added')) return 'A';
  if (normalized.startsWith('deleted')) return 'D';
  if (normalized.startsWith('renamed')) return 'R';
  if (normalized.startsWith('copied')) return 'C';
  if (normalized.startsWith('untracked')) return 'U';
  if (normalized.startsWith('conflicted')) return '!';
  return 'C';
}

export function acpDiffLineClass(line: string): string {
  if (line.startsWith('+++') || line.startsWith('---')) return 'is-file';
  if (line.startsWith('@@')) return 'is-hunk';
  if (line.startsWith('+')) return 'is-added';
  if (line.startsWith('-')) return 'is-removed';
  return '';
}
