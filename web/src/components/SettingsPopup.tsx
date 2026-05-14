import React, { useState } from 'react';
import { WebShortcut, WebLauncher } from '../types';

interface Props {
  shortcuts: WebShortcut[];
  launchers: WebLauncher[];
  defaultShell: string;
  onClose: () => void;
}

export const SettingsPopup: React.FC<Props> = ({ shortcuts, launchers, defaultShell, onClose }) => {
  const [tab, setTab] = useState<'shortcuts' | 'launchers' | 'general'>('shortcuts');

  return (
    <div style={{
      position: 'fixed',
      inset: 0,
      background: 'rgba(0,0,0,0.7)',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      zIndex: 1000,
    }}>
      <div style={{
        width: 600,
        maxHeight: '80vh',
        background: '#1a1a1a',
        border: '1px solid #444',
        borderRadius: 8,
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
      }}>
        <div style={{ padding: '12px 16px', borderBottom: '1px solid #333', display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <strong style={{ color: '#e0e0e0' }}>Settings</strong>
          <button onClick={onClose} style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 18 }}>×</button>
        </div>
        <div style={{ display: 'flex', borderBottom: '1px solid #333' }}>
          {(['shortcuts', 'launchers', 'general'] as const).map(t => (
            <button
              key={t}
              onClick={() => setTab(t)}
              style={{
                flex: 1,
                padding: '8px 12px',
                background: tab === t ? '#264f78' : 'transparent',
                border: 'none',
                color: tab === t ? '#4fc3f7' : '#888',
                cursor: 'pointer',
                fontSize: 12,
                textTransform: 'capitalize',
              }}
            >
              {t}
            </button>
          ))}
        </div>
        <div style={{ flex: 1, overflow: 'auto', padding: 12 }}>
          {tab === 'shortcuts' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
              {shortcuts.map(s => (
                <div key={s.id} style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 8px', background: '#222', borderRadius: 4 }}>
                  <span style={{ fontSize: 11, color: '#4fc3f7', minWidth: 60 }}>{s.key}</span>
                  <span style={{ fontSize: 11, color: '#e0e0e0', flex: 1 }}>{s.label}</span>
                  <span style={{ fontSize: 10, color: '#888' }}>{s.command}</span>
                  <span style={{ fontSize: 10, color: s.enabled ? '#4caf50' : '#f44336' }}>{s.enabled ? 'ON' : 'OFF'}</span>
                </div>
              ))}
            </div>
          )}
          {tab === 'launchers' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
              {launchers.map(l => (
                <div key={l.id} style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 8px', background: '#222', borderRadius: 4 }}>
                  <span style={{ fontSize: 11, color: '#e0e0e0', flex: 1 }}>{l.display_name}</span>
                  <span style={{ fontSize: 10, color: '#888' }}>{l.command}</span>
                  <span style={{ fontSize: 10, color: l.enabled ? '#4caf50' : '#f44336' }}>{l.enabled ? 'ON' : 'OFF'}</span>
                </div>
              ))}
            </div>
          )}
          {tab === 'general' && (
            <div style={{ fontSize: 12, color: '#e0e0e0' }}>
              <div style={{ marginBottom: 12 }}>
                <div style={{ color: '#888', marginBottom: 4 }}>Default Shell</div>
                <div>{defaultShell}</div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
