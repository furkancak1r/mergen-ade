import React, { useState, useEffect, useRef, useCallback } from 'react';
import type { AcpChatSession, AcpChatMessage, ProjectRecord } from '../../../shared/types';

const api = (window as unknown as { mergenApi: { invoke: (channel: string, ...args: unknown[]) => Promise<unknown>; on: (channel: string, cb: (...args: any[]) => void) => () => void } }).mergenApi;

interface AcpChatPanelProps {
  project: ProjectRecord;
  chatId: string;
  onClose?: () => void;
}

export const AcpChatPanel: React.FC<AcpChatPanelProps> = ({ project, chatId, onClose }) => {
  const [session, setSession] = useState<AcpChatSession | null>(null);
  const [input, setInput] = useState('');
  const [attachments, setAttachments] = useState<string[]>([]);
  const messagesEndRef = useRef<HTMLDivElement>(null);

  const refreshSession = useCallback(async () => {
    const s = await api.invoke('acp:getSession', chatId) as AcpChatSession | null;
    setSession(s);
  }, [chatId]);

  useEffect(() => {
    refreshSession();
    const unsub = api.on('acp:event', (eventChatId: string, event: unknown) => {
      if (eventChatId === chatId) {
        refreshSession();
      }
    });
    return () => { unsub(); };
  }, [chatId, refreshSession]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [session?.messages]);

  const send = useCallback(async () => {
    if (!input.trim() && attachments.length === 0) return;
    const promptText = input.trim();
    await api.invoke('acp:send', { chatId, promptText, attachments });
    setInput('');
    setAttachments([]);
  }, [chatId, input, attachments]);

  const cancel = useCallback(async () => {
    await api.invoke('acp:cancel', chatId);
  }, [chatId]);

  const isRunning = session?.status === 'running' || session?.status === 'permission';
  const canSend = (input.trim() || attachments.length > 0) && !isRunning;

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', background: '#0c0c0c' }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '8px 12px', borderBottom: '1px solid #222' }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: '#eee' }}>
          ACP Chat — {project.name}
        </span>
        <span style={{ fontSize: 11, color: '#888' }}>
          {session?.status || 'Loading...'}
        </span>
        {onClose && (
          <button onClick={onClose} style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 14 }}>
            ✕
          </button>
        )}
      </div>

      <div style={{ flex: 1, overflow: 'auto', padding: '8px 12px' }}>
        {session?.messages.length === 0 && (
          <div style={{ color: '#666', fontSize: 12, textAlign: 'center', marginTop: 40 }}>
            Welcome to ACP Chat. Type a message to start.
          </div>
        )}
        {session?.messages.map((msg, i) => (
          <MessageBubble key={i} message={msg} />
        ))}
        <div ref={messagesEndRef} />
      </div>

      <div style={{ padding: '8px 12px', borderTop: '1px solid #222' }}>
        {attachments.length > 0 && (
          <div style={{ display: 'flex', gap: 4, marginBottom: 6, flexWrap: 'wrap' }}>
            {attachments.map((a, i) => (
              <span key={i} style={{ fontSize: 11, color: '#aaa', background: '#1a1a1a', padding: '2px 6px', borderRadius: 3, display: 'flex', alignItems: 'center', gap: 4 }}>
                {a}
                <button onClick={() => setAttachments((prev) => prev.filter((_, idx) => idx !== i))} style={{ background: 'transparent', border: 'none', color: '#888', cursor: 'pointer', fontSize: 10 }}>✕</button>
              </span>
            ))}
          </div>
        )}
        <div style={{ display: 'flex', gap: 8, alignItems: 'flex-end' }}>
          <button
            onClick={async () => {
              const paths = await api.invoke('dialog:showOpen', { properties: ['openFile', 'multiSelections'] }) as string[] | undefined;
              if (paths) setAttachments((prev) => [...prev, ...paths]);
            }}
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
            +
          </button>
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.ctrlKey && !e.shiftKey) {
                e.preventDefault();
                if (canSend) send();
              }
              if (e.key === 'Enter' && e.ctrlKey) {
                e.preventDefault();
                setInput((prev) => prev + '\n');
              }
            }}
            placeholder="Type a message..."
            style={{
              flex: 1,
              background: '#1a1a1a',
              border: '1px solid #333',
              borderRadius: 8,
              padding: '8px 12px',
              color: '#ccc',
              fontSize: 13,
              resize: 'none',
              minHeight: 36,
              maxHeight: 120,
              outline: 'none',
            }}
            rows={1}
          />
          <button
            onClick={isRunning ? cancel : send}
            disabled={!isRunning && !canSend}
            style={{
              width: 28,
              height: 28,
              borderRadius: '50%',
              background: isRunning ? '#c44' : '#1a1a1a',
              border: '1px solid #333',
              color: '#ccc',
              cursor: (!isRunning && !canSend) ? 'not-allowed' : 'pointer',
              fontSize: 14,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              flexShrink: 0,
              opacity: (!isRunning && !canSend) ? 0.5 : 1,
            }}
          >
            {isRunning ? '✕' : '➤'}
          </button>
        </div>
      </div>
    </div>
  );
};

const MessageBubble: React.FC<{ message: AcpChatMessage }> = ({ message }) => {
  const isUser = message.role === 'user';
  return (
    <div style={{
      display: 'flex',
      justifyContent: isUser ? 'flex-end' : 'flex-start',
      marginBottom: 8,
    }}>
      <div style={{
        maxWidth: '80%',
        padding: '8px 12px',
        borderRadius: 12,
        background: isUser ? '#1f3a4c' : '#1a1a1a',
        color: '#ccc',
        fontSize: 13,
        whiteSpace: 'pre-wrap',
        wordBreak: 'break-word',
      }}>
        {message.text}
      </div>
    </div>
  );
};
