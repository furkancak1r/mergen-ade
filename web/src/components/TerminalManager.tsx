import React, { useState } from 'react';
import { WebTerminal, WebProject } from '../types';

interface Props {
  terminals: WebTerminal[];
  activeTerminalId: number | null;
  onActivate: (id: number) => void;
  selectedProjectId: number | null;
  projects: WebProject[];
  onSpawn: (projectId: number, shell: string, kind: string) => void;
  onClose: (id: number) => void;
  onSendSavedMessage: (terminalId: number, message: string) => void;
  onSendShortcut: (terminalId: number, command: string) => void;
  configLaunchers: { id: string; display_name: string; command: string; enabled: boolean }[];
  configShortcuts: { id: string; label: string; key: string; command: string; enabled: boolean }[];
}

export const TerminalManager: React.FC<Props> = ({
  terminals,
  activeTerminalId,
  onActivate,
  selectedProjectId,
  projects,
  onSpawn,
  onClose,
  onSendSavedMessage,
  onSendShortcut,
  configLaunchers,
  configShortcuts,
}) => {
  const [filter, setFilter] = useState<'foreground' | 'background' | 'all'>('foreground');
  const [showLaunchers, setShowLaunchers] = useState(false);
  const [activeMsgMenu, setActiveMsgMenu] = useState<number | null>(null);
  const [activeShortcutMenu, setActiveShortcutMenu] = useState<number | null>(null);

  const filtered = terminals.filter(t => {
    if (filter === 'all') return true;
    return t.kind === filter;
  });

  const selectedProject = projects.find(p => p.id === selectedProjectId);

  return (
    <div style={{ padding: 8, display: 'flex', flexDirection: 'column', height: '100%' }}>
      <div style={{ display: 'flex', gap: 4, marginBottom: 8 }}>
        {(['foreground', 'background', 'all'] as const).map(f => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            style={{
              flex: 1,
              fontSize: 10,
              background: filter === f ? '#264f78' : '#222',
              border: '1px solid #444',
              color: '#e0e0e0',
              cursor: 'pointer',
              padding: '2px 4px',
            }}
          >
            {f[0].toUpperCase() + f.slice(1)}
          </button>
        ))}
      </div>

      {selectedProjectId && (
        <div style={{ display: 'flex', gap: 4, marginBottom: 8 }}>
          <button
            onClick={() => setShowLaunchers(v => !v)}
            style={{ flex: 1, fontSize: 11, background: '#1e3a5f', border: '1px solid #4fc3f7', color: '#4fc3f7', cursor: 'pointer' }}
          >
            + Foreground ▾
          </button>
          <button
            onClick={() => onSpawn(selectedProjectId, 'powershell', 'background')}
            style={{ flex: 1, fontSize: 11, background: '#2a2a2a', border: '1px solid #888', color: '#888', cursor: 'pointer' }}
          >
            + Background
          </button>
        </div>
      )}

      {showLaunchers && selectedProjectId && (
        <div style={{ marginBottom: 8, background: '#1a1a1a', border: '1px solid #444', borderRadius: 4, padding: 4, display: 'flex', flexDirection: 'column', gap: 2 }}>
          {configLaunchers.filter(l => l.enabled).map(l => (
            <button
              key={l.id}
              onClick={() => {
                onSpawn(selectedProjectId, 'powershell', 'foreground');
                setShowLaunchers(false);
              }}
              style={{ textAlign: 'left', fontSize: 11, background: 'transparent', border: 'none', color: '#e0e0e0', cursor: 'pointer', padding: '4px 6px' }}
            >
              <div style={{ fontWeight: 'bold' }}>{l.display_name}</div>
              <div style={{ fontSize: 9, color: '#888' }}>{l.command}</div>
            </button>
          ))}
        </div>
      )}

      <div style={{ flex: 1, overflow: 'auto', display: 'flex', flexDirection: 'column', gap: 2 }}>
        {filtered.map(t => {
          const isActive = activeTerminalId === t.id;
          return (
            <div key={t.id}>
              <div
                onClick={() => onActivate(t.id)}
                style={{
                  padding: '4px 6px',
                  borderRadius: 4,
                  cursor: 'pointer',
                  background: isActive ? '#1e3a5f' : 'transparent',
                  fontSize: 11,
                  color: t.exited ? '#666' : '#e0e0e0',
                  borderLeft: `2px solid ${t.kind === 'foreground' ? '#4fc3f7' : '#888'}`,
                  display: 'flex',
                  alignItems: 'center',
                  gap: 4,
                }}
              >
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontWeight: 'bold', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    {t.title || `Terminal #${t.id}`}
                  </div>
                  <div style={{ fontSize: 10, color: '#888' }}>
                    {t.shell} {t.ai_status && `| ${t.ai_status}`}
                  </div>
                </div>
                {!t.exited && (
                  <>
                    <button
                      onClick={e => { e.stopPropagation(); setActiveMsgMenu(v => v === t.id ? null : t.id); }}
                      style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 12, padding: '0 2px' }}
                      title="Saved messages"
                    >
                      💬
                    </button>
                    <button
                      onClick={e => { e.stopPropagation(); setActiveShortcutMenu(v => v === t.id ? null : t.id); }}
                      style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 12, padding: '0 2px' }}
                      title="Shortcuts"
                    >
                      ⌨️
                    </button>
                    <button
                      onClick={e => { e.stopPropagation(); onClose(t.id); }}
                      style={{ background: 'transparent', border: 'none', color: '#f44336', cursor: 'pointer', fontSize: 12, padding: '0 2px' }}
                      title="Close"
                    >
                      ✕
                    </button>
                  </>
                )}
              </div>

              {activeMsgMenu === t.id && selectedProject && (
                <div style={{ background: '#1a1a1a', border: '1px solid #444', borderRadius: 4, padding: 4, margin: '2px 0' }}>
                  {selectedProject.saved_messages.length === 0 && (
                    <div style={{ fontSize: 10, color: '#666', padding: '2px 4px' }}>No saved messages</div>
                  )}
                  {selectedProject.saved_messages.map((msg, idx) => (
                    <button
                      key={idx}
                      onClick={() => { onSendSavedMessage(t.id, msg); setActiveMsgMenu(null); }}
                      style={{ display: 'block', width: '100%', textAlign: 'left', fontSize: 10, background: 'transparent', border: 'none', color: '#e0e0e0', cursor: 'pointer', padding: '3px 4px', overflow: 'hidden', textOverflow: 'ellipsis' }}
                    >
                      {msg}
                    </button>
                  ))}
                </div>
              )}

              {activeShortcutMenu === t.id && (
                <div style={{ background: '#1a1a1a', border: '1px solid #444', borderRadius: 4, padding: 4, margin: '2px 0' }}>
                  {configShortcuts.filter(s => s.enabled).map(s => (
                    <button
                      key={s.id}
                      onClick={() => { onSendShortcut(t.id, s.command); setActiveShortcutMenu(null); }}
                      style={{ display: 'block', width: '100%', textAlign: 'left', fontSize: 10, background: 'transparent', border: 'none', color: '#e0e0e0', cursor: 'pointer', padding: '3px 4px' }}
                    >
                      <span style={{ color: '#4fc3f7', minWidth: 40, display: 'inline-block' }}>{s.key}</span>
                      {s.label}
                    </button>
                  ))}
                </div>
              )}
            </div>
          );
        })}
        {filtered.length === 0 && (
          <div style={{ fontSize: 11, color: '#666', textAlign: 'center', padding: 8 }}>No terminals</div>
        )}
      </div>
    </div>
  );
};
