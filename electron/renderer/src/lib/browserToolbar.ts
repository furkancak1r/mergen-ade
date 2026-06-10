import type { BrowserTab } from '../../../shared/types';
import type { BrowserScopeKey } from '../../../shared/types';
import { BROWSER_MAX_TABS_PER_SCOPE, BrowserScopeKeyType } from '../../../shared/types';

export type BrowserScreenshotMode = 'fullPage' | 'visibleArea';
export type BrowserToolbarAction = 'refresh' | 'clearUrl' | 'designInspect';

export interface BrowserScreenshotButtonMeta {
  mode: BrowserScreenshotMode;
  fullPage: boolean;
  icon: string;
  tooltip: string;
  ariaLabel: string;
}

export interface BrowserToolbarButtonMeta {
  action: BrowserToolbarAction;
  icon: string;
  tooltip: string;
  ariaLabel: string;
  selected: boolean;
}

export function browserToolbarCanClearUrl(urlDraft: string, activeTab?: Pick<BrowserTab, 'url' | 'title'>): boolean {
  return Boolean(urlDraft.trim() || activeTab?.url.trim() || (activeTab?.title && activeTab.title !== 'New Tab'));
}

export function clearBrowserActiveTabUrl(tabs: BrowserTab[], activeTabId: string | null): BrowserTab[] {
  if (!activeTabId) return tabs;
  return tabs.map((tab) => (
    tab.id === activeTabId
      ? { ...tab, url: '', title: 'New Tab' }
      : tab
  ));
}

export function browserProjectIdsAfterScopeEmpty(openProjectIds: ReadonlySet<number>, scope: BrowserScopeKey): Set<number> {
  const next = new Set(openProjectIds);
  if (scope.type === BrowserScopeKeyType.Project) {
    next.delete(scope.projectId);
  }
  return next;
}

export function browserTabTitle(tab: Pick<BrowserTab, 'url' | 'title'>): string {
  return tab.title?.trim() || tab.url.trim() || 'New Tab';
}

export function browserCanAddTab(tabCount: number, maxTabs = BROWSER_MAX_TABS_PER_SCOPE): boolean {
  return tabCount < maxTabs;
}

export function browserAddTabTooltip(tabCount: number, maxTabs = BROWSER_MAX_TABS_PER_SCOPE): string {
  return browserCanAddTab(tabCount, maxTabs)
    ? 'New tab'
    : `Browser tab limit reached (${maxTabs})`;
}

export function browserScreenshotButtonMeta(mode: BrowserScreenshotMode, pending = false): BrowserScreenshotButtonMeta {
  const fullPage = mode === 'fullPage';
  const label = fullPage ? 'Full page screenshot' : 'Visible area screenshot';
  return {
    mode,
    fullPage,
    icon: fullPage ? '▣' : '▤',
    tooltip: pending ? `${label} (pending...)` : label,
    ariaLabel: label,
  };
}

export function browserToolbarButtonMeta(action: BrowserToolbarAction, selected = false): BrowserToolbarButtonMeta {
  if (action === 'refresh') {
    return {
      action,
      icon: '↻',
      tooltip: 'Refresh',
      ariaLabel: 'Refresh',
      selected: false,
    };
  }
  if (action === 'clearUrl') {
    return {
      action,
      icon: '⌫',
      tooltip: 'Clear URL',
      ariaLabel: 'Clear URL',
      selected: false,
    };
  }
  return {
    action,
    icon: '✎',
    tooltip: selected ? 'Design Inspect: ON (click to disable)' : 'Design Inspect: OFF (click to enable)',
    ariaLabel: selected ? 'Disable Design Inspect' : 'Enable Design Inspect',
    selected,
  };
}
