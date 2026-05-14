import React from 'react';
import { WebTerminal } from '../types';
import { PanelHeader, ScrollArea, EmptyState, Row } from '../components/ui';

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
      <PanelHeader title="Input History" />
      <ScrollArea>
        <div style={{ padding: 'var(--space-xs)' }}>
          {foreground.length === 0 && <EmptyState message="No foreground terminals" />}
          {foreground.map(t => (
            <Row key={t.id} style={{ flexDirection: 'column', alignItems: 'flex-start', gap: 'var(--space-xs)' }}>
              <div style={{ fontSize: 'var(--font-xs)', color: 'var(--accent)', fontWeight: 500 }}>
                #{t.id} {t.title}
              </div>
              <div style={{ fontSize: 'var(--font-sm)', color: 'var(--text-primary)' }}>
                {t.ai_status || 'Idle'}
              </div>
            </Row>
          ))}
        </div>
      </ScrollArea>
    </div>
  );
};
