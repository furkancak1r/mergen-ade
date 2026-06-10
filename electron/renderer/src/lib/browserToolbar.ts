import type { BrowserTab } from '../../../shared/types';
import type { BrowserScopeKey } from '../../../shared/types';
import { BrowserScopeKeyType } from '../../../shared/types';

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
