import React, { useState } from 'react';
import { WebProject } from '../types';
import { PanelHeader, Button, Input, ScrollArea, EmptyState, Row } from '../components/ui';

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
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
      <PanelHeader title="Projects">
        <Button variant="secondary" onClick={() => setShowAdd(v => !v)} style={{ width: 28, height: 28, padding: 0, fontSize: 16 }}>
          +
        </Button>
      </PanelHeader>

      {showAdd && (
        <div style={{ padding: 'var(--space-md)', display: 'flex', flexDirection: 'column', gap: 'var(--space-sm)', borderBottom: '1px solid var(--border-subtle)' }}>
          <Input placeholder="Name" value={name} onChange={e => setName(e.target.value)} />
          <Input placeholder="Path" value={path} onChange={e => setPath(e.target.value)} />
          <Button
            variant="primary"
            onClick={() => { onAddProject(name, path); setName(''); setPath(''); setShowAdd(false); }}
          >
            Add
          </Button>
        </div>
      )}

      <ScrollArea maxHeight={260}>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--space-xs)', padding: 'var(--space-xs)' }}>
          {projects.map(p => (
            <Row
              key={p.id}
              active={selectedProjectId === p.id}
              onClick={() => onSelectProject(p.id)}
              style={{
                flexDirection: 'column',
                alignItems: 'flex-start',
                gap: 'var(--space-xs)',
                borderLeft: `2px solid ${p.is_worktree ? 'var(--warning)' : 'var(--accent)'}`,
                borderRadius: '0 var(--radius-md) var(--radius-md) 0',
              }}
            >
              <div style={{ fontWeight: 600, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', width: '100%' }}>
                {p.name}
              </div>
              <div style={{ fontSize: 'var(--font-xs)', color: 'var(--text-muted)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', width: '100%' }}>
                {p.path}
              </div>
            </Row>
          ))}
          {projects.length === 0 && <EmptyState message="No projects" />}
        </div>
      </ScrollArea>
    </div>
  );
};
