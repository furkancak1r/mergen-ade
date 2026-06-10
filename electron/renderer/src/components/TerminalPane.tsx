import React, { useEffect, useRef } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { BrowserScopeKeyType } from '../../../shared/types';
import '@xterm/xterm/css/xterm.css';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown>; on: (channel: string, cb: (...args: any[]) => void) => () => void } }).mergenApi;

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
  const fitRef = useRef<FitAddon | null>(null);
  const dataUnsubRef = useRef<(() => void) | null>(null);
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

    const term = new Terminal({
      cursorBlink: true,
      fontFamily: 'Consolas, "Courier New", monospace',
      fontSize: 14,
      theme: {
        background: '#0c0c0c',
        foreground: '#c8c8c8',
        cursor: '#c8c8c8',
        selectionBackground: '#264f78',
      },
      scrollback: 10000,
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
    api.invoke('pty:resize', terminalId, cols, rows);

    term.onData((data) => {
      api.invoke('pty:write', terminalId, data);
    });

    const unsub = api.on('pty:data', (id: number, data: string) => {
      if (id === terminalId) {
        term.write(data);
        // Track mouse reporting state (DECSET 1000/1002/1006) for wheel forwarding
        const mouseEnable = /\x1b\[\?100[026]h/.test(data);
        const mouseDisable = /\x1b\[\?100[026]l/.test(data);
        if (mouseEnable) mouseReportingEnabledRef.current = true;
        if (mouseDisable) mouseReportingEnabledRef.current = false;
        // Track manual scroll detach: if viewport is not at bottom after data write, mark detached
        if (isOpenCodeActiveRef.current && viewport) {
          const scrollTop = viewport.scrollTop;
          const scrollHeight = viewport.scrollHeight;
          const clientHeight = viewport.clientHeight;
          const atBottom = scrollTop + clientHeight >= scrollHeight - 2;
          if (!atBottom) {
            onScrollDetachedRef.current?.(true);
          }
        }
      }
    });
    dataUnsubRef.current = unsub;

    termRef.current = term;
    fitRef.current = fit;

    // Clamp horizontal scroll offset to 0 after each scroll event
    const viewport = containerRef.current.querySelector('.xterm-viewport') as HTMLElement | null;
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
      viewport?.removeEventListener('scroll', handleScroll);
      document.removeEventListener('mousedown', handleMouseDown);
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
      if (scrollRaf) cancelAnimationFrame(scrollRaf);
      unsub();
      term.dispose();
    };
  }, [terminalId]);

  // Wheel forwarding: only attach when OpenCode is active and mouse reporting is on.
  // When inactive, xterm.js handles scroll natively without interference.
  useEffect(() => {
    if (!containerRef.current || !isOpenCodeActive || !wheelEnabled) return;
    const handleWheel = (e: WheelEvent) => {
      if (!containerRef.current?.contains(e.target as Node)) return;
      if (e.ctrlKey || e.altKey || e.metaKey) return;
      if (!mouseReportingEnabledRef.current) return;
      const button = e.deltaY < 0 ? 64 : 65;
      const x = e.offsetX + 1;
      const y = e.offsetY + 1;
      const cx = String.fromCharCode(Math.min(x, 255));
      const cy = String.fromCharCode(Math.min(y, 255));
      const seq = `\x1b[M${String.fromCharCode(button + 32)}${cx}${cy}`;
      api.invoke('pty:write', terminalId, seq);
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

    const ro = new ResizeObserver(() => {
      const fit = fitRef.current;
      const term = termRef.current;
      const container = containerRef.current;
      if (!fit || !term || !container) return;
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
      const { cols, rows } = term;
      api.invoke('pty:resize', terminalId, cols, rows);
    });

    ro.observe(parent);
    return () => ro.disconnect();
  }, [terminalId]);

  return (
    <div
      ref={containerRef}
      className="terminal-pane"
      style={{
        width: '100%',
        height: '100%',
        outline: active ? '1px solid #0078d4' : 'none',
        cursor: 'text',
      }}
      onClick={(e) => {
        onClick?.();
        // Only trigger terminal output click if not clicking on the xterm selection
        if ((e.target as HTMLElement).classList.contains('xterm-screen') || (e.target as HTMLElement).classList.contains('xterm-rows')) {
          onTerminalOutputClick?.();
        }
      }}
    />
  );
};
