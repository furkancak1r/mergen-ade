import React, { useState, useEffect } from 'react';
import { WebSourceControlFile, WebProject } from '../types';

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

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
      <div style={{ padding: 8, borderBottom: '1px solid #333', display: 'flex', alignItems: 'center', gap: 4 }}>
        <strong style={{ fontSize: 12, color: '#aaa', flex: 1 }}>Source Control</strong>
      </div>
      <div style={{ padding: 8, overflow: 'auto' }}>
        {loading && <div style={{ fontSize: 11, color: '#888' }}>Loading...</div>}
        {!project && <div style={{ fontSize: 11, color: '#666' }}>Select a project</div>}
        {project && (
          <>
            <div style={{ fontSize: 12, color: '#4fc3f7', marginBottom: 8 }}>
              {branch ? `🌿 ${branch}` : 'No git repository'}
            </div>
            {project.is_worktree && (
              <div style={{ fontSize: 11, color: '#ff9800', marginBottom: 8 }}>
                Worktree of {project.repo_root}
              </div>
            )}
            {files.map(f => (
              <div key={f.path} style={{ fontSize: 11, padding: '2px 0', display: 'flex', gap: 4 }}>
                <span style={{ color: f.status === 'modified' ? '#ff9800' : f.status === 'added' ? '#4caf50' : '#e0e0e0', width: 14 }}>
                  {f.status === 'modified' ? 'M' : f.status === 'added' ? 'A' : f.status === 'deleted' ? 'D' : '?'}
                </span>
                <span style={{ color: '#e0e0e0', overflow: 'hidden', textOverflow: 'ellipsis' }}>{f.path}</span>
              </div>
            ))}
            {files.length === 0 && branch && (
              <div style={{ fontSize: 11, color: '#666' }}>No changes</div>
            )}
          </>
        )}
      </div>
    </div>
  );
};
