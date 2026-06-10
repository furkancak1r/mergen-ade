import { describe, expect, it } from 'vitest';
import { defaultAppConfig, LeftSidebarTab, TerminalManagerFilter } from '../../../shared/types';
import { activityRailItem, activityRailItems, isLeftSidebarTabActive, withLeftSidebarRailToggle, withLeftSidebarTabOpen } from './activityRail';

describe('activityRail', () => {
  it('keeps the Rust activity rail order and tooltip copy', () => {
    expect(activityRailItems.map((item) => item.id)).toEqual([
      'directory',
      'sourceControl',
      'terminalManager',
      'inputHistory',
      'browser',
      'checklist',
      'settings',
    ]);
    expect(activityRailItem('directory')).toMatchObject({ icon: '▦', title: 'Open Directory', tab: LeftSidebarTab.Directory });
    expect(activityRailItem('sourceControl')).toMatchObject({ icon: '⑂', title: 'Open Source Control', tab: LeftSidebarTab.SourceControl });
    expect(activityRailItem('terminalManager')).toMatchObject({ icon: '>_', title: 'Open Terminal Manager', tab: LeftSidebarTab.TerminalManager });
    expect(activityRailItem('inputHistory')).toMatchObject({ icon: '◷', title: 'Open Input History', tab: LeftSidebarTab.InputHistory });
    expect(activityRailItem('browser')).toMatchObject({ icon: '◎', title: 'Toggle Browser Panel' });
    expect(activityRailItem('checklist')).toMatchObject({ icon: '✓', title: 'Toggle Check-list' });
    expect(activityRailItem('settings')).toMatchObject({ icon: '⚙', title: 'Settings' });
  });

  it('collapses the active left sidebar tab like the Rust rail', () => {
    const config = defaultAppConfig();
    config.ui.leftSidebarTab = LeftSidebarTab.Directory;
    config.ui.projectExplorerExpanded = true;

    const next = withLeftSidebarRailToggle(config, LeftSidebarTab.Directory);

    expect(next.ui.projectExplorerExpanded).toBe(false);
    expect(isLeftSidebarTabActive(next, LeftSidebarTab.Directory)).toBe(false);
  });

  it('opens the requested left sidebar tab and resets Terminal Manager to Foreground', () => {
    const config = defaultAppConfig();
    config.ui.projectExplorerExpanded = false;
    config.ui.leftSidebarTab = LeftSidebarTab.Directory;
    config.ui.terminalManagerFilter = TerminalManagerFilter.Background;

    const next = withLeftSidebarRailToggle(config, LeftSidebarTab.TerminalManager);

    expect(next.ui.showProjectExplorer).toBe(true);
    expect(next.ui.projectExplorerExpanded).toBe(true);
    expect(next.ui.leftSidebarTab).toBe(LeftSidebarTab.TerminalManager);
    expect(next.ui.terminalManagerFilter).toBe(TerminalManagerFilter.Foreground);
    expect(isLeftSidebarTabActive(next, LeftSidebarTab.TerminalManager)).toBe(true);
  });

  it('programmatic left sidebar open does not collapse an already-active tab', () => {
    const config = defaultAppConfig();
    config.ui.leftSidebarTab = LeftSidebarTab.TerminalManager;
    config.ui.projectExplorerExpanded = true;

    const next = withLeftSidebarTabOpen(config, LeftSidebarTab.TerminalManager);

    expect(next.ui.projectExplorerExpanded).toBe(true);
    expect(next.ui.leftSidebarTab).toBe(LeftSidebarTab.TerminalManager);
  });
});
