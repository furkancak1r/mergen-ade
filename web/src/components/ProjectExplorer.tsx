import React, { useState } from 'react';
import { WebProject } from '../types';

interface Props {
  projects: WebProject[];
  selectedProjectId: number | null;
  onSelectProject: (id: number) => void;
  onAddProject: (name: string, path: string) => void;
}

export const ProjectExplorer: React.FC<Props> = ({
  projects,
  selectedProjectId,
  onSelectProject,
  onAddProject,
}) => {
  const [showAdd, setShowAdd] = useState(false);
  const [name, setName] = useState('');
  const [path, setPath] = useState('');

  return (
    <div style={{ padding: 8, display: 'flex', flexDirection: 'column', gap: 4 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
        <strong style={{ fontSize: 12, color: '#aaa', flex: 1 }}>Projects</strong>
        <button
          onClick={() => setShowAdd(v => !v)}
          style={{ background: '#222', border: '1px solid #444', color: '#e0e0e0', fontSize: 14, width: 22, height: 22, cursor: 'pointer', lineHeight: 1 }}
        >
          +
        </button>
      </div>

      {showAdd && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 4, marginBottom: 4 }}>
          <input
            placeholder="Name"
            value={name}
            onChange={e => setName(e.target.value)}
            style={{ background: '#1a1a1a', border: '1px solid #444', color: '#e0e0e0', fontSize: 11, padding: '2px 4px' }}
          />
          <input
            placeholder="Path"
            value={path}
            onChange={e => setPath(e.target.value)}
            style={{ background: '#1a1a1a', border: '1px solid #444', color: '#e0e0e0', fontSize: 11, padding: '2px 4px' }}
          />
          <button
            onClick={() => { onAddProject(name, path); setName(''); setPath(''); setShowAdd(false); }}
            style={{ background: '#1e3a5f', border: '1px solid #4fc3f7', color: '#4fc3f7', fontSize: 11, cursor: 'pointer' }}
          >
            Add
          </button>
        </div>
      )}

      <div style={{ display: 'flex', flexDirection: 'column', gap: 2, maxHeight: 200, overflow: 'auto' }}>
        {projects.map(p => (
          <div
            key={p.id}
            onClick={() => onSelectProject(p.id)}
            style={{
              padding: '4px 6px',
              borderRadius: 4,
              cursor: 'pointer',
              background: selectedProjectId === p.id ? '#1e3a5f' : 'transparent',
              fontSize: 11,
              color: '#e0e0e0',
              borderLeft: `2px solid ${p.is_worktree ? '#ff9800' : '#4fc3f7'}`,
            }}
          >
            <div style={{ fontWeight: 'bold', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
              {p.name}
            </div>
            <div style={{ fontSize: 10, color: '#888', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
              {p.path}
            </div>
          </div>
        ))}
        {projects.length === 0 && (
          <div style={{ fontSize: 11, color: '#666', textAlign: 'center', padding: 8 }}>
            No projects
          </div>
        )}
      </div>
    </div>
  );
};
