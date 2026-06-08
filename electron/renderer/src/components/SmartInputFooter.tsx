import React, { useState, useCallback } from 'react';
import type { SmartInputState, SmartInputTask, SmartInputAttachment } from '../../../shared/types';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown> } }).mergenApi;

interface SmartInputFooterProps {
  terminalId: number;
  state: SmartInputState;
  onUpdateState: (state: SmartInputState) => void;
  onSendToTerminal: (terminalId: number, text: string, attachments: SmartInputAttachment[]) => void;
}

export const SmartInputFooter: React.FC<SmartInputFooterProps> = ({ terminalId, state, onUpdateState, onSendToTerminal }) => {
  const [mode, setMode] = useState<'now' | 'after'>('now');

  const updateDraft = useCallback((text: string) => {
    onUpdateState({ ...state, draftText: text });
  }, [state, onUpdateState]);

  const addTask = useCallback(() => {
    if (!state.draftText.trim() && state.draftAttachments.length === 0) return;
    const task: SmartInputTask = {
      text: state.draftText.trim(),
      attachments: [...state.draftAttachments],
      modeId: 'build',
      afterDone: mode === 'after',
    };
    onUpdateState({
      ...state,
      queue: [...state.queue, task],
      draftText: '',
      draftAttachments: [],
    });
  }, [state, mode, onUpdateState]);

  const sendNow = useCallback(() => {
    if (!state.draftText.trim() && state.draftAttachments.length === 0) return;
    onSendToTerminal(terminalId, state.draftText.trim(), state.draftAttachments);
    onUpdateState({ ...state, draftText: '', draftAttachments: [] });
  }, [state, terminalId, onSendToTerminal, onUpdateState]);

  const removeTask = useCallback((index: number) => {
    const queue = state.queue.filter((_, i) => i !== index);
    onUpdateState({ ...state, queue });
  }, [state, onUpdateState]);

  const submit = useCallback(() => {
    if (mode === 'now') {
      sendNow();
    } else {
      addTask();
    }
  }, [mode, sendNow, addTask]);

  return (
    <div style={{ borderTop: '1px solid #222', padding: '6px 8px', background: '#0c0c0c' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
        <span style={{ fontSize: 11, color: '#888', fontWeight: 600 }}>Smart Input</span>
        <div style={{ display: 'flex', gap: 4 }}>
          <button
            onClick={() => setMode('now')}
            style={{ fontSize: 10, padding: '2px 6px', borderRadius: 3, border: '1px solid #333', background: mode === 'now' ? '#1f3a4c' : 'transparent', color: '#ccc', cursor: 'pointer' }}
          >
            Steer Now
          </button>
          <button
            onClick={() => setMode('after')}
            style={{ fontSize: 10, padding: '2px 6px', borderRadius: 3, border: '1px solid #333', background: mode === 'after' ? '#1f3a4c' : 'transparent', color: '#ccc', cursor: 'pointer' }}
          >
            After Done
          </button>
        </div>
      </div>

      {state.queue.length > 0 && (
        <div style={{ marginBottom: 6, maxHeight: 120, overflow: 'auto' }}>
          {state.queue.map((task, i) => (
            <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '3px 0', fontSize: 11, color: '#aaa' }}>
              <span style={{ color: '#666' }}>{i + 1}.</span>
              <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis' }}>{task.text || '(attachment)'}</span>
              <button onClick={() => removeTask(i)} style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 10 }}>✕</button>
            </div>
          ))}
        </div>
      )}

      <div style={{ display: 'flex', gap: 6, alignItems: 'flex-end' }}>
        <textarea
          value={state.draftText}
          onChange={(e) => updateDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !e.ctrlKey && !e.shiftKey) {
              e.preventDefault();
              submit();
            }
            if (e.key === 'Enter' && e.ctrlKey) {
              e.preventDefault();
              updateDraft(state.draftText + '\n');
            }
          }}
          placeholder="Type a prompt..."
          style={{
            flex: 1,
            background: '#1a1a1a',
            border: '1px solid #333',
            borderRadius: 6,
            padding: '6px 8px',
            color: '#ccc',
            fontSize: 12,
            resize: 'none',
            minHeight: 32,
            maxHeight: 80,
            outline: 'none',
          }}
          rows={1}
        />
        <button
          onClick={submit}
          style={{
            width: 28,
            height: 28,
            borderRadius: '50%',
            background: '#1a1a1a',
            border: '1px solid #333',
            color: '#ccc',
            cursor: 'pointer',
            fontSize: 14,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            flexShrink: 0,
          }}
        >
          ➤
        </button>
      </div>
    </div>
  );
};
