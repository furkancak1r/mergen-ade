import React, { useState } from 'react';
import { WebTerminal } from '../types';
import { Button, TextArea, Icon, PopupMenu, PopupMenuItem } from '../components/ui';

interface Props {
  terminal: WebTerminal | null;
  onSubmit: (text: string, mode: 'steer_now' | 'after_done') => void;
}

export const SmartInput: React.FC<Props> = ({ terminal, onSubmit }) => {
  const [text, setText] = useState('');
  const [mode, setMode] = useState<'steer_now' | 'after_done'>('steer_now');
  const [queue, setQueue] = useState<string[]>([]);

  const handleSubmit = () => {
    if (!text.trim()) return;
    onSubmit(text, mode);
    if (mode === 'after_done') {
      setQueue(prev => [...prev, text]);
    }
    setText('');
  };

  if (!terminal || terminal.exited) return null;

  return (
    <div style={{ borderTop: '1px solid var(--border-subtle)', background: 'var(--bg-surface)', padding: 'var(--space-sm) var(--space-md)', flexShrink: 0 }}>
      {queue.length > 0 && (
        <div style={{ marginBottom: 'var(--space-sm)', display: 'flex', flexDirection: 'column', gap: 'var(--space-xs)' }}>
          {queue.map((q, i) => (
            <div
              key={i}
              style={{
                fontSize: 'var(--font-xs)',
                color: 'var(--text-secondary)',
                padding: 'var(--space-xs) var(--space-sm)',
                background: 'var(--bg-elevated)',
                borderRadius: 'var(--radius-sm)',
                display: 'flex',
                alignItems: 'center',
                gap: 'var(--space-sm)',
              }}
            >
              <Icon symbol="⏳" size={10} color="var(--warning)" />
              <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{q}</span>
            </div>
          ))}
        </div>
      )}
      <div style={{ display: 'flex', gap: 'var(--space-md)', alignItems: 'flex-end' }}>
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 'var(--space-xs)' }}>
          <div style={{ display: 'flex', gap: 'var(--space-lg)', fontSize: 'var(--font-xs)' }}>
            <label
              onClick={() => setMode('steer_now')}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 'var(--space-xs)',
                color: mode === 'steer_now' ? 'var(--accent)' : 'var(--text-secondary)',
                cursor: 'pointer',
                fontWeight: mode === 'steer_now' ? 600 : 400,
              }}
            >
              <span style={{
                width: 6,
                height: 6,
                borderRadius: '50%',
                background: mode === 'steer_now' ? 'var(--accent)' : 'var(--text-muted)',
                display: 'inline-block',
              }} />
              Steer Now
            </label>
            <label
              onClick={() => setMode('after_done')}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: 'var(--space-xs)',
                color: mode === 'after_done' ? 'var(--accent)' : 'var(--text-secondary)',
                cursor: 'pointer',
                fontWeight: mode === 'after_done' ? 600 : 400,
              }}
            >
              <span style={{
                width: 6,
                height: 6,
                borderRadius: '50%',
                background: mode === 'after_done' ? 'var(--accent)' : 'var(--text-muted)',
                display: 'inline-block',
              }} />
              After Done
            </label>
          </div>
          <TextArea
            value={text}
            onChange={e => setText(e.target.value)}
            onKeyDown={e => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                handleSubmit();
              }
            }}
            placeholder="Type a task for Smart Input…"
            rows={1}
            maxLength={4096}
          />
        </div>
        <Button
          variant="primary"
          onClick={handleSubmit}
          style={{ minHeight: 44, minWidth: 60, fontSize: 'var(--font-base)', fontWeight: 700 }}
        >
          Send
        </Button>
      </div>
    </div>
  );
};
