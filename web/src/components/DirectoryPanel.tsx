import React, { useState, useEffect } from 'react';
import { WebDirectoryNode } from '../types';

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
            paddingLeft: 8 + depth * 16,
            paddingRight: 8,
            paddingTop: 2,
            paddingBottom: 2,
            cursor: node.is_dir ? 'pointer' : 'default',
            display: 'flex',
            alignItems: 'center',
            gap: 4,
            fontSize: 12,
            color: matches ? '#e0e0e0' : '#666',
          }}
        >
          <span style={{ width: 14, textAlign: 'center', fontSize: 10 }}>
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
      <div style={{ padding: 8, borderBottom: '1px solid #333', display: 'flex', alignItems: 'center', gap: 4 }}>
        <strong style={{ fontSize: 12, color: '#aaa', flex: 1 }}>Directory</strong>
      </div>
      <input
        placeholder="Search files..."
        value={search}
        onChange={e => setSearch(e.target.value)}
        style={{ margin: 8, background: '#1a1a1a', border: '1px solid #444', color: '#e0e0e0', fontSize: 11, padding: '4px 6px' }}
      />
      <div style={{ flex: 1, overflow: 'auto' }}>
        {loading && <div style={{ padding: 8, fontSize: 11, color: '#888' }}>Loading...</div>}
        {!projectId && <div style={{ padding: 8, fontSize: 11, color: '#666' }}>Select a project</div>}
        {root && renderNode(root, 0)}
      </div>
    </div>
  );
};
