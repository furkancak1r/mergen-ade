import React, { useEffect, useRef, useState, useCallback } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { BrowserScopeKeyType } from '../../../shared/types';
import {
  isTerminalViewportAtBottom,
  nextTerminalViewportScrollTop,
  type TerminalViewportScrollSnapshot,
} from '../lib/terminalViewport';
import '@xterm/xterm/css/xterm.css';

interface TerminalContextMenu {
  x: number;
  y: number;
  hasSelection: boolean;
}

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown>; on: (channel: string, cb: (...args: any[]) => void) => () => void } }).mergenApi;

function captureTerminalViewportScroll(viewport: HTMLElement): TerminalViewportScrollSnapshot {
  return {
    scrollTop: viewport.scrollTop,
    scrollHeight: viewport.scrollHeight,
    clientHeight: viewport.clientHeight,
    atBottom: isTerminalViewportAtBottom(viewport.scrollTop, viewport.scrollHeight, viewport.clientHeight),
  };
}

function restoreTerminalViewportScroll(viewport: HTMLElement, snapshot: TerminalViewportScrollSnapshot) {
  const nextScrollTop = nextTerminalViewportScrollTop(snapshot, viewport.scrollHeight, viewport.clientHeight);
  if (Math.abs(viewport.scrollTop - nextScrollTop) > 1) {
    viewport.scrollTop = nextScrollTop;
  }
}

interface TerminalPaneProps {
  terminalId: number;
  projectId: number;
  active: boolean;
  onClick?: () => void;
  onTerminalOutputClick?: () => void;
  wheelEnabled?: boolean;
  isOpenCodeActive?: boolean;
  opencodeManualScrollDetached?: boolean;
  opencodeLeadingBlankRows?: number;
  onScrollDetached?: (detached: boolean) => void;
}

export const TerminalPane: React.FC<TerminalPaneProps> = ({ terminalId, projectId, active, onClick, onTerminalOutputClick, wheelEnabled = true, isOpenCodeActive = false, opencodeManualScrollDetached = false, opencodeLeadingBlankRows = 0, onScrollDetached }) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const [contextMenu, setContextMenu] = useState<TerminalContextMenu | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const dataUnsubRef = useRef<(() => void) | null>(null);
  const lastPtySizeRef = useRef<{ cols: number; rows: number } | null>(null);
  const isOpenCodeActiveRef = useRef(isOpenCodeActive);
  useEffect(() => {
    isOpenCodeActiveRef.current = isOpenCodeActive;
  }, [isOpenCodeActive]);
  const opencodeManualScrollDetachedRef = useRef(opencodeManualScrollDetached);
  const opencodeLeadingBlankRowsRef = useRef(opencodeLeadingBlankRows);
  const onScrollDetachedRef = useRef(onScrollDetached);
  useEffect(() => {
    opencodeManualScrollDetachedRef.current = opencodeManualScrollDetached;
    opencodeLeadingBlankRowsRef.current = opencodeLeadingBlankRows;
    onScrollDetachedRef.current = onScrollDetached;
  }, [opencodeManualScrollDetached, opencodeLeadingBlankRows, onScrollDetached]);
  const mouseReportingEnabledRef = useRef(false);

  useEffect(() => {
    if (!containerRef.current) return;
    lastPtySizeRef.current = null;

    const term = new Terminal({
      cursorBlink: true,
      fontFamily: '"Cascadia Code", "Cascadia Mono", Consolas, "Courier New", monospace',
      fontSize: 14,
      theme: {
        background: '#0c0c0c',
        foreground: '#c8c8c8',
        cursor: '#c8c8c8',
        selectionBackground: '#264f78',
      },
      scrollback: 10000,
      allowProposedApi: true,
    });

    const fit = new FitAddon();
    term.loadAddon(fit);
    const webLinks = new WebLinksAddon((_event, uri) => {
      api.invoke('browser:navigate', { scope: { type: BrowserScopeKeyType.Project, projectId }, url: uri });
    });
    term.loadAddon(webLinks);
    term.open(containerRef.current);
    fit.fit();

    // Quantize height to rows * cellHeight to align viewport with PTY grid
    const core = (term as any)._core;
    const cellHeight = core?._renderService?.dimensions?.css?.cell?.height ?? 17;
    const exactHeight = Math.round(term.rows * cellHeight);
    containerRef.current.style.height = `${exactHeight}px`;
    containerRef.current.style.alignSelf = 'start';
    fit.fit();

    const { cols, rows } = term;
    lastPtySizeRef.current = { cols, rows };
    api.invoke('pty:resize', terminalId, cols, rows);

    term.onData((data) => {
      api.invoke('pty:write', terminalId, data);
    });

    // Ctrl+C: copy selection to clipboard when text is selected, otherwise send to PTY
    term.attachCustomKeyEventHandler((event) => {
      if (event.type === 'keydown' && event.ctrlKey && !event.altKey && !event.metaKey && event.key === 'c') {
        const selection = term.getSelection();
        if (selection) {
          navigator.clipboard.writeText(selection).catch(() => {});
          return false; // prevent xterm from sending \x03 to PTY
        }
      }
      return true; // let xterm handle normally
    });

    const viewport = containerRef.current.querySelector('.xterm-viewport') as HTMLElement | null;
    let restoreScrollRaf: number | null = null;
    const restoreAfterWrite = (snapshot: TerminalViewportScrollSnapshot) => {
      if (!viewport) return;
      restoreTerminalViewportScroll(viewport, snapshot);
      if (restoreScrollRaf !== null) cancelAnimationFrame(restoreScrollRaf);
      restoreScrollRaf = requestAnimationFrame(() => {
        restoreScrollRaf = null;
        if (viewport.isConnected) {
          restoreTerminalViewportScroll(viewport, snapshot);
        }
      });
    };

    const unsub = api.on('pty:data', (id: number, data: string) => {
      if (id === terminalId) {
        const scrollSnapshot = viewport ? captureTerminalViewportScroll(viewport) : null;
        // Track mouse reporting state (DECSET 1000/1002/1006) for wheel forwarding
        const mouseEnable = /\x1b\[\?100[026]h/.test(data);
        const mouseDisable = /\x1b\[\?100[026]l/.test(data);
        if (mouseEnable) mouseReportingEnabledRef.current = true;
        if (mouseDisable) mouseReportingEnabledRef.current = false;

        term.write(data, () => {
          if (scrollSnapshot) {
            restoreAfterWrite(scrollSnapshot);
          }
          // Track manual scroll detach: if viewport is not at bottom after data write, mark detached
          if (isOpenCodeActiveRef.current && viewport) {
            const atBottom = isTerminalViewportAtBottom(viewport.scrollTop, viewport.scrollHeight, viewport.clientHeight);
            if (!atBottom) {
              onScrollDetachedRef.current?.(true);
            }
          }
        });
      }
    });
    dataUnsubRef.current = unsub;

    termRef.current = term;
    fitRef.current = fit;

    // Clamp horizontal scroll offset to 0 after each scroll event
    const handleScroll = () => {
      if (viewport && viewport.scrollLeft !== 0) {
        viewport.scrollLeft = 0;
      }
      // OpenCode manual scroll detach: clamp scroll offset so viewport never sits above first real content row
      if (viewport && opencodeManualScrollDetachedRef.current && opencodeLeadingBlankRowsRef.current > 0) {
        const minScrollTop = opencodeLeadingBlankRowsRef.current * cellHeight;
        if (viewport.scrollTop < minScrollTop) {
          viewport.scrollTop = minScrollTop;
        }
      }
    };
    viewport?.addEventListener('scroll', handleScroll);

    // Drag selection edge autoscroll
    let isDragging = false;
    let lastMouseEvent: MouseEvent | null = null;
    let scrollRaf: number | null = null;

    const updateScroll = () => {
      if (!isDragging || !lastMouseEvent || !termRef.current) {
        scrollRaf = null;
        return;
      }
      const container = containerRef.current;
      if (!container) return;
      const rect = container.getBoundingClientRect();
      const edgeSize = 40;
      const topEdge = rect.top + edgeSize;
      const bottomEdge = rect.bottom - edgeSize;
      const y = lastMouseEvent.clientY;

      let delta = 0;
      if (y < topEdge) {
        delta = -Math.min(8, Math.max(1, Math.round((topEdge - y) / 5)));
      } else if (y > bottomEdge) {
        delta = Math.min(8, Math.max(1, Math.round((y - bottomEdge) / 5)));
      }

      if (delta !== 0) {
        termRef.current.scrollLines(delta);
        // Dispatch synthetic mousemove so xterm.js selection service updates
        const event = new MouseEvent('mousemove', {
          clientX: lastMouseEvent.clientX,
          clientY: lastMouseEvent.clientY,
          bubbles: true,
        });
        document.dispatchEvent(event);
        scrollRaf = requestAnimationFrame(updateScroll);
      } else {
        scrollRaf = null;
      }
    };

    const handleMouseDown = (e: MouseEvent) => {
      if (containerRef.current && containerRef.current.contains(e.target as Node)) {
        isDragging = true;
      }
    };
    const handleMouseMove = (e: MouseEvent) => {
      lastMouseEvent = e;
      if (isDragging && !scrollRaf) {
        scrollRaf = requestAnimationFrame(updateScroll);
      }
    };
    const handleMouseUp = () => {
      isDragging = false;
      lastMouseEvent = null;
      if (scrollRaf) {
        cancelAnimationFrame(scrollRaf);
        scrollRaf = null;
      }
    };

    document.addEventListener('mousedown', handleMouseDown);
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      if (restoreScrollRaf !== null) cancelAnimationFrame(restoreScrollRaf);
      viewport?.removeEventListener('scroll', handleScroll);
      document.removeEventListener('mousedown', handleMouseDown);
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      if (scrollRaf) cancelAnimationFrame(scrollRaf);
      unsub();
      term.dispose();
      lastPtySizeRef.current = null;
    };
  }, [terminalId]);

  // Wheel handling: when OpenCode is active and mouse reporting is on,
  // prevent the browser from scrolling the viewport so xterm.js can forward
  // mouse sequences to the PTY natively. When inactive, xterm.js scrolls
  // the viewport normally.
  useEffect(() => {
    if (!containerRef.current || !isOpenCodeActive || !wheelEnabled) return;
    const handleWheel = (e: WheelEvent) => {
      if (!containerRef.current?.contains(e.target as Node)) return;
      if (e.ctrlKey || e.altKey || e.metaKey) return;
      if (!mouseReportingEnabledRef.current) return;
      // xterm.js forwards SGR mouse sequences natively; we only block the
      // browser default scroll to keep the viewport pinned while OpenCode TUI
      // handles wheel via the PTY mouse protocol.
      e.preventDefault();
    };
    const container = containerRef.current;
    container.addEventListener('wheel', handleWheel, { passive: false });
    return () => {
      container.removeEventListener('wheel', handleWheel);
    };
  }, [isOpenCodeActive, terminalId, wheelEnabled]);

  useEffect(() => {
    if (!containerRef.current || !termRef.current || !fitRef.current) return;

    const parent = containerRef.current.parentElement;
    if (!parent) return;

    let lastParentWidth = parent.clientWidth;
    let lastParentHeight = parent.clientHeight;
    const ro = new ResizeObserver(() => {
      const fit = fitRef.current;
      const term = termRef.current;
      const container = containerRef.current;
      if (!fit || !term || !container) return;
      const parentWidth = parent.clientWidth;
      const parentHeight = parent.clientHeight;
      if (parentWidth === lastParentWidth && parentHeight === lastParentHeight) return;
      lastParentWidth = parentWidth;
      lastParentHeight = parentHeight;
      const viewport = container.querySelector('.xterm-viewport') as HTMLElement | null;
      const scrollSnapshot = viewport ? captureTerminalViewportScroll(viewport) : null;
      // Reset height to 100% so fit.fit() sees the full parent size
      container.style.height = '100%';
      fit.fit();
      // Quantize height to rows * cellHeight to align viewport with PTY grid
      const core = (term as any)._core;
      const cellHeight = core?._renderService?.dimensions?.css?.cell?.height ?? 17;
      const exactHeight = Math.round(term.rows * cellHeight);
      container.style.height = `${exactHeight}px`;
      container.style.alignSelf = 'start';
      fit.fit();
      if (scrollSnapshot && viewport) {
        restoreTerminalViewportScroll(viewport, scrollSnapshot);
      }
      const { cols, rows } = term;
      const lastSize = lastPtySizeRef.current;
      if (!lastSize || lastSize.cols !== cols || lastSize.rows !== rows) {
        lastPtySizeRef.current = { cols, rows };
        api.invoke('pty:resize', terminalId, cols, rows);
      }
    });

    ro.observe(parent);
    return () => ro.disconnect();
  }, [terminalId]);

  // Close context menu on click outside
  useEffect(() => {
    if (!contextMenu) return;
    const close = () => setContextMenu(null);
    window.addEventListener('click', close);
    window.addEventListener('keydown', (e) => { if (e.key === 'Escape') close(); });
    return () => {
      window.removeEventListener('click', close);
    };
  }, [contextMenu]);

  const handleContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    const term = termRef.current;
    const hasSelection = term ? term.hasSelection() : false;
    setContextMenu({ x: e.clientX, y: e.clientY, hasSelection });
  }, []);

  const handleCopy = useCallback(() => {
    const term = termRef.current;
    if (term && term.hasSelection()) {
      const selection = term.getSelection();
      navigator.clipboard.writeText(selection).catch(() => {});
    }
    setContextMenu(null);
  }, []);

  const handlePaste = useCallback(async () => {
    const term = termRef.current;
    if (term) {
      try {
        const text = await navigator.clipboard.readText();
        if (text) {
          api.invoke('pty:write', terminalId, text);
        }
      } catch {
        // ignore
      }
    }
    setContextMenu(null);
  }, [terminalId]);

  const handleSelectAll = useCallback(() => {
    const term = termRef.current;
    if (term) {
      term.selectAll();
    }
    setContextMenu(null);
  }, []);

  return (
    <>
      <div
        ref={containerRef}
        className="terminal-pane"
        style={{
          width: '100%',
          height: '100%',
          cursor: 'text',
        }}
        onClick={(e) => {
          onClick?.();
          // Only trigger terminal output click if not clicking on the xterm selection
          if ((e.target as HTMLElement).classList.contains('xterm-screen') || (e.target as HTMLElement).classList.contains('xterm-rows')) {
            onTerminalOutputClick?.();
          }
        }}
        onContextMenu={handleContextMenu}
      />
      {contextMenu && (
        <div
          style={{
            position: 'fixed',
            left: contextMenu.x,
            top: contextMenu.y,
            zIndex: 1000,
            background: '#1a1a1a',
            border: '1px solid #333',
            borderRadius: 4,
            padding: '4px 0',
            fontSize: 12,
            color: '#ccc',
            minWidth: 120,
            boxShadow: '0 4px 12px rgba(0,0,0,0.4)',
          }}
          onClick={(e) => e.stopPropagation()}
        >
          {contextMenu.hasSelection && (
            <button
              onClick={handleCopy}
              style={{
                display: 'block',
                width: '100%',
                textAlign: 'left',
                padding: '4px 12px',
                background: 'transparent',
                border: 'none',
                color: '#ccc',
                cursor: 'pointer',
                fontSize: 12,
              }}
              onMouseEnter={(e) => { (e.target as HTMLElement).style.background = '#333'; }}
              onMouseLeave={(e) => { (e.target as HTMLElement).style.background = 'transparent'; }}
            >
              Copy
            </button>
          )}
          <button
            onClick={handlePaste}
            style={{
              display: 'block',
              width: '100%',
              textAlign: 'left',
              padding: '4px 12px',
              background: 'transparent',
              border: 'none',
              color: '#ccc',
              cursor: 'pointer',
              fontSize: 12,
            }}
            onMouseEnter={(e) => { (e.target as HTMLElement).style.background = '#333'; }}
            onMouseLeave={(e) => { (e.target as HTMLElement).style.background = 'transparent'; }}
          >
            Paste
          </button>
          <button
            onClick={handleSelectAll}
            style={{
              display: 'block',
              width: '100%',
              textAlign: 'left',
              padding: '4px 12px',
              background: 'transparent',
              border: 'none',
              color: '#ccc',
              cursor: 'pointer',
              fontSize: 12,
            }}
            onMouseEnter={(e) => { (e.target as HTMLElement).style.background = '#333'; }}
            onMouseLeave={(e) => { (e.target as HTMLElement).style.background = 'transparent'; }}
          >
            Select All
          </button>
        </div>
      )}
    </>
  );
};
