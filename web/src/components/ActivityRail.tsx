import React from 'react';

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
  browser: { icon: '▣', label: 'Browser' },
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
      <button
        key={item}
        title={label}
        onClick={() => onTogglePanel(item)}
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          background: isActive ? '#1e3a5f' : 'transparent',
          border: 'none',
          borderRadius: 6,
          cursor: 'pointer',
          fontSize: isMobile ? 18 : 16,
          color: isActive ? '#4fc3f7' : '#888',
          transition: 'background 0.15s',
          minWidth: 44,
          minHeight: 44,
          flex: isMobile ? 1 : undefined,
        }}
      >
        {icon}
      </button>
    );
  };

  return (
    <>
      {/* Desktop vertical rail */}
      <div className="activity-rail-desktop" style={{ width: 48, flexShrink: 0, background: '#111', borderRight: '1px solid #333', display: 'flex', flexDirection: 'column', alignItems: 'center', padding: '8px 0', gap: 4 }}>
        {items.map(item => renderButton(item, false))}
      </div>

      {/* Mobile bottom bar */}
      <div className="activity-rail-mobile" style={{ display: 'none' }}>
        {items.map(item => renderButton(item, true))}
      </div>
    </>
  );
};
