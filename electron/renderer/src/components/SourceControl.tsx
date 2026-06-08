import React, { useEffect, useState, useCallback } from 'react';
import type { ProjectRecord, GitWorktreeInfo, SourceControlSnapshot } from '../../../shared/types';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown> } }).mergenApi;

interface SourceControlProps {
  project: ProjectRecord;
}

export const SourceControl: React.FC<SourceControlProps> = ({ project }) => {
  const [snapshot, setSnapshot] = useState<SourceControlSnapshot>({ loading: true, files: [], worktrees: [] });
  const [query, setQuery] = useState('');

  const refresh = useCallback(async () => {
    setSnapshot((prev) => ({ ...prev, loading: true }));
    const result = await api.invoke('git:status', project.path) as { files: { path: string; status: string; staged: boolean }[]; branch: string };
    const worktrees = await api.invoke('git:discoverWorktrees', project.path) as GitWorktreeInfo[];
    setSnapshot({
      loading: false,
      files: result.files,
      worktrees: worktrees.filter((w) => w.path !== project.path),
      lastUpdated: Date.now(),
    });
  }, [project.path]);

  useEffect(() => {
    refresh();
    const interval = setInterval(refresh, 30000);
    return () => clearInterval(interval);
  }, [refresh]);

  const statusLabel = (status: string): string => {
    const map: Record<string, string> = {
      M: 'Modified', A: 'Added', D: 'Deleted', R: 'Renamed', C: 'Copied', U: 'Updated', '?': 'Untracked',
    };
    return map[status] || status;
  };

  const filteredFiles = snapshot.files.filter((f) => {
    if (!query) return true;
    const q = query.toLowerCase();
    return f.path.toLowerCase().includes(q) || statusLabel(f.status).toLowerCase().includes(q);
  });

  const filteredWorktrees = snapshot.worktrees.filter((w) => {
    if (!query) return true;
    const q = query.toLowerCase();
    return w.branch.toLowerCase().includes(q) || w.path.toLowerCase().includes(q);
  });

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
      <div style={{ padding: '8px 12px', borderBottom: '1px solid #222', display: 'flex', alignItems: 'center', gap: 8 }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: '#eee' }}>
          {project.name}
        </span>
        <span style={{ fontSize: 11, color: '#888' }}>
          {snapshot.files.length > 0 ? `${snapshot.files.length} changes` : 'Clean'}
        </span>
        <button
          onClick={refresh}
          style={{
            marginLeft: 'auto',
            padding: '2px 8px',
            fontSize: 11,
            background: '#1a1a1a',
            border: '1px solid #333',
            color: '#ccc',
            borderRadius: 3,
            cursor: 'pointer',
          }}
        >
          ↻
        </button>
      </div>
      <div style={{ padding: '6px 12px', borderBottom: '1px solid #222' }}>
        <input
          type="text"
          placeholder="Search files..."
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          style={{
            width: '100%',
            background: '#1a1a1a',
            border: '1px solid #333',
            borderRadius: 4,
            padding: '4px 8px',
            color: '#ccc',
            fontSize: 12,
            outline: 'none',
          }}
        />
      </div>
      <div style={{ flex: 1, overflow: 'auto', padding: '4px 0' }}>
        {snapshot.loading && <div style={{ padding: 12, color: '#888', fontSize: 12 }}>Refreshing source control...</div>}
        {!snapshot.loading && snapshot.files.length === 0 && snapshot.worktrees.length === 0 && (
          <div style={{ padding: 12, color: '#888', fontSize: 12 }}>Working tree clean.</div>
        )}

        {filteredWorktrees.length > 0 && (
          <div style={{ marginBottom: 8 }}>
            <div style={{ padding: '4px 12px', fontSize: 11, color: '#888', fontWeight: 600 }}>Worktrees</div>
            {filteredWorktrees.map((w) => (
              <div key={w.path} style={{ padding: '3px 12px', display: 'flex', alignItems: 'center', gap: 6 }}>
                <span style={{ fontSize: 10 }}>🌿</span>
                <span style={{ fontSize: 11, color: '#aaa', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis' }}>
                  {w.branch || '(detached)'}
                </span>
                <span style={{ fontSize: 10, color: '#666' }}>{w.path}</span>
              </div>
            ))}
          </div>
        )}

        {filteredFiles.length > 0 && (
          <div>
            <div style={{ padding: '4px 12px', fontSize: 11, color: '#888', fontWeight: 600 }}>Changed Files</div>
            {filteredFiles.map((f) => (
              <div key={f.path} style={{ padding: '3px 12px', display: 'flex', alignItems: 'center', gap: 6 }}>
                <span style={{
                  fontSize: 9,
                  fontWeight: 600,
                  color: f.staged ? '#64c864' : '#e8a838',
                  width: 14,
                  textAlign: 'center',
                }}>
                  {f.status}
                </span>
                <span style={{ fontSize: 11, color: '#aaa', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis' }}>
                  {f.path}
                </span>
              </div>
            ))}
          </div>
        )}

        {query && filteredFiles.length === 0 && filteredWorktrees.length === 0 && (
          <div style={{ padding: 12, color: '#888', fontSize: 12 }}>No matching files or worktrees.</div>
        )}
      </div>
    </div>
  );
};
