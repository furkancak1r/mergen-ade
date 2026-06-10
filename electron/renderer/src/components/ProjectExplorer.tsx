import React, { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import type { DirectoryNode, ProjectRecord } from '../../../shared/types';
import { repairMojibakeDisplay } from '../lib/mojibake';
import { collectLoadedDirectoryPaths, directoryNodeContextActions, directoryTreeHasCollapsedFolders, type DirectoryNodeContextAction } from '../lib/directoryTree';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown>; on: (channel: string, cb: (...args: unknown[]) => void) => () => void } }).mergenApi;

const DEFERRED_DIRS = new Set([
  '.git', 'node_modules', 'target', 'dist', '.next', 'build', 'out', '.turbo', '.cache', 'cache',
  '.venv', 'venv', 'env', '.env', '__pycache__', '.pytest_cache', '.mypy_cache', '.gradle', '.idea',
  '.vscode', 'coverage', '.nyc_output', 'tmp', 'temp', '.tmp', 'logs', '.log', 'vendor', 'Pods',
  '.DS_Store', 'Thumbs.db',
]);

function getFileIcon(name: string, isDirectory: boolean): string {
  if (isDirectory) return '📁';
  const ext = name.split('.').pop()?.toLowerCase() ?? '';
  const iconMap: Record<string, string> = {
    ts: 'TS', tsx: 'TSX', js: 'JS', jsx: 'JSX', py: 'PY', rs: 'RS', go: 'GO', java: 'JV', cpp: 'C++', c: 'C', h: 'H',
    json: 'JSON', md: 'MD', yml: 'YML', yaml: 'YML', toml: 'TOML', xml: 'XML', html: 'HTML', css: 'CSS', scss: 'SCSS',
    svg: 'SVG', png: 'PNG', jpg: 'JPG', jpeg: 'JPG', gif: 'GIF', ico: 'ICO', webp: 'WEBP',
    sh: 'SH', ps1: 'PS1', bat: 'BAT', cmd: 'CMD', dockerfile: 'DOCKER', makefile: 'MAKE', gitignore: 'GIT', gitattributes: 'GIT',
    sql: 'SQL', db: 'DB', sqlite: 'DB', prisma: 'PRISMA', graphql: 'GQL', gql: 'GQL',
    pdf: 'PDF', doc: 'DOC', docx: 'DOC', xls: 'XLS', xlsx: 'XLS', ppt: 'PPT', pptx: 'PPT',
    zip: 'ZIP', tar: 'TAR', gz: 'GZ', rar: 'RAR', '7z': '7Z',
    exe: 'EXE', dll: 'DLL', msi: 'MSI', app: 'APP', dmg: 'DMG',
    txt: 'TXT', log: 'LOG', ini: 'INI', cfg: 'CFG', env: 'ENV', lock: 'LOCK',
    map: 'MAP', wasm: 'WASM', vue: 'VUE', svelte: 'SVEL', astro: 'ASTRO',
  };
  return iconMap[ext] || '📄';
}

function shouldDefer(name: string): boolean {
  return DEFERRED_DIRS.has(name.toLowerCase());
}

function joinPath(a: string, b: string): string {
  const sep = a.includes('\\') || b.includes('\\') ? '\\' : '/';
  const aTrimmed = a.replace(/[\\/]+$/, '');
  return aTrimmed + sep + b;
}

function highlightMatch(text: string, query: string): React.ReactNode {
  if (!query) return text;
  const q = query.toLowerCase();
  const t = text.toLowerCase();
  const parts: React.ReactNode[] = [];
  let i = 0;
  while (i < text.length) {
    const idx = t.indexOf(q, i);
    if (idx === -1) {
      parts.push(text.slice(i));
      break;
    }
    if (idx > i) parts.push(text.slice(i, idx));
    parts.push(<mark key={idx} style={{ color: '#ff9d4d', background: 'transparent' }}>{text.slice(idx, idx + query.length)}</mark>);
    i = idx + query.length;
  }
  return parts;
}

function directoryContextActionLabel(action: DirectoryNodeContextAction): string {
  switch (action) {
    case 'openInEditor':
      return 'Open in Editor';
    case 'openWithDefaultApp':
      return 'Open with Default App';
    case 'revealInFolder':
      return 'Reveal in Folder';
    case 'copyPath':
      return 'Copy Path';
  }
}

interface ProjectExplorerProps {
  project: ProjectRecord;
  projects?: ProjectRecord[];
  selectedProjectId?: number | null;
  selectedPath?: string;
  onSelectProject?: (projectId: number) => void;
  onAddProject?: () => void;
  onRemoveProject?: (project: ProjectRecord) => void;
  onSelectPath?: (path: string) => void;
  onOpenFile?: (path: string) => void;
}

export const ProjectExplorer: React.FC<ProjectExplorerProps> = ({ project, projects, selectedProjectId, selectedPath, onSelectProject, onAddProject, onRemoveProject, onSelectPath, onOpenFile }) => {
  const [rootNode, setRootNode] = useState<DirectoryNode | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const [debouncedQuery, setDebouncedQuery] = useState('');
  const expandedRef = useRef<Set<string>>(new Set());
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [searchLoading, setSearchLoading] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);
  const refreshRequestRef = useRef(0);
  const feedbackTimerRef = useRef<number | null>(null);
  const [feedback, setFeedback] = useState<string | null>(null);
  const [nodeContextMenu, setNodeContextMenu] = useState<{ x: number; y: number; node: DirectoryNode } | null>(null);

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedQuery(query), 250);
    return () => clearTimeout(timer);
  }, [query]);

  const loadDirectory = useCallback(async (dirPath: string, shallow: boolean): Promise<DirectoryNode[]> => {
    try {
      const entries = await api.invoke('fs:readDir', dirPath) as { name: string; isDirectory: boolean; isSymlink: boolean }[];
      return entries.map((e) => {
        const fullPath = joinPath(dirPath, e.name);
        const isDir = e.isDirectory && !e.isSymlink; // Symlinks are never treated as directories for descent
        const isDeferred = isDir && shouldDefer(e.name);
        const repairedName = repairMojibakeDisplay(e.name);
        return {
          name: repairedName,
          path: fullPath,
          isDirectory: isDir,
          isDeferred: isDeferred,
          isSymlink: e.isSymlink,
          isExpanded: false,
          isLoading: false,
          children: isDeferred ? undefined : (isDir && !shallow ? [] : undefined),
        };
      });
    } catch (err) {
      return [{ name: 'Error', path: dirPath, isDirectory: false, isDeferred: false, isSymlink: false, isExpanded: false, isLoading: false, error: String(err) }];
    }
  }, []);

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

  useEffect(() => () => {
    if (feedbackTimerRef.current !== null) {
      window.clearTimeout(feedbackTimerRef.current);
    }
  }, []);

  useEffect(() => {
    if (!nodeContextMenu) return undefined;

    const closeMenu = () => setNodeContextMenu(null);
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') closeMenu();
    };
    window.addEventListener('click', closeMenu);
    window.addEventListener('keydown', closeOnEscape);
    return () => {
      window.removeEventListener('click', closeMenu);
      window.removeEventListener('keydown', closeOnEscape);
    };
  }, [nodeContextMenu]);

  const refreshRoot = useCallback(async (): Promise<boolean> => {
    const requestId = ++refreshRequestRef.current;
    setLoading(true);
    setError(null);
    setSearchLoading(false);
    try {
      const children = await loadDirectory(project.path, true);
      if (requestId !== refreshRequestRef.current) return false;
      const root: DirectoryNode = {
        name: project.name,
        path: project.path,
        isDirectory: true,
        isDeferred: false,
        isSymlink: false,
        isExpanded: true,
        isLoading: false,
        children,
      };
      expandedRef.current = new Set([project.path]);
      setRootNode(root);
      setExpanded(new Set(expandedRef.current));
      return true;
    } catch (err) {
      if (requestId === refreshRequestRef.current) {
        setError(String(err));
      }
      return false;
    } finally {
      if (requestId === refreshRequestRef.current) {
        setLoading(false);
      }
    }
  }, [project.path, project.name, loadDirectory]);

  useEffect(() => {
    refreshRoot();
    return () => {
      refreshRequestRef.current += 1;
    };
  }, [refreshRoot]);

  const copyProjectPath = useCallback(() => {
    api.invoke('clipboard:writeText', project.path).catch(() => {});
    showFeedback(`Copied path for project '${project.name}'`);
  }, [project.path, project.name, showFeedback]);

  const openProjectFolder = useCallback(() => {
    api.invoke('shell:showItemInFolder', project.path)
      .then(() => showFeedback(`Opened project '${project.name}' in Explorer`))
      .catch((err) => showFeedback(`Open folder failed: ${err instanceof Error ? err.message : String(err)}`));
  }, [project.path, project.name, showFeedback]);

  const refreshDirectoryIndex = useCallback(() => {
    refreshRoot()
      .then((ok) => showFeedback(ok ? 'Directory index refreshed' : 'Directory index refresh failed'))
      .catch((err) => showFeedback(`Directory index refresh failed: ${err instanceof Error ? err.message : String(err)}`));
  }, [refreshRoot, showFeedback]);

  const runNodeContextAction = useCallback((action: DirectoryNodeContextAction, node: DirectoryNode) => {
    switch (action) {
      case 'openInEditor':
        if (onOpenFile) {
          onSelectPath?.(node.path);
          onOpenFile(node.path);
          showFeedback(`Opening ${node.name} in editor...`);
        } else {
          showFeedback('Editor is not available for this file');
        }
        break;
      case 'openWithDefaultApp':
        api.invoke('shell:openPath', node.path)
          .then((result) => {
            const message = typeof result === 'string' && result ? `Open file failed: ${result}` : `Opened file: ${node.name}`;
            showFeedback(message);
          })
          .catch((err) => showFeedback(`Open file failed: ${err instanceof Error ? err.message : String(err)}`));
        break;
      case 'revealInFolder':
        api.invoke('shell:showItemInFolder', node.path)
          .then(() => showFeedback(`Revealed file in folder: ${node.name}`))
          .catch((err) => showFeedback(`Open folder failed: ${err instanceof Error ? err.message : String(err)}`));
        break;
      case 'copyPath':
        api.invoke('clipboard:writeText', node.path).catch(() => {});
        showFeedback(`Copied path for ${node.name}`);
        break;
    }
  }, [onOpenFile, onSelectPath, showFeedback]);

  const expandNode = useCallback(async (node: DirectoryNode) => {
    if (node.isSymlink) return;
    if (!node.isDirectory || !node.isDeferred && node.children && node.children.length > 0) {
      expandedRef.current.add(node.path);
      setExpanded(new Set(expandedRef.current));
      return;
    }
    expandedRef.current.add(node.path);
    setExpanded(new Set(expandedRef.current));

    const children = await loadDirectory(node.path, false);
    setRootNode((prev) => {
      if (!prev) return prev;
      const next = structuredClone(prev);
      const target = findNode(next, node.path);
      if (target) {
        target.children = children;
        target.isLoading = false;
      }
      return next;
    });
  }, [loadDirectory]);

  // Progressive deferred loading for search with adaptive caps and hidden queue
  useEffect(() => {
    if (!debouncedQuery || !rootNode) {
      setSearchLoading(false);
      return;
    }
    // Use char count for Unicode-aware minimum query length
    const charCount = Array.from(debouncedQuery).length;
    if (charCount < 2) {
      setSearchLoading(false);
      return;
    }
    const q = debouncedQuery.toLowerCase();

    // Find deferred directories that haven't been loaded yet
    const deferredPaths: string[] = [];
    function collectDeferred(node: DirectoryNode) {
      if (node.isDeferred && !node.children) {
        deferredPaths.push(node.path);
      }
      if (node.children) {
        for (const child of node.children) collectDeferred(child);
      }
    }
    collectDeferred(rootNode);

    if (deferredPaths.length === 0) {
      setSearchLoading(false);
      return;
    }

    // Determine if any results are already visible to set adaptive cap
    const hasVisibleResults = (() => {
      function hasMatch(node: DirectoryNode): boolean {
        if (node.name.toLowerCase().includes(q)) return true;
        if (node.children) {
          for (const child of node.children) {
            if (hasMatch(child)) return true;
          }
        }
        return false;
      }
      return hasMatch(rootNode);
    })();

    // Adaptive cap: aggressive (8) when no results yet, conservative (2) when results visible
    const cap = hasVisibleResults ? 2 : 8;

    setSearchLoading(true);
    let cancelled = false;

    async function loadDeferred() {
      const batch = deferredPaths.slice(0, cap);
      for (const dirPath of batch) {
        if (cancelled) return;
        const children = await loadDirectory(dirPath, false);
        if (cancelled) return;
        setRootNode((prev) => {
          if (!prev) return prev;
          const next = structuredClone(prev);
          const target = findNode(next, dirPath);
          if (target) {
            target.children = children;
            target.isDeferred = false;
          }
          return next;
        });
        // Only auto-expand if the directory name matches the query
        const dirName = dirPath.split(/[\\/]/).pop() || '';
        if (dirName.toLowerCase().includes(q)) {
          expandedRef.current.add(dirPath);
        }
      }
      setExpanded(new Set(expandedRef.current));
      // If there are more deferred paths, schedule another load batch via state nudge
      if (deferredPaths.length > cap) {
        setTimeout(() => {
          if (!cancelled) {
            setDebouncedQuery((prev) => prev);
          }
        }, 50);
      } else {
        setSearchLoading(false);
      }
    }

    loadDeferred();
    return () => { cancelled = true; };
  }, [debouncedQuery, rootNode, loadDirectory]);

  const collapseNode = useCallback((node: DirectoryNode) => {
    expandedRef.current.delete(node.path);
    setExpanded(new Set(expandedRef.current));
  }, []);

  const toggleNode = useCallback((node: DirectoryNode) => {
    if (expandedRef.current.has(node.path)) {
      collapseNode(node);
    } else {
      expandNode(node);
    }
  }, [expandNode, collapseNode]);

  const matchesQuery = useCallback((node: DirectoryNode): boolean => {
    if (!debouncedQuery) return true;
    const q = debouncedQuery.toLowerCase();
    return node.name.toLowerCase().includes(q);
  }, [debouncedQuery]);

  const filterTree = useCallback((node: DirectoryNode): DirectoryNode | null => {
    const selfMatch = matchesQuery(node);
    if (!node.isDirectory || !node.children) {
      return selfMatch ? node : null;
    }
    const filteredChildren: DirectoryNode[] = [];
    for (const child of node.children) {
      const filtered = filterTree(child);
      if (filtered) filteredChildren.push(filtered);
    }
    if (selfMatch || filteredChildren.length > 0) {
      return { ...node, children: filteredChildren };
    }
    return null;
  }, [matchesQuery]);

  const filteredRoot = useMemo(() => {
    if (!debouncedQuery || !rootNode) return rootNode;
    return filterTree(rootNode);
  }, [debouncedQuery, rootNode, filterTree]);
  const hasCollapsedFolders = useMemo(() => {
    return rootNode ? directoryTreeHasCollapsedFolders(rootNode, expanded) : false;
  }, [rootNode, expanded]);
  const searchActive = query.trim().length > 0;
  const toggleAllFolders = useCallback(() => {
    if (!rootNode || searchActive) return;
    if (hasCollapsedFolders) {
      expandedRef.current = new Set(collectLoadedDirectoryPaths(rootNode));
    } else {
      expandedRef.current = new Set([rootNode.path]);
    }
    setExpanded(new Set(expandedRef.current));
  }, [rootNode, searchActive, hasCollapsedFolders]);

  const renderNode = (node: DirectoryNode, depth: number): React.ReactNode => {
    const isExpanded = expanded.has(node.path);
    const paddingLeft = depth * 12 + 4;
    const isSelected = selectedPath === node.path;

    return (
      <div key={node.path}>
        <div
          className="dir-row"
          style={{
            paddingLeft,
            display: 'flex',
            alignItems: 'center',
            gap: 4,
            cursor: 'pointer',
            paddingTop: 2,
            paddingBottom: 2,
            borderRadius: 3,
            background: isSelected ? 'rgba(0,120,212,0.25)' : 'transparent',
          }}
          onClick={() => {
            if (node.isDirectory) {
              toggleNode(node);
              onSelectPath?.(node.path);
            } else {
              onOpenFile?.(node.path);
            }
          }}
          onContextMenu={(event) => {
            event.preventDefault();
            event.stopPropagation();
            onSelectPath?.(node.path);
            setNodeContextMenu({ x: event.clientX, y: event.clientY, node });
          }}
          onDoubleClick={() => {
            if (node.isDirectory) {
              toggleNode(node);
            }
          }}
        >
          {node.isDirectory && (
            <span style={{ width: 12, display: 'inline-flex', justifyContent: 'center', fontSize: 10, color: '#888' }}>
              {isExpanded ? '▼' : '▶'}
            </span>
          )}
          {!node.isDirectory && <span style={{ width: 12, display: 'inline-block' }} />}
          <span style={{ fontSize: 12, color: node.isDirectory ? '#c8c8c8' : '#a0a0a0', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
            <span style={{ marginRight: 4, fontSize: 10 }}>{getFileIcon(node.name, node.isDirectory)}</span>
            {debouncedQuery ? highlightMatch(node.name, debouncedQuery) : node.name}
          </span>
        </div>
        {node.isDirectory && isExpanded && node.children && (
          <div>
            {node.children.map((child) => renderNode(child, depth + 1))}
          </div>
        )}
        {node.isDirectory && isExpanded && node.isDeferred && (!node.children || node.children.length === 0) && (
          <div style={{ paddingLeft: paddingLeft + 16, color: '#666', fontSize: 11, paddingTop: 2, paddingBottom: 2 }}>
            {node.isLoading ? 'Loading...' : 'Deferred'}
          </div>
        )}
      </div>
    );
  };

  return (
    <div className="project-explorer" style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden', position: 'relative' }}>
      <div style={{ padding: '8px 12px', borderBottom: '1px solid #222', display: 'flex', alignItems: 'center', gap: 8 }}>
        {projects && projects.length > 0 && onSelectProject ? (
          <select
            className="project-explorer-project-select"
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
          <span style={{ fontSize: 12, fontWeight: 600, color: '#eee', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis', flex: 1 }}>
            {repairMojibakeDisplay(project.name)}
          </span>
        )}
        {onAddProject && (
          <button type="button" className="project-explorer-toolbar-btn" title="Add Project" onClick={onAddProject}>
            +
          </button>
        )}
        <button type="button" className="project-explorer-toolbar-btn" title="Copy Path" onClick={copyProjectPath}>
          ⧉
        </button>
        <button type="button" className="project-explorer-toolbar-btn" title="Open in Folder" onClick={openProjectFolder}>
          📁
        </button>
        <button type="button" className="project-explorer-toolbar-btn" title="Refresh Directory Index" onClick={refreshDirectoryIndex} disabled={loading}>
          ↻
        </button>
        <button
          type="button"
          className="project-explorer-toolbar-btn"
          title={hasCollapsedFolders ? 'Expand All Folders' : 'Collapse All Folders'}
          onClick={toggleAllFolders}
          disabled={!rootNode || searchActive}
        >
          {hasCollapsedFolders ? '⊞' : '⊟'}
        </button>
        {onRemoveProject && (
          <button
            type="button"
            className="project-explorer-toolbar-btn danger"
            title="Remove Project"
            onClick={() => {
              if (window.confirm(`Remove project '${project.name}' from Mergen? This does not delete files from disk.`)) {
                onRemoveProject(project);
              }
            }}
          >
            ×
          </button>
        )}
      </div>
      <div style={{ padding: '6px 12px', borderBottom: '1px solid #222' }}>
        <input
          ref={searchRef}
          type="text"
          placeholder="Search files and folders"
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
        {loading && (
          <div style={{ padding: 12, color: '#888', fontSize: 12 }}>Loading directory...</div>
        )}
        {error && (
          <div style={{ padding: 12, color: '#c44', fontSize: 12 }}>Error: {error}</div>
        )}
        {filteredRoot && renderNode(filteredRoot, 0)}
        {!loading && !error && searchLoading && (
          <div style={{ padding: 12, color: '#888', fontSize: 12 }}>Searching folders...</div>
        )}
        {!loading && !error && !searchLoading && filteredRoot === null && (
          <div style={{ padding: 12, color: '#888', fontSize: 12 }}>No matching files or folders.</div>
        )}
      </div>
      {feedback && (
        <div className="project-explorer-feedback-toast" role="status">
          {feedback}
        </div>
      )}
      {nodeContextMenu && (
        <div
          className="project-explorer-context-menu"
          style={{ left: nodeContextMenu.x, top: nodeContextMenu.y }}
          onClick={(event) => event.stopPropagation()}
          onContextMenu={(event) => event.preventDefault()}
        >
          {directoryNodeContextActions(nodeContextMenu.node).map((action) => (
            <button
              key={action}
              type="button"
              onClick={() => {
                runNodeContextAction(action, nodeContextMenu.node);
                setNodeContextMenu(null);
              }}
            >
              {directoryContextActionLabel(action)}
            </button>
          ))}
        </div>
      )}
    </div>
  );
};

function findNode(root: DirectoryNode, path: string): DirectoryNode | null {
  if (root.path === path) return root;
  if (!root.children) return null;
  for (const child of root.children) {
    const found = findNode(child, path);
    if (found) return found;
  }
  return null;
}
