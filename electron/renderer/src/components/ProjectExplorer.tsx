import React, { useState, useEffect, useCallback, useRef, useMemo } from 'react';
import type { DirectoryNode, ProjectRecord } from '../../../shared/types';

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

interface ProjectExplorerProps {
  project: ProjectRecord;
  selectedPath?: string;
  onSelectPath?: (path: string) => void;
  onOpenFile?: (path: string) => void;
}

export const ProjectExplorer: React.FC<ProjectExplorerProps> = ({ project, selectedPath, onSelectPath, onOpenFile }) => {
  const [rootNode, setRootNode] = useState<DirectoryNode | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState('');
  const [debouncedQuery, setDebouncedQuery] = useState('');
  const expandedRef = useRef<Set<string>>(new Set());
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const searchRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedQuery(query), 250);
    return () => clearTimeout(timer);
  }, [query]);

  const loadDirectory = useCallback(async (dirPath: string, shallow: boolean): Promise<DirectoryNode[]> => {
    try {
      const entries = await api.invoke('fs:readDir', dirPath) as { name: string; isDirectory: boolean; isSymlink: boolean }[];
      return entries.map((e) => {
        const fullPath = `${dirPath}/${e.name}`;
        const isDir = e.isDirectory;
        const isDeferred = isDir && shouldDefer(e.name);
        return {
          name: e.name,
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

  useEffect(() => {
    let cancelled = false;
    async function init() {
      setLoading(true);
      setError(null);
      try {
        const children = await loadDirectory(project.path, true);
        if (cancelled) return;
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
        setRootNode(root);
        expandedRef.current.add(project.path);
        setExpanded(new Set(expandedRef.current));
      } catch (err) {
        if (!cancelled) setError(String(err));
      } finally {
        if (!cancelled) setLoading(false);
      }
    }
    init();
    return () => { cancelled = true; };
  }, [project.path, project.name, loadDirectory]);

  const expandNode = useCallback(async (node: DirectoryNode) => {
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
    <div className="project-explorer" style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
      <div style={{ padding: '8px 12px', borderBottom: '1px solid #222', display: 'flex', alignItems: 'center', gap: 8 }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: '#eee', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
          {project.name}
        </span>
      </div>
      <div style={{ padding: '6px 12px', borderBottom: '1px solid #222' }}>
        <input
          ref={searchRef}
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
        {loading && (
          <div style={{ padding: 12, color: '#888', fontSize: 12 }}>Loading directory...</div>
        )}
        {error && (
          <div style={{ padding: 12, color: '#c44', fontSize: 12 }}>Error: {error}</div>
        )}
        {filteredRoot && renderNode(filteredRoot, 0)}
        {!loading && !error && filteredRoot === null && (
          <div style={{ padding: 12, color: '#888', fontSize: 12 }}>No matching files or folders.</div>
        )}
      </div>
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
