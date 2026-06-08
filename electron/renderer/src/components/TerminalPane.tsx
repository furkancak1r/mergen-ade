import React, { useEffect, useRef } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import '@xterm/xterm/css/xterm.css';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown>; on: (channel: string, cb: (...args: any[]) => void) => () => void } }).mergenApi;

interface TerminalPaneProps {
  terminalId: number;
  active: boolean;
  onClick?: () => void;
}

export const TerminalPane: React.FC<TerminalPaneProps> = ({ terminalId, active, onClick }) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);
  const dataUnsubRef = useRef<(() => void) | null>(null);

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
    term.open(containerRef.current);
    fit.fit();

    const { cols, rows } = term;
    api.invoke('pty:resize', terminalId, cols, rows);

    term.onData((data) => {
      api.invoke('pty:write', terminalId, data);
    });

    const unsub = api.on('pty:data', (id: number, data: string) => {
      if (id === terminalId) {
        term.write(data);
      }
    });
    dataUnsubRef.current = unsub;

    termRef.current = term;
    fitRef.current = fit;

    return () => {
      unsub();
      term.dispose();
    };
  }, [terminalId]);

  useEffect(() => {
    if (!containerRef.current || !termRef.current || !fitRef.current) return;

    const ro = new ResizeObserver(() => {
      const fit = fitRef.current;
      const term = termRef.current;
      if (!fit || !term) return;
      fit.fit();
      const { cols, rows } = term;
      api.invoke('pty:resize', terminalId, cols, rows);
    });

    ro.observe(containerRef.current);
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
      onClick={onClick}
    />
  );
};
