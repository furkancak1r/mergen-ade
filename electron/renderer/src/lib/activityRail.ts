import type { AppConfig } from '../../../shared/types';
import { LeftSidebarTab, TerminalManagerFilter } from '../../../shared/types';

export interface ActivityRailItem {
  id: 'directory' | 'terminalManager' | 'inputHistory' | 'tools' | 'settings';
  icon: string;
  title: string;
  tab?: LeftSidebarTab;
}

export const activityRailItems: ActivityRailItem[] = [
  { id: 'directory', icon: '▦', title: 'Directory', tab: LeftSidebarTab.Directory },
  { id: 'terminalManager', icon: '>_', title: 'Terminal Manager', tab: LeftSidebarTab.TerminalManager },
  { id: 'inputHistory', icon: '◷', title: 'Input History', tab: LeftSidebarTab.InputHistory },
  { id: 'tools', icon: '⑂', title: 'Source Control & Browser' },
  { id: 'settings', icon: '⚙', title: 'Settings' },
];

export function activityRailItem(id: ActivityRailItem['id']): ActivityRailItem {
  const item = activityRailItems.find((candidate) => candidate.id === id);
  if (!item) throw new Error(`Unknown activity rail item: ${id}`);
  return item;
}

export function isLeftSidebarTabActive(config: AppConfig | null | undefined, tab: LeftSidebarTab): boolean {
  return Boolean(config?.ui.showProjectExplorer && config.ui.projectExplorerExpanded && config.ui.leftSidebarTab === tab);
}

export function withLeftSidebarTabOpen(config: AppConfig, tab: LeftSidebarTab): AppConfig {
  return {
    ...config,
    ui: {
      ...config.ui,
      showProjectExplorer: true,
      projectExplorerExpanded: true,
      leftSidebarTab: tab,
      terminalManagerFilter: tab === LeftSidebarTab.TerminalManager
        ? TerminalManagerFilter.Foreground
        : config.ui.terminalManagerFilter,
    },
  };
}

export function withLeftSidebarRailToggle(config: AppConfig, tab: LeftSidebarTab): AppConfig {
  if (isLeftSidebarTabActive(config, tab)) {
    return {
      ...config,
      ui: {
        ...config.ui,
        projectExplorerExpanded: false,
      },
    };
  }

  return withLeftSidebarTabOpen(config, tab);
}
