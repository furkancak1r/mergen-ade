import { describe, expect, it } from 'vitest';
import { defaultAppConfig, LeftSidebarTab, TerminalManagerFilter } from '../../../shared/types';
import {
  normalizeTerminalManagerStartupState,
  shouldShowForegroundLauncherButton,
  shouldShowOpenCodeAcpButton,
  terminalManagerPathMenuLabel,
  withTerminalManagerFilter,
  withTerminalManagerOpened,
  withToggledTerminalManagerHideInactive,
} from './terminalManagerState';

describe('terminalManagerState', () => {
  it('updates the persisted terminal manager filter', () => {
    const config = defaultAppConfig();
    const next = withTerminalManagerFilter(config, TerminalManagerFilter.Background);

    expect(next).not.toBe(config);
    expect(next.ui.terminalManagerFilter).toBe(TerminalManagerFilter.Background);
    expect(config.ui.terminalManagerFilter).toBe(TerminalManagerFilter.Foreground);
  });

  it('keeps the same config object when filter does not change', () => {
    const config = defaultAppConfig();
    const next = withTerminalManagerFilter(config, TerminalManagerFilter.Foreground);

    expect(next).toBe(config);
  });

  it('toggles hidden inactive project setting', () => {
    const config = defaultAppConfig();
    const hidden = withToggledTerminalManagerHideInactive(config);
    const visible = withToggledTerminalManagerHideInactive(hidden);

    expect(hidden.ui.terminalManagerHideInactiveProjects).toBe(true);
    expect(visible.ui.terminalManagerHideInactiveProjects).toBe(false);
  });

  it('opens Terminal Manager with Foreground filter', () => {
    const config = defaultAppConfig();
    config.ui.leftSidebarTab = LeftSidebarTab.Directory;
    config.ui.terminalManagerFilter = TerminalManagerFilter.Background;
    config.ui.projectExplorerExpanded = false;

    const next = withTerminalManagerOpened(config);

    expect(next.ui.showProjectExplorer).toBe(true);
    expect(next.ui.projectExplorerExpanded).toBe(true);
    expect(next.ui.leftSidebarTab).toBe(LeftSidebarTab.TerminalManager);
    expect(next.ui.terminalManagerFilter).toBe(TerminalManagerFilter.Foreground);
  });

  it('normalizes startup state like the original app', () => {
    const config = defaultAppConfig();
    config.ui.leftSidebarTab = LeftSidebarTab.TerminalManager;
    config.ui.terminalManagerFilter = TerminalManagerFilter.Background;
    config.ui.terminalManagerHideInactiveProjects = true;

    const next = normalizeTerminalManagerStartupState(config);

    expect(next.ui.terminalManagerFilter).toBe(TerminalManagerFilter.Foreground);
    expect(next.ui.terminalManagerHideInactiveProjects).toBe(false);
  });

  it('keeps a non-Terminal Manager startup filter but still shows inactive projects', () => {
    const config = defaultAppConfig();
    config.ui.leftSidebarTab = LeftSidebarTab.Directory;
    config.ui.terminalManagerFilter = TerminalManagerFilter.Background;
    config.ui.terminalManagerHideInactiveProjects = true;

    const next = normalizeTerminalManagerStartupState(config);

    expect(next.ui.terminalManagerFilter).toBe(TerminalManagerFilter.Background);
    expect(next.ui.terminalManagerHideInactiveProjects).toBe(false);
  });

  it('uses icon-prefixed labels for path context menu actions', () => {
    expect(terminalManagerPathMenuLabel('copy_path')).toBe('⧉ Copy Path');
    expect(terminalManagerPathMenuLabel('open_folder')).toBe('📂 Open in Folder');
  });

  it('shows OpenCode ACP button in Foreground without requiring an OpenCode terminal', () => {
    expect(shouldShowOpenCodeAcpButton(TerminalManagerFilter.Foreground, false)).toBe(true);
    expect(shouldShowOpenCodeAcpButton(TerminalManagerFilter.Foreground, true)).toBe(false);
    expect(shouldShowOpenCodeAcpButton(TerminalManagerFilter.Background, false)).toBe(false);
  });

  it('shows the foreground launcher for any project row in Foreground filter', () => {
    expect(shouldShowForegroundLauncherButton(TerminalManagerFilter.Foreground)).toBe(true);
    expect(shouldShowForegroundLauncherButton(TerminalManagerFilter.Background)).toBe(false);
  });
});
