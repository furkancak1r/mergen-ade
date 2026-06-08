import React, { useState, useCallback } from 'react';
import type { ProjectRecord } from '../../../shared/types';

interface ChecklistProps {
  projects: ProjectRecord[];
}

export const Checklist: React.FC<ChecklistProps> = ({ projects }) => {
  const [collapsed, setCollapsed] = useState<Set<number>>(new Set());

  const toggleProject = useCallback((projectId: number) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(projectId)) {
        next.delete(projectId);
      } else {
        next.add(projectId);
      }
      return next;
    });
  }, []);

  const copyAll = useCallback((project: ProjectRecord) => {
    const text = project.checklist.join('\n\n');
    navigator.clipboard.writeText(text).catch(() => {});
  }, []);

  return (
    <div style={{ position: 'fixed', bottom: 16, right: 16, width: 360, maxHeight: 500, background: '#141414', border: '1px solid #333', borderRadius: 8, display: 'flex', flexDirection: 'column', overflow: 'hidden', zIndex: 100 }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '8px 12px', borderBottom: '1px solid #222' }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: '#eee' }}>Checklist</span>
      </div>
      <div style={{ flex: 1, overflow: 'auto', padding: '4px 0' }}>
        {projects.map((project) => (
          <div key={project.id} style={{ marginBottom: 4 }}>
            <div
              style={{ display: 'flex', alignItems: 'center', gap: 4, padding: '4px 12px', cursor: 'pointer' }}
              onClick={() => toggleProject(project.id)}
            >
              <span style={{ fontSize: 10, color: '#888' }}>{collapsed.has(project.id) ? '▶' : '▼'}</span>
              <span style={{ fontSize: 12, fontWeight: 600, color: '#ccc', flex: 1 }}>{project.name}</span>
              <span style={{ fontSize: 10, color: '#666' }}>{project.checklist.length}</span>
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  copyAll(project);
                }}
                style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 10 }}
                title="Copy all"
              >
                📋
              </button>
            </div>
            {!collapsed.has(project.id) && (
              <div>
                {project.checklist.map((item, i) => (
                  <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '3px 12px 3px 24px' }}>
                    <span style={{ fontSize: 11, color: '#aaa' }}>☐</span>
                    <span style={{ fontSize: 11, color: '#aaa', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis' }}>{item}</span>
                  </div>
                ))}
                {project.checklist.length === 0 && (
                  <div style={{ padding: '2px 12px 2px 24px', fontSize: 11, color: '#666' }}>No items</div>
                )}
              </div>
            )}
          </div>
        ))}
        {projects.length === 0 && (
          <div style={{ padding: 12, color: '#888', fontSize: 12 }}>No projects with checklist items.</div>
        )}
      </div>
    </div>
  );
};
