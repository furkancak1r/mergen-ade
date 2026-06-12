import React, { useCallback, useEffect, useMemo, useState } from 'react';
import type { GitFileDiff, SourceControlFile, SourceControlStatus } from '../../../shared/types';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown> } }).mergenApi;

interface AcpChangesPanelProps {
  repoPath: string;
  refreshKey: number;
}

export const AcpChangesPanel: React.FC<AcpChangesPanelProps> = ({ repoPath, refreshKey }) => {
  const [files, setFiles] = useState<SourceControlFile[]>([]);
  const [branch, setBranch] = useState('');
  const [error, setError] = useState<string | undefined>();
  const [selectedPath, setSelectedPath] = useState<string | undefined>();
  const [diff, setDiff] = useState<GitFileDiff | undefined>();
  const [loading, setLoading] = useState(false);
  const [diffLoading, setDiffLoading] = useState(false);

  const refreshStatus = useCallback(async () => {
    setLoading(true);
    try {
      const status = await api.invoke('git:status', repoPath, false) as SourceControlStatus;
      setFiles(status.files);
      setBranch(status.branch);
      setError(status.error);
      setSelectedPath((previous) => {
        if (previous && status.files.some((file) => file.path === previous)) return previous;
        return status.files[0]?.path;
      });
    } finally {
      setLoading(false);
    }
  }, [repoPath]);

  useEffect(() => {
    refreshStatus();
  }, [refreshStatus, refreshKey]);

  useEffect(() => {
    if (!selectedPath) {
      setDiff(undefined);
      return;
    }
    let cancelled = false;
    setDiffLoading(true);
    api.invoke('git:fileDiff', { repoPath, filePath: selectedPath })
      .then((result) => {
        if (!cancelled) setDiff(result as GitFileDiff);
      })
      .finally(() => {
        if (!cancelled) setDiffLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [repoPath, selectedPath, refreshKey]);

  const totals = useMemo(() => {
    if (!diff || diff.status !== 'ready') return undefined;
    return `+${diff.addedLines} -${diff.removedLines}`;
  }, [diff]);

  return (
    <aside className="acp-changes-panel">
      <div className="acp-changes-header">
        <div>
          <div className="acp-changes-title">Changes</div>
          <div className="acp-changes-branch">{branch || 'No branch'}</div>
        </div>
        <button className="acp-icon-button" onClick={refreshStatus} title="Refresh changes">
          ↻
        </button>
      </div>

      <div className="acp-changes-files">
        {loading && <div className="acp-changes-empty">Loading...</div>}
        {!loading && error && <div className="acp-changes-empty">{error}</div>}
        {!loading && !error && files.length === 0 && <div className="acp-changes-empty">Working tree is clean</div>}
        {!loading && !error && files.map((file) => (
          <button
            key={file.path}
            className={`acp-change-file ${file.path === selectedPath ? 'is-selected' : ''}`}
            onClick={() => setSelectedPath(file.path)}
            title={file.path}
          >
            <span className={`acp-change-state ${file.staged ? 'is-staged' : ''}`}>{file.staged ? 'S' : 'U'}</span>
            <span className="acp-change-path">{file.path}</span>
            <span className="acp-change-status">{file.status}</span>
          </button>
        ))}
      </div>

      <div className="acp-diff-header">
        <span>{selectedPath || 'No file selected'}</span>
        {totals && <span className="acp-diff-totals">{totals}</span>}
      </div>
      <div className="acp-diff-view">
        {diffLoading && <div className="acp-changes-empty">Loading diff...</div>}
        {!diffLoading && diff?.status === 'error' && <div className="acp-changes-empty">{diff.error || 'Diff unavailable'}</div>}
        {!diffLoading && diff?.status === 'ready' && diff.binary && <div className="acp-changes-empty">Binary file diff</div>}
        {!diffLoading && diff?.status === 'ready' && !diff.binary && (
          diff.patch.trim().length > 0
            ? <UnifiedDiff patch={diff.patch} />
            : <div className="acp-changes-empty">No text diff</div>
        )}
      </div>
    </aside>
  );
};

const UnifiedDiff: React.FC<{ patch: string }> = ({ patch }) => {
  return (
    <pre className="acp-unified-diff">
      {patch.split(/\r?\n/).map((line, index) => (
        <div key={`${index}-${line}`} className={`acp-diff-line ${diffLineClass(line)}`}>
          <span className="acp-diff-line-no">{index + 1}</span>
          <span className="acp-diff-line-text">{line || ' '}</span>
        </div>
      ))}
    </pre>
  );
};

function diffLineClass(line: string): string {
  if (line.startsWith('+++') || line.startsWith('---')) return 'is-file';
  if (line.startsWith('@@')) return 'is-hunk';
  if (line.startsWith('+')) return 'is-added';
  if (line.startsWith('-')) return 'is-removed';
  return '';
}
