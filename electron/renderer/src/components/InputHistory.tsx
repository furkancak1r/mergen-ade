import React, { useState } from 'react';
import type { TerminalKind, TerminalInputHistoryFilter, ProjectRecord } from '../../../shared/types';
import { TerminalInputHistoryFilter as TerminalInputHistoryFilterEnum } from '../../../shared/types';
import type { TerminalInstance } from '../hooks/usePty';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown> } }).mergenApi;

interface InputHistoryProps {
  config: {
    projects: ProjectRecord[];
  };
  terminals: TerminalInstance[];
  activeTerminalId: number | null;
  onActivateTerminal: (id: number) => void;
}

function formatTime(ts: number): string {
  const d = new Date(ts);
  return `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}`;
}

export const InputHistory: React.FC<InputHistoryProps> = ({ config, terminals, activeTerminalId, onActivateTerminal }) => {
  const [filter, setFilter] = useState<TerminalInputHistoryFilter>(TerminalInputHistoryFilterEnum.Foreground);

  const filteredTerminals = terminals.filter((t) => {
    if (filter === TerminalInputHistoryFilterEnum.Foreground) return t.kind === 'foreground';
    if (filter === TerminalInputHistoryFilterEnum.Background) return t.kind === 'background';
    return true;
  });

  // Collect recent inputs with metadata
  const entries: { text: string; terminalId: number; projectId: number; projectName: string; kind: TerminalKind; timestamp: number }[] = [];
  for (const t of filteredTerminals) {
    const project = config.projects.find((p) => p.id === t.projectId);
    const projectName = project?.name || 'Unknown';
    for (const text of t.recentInputs) {
      entries.push({ text, terminalId: t.id, projectId: t.projectId, projectName, kind: t.kind, timestamp: Date.now() });
    }
  }

  // For foreground filter, limit to 5 most recent entries
  const displayEntries = filter === TerminalInputHistoryFilterEnum.Foreground
    ? entries.slice(0, 5)
    : entries;

  return (
    <div className="input-history" style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
      <div style={{ display: 'flex', gap: 2, padding: '6px 8px', borderBottom: '1px solid #222' }}>
        {([TerminalInputHistoryFilterEnum.Foreground, TerminalInputHistoryFilterEnum.Background] as TerminalInputHistoryFilter[]).map((f) => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            style={{
              flex: 1,
              padding: '4px 8px',
              fontSize: 11,
              background: filter === f ? '#1f3a4c' : 'transparent',
              color: filter === f ? '#7ec0ee' : '#888',
              border: '1px solid ' + (filter === f ? '#1f3a4c' : '#333'),
              borderRadius: 4,
              cursor: 'pointer',
            }}
          >
            {f === TerminalInputHistoryFilterEnum.Foreground ? 'FG' : 'BG'}
          </button>
        ))}
        <button
          onClick={() => setFilter(TerminalInputHistoryFilterEnum.Foreground)}
          style={{
            flex: 1,
            padding: '4px 8px',
            fontSize: 11,
            background: filter !== TerminalInputHistoryFilterEnum.Foreground && filter !== TerminalInputHistoryFilterEnum.Background ? '#1f3a4c' : 'transparent',
            color: filter !== TerminalInputHistoryFilterEnum.Foreground && filter !== TerminalInputHistoryFilterEnum.Background ? '#7ec0ee' : '#888',
            border: '1px solid ' + (filter !== TerminalInputHistoryFilterEnum.Foreground && filter !== TerminalInputHistoryFilterEnum.Background ? '#1f3a4c' : '#333'),
            borderRadius: 4,
            cursor: 'pointer',
          }}
        >
          All
        </button>
      </div>
      <div style={{ flex: 1, overflow: 'auto', padding: '4px 0' }}>
        {displayEntries.length === 0 && (
          <div style={{ padding: 12, color: '#888', fontSize: 12 }}>No input history.</div>
        )}
        {displayEntries.map((entry, i) => (
          <div
            key={i}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 4,
              padding: '3px 8px',
              cursor: 'pointer',
              borderRadius: 3,
              background: entry.terminalId === activeTerminalId ? 'rgba(0,120,212,0.2)' : 'transparent',
            }}
            onClick={() => {
              onActivateTerminal(entry.terminalId);
              // Send the command to the terminal with bracketed paste
              setTimeout(() => {
                api.invoke('pty:write', entry.terminalId, `\x1b[200~${entry.text}\x1b[201~`);
                api.invoke('pty:write', entry.terminalId, '\r');
              }, 100);
            }}
            title={`${entry.projectName} — ${entry.kind}`}
          >
            <span style={{ fontSize: 10, color: '#666', flexShrink: 0 }}>{entry.projectName}</span>
            <span style={{ fontSize: 11, color: '#aaa', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
              {entry.text}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
};
