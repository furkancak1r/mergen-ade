import React, { useState } from 'react';
import { WebShortcut, WebLauncher } from '../types';
import { Button, Icon } from '../components/ui';

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
        background: 'var(--bg-elevated)',
        border: '1px solid var(--border-default)',
        borderRadius: 'var(--radius-xl)',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
      }}>
        <div style={{
          padding: 'var(--space-lg) var(--space-xl)',
          borderBottom: '1px solid var(--border-subtle)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
        }}>
          <strong style={{ color: 'var(--text-primary)', fontSize: 'var(--font-lg)' }}>Settings</strong>
          <Button variant="ghost" onClick={onClose} style={{ fontSize: 'var(--font-xl)', padding: 'var(--space-xs)', minWidth: 32, minHeight: 32 }}>
            <Icon symbol="✕" size={16} />
          </Button>
        </div>

        <div style={{ display: 'flex', borderBottom: '1px solid var(--border-subtle)' }}>
          {(['shortcuts', 'launchers', 'general'] as const).map(t => {
            const isActive = tab === t;
            return (
              <button
                key={t}
                onClick={() => setTab(t)}
                style={{
                  flex: 1,
                  padding: 'var(--space-md) var(--space-lg)',
                  background: isActive ? 'var(--bg-active)' : 'transparent',
                  border: 'none',
                  borderBottom: isActive ? '2px solid var(--accent)' : '2px solid transparent',
                  color: isActive ? 'var(--accent)' : 'var(--text-secondary)',
                  cursor: 'pointer',
                  fontSize: 'var(--font-base)',
                  textTransform: 'capitalize',
                  fontWeight: isActive ? 600 : 400,
                  transition: 'background 0.12s, color 0.12s',
                }}
                onMouseEnter={e => {
                  if (!isActive) e.currentTarget.style.background = 'var(--bg-hover)';
                }}
                onMouseLeave={e => {
                  if (!isActive) e.currentTarget.style.background = 'transparent';
                }}
              >
                {t}
              </button>
            );
          })}
        </div>

        <div style={{ flex: 1, overflow: 'auto', padding: 'var(--space-lg)' }}>
          {tab === 'shortcuts' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--space-sm)' }}>
              {shortcuts.map(s => (
                <div
                  key={s.id}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 'var(--space-md)',
                    padding: 'var(--space-sm) var(--space-md)',
                    background: 'var(--bg-hover)',
                    borderRadius: 'var(--radius-md)',
                  }}
                >
                  <span style={{ fontSize: 'var(--font-sm)', color: 'var(--accent)', minWidth: 60, fontWeight: 500 }}>
                    {s.key}
                  </span>
                  <span style={{ fontSize: 'var(--font-sm)', color: 'var(--text-primary)', flex: 1 }}>
                    {s.label}
                  </span>
                  <span style={{ fontSize: 'var(--font-xs)', color: 'var(--text-muted)' }}>
                    {s.command}
                  </span>
                  <span style={{ fontSize: 'var(--font-xs)', color: s.enabled ? 'var(--success)' : 'var(--danger)', fontWeight: 600 }}>
                    {s.enabled ? 'ON' : 'OFF'}
                  </span>
                </div>
              ))}
            </div>
          )}
          {tab === 'launchers' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--space-sm)' }}>
              {launchers.map(l => (
                <div
                  key={l.id}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 'var(--space-md)',
                    padding: 'var(--space-sm) var(--space-md)',
                    background: 'var(--bg-hover)',
                    borderRadius: 'var(--radius-md)',
                  }}
                >
                  <span style={{ fontSize: 'var(--font-sm)', color: 'var(--text-primary)', flex: 1 }}>
                    {l.display_name}
                  </span>
                  <span style={{ fontSize: 'var(--font-xs)', color: 'var(--text-muted)' }}>
                    {l.command}
                  </span>
                  <span style={{ fontSize: 'var(--font-xs)', color: l.enabled ? 'var(--success)' : 'var(--danger)', fontWeight: 600 }}>
                    {l.enabled ? 'ON' : 'OFF'}
                  </span>
                </div>
              ))}
            </div>
          )}
          {tab === 'general' && (
            <div style={{ fontSize: 'var(--font-base)', color: 'var(--text-primary)' }}>
              <div style={{ marginBottom: 'var(--space-lg)' }}>
                <div style={{ color: 'var(--text-secondary)', marginBottom: 'var(--space-sm)', fontSize: 'var(--font-sm)', fontWeight: 500 }}>
                  Default Shell
                </div>
                <div style={{ background: 'var(--bg-hover)', padding: 'var(--space-sm) var(--space-md)', borderRadius: 'var(--radius-md)' }}>
                  {defaultShell}
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
