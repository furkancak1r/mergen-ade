import React, { useState, useEffect, useRef, useCallback } from 'react';
import { BrowserScopeKeyType } from '../../../shared/types';
import type { BrowserScopeKey, ProjectRecord, BrowserTab } from '../../../shared/types';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown>; on: (channel: string, cb: (...args: any[]) => void) => () => void } }).mergenApi;

interface BrowserPanelProps {
  project: ProjectRecord;
  activeTerminalId?: number | null;
  visibleScopeOverride?: BrowserScopeKey;
  onClose?: () => void;
}

export const BrowserPanel: React.FC<BrowserPanelProps> = ({ project, activeTerminalId, visibleScopeOverride, onClose }) => {
  const [tabs, setTabs] = useState<BrowserTab[]>([]);
  const [activeTabId, setActiveTabId] = useState<string | null>(null);
  const [urlDraft, setUrlDraft] = useState('');
  const [designInspect, setDesignInspect] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  const scope: BrowserScopeKey = visibleScopeOverride ?? (activeTerminalId
    ? { type: BrowserScopeKeyType.Terminal, projectId: project.id, terminalId: activeTerminalId }
    : { type: BrowserScopeKeyType.Project, projectId: project.id });

  useEffect(() => {
    const unsub = api.on('browser:urlChanged', (evScope: BrowserScopeKey, url: string) => {
      if (scopeKeyEqual(evScope, scope)) {
        setUrlDraft(url);
      }
    });
    return () => { unsub(); };
  }, [scope]);

  useEffect(() => {
    if (!containerRef.current) return;
    const rect = containerRef.current.getBoundingClientRect();
    api.invoke('browser:syncBounds', {
      scope,
      x: Math.round(rect.x),
      y: Math.round(rect.y),
      width: Math.round(rect.width),
      height: Math.round(rect.height),
    });
    api.invoke('browser:show', scope);
    return () => {
      api.invoke('browser:hide', scope);
    };
  }, [scope]);

  useEffect(() => {
    if (project.browserLastUrl && tabs.length === 0) {
      const tabId = `tab-${Date.now()}`;
      setTabs([{ id: tabId, url: project.browserLastUrl }]);
      setActiveTabId(tabId);
      setUrlDraft(project.browserLastUrl);
      api.invoke('browser:navigate', { scope, url: project.browserLastUrl });
    }
  }, [project.browserLastUrl, tabs.length, scope]);

  const addTab = useCallback(async () => {
    const tabId = await api.invoke('browser:addTab', scope) as string;
    setTabs((prev) => [...prev, { id: tabId, url: '' }]);
    setActiveTabId(tabId);
    setUrlDraft('');
  }, [scope]);

  const closeTab = useCallback((tabId: string) => {
    api.invoke('browser:closeTab', { scope, tabId });
    setTabs((prev) => {
      const next = prev.filter((t) => t.id !== tabId);
      if (next.length === 0) {
        setActiveTabId(null);
        setUrlDraft('');
      } else if (activeTabId === tabId) {
        setActiveTabId(next[0].id);
      }
      return next;
    });
  }, [scope, activeTabId]);

  const switchTab = useCallback((tabId: string) => {
    api.invoke('browser:switchTab', { scope, tabId });
    setActiveTabId(tabId);
    const tab = tabs.find((t) => t.id === tabId);
    if (tab?.url) {
      setUrlDraft(tab.url);
    }
  }, [scope, tabs]);

  const go = useCallback(() => {
    api.invoke('browser:navigate', { scope, url: urlDraft });
  }, [scope, urlDraft]);

  const toggleDesignInspect = useCallback(() => {
    const next = !designInspect;
    setDesignInspect(next);
    api.invoke('browser:designInspect', { scope, enabled: next });
  }, [designInspect, scope]);

  const takeScreenshot = useCallback(async (fullPage: boolean) => {
    const dataUrl = await api.invoke('browser:screenshot', { scope, fullPage }) as string;
    if (dataUrl) {
      // Open screenshot in a new window or save
      const link = document.createElement('a');
      link.href = dataUrl;
      link.download = `screenshot-${Date.now()}.png`;
      link.click();
    }
  }, [scope]);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#0c0c0c' }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '6px 8px', borderBottom: '1px solid #222' }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: '#eee' }}>Browser</span>
        <div style={{ display: 'flex', gap: 6 }}>
          <button onClick={toggleDesignInspect} style={{ background: designInspect ? '#1f3a4c' : 'transparent', border: '1px solid #333', color: '#ccc', cursor: 'pointer', fontSize: 10, padding: '2px 6px', borderRadius: 3 }}>
            Inspect
          </button>
          {onClose && (
            <button onClick={onClose} style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 14 }}>
              ✕
            </button>
          )}
        </div>
      </div>

      <div style={{ display: 'flex', gap: 4, padding: '4px 8px', borderBottom: '1px solid #222', overflowX: 'auto' }}>
        {tabs.map((tab) => (
          <div
            key={tab.id}
            onClick={() => switchTab(tab.id)}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 4,
              padding: '2px 8px',
              borderRadius: 4,
              background: activeTabId === tab.id ? '#1a1a1a' : 'transparent',
              border: '1px solid #333',
              cursor: 'pointer',
              fontSize: 11,
              color: '#ccc',
              whiteSpace: 'nowrap',
            }}
          >
            <span>{tab.title || tab.url || 'New Tab'}</span>
            <button
              onClick={(e) => { e.stopPropagation(); closeTab(tab.id); }}
              style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 10 }}
            >
              ✕
            </button>
          </div>
        ))}
        <button
          onClick={addTab}
          style={{ background: 'transparent', border: '1px solid #333', color: '#ccc', cursor: 'pointer', fontSize: 14, width: 22, height: 22, borderRadius: 4, display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}
        >
          +
        </button>
      </div>

      <div style={{ display: 'flex', gap: 4, padding: '4px 8px', borderBottom: '1px solid #222', alignItems: 'center' }}>
        <input
          value={urlDraft}
          onChange={(e) => setUrlDraft(e.target.value)}
          onKeyDown={(e) => { if (e.key === 'Enter') go(); }}
          placeholder="Enter URL..."
          style={{ flex: 1, background: '#1a1a1a', border: '1px solid #333', borderRadius: 4, padding: '4px 8px', color: '#ccc', fontSize: 12, outline: 'none', minWidth: 100 }}
        />
        <button onClick={go} style={{ background: 'transparent', border: '1px solid #333', color: '#ccc', cursor: 'pointer', fontSize: 10, padding: '2px 6px', borderRadius: 3 }}>Go</button>
        <button onClick={() => setUrlDraft('')} style={{ background: 'transparent', border: '1px solid #333', color: '#ccc', cursor: 'pointer', fontSize: 10, padding: '2px 6px', borderRadius: 3 }}>Clear</button>
        <div style={{ display: 'flex', border: '1px solid #333', borderRadius: 3, overflow: 'hidden' }}>
          <button onClick={() => takeScreenshot(true)} style={{ background: 'transparent', border: 'none', borderRight: '1px solid #333', color: '#ccc', cursor: 'pointer', fontSize: 10, padding: '2px 6px' }}>Full</button>
          <button onClick={() => takeScreenshot(false)} style={{ background: 'transparent', border: 'none', color: '#ccc', cursor: 'pointer', fontSize: 10, padding: '2px 6px' }}>Visible</button>
        </div>
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
