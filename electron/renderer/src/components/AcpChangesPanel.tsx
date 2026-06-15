import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { GitFileDiff, SourceControlFile, SourceControlStatus } from '../../../shared/types';
import {
  acpChangeStatusAbbreviation,
  acpDiffLineClass,
  acpDiffTotals,
  groupAcpChanges,
  nextSelectedChangePath,
} from '../lib/acpChanges';
import { repairMojibakeDisplay } from '../lib/mojibake';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown> } }).mergenApi;

interface AcpChangesPanelProps {
  repoPath: string;
  refreshKey: number;
}

export const AcpChangesPanel: React.FC<AcpChangesPanelProps> = ({ repoPath, refreshKey }) => {
  const [files, setFiles] = useState<SourceControlFile[]>([]);
  const [branch, setBranch] = useState('');
  const [ahead, setAhead] = useState(0);
  const [behind, setBehind] = useState(0);
  const [error, setError] = useState<string | undefined>();
  const [selectedPath, setSelectedPath] = useState<string | undefined>();
  const [diff, setDiff] = useState<GitFileDiff | undefined>();
  const [loading, setLoading] = useState(false);
  const [diffLoading, setDiffLoading] = useState(false);
  const [collapsed, setCollapsed] = useState(true);
  const statusRequestIdRef = useRef(0);

  const refreshStatus = useCallback(async () => {
    const requestId = ++statusRequestIdRef.current;
    setLoading(true);
    try {
      const status = await api.invoke('git:status', repoPath, false) as SourceControlStatus;
      if (requestId !== statusRequestIdRef.current) return;
      setFiles(status.files);
      setBranch(status.branch);
      setAhead(status.ahead);
      setBehind(status.behind);
      setError(status.error);
      setSelectedPath((previous) => nextSelectedChangePath(previous, status.files));
    } catch (err) {
      if (requestId !== statusRequestIdRef.current) return;
      setFiles([]);
      setBranch('');
      setAhead(0);
      setBehind(0);
      setError(err instanceof Error ? err.message : String(err));
      setSelectedPath(undefined);
    } finally {
      if (requestId === statusRequestIdRef.current) {
        setLoading(false);
      }
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
    setDiff(undefined);
    setDiffLoading(true);
    api.invoke('git:fileDiff', { repoPath, filePath: selectedPath })
      .then((result) => {
        if (!cancelled) setDiff(result as GitFileDiff);
      })
      .catch((err) => {
        if (!cancelled) {
          setDiff({
            status: 'error',
            filePath: selectedPath,
            patch: '',
            addedLines: 0,
            removedLines: 0,
            binary: false,
            error: err instanceof Error ? err.message : String(err),
          });
        }
      })
      .finally(() => {
        if (!cancelled) setDiffLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [repoPath, selectedPath, refreshKey]);

  const groups = useMemo(() => groupAcpChanges(files), [files]);
  const totals = useMemo(() => acpDiffTotals(diff), [diff]);
  const branchMeta = useMemo(() => {
    const parts: string[] = [];
    if (ahead > 0) parts.push(`ahead ${ahead}`);
    if (behind > 0) parts.push(`behind ${behind}`);
    return parts.length > 0 ? parts.join(' / ') : 'Local';
  }, [ahead, behind]);
  const selectedFile = files.find((file) => file.path === selectedPath);

  return (
    <aside className={`acp-changes-panel ${collapsed ? 'is-collapsed' : ''}`}>
      <div className="acp-changes-header" onClick={() => setCollapsed((v) => !v)} style={{ cursor: 'pointer' }}>
        <div className="acp-changes-heading">
          <div className="acp-changes-title-row">
            <span className="acp-changes-icon" aria-hidden="true">⑂</span>
            <span className="acp-changes-title">Source Control</span>
            <span className="acp-changes-count">{files.length}</span>
            <span className="acp-changes-chevron">{collapsed ? '◂' : '▸'}</span>
          </div>
          {!collapsed && (
            <div className="acp-changes-branch" data-tooltip={repoPath}>
              <span>{branch || 'No branch'}</span>
              <span>{branchMeta}</span>
            </div>
          )}
        </div>
        {!collapsed && (
          <button className="acp-icon-button" onClick={(e) => { e.stopPropagation(); refreshStatus(); }} data-tooltip="Refresh changes" aria-label="Refresh changes">
            ↻
          </button>
        )}
      </div>

      {!collapsed && (
        <>
          <div className="acp-changes-files">
            {loading && <div className="acp-changes-empty">Loading changes...</div>}
            {!loading && error && <div className="acp-changes-empty is-error">{error}</div>}
            {!loading && !error && files.length === 0 && <div className="acp-changes-empty">Working tree is clean</div>}
            {!loading && !error && (
              <>
                <ChangeSection
                  title="Staged Changes"
                  files={groups.staged}
                  selectedPath={selectedPath}
                  onSelect={setSelectedPath}
                />
                <ChangeSection
                  title="Changes"
                  files={groups.unstaged}
                  selectedPath={selectedPath}
                  onSelect={setSelectedPath}
                />
              </>
            )}
          </div>

          <div className="acp-diff-header">
            <span>{repairMojibakeDisplay(selectedFile?.path || selectedPath || 'No file selected')}</span>
            {totals && <span className="acp-diff-totals">{totals}</span>}
          </div>
          <div className="acp-diff-view">
            {diffLoading && <div className="acp-changes-empty">Loading diff...</div>}
            {!diffLoading && diff?.status === 'error' && <div className="acp-changes-empty is-error">{diff.error || 'Diff unavailable'}</div>}
            {!diffLoading && diff?.status === 'ready' && diff.binary && <div className="acp-changes-empty">Binary file diff</div>}
            {!diffLoading && diff?.status === 'ready' && !diff.binary && (
              diff.patch.trim().length > 0
                ? <UnifiedDiff patch={diff.patch} />
                : <div className="acp-changes-empty">No text diff</div>
            )}
          </div>
        </>
      )}
    </aside>
  );
};

const ChangeSection: React.FC<{
  title: string;
  files: SourceControlFile[];
  selectedPath: string | undefined;
  onSelect: (path: string) => void;
}> = ({ title, files, selectedPath, onSelect }) => {
  if (files.length === 0) return null;
  return (
    <section className="acp-change-section">
      <div className="acp-change-section-title">{title} <span>{files.length}</span></div>
      {files.map((file) => (
        <button
          key={`${file.staged ? 'staged' : 'unstaged'}:${file.path}`}
          className={`acp-change-file ${file.path === selectedPath ? 'is-selected' : ''}`}
          onClick={() => onSelect(file.path)}
          data-tooltip={file.path}
        >
          <span className={`acp-change-state ${file.staged ? 'is-staged' : ''}`}>{acpChangeStatusAbbreviation(file.status)}</span>
          <span className="acp-change-path">
            <span className="acp-change-basename">{repairMojibakeDisplay(file.path.split(/[\\/]/).pop() || file.path)}</span>
            <span className="acp-change-dir">{repairMojibakeDisplay(parentPath(file.path))}</span>
          </span>
          <span className="acp-change-status">{file.status}</span>
        </button>
      ))}
    </section>
  );
};

const UnifiedDiff: React.FC<{ patch: string }> = ({ patch }) => {
  return (
    <pre className="acp-unified-diff">
      {patch.split(/\r?\n/).map((line, index) => (
        <div key={`${index}-${line}`} className={`acp-diff-line ${acpDiffLineClass(line)}`}>
          <span className="acp-diff-line-no">{index + 1}</span>
          <span className="acp-diff-line-text">{line || ' '}</span>
        </div>
      ))}
    </pre>
  );
};

function parentPath(filePath: string): string {
  const parts = filePath.split(/[\\/]/);
  parts.pop();
  return parts.join('/');
}
