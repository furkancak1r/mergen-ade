import React, { useState, useEffect } from 'react';
import { WebDirectoryNode } from '../types';
import { PanelHeader, Input, ScrollArea, EmptyState, LoadingState, Icon } from '../components/ui';

interface Props {
  projectId: number | null;
  apiUrl: string;
}

export const DirectoryPanel: React.FC<Props> = ({ projectId, apiUrl }) => {
  const [root, setRoot] = useState<WebDirectoryNode | null>(null);
  const [loading, setLoading] = useState(false);
  const [search, setSearch] = useState('');
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  useEffect(() => {
    if (!projectId) {
      setRoot(null);
      return;
    }
    setLoading(true);
    fetch(`${apiUrl}/api/directory?project_id=${projectId}`)
      .then(r => r.json())
      .then(data => {
        if (data.success && data.data) {
          setRoot(data.data);
        }
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, [projectId, apiUrl]);

  const toggleExpand = (path: string) => {
    setExpanded(prev => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

  const renderNode = (node: WebDirectoryNode, depth: number) => {
    const isExpanded = expanded.has(node.path);
    const matches = !search || node.name.toLowerCase().includes(search.toLowerCase());
    const hasVisibleChildren = node.children.some(c =>
      !search || c.name.toLowerCase().includes(search.toLowerCase()) || c.children.length > 0
    );

    return (
      <div key={node.path}>
        <div
          onClick={() => node.is_dir && toggleExpand(node.path)}
          style={{
            paddingLeft: `calc(var(--space-sm) + ${depth * 16}px)`,
            paddingRight: 'var(--space-sm)',
            paddingTop: 'var(--space-xs)',
            paddingBottom: 'var(--space-xs)',
            cursor: node.is_dir ? 'pointer' : 'default',
            display: 'flex',
            alignItems: 'center',
            gap: 'var(--space-xs)',
            fontSize: 'var(--font-base)',
            color: matches ? 'var(--text-primary)' : 'var(--text-muted)',
            borderRadius: 'var(--radius-sm)',
            transition: 'background 0.1s',
          }}
          onMouseEnter={e => { if (node.is_dir) e.currentTarget.style.background = 'var(--bg-hover)'; }}
          onMouseLeave={e => { e.currentTarget.style.background = 'transparent'; }}
        >
          <span style={{ width: 14, textAlign: 'center', fontSize: 'var(--font-sm)', flexShrink: 0 }}>
            {node.is_dir ? (isExpanded ? '📂' : '📁') : '📄'}
          </span>
          <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            {node.name}
          </span>
        </div>
        {node.is_dir && isExpanded && node.children.map(child => renderNode(child, depth + 1))}
      </div>
    );
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
      <PanelHeader title="Directory" />
      <div style={{ padding: 'var(--space-md)' }}>
        <Input
          placeholder="Search files…"
          value={search}
          onChange={e => setSearch(e.target.value)}
        />
      </div>
      <ScrollArea>
        {loading && <LoadingState />}
        {!projectId && <EmptyState message="Select a project" />}
        {root && (
          <div style={{ padding: '0 var(--space-xs)' }}>
            {renderNode(root, 0)}
          </div>
        )}
      </ScrollArea>
    </div>
  );
};
