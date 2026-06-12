import React, { useEffect, useRef, useCallback } from 'react';
import { BrowserScopeKeyType } from '../../../shared/types';
import type { BrowserScopeKey, ProjectRecord, BrowserTab } from '../../../shared/types';
import { normalizeBrowserUrl } from '../lib/urlNormalize';
import { browserAddTabTooltip, browserCanAddTab, browserScreenshotButtonMeta, browserTabTitle, browserToolbarButtonMeta, browserToolbarCanClearUrl, clearBrowserActiveTabUrl } from '../lib/browserToolbar';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown>; on: (channel: string, cb: (...args: any[]) => void) => () => void } }).mergenApi;

interface BrowserPanelProps {
  project: ProjectRecord;
  activeTerminalId?: number | null;
  visibleScopeOverride?: BrowserScopeKey;
  onClose?: () => void;
  hidden?: boolean;
  tabsByScope: Map<string, BrowserTab[]>;
  activeTabByScope: Map<string, string | null>;
  urlDraftByScope: Map<string, string>;
  designInspectByScope: Map<string, boolean>;
  onTabsChange: React.Dispatch<React.SetStateAction<Map<string, BrowserTab[]>>>;
  onActiveTabChange: React.Dispatch<React.SetStateAction<Map<string, string | null>>>;
  onUrlDraftChange: React.Dispatch<React.SetStateAction<Map<string, string>>>;
  onDesignInspectChange: React.Dispatch<React.SetStateAction<Map<string, boolean>>>;
  onScopeEmpty?: (scope: BrowserScopeKey) => void;
  onClearProjectBrowserLastUrl?: (projectId: number) => void;
}

function scopeKeyString(scope: BrowserScopeKey): string {
  if (scope.type === BrowserScopeKeyType.Terminal) {
    return `terminal:${scope.projectId}:${scope.terminalId}`;
  }
  return `project:${scope.projectId}`;
}

function TooltipAbove({ children, text, disabled }: { children: React.ReactElement; text: string; disabled?: boolean }) {
  const [show, setShow] = React.useState(false);
  const [visible, setVisible] = React.useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  React.useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);

  if (disabled) return children;

  return (
    <div
      ref={ref}
      style={{ position: 'relative', display: 'inline-flex' }}
      onMouseEnter={() => {
        timerRef.current = setTimeout(() => setVisible(true), 1000);
        setShow(true);
      }}
      onMouseLeave={() => {
        if (timerRef.current) clearTimeout(timerRef.current);
        setShow(false);
        setVisible(false);
      }}
    >
      {children}
      {show && visible && (
        <div
          style={{
            position: 'absolute',
            bottom: 'calc(100% + 6px)',
            left: '50%',
            transform: 'translateX(-50%)',
            background: '#1a1a1a',
            border: '1px solid #333',
            borderRadius: 4,
            padding: '4px 8px',
            fontSize: 11,
            color: '#ccc',
            whiteSpace: 'nowrap',
            zIndex: 1000,
            pointerEvents: 'none',
          }}
        >
          {text}
        </div>
      )}
    </div>
  );
}

export const BrowserPanel: React.FC<BrowserPanelProps> = ({
  project,
  activeTerminalId,
  visibleScopeOverride,
  onClose,
  hidden,
  tabsByScope,
  activeTabByScope,
  urlDraftByScope,
  designInspectByScope,
  onTabsChange,
  onActiveTabChange,
  onUrlDraftChange,
  onDesignInspectChange,
  onScopeEmpty,
  onClearProjectBrowserLastUrl,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const hasAutoCreatedRef = useRef(false);
  const [screenshotPendingCount, setScreenshotPendingCount] = React.useState(0);

  const terminalScope = activeTerminalId ? { type: BrowserScopeKeyType.Terminal, projectId: project.id, terminalId: activeTerminalId } : null;
  const terminalScopeHasTabs = terminalScope && (tabsByScope.get(scopeKeyString(terminalScope)) ?? []).length > 0;
  const scope: BrowserScopeKey = visibleScopeOverride ?? (terminalScopeHasTabs ? terminalScope : { type: BrowserScopeKeyType.Project, projectId: project.id });

  const scopeKey = scopeKeyString(scope);

  const tabs = tabsByScope.get(scopeKey) ?? [];
  const activeTabId = activeTabByScope.get(scopeKey) ?? null;
  const urlDraft = urlDraftByScope.get(scopeKey) ?? '';
  const designInspect = designInspectByScope.get(scopeKey) ?? false;
  const activeTab = tabs.find((tab) => tab.id === activeTabId);
  const canClearUrl = browserToolbarCanClearUrl(urlDraft, activeTab);
  const screenshotPending = screenshotPendingCount > 0;
  const fullPageScreenshot = browserScreenshotButtonMeta('fullPage', screenshotPending);
  const visibleAreaScreenshot = browserScreenshotButtonMeta('visibleArea', screenshotPending);
  const canAddTab = browserCanAddTab(tabs.length);
  const addTabTooltip = browserAddTabTooltip(tabs.length);
  const refreshButton = browserToolbarButtonMeta('refresh');
  const clearButton = browserToolbarButtonMeta('clearUrl');
  const inspectButton = browserToolbarButtonMeta('designInspect', designInspect);

  const setTabs = useCallback((next: BrowserTab[] | ((prev: BrowserTab[]) => BrowserTab[])) => {
    onTabsChange((prev) => {
      const current = prev.get(scopeKey) ?? [];
      const updated = typeof next === 'function' ? next(current) : next;
      const copy = new Map(prev);
      copy.set(scopeKey, updated);
      return copy;
    });
  }, [scopeKey, onTabsChange]);

  const setActiveTabId = useCallback((next: string | null | ((prev: string | null) => string | null)) => {
    onActiveTabChange((prev) => {
      const current = prev.get(scopeKey) ?? null;
      const updated = typeof next === 'function' ? next(current) : next;
      const copy = new Map(prev);
      copy.set(scopeKey, updated);
      return copy;
    });
  }, [scopeKey, onActiveTabChange]);

  const setUrlDraft = useCallback((next: string | ((prev: string) => string)) => {
    onUrlDraftChange((prev) => {
      const current = prev.get(scopeKey) ?? '';
      const updated = typeof next === 'function' ? next(current) : next;
      const copy = new Map(prev);
      copy.set(scopeKey, updated);
      return copy;
    });
  }, [scopeKey, onUrlDraftChange]);

  const setDesignInspect = useCallback((next: boolean | ((prev: boolean) => boolean)) => {
    onDesignInspectChange((prev) => {
      const current = prev.get(scopeKey) ?? false;
      const updated = typeof next === 'function' ? next(current) : next;
      const copy = new Map(prev);
      copy.set(scopeKey, updated);
      return copy;
    });
  }, [scopeKey, onDesignInspectChange]);

  useEffect(() => {
    const unsub = api.on('browser:urlChanged', (evScope: BrowserScopeKey, url: string) => {
      if (scopeKeyEqual(evScope, scope)) {
        setUrlDraft(url);
        // Update active tab URL so it stays in sync with WebView
        setTabs((prev) => {
          const next = prev.map((t) =>
            t.id === activeTabId ? { ...t, url } : t
          );
          return next;
        });
      }
    });
    return () => { unsub(); };
  }, [scope, activeTabId, setUrlDraft, setTabs]);

  useEffect(() => {
    const unsub = api.on('browser:designElementClicked', (evScope: BrowserScopeKey, elementInfo: string) => {
      if (scopeKeyEqual(evScope, scope)) {
        // Route to active terminal
        if (activeTerminalId) {
          api.invoke('pty:write', activeTerminalId, elementInfo + '\r');
        }
        setDesignInspect(false);
      }
    });
    return () => { unsub(); };
  }, [scope, activeTerminalId, setDesignInspect]);

  useEffect(() => {
    if (!containerRef.current) return;
    if (hidden) {
      api.invoke('browser:hide', scope);
      return;
    }
    const sync = () => {
      const rect = containerRef.current!.getBoundingClientRect();
      api.invoke('browser:syncBounds', {
        scope,
        x: Math.round(rect.x),
        y: Math.round(rect.y),
        width: Math.round(rect.width),
        height: Math.round(rect.height),
      });
    };
    sync();
    api.invoke('browser:show', scope);
    const ro = new ResizeObserver(sync);
    ro.observe(containerRef.current);
    return () => {
      ro.disconnect();
      api.invoke('browser:hide', scope);
    };
  }, [scope, hidden]);

  // Mark that tabs have existed so closing the last tab does not auto-recreate
  useEffect(() => {
    if (tabs.length > 0) {
      hasAutoCreatedRef.current = true;
    }
  }, [tabs.length]);

  useEffect(() => {
    if (hidden) return;
    if (hasAutoCreatedRef.current) return;
    if (project.browserLastUrl && tabs.length === 0) {
      hasAutoCreatedRef.current = true;
      const tabId = `tab-${Date.now()}`;
      setTabs([{ id: tabId, url: project.browserLastUrl }]);
      setActiveTabId(tabId);
      setUrlDraft(project.browserLastUrl);
      api.invoke('browser:navigate', { scope, url: project.browserLastUrl });
    }
  }, [project.browserLastUrl, tabs.length, scope, setTabs, setActiveTabId, setUrlDraft, hidden]);

  const addTab = useCallback(async () => {
    if (!browserCanAddTab(tabs.length)) return;
    const tabId = await api.invoke('browser:addTab', scope) as string;
    setTabs((prev) => [...prev, { id: tabId, url: '' }]);
    setActiveTabId(tabId);
    setUrlDraft('');
  }, [scope, tabs.length, setTabs, setActiveTabId, setUrlDraft]);

  const closeTab = useCallback((tabId: string) => {
    api.invoke('browser:closeTab', { scope, tabId });
    setTabs((prev) => {
      const next = prev.filter((t) => t.id !== tabId);
      if (next.length === 0) {
        setActiveTabId(null);
        setUrlDraft('');
        onScopeEmpty?.(scope);
      } else if (activeTabId === tabId) {
        setActiveTabId(next[0].id);
      }
      return next;
    });
  }, [scope, activeTabId, setTabs, setActiveTabId, setUrlDraft, onScopeEmpty]);

  const switchTab = useCallback((tabId: string) => {
    api.invoke('browser:switchTab', { scope, tabId });
    setActiveTabId(tabId);
    const tab = tabs.find((t) => t.id === tabId);
    if (tab?.url) {
      setUrlDraft(tab.url);
    }
  }, [scope, tabs, setActiveTabId, setUrlDraft]);

  const go = useCallback(() => {
    const normalized = normalizeBrowserUrl(urlDraft);
    if (normalized) {
      setUrlDraft(normalized);
      api.invoke('browser:navigate', { scope, url: normalized });
    }
  }, [scope, urlDraft, setUrlDraft]);

  const refresh = useCallback(() => {
    api.invoke('browser:reload', scope);
  }, [scope]);

  const clearUrl = useCallback(() => {
    setUrlDraft('');
    setTabs((prev) => clearBrowserActiveTabUrl(prev, activeTabId));
    if (scope.type === BrowserScopeKeyType.Project) {
      onClearProjectBrowserLastUrl?.(scope.projectId);
    }
  }, [activeTabId, scope, setTabs, setUrlDraft, onClearProjectBrowserLastUrl]);

  const toggleDesignInspect = useCallback(() => {
    const next = !designInspect;
    setDesignInspect(next);
    api.invoke('browser:designInspect', { scope, enabled: next });
  }, [designInspect, scope, setDesignInspect]);

  const takeScreenshot = useCallback(async (fullPage: boolean) => {
    setScreenshotPendingCount((count) => count + 1);
    try {
      const dataUrl = await api.invoke('browser:screenshot', { scope, fullPage }) as string;
      if (dataUrl) {
        const link = document.createElement('a');
        link.href = dataUrl;
        link.download = `screenshot-${Date.now()}.png`;
        link.click();
      }
    } finally {
      setScreenshotPendingCount((count) => Math.max(0, count - 1));
    }
  }, [scope]);

  return (
    <div data-browser-panel style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#0c0c0c' }}>
      <div className="browser-tab-strip">
        {tabs.map((tab) => {
          const title = browserTabTitle(tab);
          return (
            <TooltipAbove key={tab.id} text={title}>
              <div
                onClick={() => switchTab(tab.id)}
                className={`browser-tab ${activeTabId === tab.id ? 'active' : ''}`}
                role="button"
                tabIndex={0}
                aria-label={`Select browser tab: ${title}`}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault();
                    switchTab(tab.id);
                  }
                }}
              >
                <span className="browser-tab-title">{title}</span>
                <button
                  onClick={(e) => { e.stopPropagation(); closeTab(tab.id); }}
                  className="browser-tab-close"
                  type="button"
                  aria-label={`Close browser tab: ${title}`}
                >
                  ✕
                </button>
              </div>
            </TooltipAbove>
          );
        })}
        <TooltipAbove text={addTabTooltip}>
          <button
            onClick={addTab}
            className="browser-add-tab-btn"
            type="button"
            disabled={!canAddTab}
            aria-label={addTabTooltip}
          >
            +
          </button>
        </TooltipAbove>
      </div>

      <div style={{ display: 'flex', gap: 4, padding: '4px 8px', borderBottom: '1px solid #222', alignItems: 'center' }}>
        <TooltipAbove text={refreshButton.tooltip}>
          <button onClick={refresh} className="browser-toolbar-btn" type="button" aria-label={refreshButton.ariaLabel}>
            {refreshButton.icon}
          </button>
        </TooltipAbove>
        <input
          data-browser-url
          value={urlDraft}
          onChange={(e) => setUrlDraft(e.target.value)}
          onKeyDown={(e) => { if (e.key === 'Enter') go(); }}
          onDoubleClick={(e) => e.currentTarget.select()}
          placeholder="Enter URL..."
          style={{ flex: 1, background: '#1a1a1a', border: '1px solid #333', borderRadius: 4, padding: '4px 8px', color: '#ccc', fontSize: 12, outline: 'none', minWidth: 100 }}
        />
        <TooltipAbove text={clearButton.tooltip}>
          <button onClick={clearUrl} className="browser-toolbar-btn" type="button" disabled={!canClearUrl} aria-label={clearButton.ariaLabel}>
            {clearButton.icon}
          </button>
        </TooltipAbove>
        <TooltipAbove text={inspectButton.tooltip}>
          <button onClick={toggleDesignInspect} className={`browser-toolbar-btn ${inspectButton.selected ? 'active' : ''}`} type="button" aria-label={inspectButton.ariaLabel}>
            {inspectButton.icon}
          </button>
        </TooltipAbove>
        <div className="browser-screenshot-group" role="group" aria-label="Browser screenshot controls">
          <TooltipAbove text={fullPageScreenshot.tooltip}>
            <button onClick={() => takeScreenshot(fullPageScreenshot.fullPage)} className="browser-screenshot-btn" type="button" aria-label={fullPageScreenshot.ariaLabel}>
              {fullPageScreenshot.icon}
            </button>
          </TooltipAbove>
          <span className="browser-screenshot-divider" aria-hidden="true" />
          <TooltipAbove text={visibleAreaScreenshot.tooltip}>
            <button onClick={() => takeScreenshot(visibleAreaScreenshot.fullPage)} className="browser-screenshot-btn" type="button" aria-label={visibleAreaScreenshot.ariaLabel}>
              {visibleAreaScreenshot.icon}
            </button>
          </TooltipAbove>
        </div>
        {onClose && (
          <TooltipAbove text="Close browser panel">
            <button onClick={onClose} style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 14 }}>
              ✕
            </button>
          </TooltipAbove>
        )}
      </div>

      <div ref={containerRef} style={{ flex: 1, background: '#0c0c0c' }} />
    </div>
  );
};

function scopeKeyEqual(a: BrowserScopeKey, b: BrowserScopeKey): boolean {
  if (a.type !== b.type) return false;
  if (a.projectId !== b.projectId) return false;
  if (a.type === BrowserScopeKeyType.Terminal && b.type === BrowserScopeKeyType.Terminal) {
    return a.terminalId === b.terminalId;
  }
  return true;
}
