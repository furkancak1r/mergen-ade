import type { AppConfig, TerminalManagerFilter } from '../../../shared/types';
import { LeftSidebarTab, TerminalManagerFilter as TerminalManagerFilterEnum } from '../../../shared/types';

export type TerminalManagerPathMenuAction = 'copy_path' | 'open_folder';

export function terminalManagerPathMenuLabel(action: TerminalManagerPathMenuAction): string {
  switch (action) {
    case 'copy_path':
      return '⧉ Copy Path';
    case 'open_folder':
      return '📂 Open in Folder';
  }
}

export function shouldShowForegroundLauncherButton(
  terminalManagerFilter: TerminalManagerFilter,
): boolean {
  return terminalManagerFilter === TerminalManagerFilterEnum.Foreground;
}

export function withTerminalManagerFilter(
  config: AppConfig,
  terminalManagerFilter: TerminalManagerFilter,
): AppConfig {
  if (config.ui.terminalManagerFilter === terminalManagerFilter) return config;
  return {
    ...config,
    ui: {
      ...config.ui,
      terminalManagerFilter,
    },
  };
}

export function withToggledTerminalManagerHideInactive(config: AppConfig): AppConfig {
  return {
    ...config,
    ui: {
      ...config.ui,
      terminalManagerHideInactiveProjects: !config.ui.terminalManagerHideInactiveProjects,
    },
  };
}

export function withTerminalManagerOpened(config: AppConfig): AppConfig {
  if (
    config.ui.leftSidebarTab === LeftSidebarTab.TerminalManager
    && config.ui.terminalManagerFilter === TerminalManagerFilterEnum.Foreground
    && config.ui.showProjectExplorer
    && config.ui.projectExplorerExpanded
  ) {
    return config;
  }

  return {
    ...config,
    ui: {
      ...config.ui,
      showProjectExplorer: true,
      projectExplorerExpanded: true,
      leftSidebarTab: LeftSidebarTab.TerminalManager,
      terminalManagerFilter: TerminalManagerFilterEnum.Foreground,
    },
  };
}

export function normalizeTerminalManagerStartupState(config: AppConfig): AppConfig {
  const shouldResetFilter = config.ui.leftSidebarTab === LeftSidebarTab.TerminalManager
    && config.ui.terminalManagerFilter !== TerminalManagerFilterEnum.Foreground;
  const shouldShowInactive = config.ui.terminalManagerHideInactiveProjects;

  if (!shouldResetFilter && !shouldShowInactive) return config;

  return {
    ...config,
    ui: {
      ...config.ui,
      terminalManagerFilter: shouldResetFilter ? TerminalManagerFilterEnum.Foreground : config.ui.terminalManagerFilter,
      terminalManagerHideInactiveProjects: false,
    },
  };
}
