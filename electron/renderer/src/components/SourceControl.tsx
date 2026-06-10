import React, { useEffect, useRef, useState, useCallback } from 'react';
import type { ProjectRecord, GitWorktreeInfo, SourceControlSnapshot, SourceControlStatus } from '../../../shared/types';
import { sourceControlBranchLine, sourceControlFileAbsolutePath, sourceControlStatusLabel } from '../../../shared/sourceControl';
import { repairMojibakeDisplay } from '../lib/mojibake';
import { sanitizeWorktreeSlug } from '../lib/worktree';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown> } }).mergenApi;

interface SourceControlProps {
  project: ProjectRecord;
  onAddWorktree?: (worktree: GitWorktreeInfo) => void;
  onRemoveWorktree?: (worktree: GitWorktreeInfo) => void;
  onDeleteGitWorktree?: (worktree: GitWorktreeInfo) => void;
  hasLiveTerminals?: (path: string) => boolean;
  registeredWorktreePaths?: string[];
  onOrphanWorktrees?: (paths: string[]) => void;
  onBranchChange?: (branch: string) => void;
}

export const SourceControl: React.FC<SourceControlProps> = ({ project, onAddWorktree, onRemoveWorktree, onDeleteGitWorktree, hasLiveTerminals, registeredWorktreePaths, onOrphanWorktrees, onBranchChange }) => {
  const [snapshot, setSnapshot] = useState<SourceControlSnapshot>({ loading: true, files: [], worktrees: [] });
  const [query, setQuery] = useState('');
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [createBranch, setCreateBranch] = useState('');
  const [createBaseBranch, setCreateBaseBranch] = useState('');
  const [createLoading, setCreateLoading] = useState(false);
  const [fileContextMenu, setFileContextMenu] = useState<{ x: number; y: number; filePath: string; relativePath: string } | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);
  const feedbackTimerRef = useRef<number | null>(null);

  const refresh = useCallback(async (manual = false) => {
    if (manual) {
      setSnapshot((prev) => ({ ...prev, loading: true }));
    }
    const result = await api.invoke('git:status', project.path) as SourceControlStatus;
    const worktrees = await api.invoke('git:discoverWorktrees', project.path) as GitWorktreeInfo[];
    setSnapshot({
      loading: false,
      files: result.files,
      worktrees: worktrees.filter((w) => w.path !== project.path),
      branch: result.branch,
      ahead: result.ahead,
      behind: result.behind,
      lastUpdated: Date.now(),
    });
  }, [project.path]);

  useEffect(() => {
    refresh();
    const interval = setInterval(() => refresh(false), 30000);
    return () => clearInterval(interval);
  }, [refresh]);

  useEffect(() => {
    if (snapshot.branch) {
      onBranchChange?.(snapshot.branch);
    }
  }, [snapshot.branch, onBranchChange]);

  useEffect(() => () => {
    if (feedbackTimerRef.current !== null) {
      window.clearTimeout(feedbackTimerRef.current);
    }
  }, []);

  useEffect(() => {
    if (!fileContextMenu) return undefined;

    const closeMenu = () => setFileContextMenu(null);
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') closeMenu();
    };
    window.addEventListener('click', closeMenu);
    window.addEventListener('keydown', closeOnEscape);
    return () => {
      window.removeEventListener('click', closeMenu);
      window.removeEventListener('keydown', closeOnEscape);
    };
  }, [fileContextMenu]);

  const showFeedback = useCallback((message: string) => {
    setFeedback(message);
    if (feedbackTimerRef.current !== null) {
      window.clearTimeout(feedbackTimerRef.current);
    }
    feedbackTimerRef.current = window.setTimeout(() => {
      setFeedback(null);
      feedbackTimerRef.current = null;
    }, 1600);
  }, []);

  // Orphan worktree cleanup: auto-remove registered worktrees that no longer exist
  useEffect(() => {
    if (!registeredWorktreePaths || registeredWorktreePaths.length === 0) return;
    if (snapshot.loading) return;
    const discoveredPaths = new Set(snapshot.worktrees.map((w) => w.path));
    async function checkOrphans() {
      const orphans: string[] = [];
      for (const wpath of registeredWorktreePaths!) {
        if (discoveredPaths.has(wpath)) continue;
        const exists = await api.invoke('fs:exists', wpath) as boolean;
        if (!exists) {
          orphans.push(wpath);
        }
      }
      if (orphans.length > 0) {
        onOrphanWorktrees?.(orphans);
      }
    }
    checkOrphans();
  }, [snapshot.worktrees, snapshot.loading, registeredWorktreePaths, onOrphanWorktrees]);

  const statusLabel = (status: string): string => {
    return status.length === 1 ? sourceControlStatusLabel(status) : status;
  };

  const filteredFiles = snapshot.files.filter((f) => {
    if (!query) return true;
    const q = query.toLowerCase();
    const stagedLabel = f.staged ? 'staged' : 'unstaged';
    return f.path.toLowerCase().includes(q) || statusLabel(f.status).toLowerCase().includes(q) || stagedLabel.includes(q);
  });

  const filteredWorktrees = snapshot.worktrees.filter((w) => {
    if (!query) return true;
    const q = query.toLowerCase();
    return w.branch.toLowerCase().includes(q) || w.path.toLowerCase().includes(q);
  });
  const branchLine = sourceControlBranchLine(snapshot);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden', position: 'relative' }}>
      <div style={{ padding: '8px 12px', borderBottom: '1px solid #222', display: 'flex', alignItems: 'center', gap: 8 }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: '#eee' }}>
          {repairMojibakeDisplay(project.name)}
        </span>
        <span style={{ fontSize: 11, color: '#888' }}>
          {snapshot.files.length > 0 ? `${snapshot.files.length} changes` : 'Clean'}
        </span>
        {branchLine && (
          <span style={{ fontSize: 11, color: '#888', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            {repairMojibakeDisplay(branchLine)}
          </span>
        )}
        <button
          onClick={() => setShowCreateModal(true)}
          style={{
            marginLeft: 'auto',
            padding: '2px 8px',
            fontSize: 11,
            background: '#1a1a1a',
            border: '1px solid #333',
            color: '#ccc',
            borderRadius: 3,
            cursor: 'pointer',
            marginRight: 4,
          }}
        >
          + Worktree
        </button>
        <button
          onClick={() => refresh(true)}
          style={{
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
                  {repairMojibakeDisplay(w.branch || '(detached)')}
                </span>
                <span style={{ fontSize: 10, color: '#666' }}>{repairMojibakeDisplay(w.path)}</span>
                {registeredWorktreePaths?.includes(w.path) ? (
                  <button
                    onClick={() => onRemoveWorktree?.(w)}
                    style={{ fontSize: 10, padding: '2px 6px', background: '#1a1a1a', border: '1px solid #333', color: '#ccc', borderRadius: 3, cursor: 'pointer' }}
                  >
                    Remove from Mergen
                  </button>
                ) : (
                  <button
                    onClick={() => onAddWorktree?.(w)}
                    style={{ fontSize: 10, padding: '2px 6px', background: '#1a1a1a', border: '1px solid #333', color: '#ccc', borderRadius: 3, cursor: 'pointer' }}
                  >
                    Add to Mergen
                  </button>
                )}
                <button
                  onClick={async () => {
                    if (hasLiveTerminals?.(w.path)) {
                      alert('Cannot delete: worktree has live terminals.');
                      return;
                    }
                    // Check worktree-specific uncommitted changes
                    const worktreeStatus = await api.invoke('git:status', w.path) as SourceControlStatus;
                    if (worktreeStatus.files.length > 0) {
                      alert('Cannot delete: worktree has uncommitted changes.');
                      return;
                    }
                    if (window.confirm(`Delete git worktree at ${w.path}? This will remove the worktree from disk.`)) {
                      onDeleteGitWorktree?.(w);
                    }
                  }}
                  style={{ fontSize: 10, padding: '2px 6px', background: '#1a1a1a', border: '1px solid #333', color: '#c44', borderRadius: 3, cursor: 'pointer' }}
                >
                  Delete Git Worktree
                </button>
              </div>
            ))}
          </div>
        )}

        {filteredFiles.length > 0 && (
          <div>
            <div style={{ padding: '4px 12px', fontSize: 11, color: '#888', fontWeight: 600 }}>Changed Files</div>
            {filteredFiles.map((f) => {
              const filePath = sourceControlFileAbsolutePath(project.path, f.path);
              return (
              <div
                key={f.path}
                style={{ padding: '3px 12px', display: 'flex', alignItems: 'center', gap: 6, minWidth: 0, cursor: 'context-menu' }}
                onContextMenu={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  setFileContextMenu({ x: event.clientX, y: event.clientY, filePath, relativePath: f.path });
                }}
              >
                <span style={{
                  fontSize: 10,
                  fontWeight: 600,
                  color: f.staged ? '#64c864' : '#e8a838',
                  width: 14,
                  textAlign: 'center',
                  flexShrink: 0,
                }}>
                  {f.staged ? '✓' : '◷'}
                </span>
                <span style={{ fontSize: 11, color: '#aaa', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', fontFamily: 'ui-monospace, SFMono-Regular, Consolas, monospace' }}>
                  {repairMojibakeDisplay(`${statusLabel(f.status)} ${f.path}`)}
                </span>
              </div>
              );
            })}
          </div>
        )}

        {query && filteredFiles.length === 0 && filteredWorktrees.length === 0 && (
          <div style={{ padding: 12, color: '#888', fontSize: 12 }}>No matching files or worktrees.</div>
        )}
      </div>

      {feedback && (
        <div className="source-control-feedback-toast" role="status">
          {feedback}
        </div>
      )}

      {fileContextMenu && (
        <div
          className="source-control-context-menu"
          style={{ left: fileContextMenu.x, top: fileContextMenu.y }}
          onClick={(event) => event.stopPropagation()}
        >
          <button
            type="button"
            onClick={() => {
              api.invoke('shell:showItemInFolder', fileContextMenu.filePath)
                .then(() => showFeedback('Opened containing folder'))
                .catch((error) => showFeedback(`Open folder failed: ${error instanceof Error ? error.message : String(error)}`));
              setFileContextMenu(null);
            }}
          >
            Open in Folder
          </button>
          <button
            type="button"
            onClick={() => {
              api.invoke('clipboard:writeText', fileContextMenu.relativePath).catch(() => {});
              showFeedback('Copied relative path');
              setFileContextMenu(null);
            }}
          >
            Copy Relative Path
          </button>
        </div>
      )}
      {/* Create Worktree Modal */}
      {showCreateModal && (
        <div
          style={{
            position: 'fixed',
            inset: 0,
            background: 'rgba(0,0,0,0.6)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            zIndex: 1000,
          }}
          onClick={(e) => {
            if (e.target === e.currentTarget) setShowCreateModal(false);
          }}
        >
          <div
            style={{
              background: '#141414',
              border: '1px solid #333',
              borderRadius: 8,
              width: 480,
              maxWidth: '90vw',
              padding: '16px',
              display: 'flex',
              flexDirection: 'column',
              gap: 12,
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <span style={{ fontSize: 14, fontWeight: 600, color: '#eee' }}>Create Worktree</span>
              <button onClick={() => setShowCreateModal(false)} style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 14 }}>
                ✕
              </button>
            </div>

            <div>
              <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 4 }}>Branch name</div>
              <input
                type="text"
                value={createBranch}
                onChange={(e) => setCreateBranch(e.target.value)}
                placeholder="feature/my-branch"
                style={{
                  width: '100%',
                  background: '#1a1a1a',
                  border: '1px solid #333',
                  color: '#ccc',
                  padding: '4px 8px',
                  fontSize: 12,
                  borderRadius: 4,
                  outline: 'none',
                }}
              />
            </div>

            <div>
              <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 4 }}>Base branch (optional)</div>
              <input
                type="text"
                value={createBaseBranch}
                onChange={(e) => setCreateBaseBranch(e.target.value)}
                placeholder="main"
                style={{
                  width: '100%',
                  background: '#1a1a1a',
                  border: '1px solid #333',
                  color: '#ccc',
                  padding: '4px 8px',
                  fontSize: 12,
                  borderRadius: 4,
                  outline: 'none',
                }}
              />
            </div>

            <div>
              <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 4 }}>Path</div>
              <input
                type="text"
                readOnly
                value={createBranch ? `../worktrees/${sanitizeWorktreeSlug(createBranch)}` : ''}
                placeholder="Auto-generated from branch name"
                style={{
                  width: '100%',
                  background: '#1a1a1a',
                  border: '1px solid #333',
                  color: '#888',
                  padding: '4px 8px',
                  fontSize: 12,
                  borderRadius: 4,
                  outline: 'none',
                }}
              />
            </div>

            {/* Existing unregistered worktrees */}
            {snapshot.worktrees.filter((w) => !registeredWorktreePaths?.includes(w.path)).length > 0 && (
              <div>
                <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 4 }}>Existing unregistered worktrees</div>
                <div style={{ maxHeight: 120, overflow: 'auto', display: 'flex', flexDirection: 'column', gap: 4 }}>
                  {snapshot.worktrees
                    .filter((w) => !registeredWorktreePaths?.includes(w.path))
                    .map((w) => (
                      <div key={w.path} style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '4px 8px', background: '#1a1a1a', borderRadius: 4 }}>
                        <span style={{ fontSize: 11, color: '#aaa', flex: 1 }}>{repairMojibakeDisplay(w.branch || '(detached)')}</span>
                        <span style={{ fontSize: 10, color: '#666' }}>{repairMojibakeDisplay(w.path)}</span>
                        <button
                          onClick={() => {
                            onAddWorktree?.(w);
                          }}
                          style={{ fontSize: 10, padding: '2px 6px', background: '#1a1a1a', border: '1px solid #333', color: '#ccc', borderRadius: 3, cursor: 'pointer' }}
                        >
                          Add to Mergen
                        </button>
                      </div>
                    ))}
                </div>
              </div>
            )}

            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8, marginTop: 4 }}>
              <button
                onClick={() => setShowCreateModal(false)}
                style={{ padding: '6px 16px', fontSize: 12, background: 'transparent', border: '1px solid #333', color: '#ccc', borderRadius: 4, cursor: 'pointer' }}
              >
                Cancel
              </button>
              <button
                disabled={!createBranch || createLoading}
                onClick={async () => {
                  if (!createBranch) return;
                  setCreateLoading(true);
                  const slug = sanitizeWorktreeSlug(createBranch);
                  const parentDir = project.path.replace(/[\\/][^\\/]+$/, '');
                  const wtPath = parentDir + '/' + 'worktrees/' + slug;
                  const ok = await api.invoke('git:createWorktree', project.path, createBranch, wtPath, createBaseBranch || undefined) as boolean;
                  setCreateLoading(false);
                  if (ok) {
                    onAddWorktree?.({ path: wtPath, branch: createBranch, head: '', detached: false, locked: false, prunable: false });
                    setShowCreateModal(false);
                    setCreateBranch('');
                    setCreateBaseBranch('');
                    refresh(true);
                  } else {
                    alert('Failed to create worktree. Check if the branch already exists or the path is valid.');
                  }
                }}
                style={{
                  padding: '6px 16px',
                  fontSize: 12,
                  background: '#1f3a4c',
                  border: '1px solid #1f3a4c',
                  color: '#ccc',
                  borderRadius: 4,
                  cursor: !createBranch || createLoading ? 'not-allowed' : 'pointer',
                  opacity: !createBranch || createLoading ? 0.5 : 1,
                }}
              >
                {createLoading ? 'Creating...' : 'Create'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
