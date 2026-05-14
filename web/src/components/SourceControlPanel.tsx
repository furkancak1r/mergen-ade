import React, { useState, useEffect } from 'react';
import { WebSourceControlFile, WebProject } from '../types';
import { PanelHeader, ScrollArea, EmptyState, LoadingState, Icon } from '../components/ui';

interface Props {
  project: WebProject | null;
  apiUrl: string;
}

export const SourceControlPanel: React.FC<Props> = ({ project, apiUrl }) => {
  const [branch, setBranch] = useState('');
  const [files, setFiles] = useState<WebSourceControlFile[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!project) {
      setBranch('');
      setFiles([]);
      return;
    }
    setLoading(true);
    fetch(`${apiUrl}/api/source-control?project_id=${project.id}`)
      .then(r => r.json())
      .then(data => {
        if (data.success && data.data) {
          setBranch(data.data.branch || '');
          setFiles(data.data.files || []);
        }
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, [project, apiUrl]);

  const statusColor = (status: string) => {
    switch (status) {
      case 'modified': return 'var(--warning)';
      case 'added': return 'var(--success)';
      case 'deleted': return 'var(--danger)';
      default: return 'var(--text-primary)';
    }
  };

  const statusLetter = (status: string) => {
    switch (status) {
      case 'modified': return 'M';
      case 'added': return 'A';
      case 'deleted': return 'D';
      default: return '?';
    }
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
      <PanelHeader title="Source Control" />
      <ScrollArea>
        <div style={{ padding: 'var(--space-md)' }}>
          {loading && <LoadingState />}
          {!project && <EmptyState message="Select a project" />}
          {project && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--space-md)' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--space-sm)', fontSize: 'var(--font-base)', color: 'var(--accent)' }}>
                <Icon symbol="⎇" size={12} color="var(--accent)" />
                {branch || 'No git repository'}
              </div>
              {project.is_worktree && (
                <div style={{ fontSize: 'var(--font-sm)', color: 'var(--warning)' }}>
                  Worktree of {project.repo_root}
                </div>
              )}
              <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--space-xs)' }}>
                {files.map(f => (
                  <div
                    key={f.path}
                    style={{
                      fontSize: 'var(--font-sm)',
                      padding: 'var(--space-xs) 0',
                      display: 'flex',
                      alignItems: 'center',
                      gap: 'var(--space-sm)',
                    }}
                  >
                    <span style={{ color: statusColor(f.status), width: 14, textAlign: 'center', fontWeight: 600, flexShrink: 0 }}>
                      {statusLetter(f.status)}
                    </span>
                    <span style={{ color: 'var(--text-primary)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                      {f.path}
                    </span>
                  </div>
                ))}
                {files.length === 0 && branch && (
                  <EmptyState message="No changes" />
                )}
              </div>
            </div>
          )}
        </div>
      </ScrollArea>
    </div>
  );
};
