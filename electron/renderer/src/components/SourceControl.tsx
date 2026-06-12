import React, { useEffect, useRef, useState, useCallback } from 'react';
import type { ProjectRecord, GitWorktreeInfo, SourceControlSnapshot, SourceControlStatus } from '../../../shared/types';
import { sourceControlFileAbsolutePath, sourceControlFileMenuActionMeta, sourceControlMenuLabel, sourceControlNoMatchesMessage, sourceControlSnapshotHasDisplayData, sourceControlStatusLabel, sourceControlToolbarButtonMeta, sourceControlWorktreeLabel, sourceControlWorktreeRowModel } from '../../../shared/sourceControl';
import { repairMojibakeDisplay } from '../lib/mojibake';
import { defaultWorktreePathForBranch } from '../lib/worktree';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown> } }).mergenApi;

interface SourceControlProps {
  project: ProjectRecord;
  projects?: ProjectRecord[];
  selectedProjectId?: number | null;
  onSelectProject?: (projectId: number) => void;
  onAddWorktree?: (worktree: GitWorktreeInfo) => void;
  onRemoveWorktree?: (worktree: GitWorktreeInfo) => void;
  onDeleteGitWorktree?: (worktree: GitWorktreeInfo) => void;
  hasLiveTerminals?: (path: string) => boolean;
  registeredWorktreePaths?: string[];
  onOrphanWorktrees?: (paths: string[]) => void;
  onBranchChange?: (branch: string) => void;
  autoOpenCreateRequestId?: number;
}

export const SourceControl: React.FC<SourceControlProps> = ({ project, projects, selectedProjectId, onSelectProject, onAddWorktree, onRemoveWorktree, onDeleteGitWorktree, hasLiveTerminals, registeredWorktreePaths, onOrphanWorktrees, onBranchChange, autoOpenCreateRequestId }) => {
  const [snapshot, setSnapshot] = useState<SourceControlSnapshot>({ loading: true, files: [], worktrees: [] });
  const [query, setQuery] = useState('');
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [createBranch, setCreateBranch] = useState('');
  const [createBaseBranch, setCreateBaseBranch] = useState('');
  const [createLoading, setCreateLoading] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);
  const [fileContextMenu, setFileContextMenu] = useState<{ x: number; y: number; filePath: string; relativePath: string } | null>(null);
  const [worktreeContextMenu, setWorktreeContextMenu] = useState<{ x: number; y: number; branchName: string } | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);
  const feedbackTimerRef = useRef<number | null>(null);
  const lastAutoOpenCreateRequestRef = useRef<number | undefined>(undefined);

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

  const refresh = useCallback(async (manual = false, runFetch = false) => {
    if (manual) {
      setSnapshot((prev) => ({ ...prev, loading: true }));
    }
    const result = await api.invoke('git:status', project.path, runFetch) as SourceControlStatus;
    const worktrees = await api.invoke('git:discoverWorktrees', project.path) as GitWorktreeInfo[];
    setSnapshot({
      loading: false,
      error: result.error,
      files: result.files,
      worktrees: worktrees.filter((w) => w.path !== project.path),
      branch: result.branch,
      ahead: result.ahead,
      behind: result.behind,
      lastUpdated: Date.now(),
    });
    if (runFetch) {
      showFeedback(result.error ?? 'Fetched and refreshed source control');
    }
  }, [project.path, showFeedback]);

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
    if (!fileContextMenu && !worktreeContextMenu) return undefined;

    const closeMenu = () => {
      setFileContextMenu(null);
      setWorktreeContextMenu(null);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') closeMenu();
    };
    window.addEventListener('click', closeMenu);
    window.addEventListener('keydown', closeOnEscape);
    return () => {
      window.removeEventListener('click', closeMenu);
      window.removeEventListener('keydown', closeOnEscape);
    };
  }, [fileContextMenu, worktreeContextMenu]);

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
    return sourceControlWorktreeLabel(w).toLowerCase().includes(q) || w.path.toLowerCase().includes(q);
  });
  const hasDisplayData = sourceControlSnapshotHasDisplayData(snapshot);
  const createWorktreePath = createBranch.trim()
    ? defaultWorktreePathForBranch(project.path, createBranch.trim())
    : '';
  const unregisteredWorktrees = snapshot.worktrees.filter((w) => !registeredWorktreePaths?.includes(w.path));
  const openCreateWorktreeModal = useCallback(() => {
    setCreateBranch('');
    setCreateBaseBranch(snapshot.branch ?? '');
    setCreateError(null);
    setShowCreateModal(true);
  }, [snapshot.branch]);
  const refreshButton = sourceControlToolbarButtonMeta('refreshStatus');
  const fetchButton = sourceControlToolbarButtonMeta('fetchAndRefresh');
  const folderButton = sourceControlToolbarButtonMeta('openProjectFolder');
  const createButton = sourceControlToolbarButtonMeta('createWorktree');
  const openFileFolderAction = sourceControlFileMenuActionMeta('openInFolder');
  const copyRelativePathAction = sourceControlFileMenuActionMeta('copyRelativePath');

  useEffect(() => {
    if (autoOpenCreateRequestId === undefined) return;
    if (lastAutoOpenCreateRequestRef.current === autoOpenCreateRequestId) return;
    if (snapshot.loading) return;
    lastAutoOpenCreateRequestRef.current = autoOpenCreateRequestId;
    openCreateWorktreeModal();
  }, [autoOpenCreateRequestId, snapshot.loading, openCreateWorktreeModal]);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden', position: 'relative' }}>
      <div className="source-control-header">
        <div className="source-control-header-label">Project</div>
        <div className="source-control-header-row">
          {projects && projects.length > 0 && onSelectProject ? (
            <select
              className="source-control-project-select"
              value={selectedProjectId ?? project.id}
              title={project.path}
              onChange={(event) => onSelectProject(Number(event.target.value))}
            >
              {projects.map((item) => (
                <option key={item.id} value={item.id}>
                  {repairMojibakeDisplay(item.name)}
                </option>
              ))}
            </select>
          ) : (
            <span className="source-control-project-name" title={project.path}>
              {repairMojibakeDisplay(project.name)}
            </span>
          )}
          <button
            onClick={() => refresh(true)}
            className="source-control-toolbar-btn"
            type="button"
            title={refreshButton.tooltip}
            aria-label={refreshButton.ariaLabel}
          >
            {refreshButton.icon}
          </button>
          <button
            onClick={() => refresh(true, true)}
            className="source-control-toolbar-btn"
            type="button"
            title={fetchButton.tooltip}
            aria-label={fetchButton.ariaLabel}
          >
            {fetchButton.icon}
          </button>
          <button
            onClick={() => {
              api.invoke('shell:showItemInFolder', project.path)
                .then(() => showFeedback('Opened project folder'))
                .catch((error) => showFeedback(`Open folder failed: ${error instanceof Error ? error.message : String(error)}`));
            }}
            className="source-control-toolbar-btn"
            type="button"
            title={folderButton.tooltip}
            aria-label={folderButton.ariaLabel}
          >
            {folderButton.icon}
          </button>
          <button
            onClick={openCreateWorktreeModal}
            className={`source-control-toolbar-btn ${createButton.accent ? 'accent' : ''}`}
            type="button"
            title={createButton.tooltip}
            aria-label={createButton.ariaLabel}
          >
            {createButton.icon}
          </button>
        </div>
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
        {snapshot.error && <div style={{ padding: 12, color: '#ff8a8a', fontSize: 12 }}>{snapshot.error}</div>}

        {filteredWorktrees.length > 0 && (
          <div style={{ marginBottom: 8 }}>
            <div style={{ padding: '4px 12px', fontSize: 11, color: '#888', fontWeight: 600 }}>Worktrees</div>
            {filteredWorktrees.map((w) => {
              const worktreeRow = sourceControlWorktreeRowModel(w, project.path, registeredWorktreePaths ?? []);
              return (
              <div
                key={w.path}
                className={`source-control-worktree-row ${worktreeRow.isCurrent ? 'current' : ''} ${worktreeRow.canAdd ? 'clickable' : ''}`}
                title={worktreeRow.tooltip}
                onClick={() => {
                  if (!worktreeRow.canAdd) return;
                  onAddWorktree?.(w);
                  showFeedback(`Added worktree '${worktreeRow.label}' as project`);
                }}
                onContextMenu={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  setFileContextMenu(null);
                  setWorktreeContextMenu({ x: event.clientX, y: event.clientY, branchName: worktreeRow.branchNameForCopy });
                }}
              >
                <span className="source-control-worktree-icon">⑂</span>
                <span className="source-control-worktree-label">
                  {repairMojibakeDisplay(worktreeRow.label)}
                </span>
              </div>
              );
            })}
          </div>
        )}

        {!snapshot.loading && !snapshot.error && !hasDisplayData && (
          <div style={{ padding: 12, color: '#888', fontSize: 12 }}>Status pending</div>
        )}
        {!snapshot.loading && !snapshot.error && hasDisplayData && snapshot.files.length === 0 && (
          <div style={{ padding: 12, color: '#888', fontSize: 12 }}>Working tree is clean</div>
        )}

        {filteredFiles.length > 0 && (
          <>
            {filteredFiles.map((f) => {
              const filePath = sourceControlFileAbsolutePath(project.path, f.path);
              return (
              <div
                key={f.path}
                style={{ padding: '3px 12px', display: 'flex', alignItems: 'center', gap: 6, minWidth: 0, cursor: 'context-menu' }}
                onContextMenu={(event) => {
                  event.preventDefault();
                  event.stopPropagation();
                  setWorktreeContextMenu(null);
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
                <span style={{ fontSize: 11, color: '#aaa', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', fontFamily: '"Cascadia Code", "Cascadia Mono", Consolas, "Courier New", monospace' }}>
                  {repairMojibakeDisplay(`${statusLabel(f.status)} ${f.path}`)}
                </span>
              </div>
              );
            })}
          </>
        )}

        {query && filteredFiles.length === 0 && filteredWorktrees.length === 0 && (
          <div style={{ padding: 12, color: '#888', fontSize: 12 }}>{sourceControlNoMatchesMessage()}</div>
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
            {sourceControlMenuLabel(openFileFolderAction)}
          </button>
          <button
            type="button"
            onClick={() => {
              api.invoke('clipboard:writeText', fileContextMenu.relativePath).catch(() => {});
              showFeedback('Copied relative path');
              setFileContextMenu(null);
            }}
          >
            {sourceControlMenuLabel(copyRelativePathAction)}
          </button>
        </div>
      )}
      {worktreeContextMenu && (
        <div
          className="source-control-context-menu"
          style={{ left: worktreeContextMenu.x, top: worktreeContextMenu.y }}
          onClick={(event) => event.stopPropagation()}
        >
          <button
            type="button"
            onClick={() => {
              api.invoke('clipboard:writeText', worktreeContextMenu.branchName).catch(() => {});
              showFeedback(`Copied branch name '${worktreeContextMenu.branchName}'`);
              setWorktreeContextMenu(null);
            }}
          >
            Copy Branch Name
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
            if (!createLoading && e.target === e.currentTarget) {
              setCreateError(null);
              setShowCreateModal(false);
            }
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
              <button
                disabled={createLoading}
                onClick={() => {
                  setCreateError(null);
                  setShowCreateModal(false);
                }}
                style={{ background: 'transparent', border: 'none', color: '#888', cursor: createLoading ? 'not-allowed' : 'pointer', fontSize: 14, opacity: createLoading ? 0.5 : 1 }}
              >
                ✕
              </button>
            </div>

            {unregisteredWorktrees.length > 0 && (
              <div>
                <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 4 }}>Existing worktrees not in Mergen</div>
                <div style={{ maxHeight: 120, overflow: 'auto', display: 'flex', flexDirection: 'column', gap: 4 }}>
                  {unregisteredWorktrees.map((w) => (
                    <div key={w.path} style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '4px 8px', background: '#1a1a1a', borderRadius: 4 }}>
                      <span style={{ fontSize: 11, color: '#aaa', flex: 1 }}>{repairMojibakeDisplay(w.branch || '(detached)')}</span>
                      <span style={{ fontSize: 10, color: '#666' }}>{repairMojibakeDisplay(w.path)}</span>
                      <button
                        disabled={createLoading}
                        onClick={() => {
                          onAddWorktree?.(w);
                        }}
                        style={{ fontSize: 10, padding: '2px 6px', background: '#1a1a1a', border: '1px solid #333', color: '#ccc', borderRadius: 3, cursor: createLoading ? 'not-allowed' : 'pointer', opacity: createLoading ? 0.5 : 1 }}
                      >
                        Add to Mergen
                      </button>
                    </div>
                  ))}
                </div>
                <div style={{ borderTop: '1px solid #333', marginTop: 8, paddingTop: 8 }}>
                  <div style={{ fontSize: 12, fontWeight: 600, color: '#eee' }}>Or create new worktree</div>
                </div>
              </div>
            )}

            <div>
              <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 4 }}>Branch name</div>
              <input
                type="text"
                value={createBranch}
                onChange={(e) => {
                  setCreateBranch(e.target.value);
                  setCreateError(null);
                }}
                disabled={createLoading}
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
                  opacity: createLoading ? 0.7 : 1,
                }}
              />
            </div>

            <div>
              <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 4 }}>Base branch (optional)</div>
              <input
                type="text"
                value={createBaseBranch}
                onChange={(e) => {
                  setCreateBaseBranch(e.target.value);
                  setCreateError(null);
                }}
                disabled={createLoading}
                placeholder="main or origin/main"
                style={{
                  width: '100%',
                  background: '#1a1a1a',
                  border: '1px solid #333',
                  color: '#ccc',
                  padding: '4px 8px',
                  fontSize: 12,
                  borderRadius: 4,
                  outline: 'none',
                  opacity: createLoading ? 0.7 : 1,
                }}
              />
            </div>

            <div>
              <div style={{ fontSize: 12, fontWeight: 600, color: '#eee', marginBottom: 4 }}>Worktree path (auto)</div>
              <input
                type="text"
                readOnly
                value={createWorktreePath}
                placeholder="auto-generated from branch"
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

            {createError ? (
              <div style={{ color: '#ff8a8a', fontSize: 12 }}>{createError}</div>
            ) : createLoading ? (
              <div style={{ color: '#888', fontSize: 12 }}>Creating worktree...</div>
            ) : null}

            <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8, marginTop: 4 }}>
              <button
                disabled={createLoading}
                onClick={() => {
                  setCreateError(null);
                  setShowCreateModal(false);
                }}
                style={{ padding: '6px 16px', fontSize: 12, background: 'transparent', border: '1px solid #333', color: '#ccc', borderRadius: 4, cursor: createLoading ? 'not-allowed' : 'pointer', opacity: createLoading ? 0.5 : 1 }}
              >
                Cancel
              </button>
              <button
                disabled={!createBranch.trim() || !createWorktreePath || createLoading}
                onClick={async () => {
                  const branch = createBranch.trim();
                  const baseBranch = createBaseBranch.trim();
                  if (!branch || !createWorktreePath) return;
                  setCreateLoading(true);
                  setCreateError(null);
                  try {
                    const ok = await api.invoke('git:createWorktree', project.path, branch, createWorktreePath, baseBranch || undefined) as boolean;
                    if (ok) {
                      onAddWorktree?.({ path: createWorktreePath, branch, head: '', detached: false, locked: false, prunable: false });
                      setShowCreateModal(false);
                      setCreateBranch('');
                      setCreateBaseBranch('');
                      setCreateError(null);
                      refresh(true);
                    } else {
                      setCreateError('Failed to create worktree. Check if the branch already exists or the path is valid.');
                    }
                  } catch (error) {
                    setCreateError(`Failed to create worktree: ${error instanceof Error ? error.message : String(error)}`);
                  } finally {
                    setCreateLoading(false);
                  }
                }}
                style={{
                  padding: '6px 16px',
                  fontSize: 12,
                  background: '#1f3a4c',
                  border: '1px solid #1f3a4c',
                  color: '#ccc',
                  borderRadius: 4,
                  cursor: !createBranch.trim() || !createWorktreePath || createLoading ? 'not-allowed' : 'pointer',
                  opacity: !createBranch.trim() || !createWorktreePath || createLoading ? 0.5 : 1,
                }}
              >
                {createLoading ? 'Creating...' : 'Create Worktree'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
