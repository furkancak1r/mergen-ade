import React, { useState } from 'react';
import { WebTerminal } from '../types';

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
    <div style={{ borderTop: '1px solid #333', background: '#141414', padding: '6px 8px', flexShrink: 0 }}>
      {queue.length > 0 && (
        <div style={{ marginBottom: 4, display: 'flex', flexDirection: 'column', gap: 2 }}>
          {queue.map((q, i) => (
            <div key={i} style={{ fontSize: 10, color: '#888', padding: '2px 6px', background: '#1a1a1a', borderRadius: 2, display: 'flex', alignItems: 'center', gap: 4 }}>
              <span style={{ color: '#ff9800' }}>⏳</span>
              <span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{q}</span>
            </div>
          ))}
        </div>
      )}
      <div style={{ display: 'flex', gap: 6, alignItems: 'flex-end' }}>
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 3 }}>
          <div style={{ display: 'flex', gap: 12, fontSize: 10 }}>
            <label
              onClick={() => setMode('steer_now')}
              style={{ display: 'flex', alignItems: 'center', gap: 3, color: mode === 'steer_now' ? '#4fc3f7' : '#888', cursor: 'pointer', fontWeight: mode === 'steer_now' ? 'bold' : 'normal' }}
            >
              <span style={{ width: 6, height: 6, borderRadius: '50%', background: mode === 'steer_now' ? '#4fc3f7' : '#555', display: 'inline-block' }} />
              Steer Now
            </label>
            <label
              onClick={() => setMode('after_done')}
              style={{ display: 'flex', alignItems: 'center', gap: 3, color: mode === 'after_done' ? '#4fc3f7' : '#888', cursor: 'pointer', fontWeight: mode === 'after_done' ? 'bold' : 'normal' }}
            >
              <span style={{ width: 6, height: 6, borderRadius: '50%', background: mode === 'after_done' ? '#4fc3f7' : '#555', display: 'inline-block' }} />
              After Done
            </label>
          </div>
          <textarea
            value={text}
            onChange={e => setText(e.target.value)}
            onKeyDown={e => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                handleSubmit();
              }
            }}
            placeholder="Type a task for Smart Input..."
            rows={1}
            style={{
              background: '#1a1a1a',
              border: '1px solid #444',
              color: '#e0e0e0',
              fontSize: 12,
              padding: '4px 8px',
              resize: 'vertical',
              minHeight: 28,
              maxHeight: 120,
              fontFamily: 'inherit',
              borderRadius: 3,
            }}
          />
        </div>
        <button
          onClick={handleSubmit}
          style={{
            background: '#1e3a5f',
            border: '1px solid #4fc3f7',
            color: '#4fc3f7',
            fontSize: 12,
            cursor: 'pointer',
            padding: '6px 14px',
            borderRadius: 4,
            fontWeight: 'bold',
          }}
        >
          Send
        </button>
      </div>
    </div>
  );
};
