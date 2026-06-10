import React, { useState, useCallback, useRef } from 'react';
import type { ProjectRecord } from '../../../shared/types';
import {
  CHECKLIST_EMPTY_MESSAGE,
  checklistCopiedItemsMessage,
  formatChecklistForClipboard,
  projectsWithChecklistItems,
} from '../lib/checklist';

const CHECKLIST_MESSAGE_MAX_HEIGHT = 120;

interface ChecklistProps {
  projects: ProjectRecord[];
  rightOffset?: number;
  onRemoveItem?: (projectId: number, index: number) => void;
  onClose?: () => void;
}

export const Checklist: React.FC<ChecklistProps> = ({ projects, rightOffset = 18, onRemoveItem, onClose }) => {
  const [collapsed, setCollapsed] = useState<Set<number>>(new Set());
  const [toast, setToast] = useState<{ text: string; id: number } | null>(null);
  const toastIdRef = useRef(0);
  const projectsWithItems = projectsWithChecklistItems(projects);

  const showToast = useCallback((text: string) => {
    const id = ++toastIdRef.current;
    setToast({ text, id });
    setTimeout(() => {
      setToast((prev) => (prev?.id === id ? null : prev));
    }, 2000);
  }, []);

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
    const text = formatChecklistForClipboard(project.checklist);
    navigator.clipboard.writeText(text).catch(() => {});
    showToast(checklistCopiedItemsMessage(project.checklist.length));
  }, [showToast]);

  const copyItem = useCallback((item: string) => {
    navigator.clipboard.writeText(item).catch(() => {});
    showToast('Copied message');
  }, [showToast]);

  return (
    <div style={{ position: 'fixed', bottom: 16, right: rightOffset, width: 360, maxHeight: 500, background: '#141414', border: '1px solid #333', borderRadius: 8, display: 'flex', flexDirection: 'column', overflow: 'hidden', zIndex: 100 }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '8px 12px', borderBottom: '1px solid #222' }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: '#eee' }}>Check-list</span>
        {onClose && (
          <button onClick={onClose} style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 14 }}>
            ✕
          </button>
        )}
      </div>
      <div style={{ flex: 1, overflow: 'auto', padding: '4px 0' }}>
        {projectsWithItems.map((project) => (
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
                  <div key={i} style={{ display: 'flex', alignItems: 'flex-start', gap: 6, padding: '3px 12px 3px 24px' }}>
                    <button
                      onClick={() => onRemoveItem?.(project.id, i)}
                      style={{ background: 'transparent', border: 'none', color: '#aaa', cursor: 'pointer', fontSize: 11, padding: 0, marginTop: 0 }}
                      title="Remove"
                    >
                      ☐
                    </button>
                    <div style={{ flex: 1, maxHeight: CHECKLIST_MESSAGE_MAX_HEIGHT, overflow: 'auto' }}>
                      <span style={{ fontSize: 11, color: '#aaa', whiteSpace: 'pre-wrap', wordBreak: 'break-word' }}>{item}</span>
                    </div>
                    <button
                      onClick={() => copyItem(item)}
                      style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 10, padding: 0, flexShrink: 0 }}
                      title="Copy"
                    >
                      📋
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}
        {projectsWithItems.length === 0 && (
          <div style={{ minHeight: 160, display: 'flex', alignItems: 'center', justifyContent: 'center', padding: 20, color: '#888', fontSize: 13, textAlign: 'center', whiteSpace: 'pre-line' }}>
            {CHECKLIST_EMPTY_MESSAGE}
          </div>
        )}
      </div>
      {toast && (
        <div style={{
          position: 'absolute',
          bottom: 8,
          left: '50%',
          transform: 'translateX(-50%)',
          background: '#1a1a1a',
          border: '1px solid #333',
          borderRadius: 4,
          padding: '6px 12px',
          fontSize: 11,
          color: '#ccc',
          zIndex: 10,
        }}>
          {toast.text}
        </div>
      )}
    </div>
  );
};
