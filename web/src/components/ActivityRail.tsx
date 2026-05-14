import React from 'react';
import { Button } from '../components/ui';

export type PanelId = 'projects' | 'directory' | 'source-control' | 'input-history' | 'terminal-manager' | 'browser' | 'checklist' | 'settings';

interface Props {
  activePanel: PanelId | null;
  onTogglePanel: (panel: PanelId) => void;
  browserOpen: boolean;
  checklistOpen: boolean;
}

const icons: Record<PanelId, { icon: string; label: string }> = {
  projects: { icon: '◆', label: 'Projects' },
  directory: { icon: '▤', label: 'Directory' },
  'source-control': { icon: '⎇', label: 'Source Control' },
  'input-history': { icon: '◈', label: 'Input History' },
  'terminal-manager': { icon: '▣', label: 'Terminals' },
  browser: { icon: '◐', label: 'Browser' },
  checklist: { icon: '☐', label: 'Checklist' },
  settings: { icon: '⚙', label: 'Settings' },
};

export const ActivityRail: React.FC<Props> = ({ activePanel, onTogglePanel, browserOpen, checklistOpen }) => {
  const items: PanelId[] = [
    'projects',
    'directory',
    'source-control',
    'input-history',
    'terminal-manager',
    'browser',
    'checklist',
    'settings',
  ];

  const renderButton = (item: PanelId, isMobile: boolean) => {
    const isActive = activePanel === item || (item === 'browser' && browserOpen) || (item === 'checklist' && checklistOpen);
    const { icon, label } = icons[item];
    return (
      <Button
        key={item}
        variant={isActive ? 'active' : 'ghost'}
        onClick={() => onTogglePanel(item)}
        title={label}
        style={{
          width: isMobile ? 48 : 44,
          height: isMobile ? 48 : 44,
          fontSize: isMobile ? 'var(--font-xl)' : 'var(--font-lg)',
          padding: 0,
          borderRadius: 'var(--radius-lg)',
        }}
      >
        {icon}
      </Button>
    );
  };

  return (
    <>
      <div
        className="activity-rail-desktop"
        style={{
          width: 48,
          flexShrink: 0,
          background: '#111',
          borderRight: '1px solid var(--border-subtle)',
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          padding: 'var(--space-md) 0',
          gap: 'var(--space-xs)',
        }}
      >
        {items.map(item => renderButton(item, false))}
      </div>

      <div className="activity-rail-mobile" style={{ display: 'none' }}>
        {items.map(item => renderButton(item, true))}
      </div>
    </>
  );
};
