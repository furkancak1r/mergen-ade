import React, { useState } from 'react';
import { WebTerminal, WebProject } from '../types';
import {
  PanelHeader,
  Button,
  Row,
  ScrollArea,
  EmptyState,
  FilterTabs,
  PopupMenu,
  PopupMenuItem,
  Icon,
} from '../components/ui';

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
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
      <PanelHeader title="Terminals" />
      <div style={{ padding: 'var(--space-md)' }}>
        <FilterTabs
          options={['foreground', 'background', 'all'] as const}
          active={filter}
          onChange={v => setFilter(v as typeof filter)}
        />

        {selectedProjectId && (
          <div style={{ display: 'flex', gap: 'var(--space-sm)', marginBottom: 'var(--space-md)' }}>
            <Button variant="primary" onClick={() => setShowLaunchers(v => !v)} style={{ flex: 1 }}>
              <Icon symbol="▸" size={10} style={{ marginRight: 2 }} />
              Foreground
            </Button>
            <Button variant="secondary" onClick={() => onSpawn(selectedProjectId, 'powershell', 'background')} style={{ flex: 1 }}>
              + Background
            </Button>
          </div>
        )}

        {showLaunchers && selectedProjectId && (
          <PopupMenu style={{ marginBottom: 'var(--space-md)' }}>
            {configLaunchers.filter(l => l.enabled).map(l => (
              <PopupMenuItem
                key={l.id}
                onClick={() => {
                  onSpawn(selectedProjectId, 'powershell', 'foreground');
                  setShowLaunchers(false);
                }}
              >
                <div>
                  <div style={{ fontWeight: 600 }}>{l.display_name}</div>
                  <div style={{ fontSize: 'var(--font-xs)', color: 'var(--text-muted)' }}>{l.command}</div>
                </div>
              </PopupMenuItem>
            ))}
          </PopupMenu>
        )}
      </div>

      <ScrollArea>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--space-xs)', padding: '0 var(--space-xs)' }}>
          {filtered.map(t => {
            const isActive = activeTerminalId === t.id;
            return (
              <div key={t.id}>
                <Row
                  active={isActive}
                  exited={t.exited}
                  onClick={() => onActivate(t.id)}
                  style={{
                    borderLeft: `2px solid ${t.kind === 'foreground' ? 'var(--accent)' : 'var(--text-secondary)'}`,
                    borderRadius: '0 var(--radius-md) var(--radius-md) 0',
                  }}
                >
                  <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 'var(--space-xs)' }}>
                    <div style={{ fontWeight: 600, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                      {t.title || `Terminal #${t.id}`}
                    </div>
                    <div style={{ fontSize: 'var(--font-xs)', color: 'var(--text-secondary)' }}>
                      {t.shell} {t.ai_status && `· ${t.ai_status}`}
                    </div>
                  </div>
                  {!t.exited && (
                    <div style={{ display: 'flex', gap: 'var(--space-xs)', flexShrink: 0 }}>
                      <Button
                        variant="ghost"
                        onClick={e => { e.stopPropagation(); setActiveMsgMenu(v => v === t.id ? null : t.id); }}
                        title="Saved messages"
                        style={{ minWidth: 24, minHeight: 24, padding: 0 }}
                      >
                        <Icon symbol="✉" size={12} />
                      </Button>
                      <Button
                        variant="ghost"
                        onClick={e => { e.stopPropagation(); setActiveShortcutMenu(v => v === t.id ? null : t.id); }}
                        title="Shortcuts"
                        style={{ minWidth: 24, minHeight: 24, padding: 0 }}
                      >
                        <Icon symbol="⌨" size={12} />
                      </Button>
                      <Button
                        variant="danger"
                        onClick={e => { e.stopPropagation(); onClose(t.id); }}
                        title="Close"
                        style={{ minWidth: 24, minHeight: 24, padding: 0 }}
                      >
                        <Icon symbol="✕" size={12} />
                      </Button>
                    </div>
                  )}
                </Row>

                {activeMsgMenu === t.id && selectedProject && (
                  <PopupMenu style={{ margin: 'var(--space-xs) 0' }}>
                    {selectedProject.saved_messages.length === 0 && (
                      <EmptyState message="No saved messages" />
                    )}
                    {selectedProject.saved_messages.map((msg, idx) => (
                      <PopupMenuItem
                        key={idx}
                        onClick={() => { onSendSavedMessage(t.id, msg); setActiveMsgMenu(null); }}
                      >
                        {msg}
                      </PopupMenuItem>
                    ))}
                  </PopupMenu>
                )}

                {activeShortcutMenu === t.id && (
                  <PopupMenu style={{ margin: 'var(--space-xs) 0' }}>
                    {configShortcuts.filter(s => s.enabled).map(s => (
                      <PopupMenuItem
                        key={s.id}
                        shortcut={s.key}
                        onClick={() => { onSendShortcut(t.id, s.command); setActiveShortcutMenu(null); }}
                      >
                        {s.label}
                      </PopupMenuItem>
                    ))}
                  </PopupMenu>
                )}
              </div>
            );
          })}
          {filtered.length === 0 && <EmptyState message="No terminals" />}
        </div>
      </ScrollArea>
    </div>
  );
};
