import React, { useState } from 'react';
import { WebProject } from '../types';
import { PanelHeader, ScrollArea, EmptyState, Button, Icon } from '../components/ui';

interface Props {
  projects: WebProject[];
  onCopyItems: (projectId: number) => void;
}

export const ChecklistPanel: React.FC<Props> = ({ projects, onCopyItems }) => {
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const [collapsed, setCollapsed] = useState<Set<number>>(new Set());
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

  const toggleCollapse = (projectId: number) => {
    setCollapsed(prev => {
      const next = new Set(prev);
      if (next.has(projectId)) next.delete(projectId);
      else next.add(projectId);
      return next;
    });
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
      <PanelHeader title="Checklist" />
      <ScrollArea>
        <div style={{ padding: 'var(--space-sm)' }}>
          {projectsWithItems.map(p => {
            const isCollapsed = collapsed.has(p.id);
            return (
              <div key={p.id} style={{ marginBottom: 'var(--space-sm)' }}>
                <div
                  onClick={() => toggleCollapse(p.id)}
                  style={{
                    padding: 'var(--space-xs) var(--space-sm)',
                    fontSize: 'var(--font-sm)',
                    color: 'var(--accent)',
                    fontWeight: 600,
                    display: 'flex',
                    alignItems: 'center',
                    gap: 'var(--space-xs)',
                    cursor: 'pointer',
                    borderRadius: 'var(--radius-sm)',
                    transition: 'background 0.1s',
                  }}
                  onMouseEnter={e => { e.currentTarget.style.background = 'var(--bg-hover)'; }}
                  onMouseLeave={e => { e.currentTarget.style.background = 'transparent'; }}
                >
                  <span style={{ fontSize: 'var(--font-xs)', width: 12, textAlign: 'center' }}>
                    {isCollapsed ? '▸' : '▾'}
                  </span>
                  <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{p.name}</span>
                  <span style={{ fontSize: 'var(--font-xs)', color: 'var(--text-muted)' }}>{p.checklist.length}</span>
                  <Button
                    variant="ghost"
                    onClick={e => { e.stopPropagation(); onCopyItems(p.id); }}
                    title="Copy all"
                    style={{ padding: 'var(--space-xs)', minWidth: 24, minHeight: 24 }}
                  >
                    <Icon symbol="📋" size={12} />
                  </Button>
                </div>
                {!isCollapsed && p.checklist.map((item, idx) => {
                  const isChecked = checked.has(`${p.id}-${idx}`);
                  return (
                    <div
                      key={idx}
                      onClick={() => toggle(p.id, idx)}
                      style={{
                        padding: 'var(--space-xs) var(--space-sm)',
                        fontSize: 'var(--font-sm)',
                        color: 'var(--text-primary)',
                        cursor: 'pointer',
                        display: 'flex',
                        alignItems: 'flex-start',
                        gap: 'var(--space-sm)',
                        textDecoration: isChecked ? 'line-through' : 'none',
                        opacity: isChecked ? 0.5 : 1,
                        borderRadius: 'var(--radius-sm)',
                        transition: 'background 0.1s',
                      }}
                      onMouseEnter={e => { e.currentTarget.style.background = 'var(--bg-hover)'; }}
                      onMouseLeave={e => { e.currentTarget.style.background = 'transparent'; }}
                    >
                      <span style={{ fontSize: 'var(--font-sm)', flexShrink: 0 }}>{isChecked ? '☑' : '☐'}</span>
                      <span style={{ flex: 1, wordBreak: 'break-word' }}>{item}</span>
                    </div>
                  );
                })}
              </div>
            );
          })}
          {projectsWithItems.length === 0 && <EmptyState message="No checklist items" />}
        </div>
      </ScrollArea>
    </div>
  );
};
