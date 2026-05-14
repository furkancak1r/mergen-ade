import React, { useRef, useEffect, useImperativeHandle, forwardRef } from 'react';
import { Terminal as XTerm } from 'xterm';
import { FitAddon } from 'xterm-addon-fit';
import { WebLinksAddon } from 'xterm-addon-web-links';
import 'xterm/css/xterm.css';
import { WebTerminal } from '../types';

export interface TerminalPanelHandle {
  writeOutput: (data: Uint8Array) => void;
}

interface Props {
  terminal: WebTerminal;
  onInput: (data: Uint8Array) => void;
  onPaste: (text: string) => void;
  onResize: (cols: number, lines: number) => void;
}

export const TerminalPanel = forwardRef<TerminalPanelHandle, Props>(
  ({ terminal, onInput, onPaste, onResize }, ref) => {
    const containerRef = useRef<HTMLDivElement>(null);
    const termRef = useRef<XTerm | null>(null);
    const fitRef = useRef<FitAddon | null>(null);

    useImperativeHandle(ref, () => ({
      writeOutput: (data: Uint8Array) => {
        if (termRef.current) {
          termRef.current.write(data);
        }
      },
    }));

    useEffect(() => {
      if (!containerRef.current) return;
      const isMobile = window.innerWidth <= 768;
      const isSmallPhone = window.innerWidth <= 480;
      const term = new XTerm({
        fontFamily: 'Consolas, "Courier New", monospace',
        fontSize: isSmallPhone ? 11 : isMobile ? 12 : 14,
        theme: {
          background: '#0c0c0c',
          foreground: '#e0e0e0',
          cursor: '#4fc3f7',
          selectionBackground: '#264f78',
          black: '#0c0c0c',
          red: '#f44336',
          green: '#4caf50',
          yellow: '#ffeb3b',
          blue: '#4fc3f7',
          magenta: '#e040fb',
          cyan: '#00bcd4',
          white: '#e0e0e0',
          brightBlack: '#666',
          brightRed: '#ff5252',
          brightGreen: '#69f0ae',
          brightYellow: '#ffff00',
          brightBlue: '#448aff',
          brightMagenta: '#e040fb',
          brightCyan: '#18ffff',
          brightWhite: '#ffffff',
        },
        cursorBlink: true,
        scrollback: 5000,
        convertEol: true,
      });
      const fit = new FitAddon();
      term.loadAddon(fit);
      term.loadAddon(new WebLinksAddon());
      term.open(containerRef.current);

      // Fit after a short delay to let container settle
      requestAnimationFrame(() => {
        fit.fit();
        onResize(term.cols, term.rows);
      });

      term.onData(data => {
        const encoder = new TextEncoder();
        onInput(encoder.encode(data));
      });
      term.onBinary(data => {
        const bytes = new Uint8Array(data.length);
        for (let i = 0; i < data.length; i++) bytes[i] = data.charCodeAt(i);
        onInput(bytes);
      });
      term.onResize(({ cols, rows }) => {
        onResize(cols, rows);
      });

      termRef.current = term;
      fitRef.current = fit;

      const handleResize = () => {
        fit.fit();
        onResize(term.cols, term.rows);
      };
      window.addEventListener('resize', handleResize);

      return () => {
        window.removeEventListener('resize', handleResize);
        term.dispose();
      };
    }, [terminal.id]);

    return (
      <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
        <div style={{ height: 28, background: '#1a1a1a', borderBottom: '1px solid #333', display: 'flex', alignItems: 'center', padding: '0 8px', gap: 8, flexShrink: 0 }}>
          <span style={{ fontSize: 12, color: '#aaa' }}>#{terminal.id}</span>
          <span style={{ fontSize: 12, color: '#e0e0e0', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            {terminal.title}
          </span>
          {terminal.exited && <span style={{ fontSize: 10, color: '#f44336', fontWeight: 'bold' }}>EXITED</span>}
          {terminal.ai_status && (
            <span style={{ fontSize: 10, color: terminal.ai_status === 'Working' ? '#4fc3f7' : '#ff9800' }}>
              {terminal.ai_status}
            </span>
          )}
        </div>
        <div ref={containerRef} style={{ flex: 1, padding: 4 }} />
      </div>
    );
  }
);
