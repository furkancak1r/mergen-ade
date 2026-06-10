export interface GitDiffSummary {
  status: 'ready' | 'error';
  addedLines: number;
  removedLines: number;
  error?: string;
}

export function parseGitNumstatLine(line: string): { added: number; removed: number } {
  const columns = line.split('\t', 3);
  if (columns.length < 2) return { added: 0, removed: 0 };

  const added = Number.parseInt(columns[0], 10);
  const removed = Number.parseInt(columns[1], 10);
  return {
    added: Number.isFinite(added) ? added : 0,
    removed: Number.isFinite(removed) ? removed : 0,
  };
}

export function parseGitNumstatTotals(stdout: string): { addedLines: number; removedLines: number } {
  let addedLines = 0;
  let removedLines = 0;

  for (const line of stdout.split(/\r?\n/)) {
    if (!line) continue;
    const parsed = parseGitNumstatLine(line);
    addedLines += parsed.added;
    removedLines += parsed.removed;
  }

  return { addedLines, removedLines };
}

export function gitDiffSummaryLabel(summary: GitDiffSummary | undefined, loading: boolean): string {
  if (loading) return '...';
  if (!summary || summary.status === 'error') return '--';
  if (summary.addedLines === 0 && summary.removedLines === 0) return '';
  return `+${summary.addedLines} -${summary.removedLines}`;
}
