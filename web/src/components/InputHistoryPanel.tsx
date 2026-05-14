import React from 'react';
import { WebTerminal } from '../types';

interface Props {
  terminals: WebTerminal[];
  onSend: (terminalId: number, text: string) => void;
}

export const InputHistoryPanel: React.FC<Props> = ({ terminals, onSend }) => {
  const foreground = terminals
    .filter(t => t.kind === 'foreground' && !t.exited)
    .slice(0, 5);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden' }}>
      <div style={{ padding: 8, borderBottom: '1px solid #333', display: 'flex', alignItems: 'center', gap: 4 }}>
        <strong style={{ fontSize: 12, color: '#aaa', flex: 1 }}>Input History</strong>
      </div>
      <div style={{ flex: 1, overflow: 'auto' }}>
        {foreground.length === 0 && (
          <div style={{ padding: 8, fontSize: 11, color: '#666' }}>No foreground terminals</div>
        )}
        {foreground.map(t => (
          <div key={t.id} style={{ padding: '4px 8px', borderBottom: '1px solid #222' }}>
            <div style={{ fontSize: 10, color: '#4fc3f7', marginBottom: 2 }}>#{t.id} {t.title}</div>
            <div style={{ fontSize: 11, color: '#e0e0e0' }}>{t.ai_status || 'Idle'}</div>
          </div>
        ))}
      </div>
    </div>
  );
};
