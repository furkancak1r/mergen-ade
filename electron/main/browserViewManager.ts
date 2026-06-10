import { BrowserView, BrowserWindow } from 'electron';
import type { BrowserScopeKey } from '../shared/types';
import { BrowserScopeKeyType } from '../shared/types';
import path from 'path';
import fs from 'fs';

interface BrowserInstance {
  view: BrowserView;
  scope: BrowserScopeKey;
  tabs: { id: string; url: string; title?: string }[];
  activeTabId?: string;
  urlDraft: string;
  designInspectEnabled: boolean;
  cachedVisible?: boolean;
  cachedBounds?: { x: number; y: number; width: number; height: number };
}

const instances = new Map<string, BrowserInstance>();
let activeScope: BrowserScopeKey | null = null;

function scopeKey(scope: BrowserScopeKey): string {
  if (scope.type === BrowserScopeKeyType.Terminal) {
    return `t-${scope.projectId}-${scope.terminalId ?? 0}`;
  }
  return `p-${scope.projectId}`;
}

function getWindow(): BrowserWindow | null {
  const wins = BrowserWindow.getAllWindows();
  return wins.find((w) => !w.isDestroyed()) ?? null;
}

export function normalizeBrowserUrl(url: string): string {
  const trimmed = url.trim();
  if (!trimmed) return '';
  const lower = trimmed.toLowerCase();
  if (lower.startsWith('http://') || lower.startsWith('https://')) {
    return trimmed;
  }
  if (lower.startsWith('localhost') || lower.startsWith('127.0.0.1') || lower.startsWith('0.0.0.0') || lower.startsWith('[::1]')) {
    return 'http://' + trimmed;
  }
  return 'https://' + trimmed;
}

function ensureBrowserUserDataDir(projectId: number): string {
  const base = path.join(process.env.APPDATA || process.env.HOME || '.', 'Mergen', 'MergenADE', 'runtime', 'webview2', 'projects', String(projectId));
  if (!fs.existsSync(base)) {
    fs.mkdirSync(base, { recursive: true });
  }
  return base;
}

export function createBrowserView(scope: BrowserScopeKey): BrowserInstance {
  const key = scopeKey(scope);
  const existing = instances.get(key);
  if (existing) return existing;

  const userDataDir = ensureBrowserUserDataDir(scope.projectId);
  const view = new BrowserView({
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      partition: `persist:project-${scope.projectId}`,
    },
  });

  const instance: BrowserInstance = {
    view,
    scope,
    tabs: [],
    urlDraft: '',
    designInspectEnabled: false,
  };

  instances.set(key, instance);

  view.webContents.on('did-navigate', (_event, url) => {
    broadcast('browser:urlChanged', scope, url);
  });

  view.webContents.on('page-title-updated', (_event, title) => {
    const tab = instance.tabs.find((t) => t.id === instance.activeTabId);
    if (tab) {
      tab.title = title;
    }
  });

  return instance;
}

export function getBrowserInstance(scope: BrowserScopeKey): BrowserInstance | undefined {
  return instances.get(scopeKey(scope));
}

export function syncBrowserBounds(scope: BrowserScopeKey, bounds: { x: number; y: number; width: number; height: number }): void {
  const instance = instances.get(scopeKey(scope));
  if (!instance) return;

  const win = getWindow();
  if (!win) return;

  const { view } = instance;
  const cached = instance.cachedBounds;
  if (cached && cached.x === bounds.x && cached.y === bounds.y && cached.width === bounds.width && cached.height === bounds.height) {
    return;
  }

  view.setBounds(bounds);
  instance.cachedBounds = bounds;
}

export function showBrowserView(scope: BrowserScopeKey): void {
  const instance = instances.get(scopeKey(scope));
  if (!instance) return;

  const win = getWindow();
  if (!win) return;

  if (instance.cachedVisible) return;
  // Guard against duplicate adds if native state drifted from cache
  const alreadyAttached = win.getBrowserViews().some((v) => v === instance.view);
  if (!alreadyAttached) {
    win.addBrowserView(instance.view);
  }
  instance.view.setBounds(instance.cachedBounds ?? { x: 0, y: 0, width: 0, height: 0 });
  instance.cachedVisible = true;
}

export function hideBrowserView(scope: BrowserScopeKey): void {
  const instance = instances.get(scopeKey(scope));
  if (!instance) return;

  const win = getWindow();
  if (!win) return;

  if (!instance.cachedVisible) return;

  win.removeBrowserView(instance.view);
  instance.cachedVisible = false;
}

export function navigateBrowser(scope: BrowserScopeKey, url: string): void {
  const instance = getBrowserInstance(scope) ?? createBrowserView(scope);
  const normalized = normalizeBrowserUrl(url);
  if (!normalized) return;

  if (instance.tabs.length === 0) {
    const tabId = `tab-${Date.now()}`;
    instance.tabs.push({ id: tabId, url: normalized });
    instance.activeTabId = tabId;
  } else {
    const active = instance.tabs.find((t) => t.id === instance.activeTabId);
    if (active) {
      active.url = normalized;
    }
  }

  instance.view.webContents.loadURL(normalized);
}

export function browserGoBack(scope: BrowserScopeKey): void {
  const instance = instances.get(scopeKey(scope));
  if (instance?.view.webContents.canGoBack()) {
    instance.view.webContents.goBack();
  }
}

export function browserGoForward(scope: BrowserScopeKey): void {
  const instance = instances.get(scopeKey(scope));
  if (instance?.view.webContents.canGoForward()) {
    instance.view.webContents.goForward();
  }
}

export function browserReload(scope: BrowserScopeKey): void {
  const instance = instances.get(scopeKey(scope));
  instance?.view.webContents.reload();
}

export function browserExecuteJs(scope: BrowserScopeKey, script: string): Promise<unknown> {
  const instance = instances.get(scopeKey(scope));
  if (!instance) return Promise.resolve(undefined);
  return instance.view.webContents.executeJavaScript(script);
}

export function browserScreenshot(scope: BrowserScopeKey, fullPage: boolean): Promise<string> {
  const instance = instances.get(scopeKey(scope));
  if (!instance) return Promise.resolve('');
  return instance.view.webContents.capturePage().then((img) => {
    return img.toDataURL();
  });
}

const DESIGN_INSPECT_SCRIPT_VERSION = '1.0';

const designInspectScript = `
(function() {
  if (window.__mergenDesignInspect) return;
  window.__mergenDesignInspect = {
    version: '${DESIGN_INSPECT_SCRIPT_VERSION}',
    enabled: false,
    hovered: null,
    overlay: null,
    init: function() {
      this.overlay = document.createElement('div');
      this.overlay.style.cssText = 'position:fixed;pointer-events:none;z-index:999999;border:2px solid #0078d4;background:rgba(0,120,212,0.1);transition:none;';
      document.body.appendChild(this.overlay);
      document.addEventListener('click', this.onClick.bind(this), true);
      document.addEventListener('mousemove', this.onMouseMove.bind(this), true);
      document.addEventListener('mouseover', this.onMouseOver.bind(this), true);
    },
    onMouseOver: function(e) {
      if (!this.enabled) return;
      this.hovered = e.target;
    },
    onMouseMove: function(e) {
      if (!this.enabled || !this.hovered) {
        this.overlay.style.display = 'none';
        return;
      }
      const rect = this.hovered.getBoundingClientRect();
      this.overlay.style.display = 'block';
      this.overlay.style.left = rect.left + 'px';
      this.overlay.style.top = rect.top + 'px';
      this.overlay.style.width = rect.width + 'px';
      this.overlay.style.height = rect.height + 'px';
    },
    onClick: function(e) {
      if (!this.enabled) return;
      e.preventDefault();
      e.stopPropagation();
      const el = e.target;
      const rect = el.getBoundingClientRect();
      const info = {
        type: 'DesignElementClicked',
        tagName: el.tagName,
        id: el.id || '',
        className: el.className || '',
        text: (el.textContent || '').slice(0, 200),
        rect: { x: rect.x, y: rect.y, width: rect.width, height: rect.height },
        href: el.href || '',
        src: el.src || '',
        pageUrl: window.location.href,
      };
      console.log('[mergen-design-inspect]' + JSON.stringify(info));
    },
    setEnabled: function(v) {
      this.enabled = v;
      if (!this.overlay) this.init();
      this.overlay.style.display = 'none';
    }
  };
  window.__mergenDesignInspect.init();
})();
`.trim();

export function browserDesignInspect(scope: BrowserScopeKey, enabled: boolean): void {
  const instance = instances.get(scopeKey(scope));
  if (!instance) return;
  instance.designInspectEnabled = enabled;

  const wc = instance.view.webContents;
  if (wc.isLoadingMainFrame()) return;

  // Inject the script if not already present
  wc.executeJavaScript(designInspectScript).then(() => {
    wc.executeJavaScript(`window.__mergenDesignInspect.setEnabled(${enabled})`).catch(() => {});
  }).catch(() => {});

  // Use console-message to capture design inspect clicks
  if (enabled) {
    // Prevent duplicate listeners if design inspect is toggled on repeatedly
    const existingHandler = (instance as any).__designInspectHandler;
    if (existingHandler) {
      wc.removeListener('console-message', existingHandler);
    }
    const handler = (_event: Electron.Event, level: number, message: string, line: number, sourceId: string) => {
      const prefix = '[mergen-design-inspect]';
      if (message.includes(prefix)) {
        try {
          const json = message.slice(message.indexOf(prefix) + prefix.length);
          const info = JSON.parse(json);
          if (info && info.type === 'DesignElementClicked') {
            const elementInfo = JSON.stringify(info);
            broadcast('browser:designElementClicked', scope, elementInfo);
            // Auto-disable after successful delivery
            browserDesignInspect(scope, false);
          }
        } catch {
          // ignore
        }
      }
    };
    wc.on('console-message', handler);
    // Store handler on instance so it can be cleaned up later
    (instance as any).__designInspectHandler = handler;
  } else {
    const handler = (instance as any).__designInspectHandler;
    if (handler) {
      wc.removeListener('console-message', handler);
      (instance as any).__designInspectHandler = undefined;
    }
  }
}


export function browserAddTab(scope: BrowserScopeKey, url?: string): string {
  const instance = getBrowserInstance(scope) ?? createBrowserView(scope);
  const tabId = `tab-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;
  const normalized = url ? normalizeBrowserUrl(url) : '';
  instance.tabs.push({ id: tabId, url: normalized });
  instance.activeTabId = tabId;
  if (normalized) {
    instance.view.webContents.loadURL(normalized);
  }
  return tabId;
}

export function browserCloseTab(scope: BrowserScopeKey, tabId: string): void {
  const key = scopeKey(scope);
  const instance = instances.get(key);
  if (!instance) return;
  instance.tabs = instance.tabs.filter((t) => t.id !== tabId);
  if (instance.activeTabId === tabId) {
    instance.activeTabId = instance.tabs[0]?.id;
    if (instance.activeTabId) {
      const active = instance.tabs.find((t) => t.id === instance.activeTabId);
      if (active?.url) {
        instance.view.webContents.loadURL(active.url);
      }
    } else {
      // Last tab closed: clean up instance state and shut down WebView
      instance.view.webContents.loadURL('about:blank');
      hideBrowserView(scope);
      instance.view.webContents.close();
      instances.delete(key);
    }
  }
}

export function browserSwitchTab(scope: BrowserScopeKey, tabId: string): void {
  const instance = instances.get(scopeKey(scope));
  if (!instance) return;
  const tab = instance.tabs.find((t) => t.id === tabId);
  if (tab) {
    instance.activeTabId = tabId;
    if (tab.url) {
      instance.view.webContents.loadURL(tab.url);
    }
  }
}

export function destroyBrowserInstance(scope: BrowserScopeKey): void {
  const key = scopeKey(scope);
  const instance = instances.get(key);
  if (instance) {
    hideBrowserView(scope);
    instance.view.webContents.close();
    instances.delete(key);
  }
}

export function hideAllBrowserViews(): void {
  for (const instance of instances.values()) {
    hideBrowserView(instance.scope);
  }
}

export function showAllBrowserViews(): void {
  for (const instance of instances.values()) {
    showBrowserView(instance.scope);
  }
}

export function setActiveBrowserScope(scope: BrowserScopeKey): void {
  activeScope = scope;
}

export function showActiveBrowserView(): void {
  if (!activeScope) return;
  showBrowserView(activeScope);
}

function broadcast(channel: string, ...args: unknown[]) {
  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed()) {
      win.webContents.send(channel, ...args);
    }
  }
}
