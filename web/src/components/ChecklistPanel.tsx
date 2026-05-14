import React, { useState } from 'react';
import { WebProject } from '../types';

interface Props {
  projects: WebProject[];
  onCopyItems: (projectId: number) => void;
}

export const ChecklistPanel: React.FC<Props> = ({ projects, onCopyItems }) => {
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const projectsWithItems = projects.filter(p => p.checklist && p.checklist.length > 0);

  const toggle = (projectId: number, idx: number) => {
    const key = `${projectId}-${idx}`;
    setChecked(prev => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
      <div style={{ padding: 8, borderBottom: '1px solid #333', display: 'flex', alignItems: 'center', gap: 4 }}>
        <strong style={{ fontSize: 12, color: '#aaa', flex: 1 }}>Checklist</strong>
      </div>
      <div style={{ flex: 1, overflow: 'auto' }}>
        {projectsWithItems.map(p => (
          <div key={p.id} style={{ marginBottom: 8 }}>
            <div style={{ padding: '4px 8px', fontSize: 11, color: '#4fc3f7', fontWeight: 'bold', display: 'flex', alignItems: 'center', gap: 4 }}>
              <span>{p.name}</span>
              <span style={{ marginLeft: 'auto', fontSize: 10, color: '#888' }}>{p.checklist.length}</span>
              <button
                onClick={() => onCopyItems(p.id)}
                style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 12 }}
                title="Copy all"
              >
                📋
              </button>
            </div>
            {p.checklist.map((item, idx) => {
              const isChecked = checked.has(`${p.id}-${idx}`);
              return (
                <div
                  key={idx}
                  onClick={() => toggle(p.id, idx)}
                  style={{
                    padding: '2px 8px',
                    fontSize: 11,
                    color: '#e0e0e0',
                    cursor: 'pointer',
                    display: 'flex',
                    alignItems: 'flex-start',
                    gap: 6,
                    textDecoration: isChecked ? 'line-through' : 'none',
                    opacity: isChecked ? 0.5 : 1,
                  }}
                >
                  <span>{isChecked ? '☑️' : '⬜'}</span>
                  <span style={{ flex: 1, wordBreak: 'break-word' }}>{item}</span>
                </div>
              );
            })}
          </div>
        ))}
        {projectsWithItems.length === 0 && (
          <div style={{ padding: 8, fontSize: 11, color: '#666' }}>No checklist items</div>
        )}
      </div>
    </div>
  );
};
